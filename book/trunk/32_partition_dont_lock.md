# 32 — Partition, don't lock

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 32](../../concepts/glossary.md#32--partition-dont-lock).*

<p align="center"><img src="../illustrations/bridge_clipboard.jpg" alt="Bridges drawn as independent spans — partition into disjoint write-sets" style="max-height: 300px; max-width: 100%;"></p>

§31 said "disjoint write-sets parallelise freely". What if the system has to write *one* table from many threads? Motion at 1M creatures wants to update `creature.pos` for every creature; the table is one. Eight threads, one table — looks like a lock case.

It is not. The fix is to *partition the data*, not to lock the access.

Each thread takes a slice of the table. Thread *t* writes slots `t * N/8 .. (t+1) * N/8` and only those slots. The slices are disjoint by construction; no thread can write where another is writing. Inside each slice, a single thread is the writer — node 25's ownership rule still holds, just at the slice level instead of the table level.

```rust,no_run
use std::thread;

thread::scope(|s| {
    let chunk = pos.len().div_ceil(8);
    let pos_chunks    = pos.chunks_mut(chunk);
    let vel_chunks    = vel.chunks(chunk);
    let energy_chunks = energy.chunks_mut(chunk);

    for ((p, v), e) in pos_chunks.zip(vel_chunks).zip(energy_chunks) {
        s.spawn(move || {
            for i in 0..p.len() {
                p[i].0 += v[i].0 * dt;
                p[i].1 += v[i].1 * dt;
                e[i] -= burn * dt;
            }
        });
    }
});
```

`chunks_mut` is the standard library's splitter for `&mut [T]` into disjoint sub-slices. Each chunk is a `&mut [T]` with its own ownership; the borrow checker is satisfied. No `Mutex`, no atomic, no contention.

The choice of partitioning matters.

**By entity range** (above): simple, works when access is uniform. Each thread does the same work on a different slice.

**By spatial cell** (after sort-for-locality, [§28](28_sort_for_locality.md)): each thread takes a region of the world. Useful when interactions are local — neighbours-only collisions, regional behaviours. Threads at boundary cells need a small synchronisation step (or a halo region copied into each thread's input).

**By hash**: each thread takes ids whose hash modulo N matches its thread number. Useful when access is uniform but you want stable thread-to-data mapping across ticks.

**By workload weight**: each thread takes a number of rows weighted by *expected work* per row. Useful when rows differ in cost (e.g. some creatures have many neighbours, others have none). Requires a profiling pass or a heuristic.

The partition shape is the design choice; the partition mechanism (slicing) is trivial in Rust.

A subtlety: even with partitioning, *false sharing* (next section) can wreck the performance gains. If two threads write adjacent fields in the same cache line, the hardware coherency protocol forces them to take turns despite the logical independence. The fix is alignment, padding, or partitioning at cache-line boundaries — [§33](33_false_sharing.md) develops it.

The pattern is the right answer to "but I have one big table". You almost never need a lock; you need a partition.

## Exercises

1. **Partition motion.** Use `chunks_mut` to split `pos`, `vel`, and `energy` into 8 chunks. Run motion across 8 `thread::scope` threads. Compare to single-threaded.
2. **Speedup at scale.** Time partitioned motion at N = 100K, 1M, 10M creatures with 1, 2, 4, 8 threads. Plot speedup. Note where the bandwidth ceiling kicks in.
3. **Spatial partition.** After running [§28](28_sort_for_locality.md)'s sort-for-locality, partition by spatial region (e.g. 8 vertical stripes of the world). Each thread handles one stripe. Compare with the entity-range partition. Does the spatial version pay off for `next_event`?
4. **Workload-weighted partition.** Suppose 90 % of creatures are idle and 10 % are active. A naive partition gives most threads almost no work and one thread all the work. Implement a partition that balances *active* count, not *total* count. Time both.
5. *(stretch)* **`rayon::par_chunks_mut`.** Replace your manual `thread::scope` + `chunks_mut` with `pos.par_chunks_mut(chunk_size)`. Same result, less code. Note that rayon's work-stealing scheduler internally rebalances unbalanced workloads.

Reference notes in [32_partition_dont_lock_solutions.md](32_partition_dont_lock_solutions.md).

## What's next

[§33 — False sharing](33_false_sharing.md) names the hardware-level pitfall that can sink the partition pattern: two threads writing different fields in the same cache line slow each other down despite logical independence.
