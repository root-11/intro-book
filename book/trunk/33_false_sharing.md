# 33 — False sharing

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 33](../../concepts/glossary.md#33--false-sharing).*

<p align="center"><img src="../illustrations/multimeter.jpg" alt="A mouse with a multimeter — false sharing is a precision-of-cost-measurement problem" style="max-height: 300px; max-width: 100%;"></p>

You partitioned the table. Each thread writes its own disjoint slice. The work is balanced. The speedup is... 1.2× on 8 cores. Where did the parallelism go?

Probably to *false sharing*.

The CPU cache works on 64-byte *cache lines*. When a thread writes to address X, the cache controller invalidates that line everywhere else — every other CPU's cache must throw away its copy and reload. If two threads are writing to *different* addresses but in the *same* cache line, every write triggers an invalidation on the other thread's cache. The threads slow each other down without ever logically conflicting.

A pathological case: eight threads each incrementing one entry in `[u64; 8]`. The array is exactly 64 bytes — one cache line. All eight threads write to that line. Every write invalidates the other seven caches. The threads run *slower* together than one thread alone — true negative scaling.

The fix is to put each thread's data on its own cache line. Either pad the underlying value:

```rust,no_run
#[repr(align(64))]
struct CachePadded(u64);

let counters: [CachePadded; 8] = std::array::from_fn(|_| CachePadded(0));
```

Or split into separate allocations (each `Vec` lives in its own heap region, normally far apart). Or use thread-local storage. Or partition at cache-line granularity from the start.

The Rust idiom for the padding pattern is `crossbeam_utils::CachePadded<T>` from the `crossbeam-utils` crate, which exists for exactly this case.

False sharing is a hardware concern, not a Rust concern. The borrow checker sees no problem with eight `&mut u64` references at disjoint addresses; the hardware sees one cache line and serialises the access. The bug is invisible at the language level. It shows up only as performance — the parallel version is mysteriously slow.

How to find it. Profile with `perf stat -e cache-misses` (or its equivalent on your platform). False sharing produces high `cache-misses` despite supposedly disjoint writes. If profiling shows your parallel system has surprisingly high cache traffic, false sharing is a likely cause.

How to avoid it without painful debugging. Make per-thread data structurally separate from the start. Each thread gets its own `Vec<T>` (separate allocation, separate cache lines). Merge at the end ([§31](31_disjoint_writes_parallelize.md)'s pattern for `to_remove` with per-thread segments). The merge is cheap; the false-sharing avoidance is structural.

The takeaway: physical layout matters even for logically disjoint data. Two `&mut`s pointing at different addresses do not parallelise freely if those addresses are within 64 bytes. The fix is alignment or separation. The detection is profiling.

## Exercises

1. **The pathological counter.** Build the 8-thread case with eight `AtomicU64` in one cache line:
   ```rust,no_run
   use std::sync::atomic::{AtomicU64, Ordering};
   let counters: [AtomicU64; 8] = std::array::from_fn(|_| AtomicU64::new(0));
   // ... 8 threads, each incrementing counters[t] in a tight loop
   ```
   Time the parallel version against a single-threaded loop doing the same total work. The parallel version should be *slower* — true negative scaling.
2. **The padded version.** Pad each counter to its own cache line via `#[repr(align(64))]`. Re-run. The parallel version should now scale near-linearly with thread count.
3. **A real example.** In your simulator's per-thread `to_remove` segments ([§31](31_disjoint_writes_parallelize.md) exercise 5), check whether the thread-local `Vec<u32>` allocations might land in the same cache line. They normally should not — separate `Vec`s have their data on the heap, which the allocator distributes — but if performance is unexpectedly poor, this is one place to look.
4. **Adjacent struct fields.** Build a struct with two `u64` fields. Spawn two threads, one writing each field. They are at adjacent addresses, same cache line. Time vs. two `u64` in separate allocations.
5. *(stretch)* **Find your cache-line size.** `getconf LEVEL1_DCACHE_LINESIZE` on Linux. Verify it is 64. Some chips use 128-byte lines (especially Apple Silicon at certain levels); if you are on one, `#[repr(align(64))]` is not enough — you need 128.

Reference notes in [33_false_sharing_solutions.md](33_false_sharing_solutions.md).

## What's next

[§34 — Order is the contract](34_order_is_the_contract.md) ties parallelism back to the determinism rule from [§16](16_determinism_by_order.md): parallelism is allowed *inside* a step, never *across* steps.
