# Solutions: 1 — The machine model

These exercises are about *measuring your machine*. Numbers vary; ratios are stable. Run them and write down what you see.

## Exercise 1 — Cache sizes

Linux: `lscpu | grep -E 'L1|L2|L3'` or `getconf -a | grep CACHE`.

Typical desktop x86-64 in 2026: L1d 32-48 KB per core, L2 1-2 MB per core, L3 16-128 MB shared. Apple Silicon: larger L1, very large shared L2.

## Exercise 2 — Sequential sum

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

## Exercise 3 — Random-access sum

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

Expect 30-100 ns per element — close to the RAM-latency cost. Each access misses cache.

## Exercise 4 — Cache cliffs

The transitions you see roughly correspond to spilling out of L1 (~32 KB), L2 (~1-2 MB), L3 (~32 MB). Below L1 you should see ~0.1-0.3 ns/elem. In L3 maybe 0.5-1.5 ns. Past L3, 0.5-3 ns (sequential, since prefetcher helps even from RAM).

For random-access cliffs (a more dramatic plot), repeat exercise 3 at sizes 1K, 10K, 100K, 1M, 10M, 100M. The transitions are sharper.

## Exercise 5 — Pointer chasing

```rust
use std::time::Instant;

struct Node { value: u64, next: Option<Box<Node>> }

fn build(n: usize) -> Box<Node> {
    let mut head = Box::new(Node { value: 1, next: None });
    for _ in 1..n {
        head = Box::new(Node { value: 1, next: Some(head) });
    }
    head
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

fn main() {
    // Playground-scaled. Use 1_000_000 locally for the real number.
    let n = 100_000;
    let head = build(n);
    let start = Instant::now();
    let s = sum(&head);
    let elapsed = start.elapsed();
    println!("sum = {s}, {elapsed:?}, {:.2} ns/elem",
             elapsed.as_nanos() as f64 / n as f64);
}
```

A `Vec<u64>` sum is roughly 1 ns/elem; the linked-list walk is roughly 50-100 ns/elem. The ratio is the L1-to-RAM gap from the prose.

Important: building the linked list with deep recursion would blow the stack at large N. The `build` function above uses a loop precisely so it can scale. The `sum` is also iterative — a recursive walk would blow the stack on the way down.

## Exercise 6 — Reading lscpu against your benchmarks

The transitions are noisy because:
- Cache levels overlap (a hot cache line might still be in L1 after spilling to L2).
- Hardware prefetchers help sequential reads.
- The OS may evict pages between runs.

If your noise is worse than your signal, run each measurement multiple times and take the median.
