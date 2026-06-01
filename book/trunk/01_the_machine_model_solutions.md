# Solutions: 1 - The machine model

These exercises are about *measuring your machine*. Numbers vary; ratios are stable. Run them and write down what you see.

## Exercise 1 - Cache sizes

Linux: `lscpu | grep -E 'L1|L2|L3'` or `getconf -a | grep CACHE`.

Typical desktop x86-64 in 2026: L1d 32-48 KB per core, L2 1-2 MB per core, L3 16-128 MB shared. Apple Silicon: larger L1, very large shared L2.

## Exercise 2 - Sequential sum

```rust
use std::time::Instant;

fn main() {
    // Playground-scaled. Use 100_000_000 locally for the real number.
    let n = 10_000_000;
    let v: Vec<u64> = vec![1; n];
    let start = Instant::now();
    let sum: u64 = v.iter().sum();
    let elapsed = start.elapsed();
    let ns_per_elem = elapsed.as_nanos() as f64 / v.len() as f64;
    println!("sum = {sum}, {elapsed:?}, {ns_per_elem:.2} ns/elem");
}
```

Expect somewhere around 0.2-1 ns per element on modern hardware. The loop is memory-bandwidth bound; the CPU is mostly waiting for RAM to deliver lines.

## Exercise 3 - Random-access sum

```rust
use std::time::Instant;

fn main() {
    // Playground-scaled. Use 100_000_000 locally for the real number.
    let n: usize = 10_000_000;
    let v: Vec<u64> = vec![1; n];
    let mut state = 0xDEAD_BEEFu64;
    let indices: Vec<usize> = (0..n)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state as usize) % n
        })
        .collect();

    let start = Instant::now();
    let mut sum = 0u64;
    for &i in &indices {
        sum += v[i];
    }
    let elapsed = start.elapsed();
    println!("sum = {sum}, {elapsed:?}, {:.2} ns/elem",
             elapsed.as_nanos() as f64 / n as f64);
}
```

Expect 30-100 ns per element - close to the RAM-latency cost. Each access misses cache.

## Exercise 4 - Cache cliffs

The transitions you see roughly correspond to spilling out of L1 (~32 KB), L2 (~1-2 MB), L3 (~32 MB). Below L1 you should see ~0.1-0.3 ns/elem. In L3 maybe 0.5-1.5 ns. Past L3, 0.5-3 ns (sequential, since prefetcher helps even from RAM).

For random-access cliffs (a more dramatic plot), repeat exercise 3 at sizes 1K, 10K, 100K, 1M, 10M, 100M. The transitions are sharper.

## Exercise 5 - Pointer chasing

The trap in this exercise is that the obvious way to build the list does not measure what you think it measures. A list built by threading boxes as you allocate them -

```rust
// anti-pattern: bad! nodes land at sequential heap addresses
let mut head = Box::new(Node { value: 0, next: None });
for i in 1..n { head = Box::new(Node { value: i, next: Some(head) }); }
```

- hands every `Box` back-to-back from the allocator. The "linked list" is a `Vec` in disguise: traversal walks straight up consecutive cache lines and the prefetcher hides every read. You will measure ~2 ns/elem and conclude pointers are free. They are not; you measured a contiguous scan.

To surface the real cost, build in two steps: allocate all the nodes first, then **shuffle the order in which you thread them**. Each box keeps its original address; the chain visits them scrambled, so each `next` is a jump to an unpredictable line.

```rust
use std::time::Instant;

struct Node { value: u64, next: Option<Box<Node>> }

fn build_shuffled(n: usize) -> Box<Node> {
    let mut nodes: Vec<Box<Node>> = (0..n as u64)
        .map(|i| Box::new(Node { value: i, next: None }))
        .collect();

    // Fisher-Yates with an inline LCG (no deps, Playground-friendly).
    let mut s = 0x1234_5678_u64;
    let mut rng = || { s = s.wrapping_mul(6364136223846793005).wrapping_add(1); s };
    for i in (1..nodes.len()).rev() {
        let j = (rng() % (i as u64 + 1)) as usize;
        nodes.swap(i, j);
    }

    let mut head: Option<Box<Node>> = None;
    while let Some(mut node) = nodes.pop() { node.next = head; head = Some(node); }
    head.unwrap()
}

fn sum(mut head: &Node) -> u64 {
    let mut s = 0;
    loop {
        s += head.value;
        match &head.next {
            Some(next) => head = next,
            None => break s,
        }
    }
}

// Recursive Drop walks `next` on the stack and overflows at large N.
// Tear the chain down iteratively.
fn drop_list(head: Box<Node>) {
    let mut cur = Some(head);
    while let Some(mut node) = cur { cur = node.next.take(); }
}

fn main() {
    // Playground-scaled. Use 1_000_000 locally for the real number.
    let n = 100_000;
    let head = build_shuffled(n);
    let start = Instant::now();
    let s = std::hint::black_box(sum(&head));
    let elapsed = start.elapsed();
    println!("sum = {s}, {elapsed:?}, {:.2} ns/elem",
             elapsed.as_nanos() as f64 / n as f64);
    drop_list(head);
}
```

A `Vec<u64>` sum runs 0.2-2 ns/elem depending on the chip; the *shuffled* linked-list walk is the same scan paying full DRAM latency on every `next`. The measured ratio is 63× on a Pi 4, ~100-120× on mid-2010s Intel, ~300× on a modern Ryzen (see `code/measurement/src/bin/pointer_chase.rs` and the cross-machine table in `code/README.md`). Without the shuffle the ratio collapses toward 1× - that is the prefetcher, not the absence of a tax.

Three stack-overflow traps hide in this exercise, all from recursion over `next`:
- Building by deep recursion overflows on the way down - the loop above scales.
- A recursive `sum` overflows likewise - walk it iteratively.
- The *implicit* `Drop` is recursive too, and fires when `head` leaves scope. At N=1M it overflows even though your code never named a recursive function. `drop_list` tears the chain down by hand.

## Exercise 6 - Reading lscpu against your benchmarks

The transitions are noisy because:
- Cache levels overlap (a hot cache line might still be in L1 after spilling to L2).
- Hardware prefetchers help sequential reads.
- The OS may evict pages between runs.

If your noise is worse than your signal, run each measurement multiple times and take the median.
