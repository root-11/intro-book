# Solutions: 33 - False sharing

## Exercise 1 - The pathological counter

```rust,no_run
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Instant;

const ITERS: u64 = 10_000_000;
let counters: [AtomicU64; 8] = std::array::from_fn(|_| AtomicU64::new(0));

let t = Instant::now();
thread::scope(|s| {
    for c in counters.iter() {
        s.spawn(|| {
            for _ in 0..ITERS {
                c.fetch_add(1, Ordering::Relaxed);
            }
        });
    }
});
println!("8 threads on one cache line: {:?}", t.elapsed());
```

Compare with a single-threaded loop doing 8 × ITERS increments on a single counter. On most chips the single-threaded version is *faster* than the 8-thread version. True negative scaling.

## Exercise 2 - The padded version

```rust,no_run
#[repr(align(64))]
struct Padded(AtomicU64);

let counters: [Padded; 8] = std::array::from_fn(|_| Padded(AtomicU64::new(0)));
```

Each `Padded` occupies a full cache line. The 8 padded counters span 8 cache lines. Re-time the parallel loop. It should now scale near-linearly - typically 6-8× faster than single-threaded.

## Exercise 3 - Per-thread `to_remove`

The data inside each thread's `Vec<u32>` lives on the heap, in regions the allocator distributes to be far apart (typically separated by at least one page = 4 KB). False sharing on the data is unlikely.

The `Vec` *headers* (the `(ptr, len, cap)` fields, 24 bytes each on 64-bit), if stored adjacent in a parent struct or array, *can* share a cache line. If you observe poor scaling, padding the parent struct fixes it:

```rust,no_run
#[repr(align(64))]
struct ThreadLocalVec(Vec<u32>);

let segments: [ThreadLocalVec; 8] = std::array::from_fn(|_| ThreadLocalVec(Vec::new()));
```

## Exercise 4 - Adjacent struct fields

```rust,no_run
struct TwoCounters { a: AtomicU64, b: AtomicU64 } // 16 bytes, one cache line

let counters = TwoCounters { a: AtomicU64::new(0), b: AtomicU64::new(0) };

thread::scope(|s| {
    s.spawn(|| {
        for _ in 0..ITERS { counters.a.fetch_add(1, Ordering::Relaxed); }
    });
    s.spawn(|| {
        for _ in 0..ITERS { counters.b.fetch_add(1, Ordering::Relaxed); }
    });
});
```

Two threads, separate fields, same cache line. Performance is similar to two threads contending on one field - the line is invalidated on every write either way.

Fix: pad each field to its own cache line.

```rust,no_run
#[repr(align(64))]
struct PaddedAtomic(AtomicU64);

struct TwoCounters { a: PaddedAtomic, b: PaddedAtomic } // 128 bytes, two lines
```

## Exercise 5 - Find your cache-line size

```sh
$ getconf LEVEL1_DCACHE_LINESIZE
64
```

x86 and most ARM Cortex-A: 64 bytes. Apple Silicon: 128 bytes for some cache levels. AArch64: variable; 64 or 128 depending on the chip. If padding to 64 bytes does not eliminate false sharing on a particular chip, try 128.

`crossbeam_utils::CachePadded<T>` checks the platform at compile time and pads to 128 on platforms that need it. For portable code, use it instead of hardcoded `#[repr(align(64))]`.
