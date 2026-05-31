# Solutions: 32 - Partition, don't lock

## Exercise 1 - Partition motion

```rust,no_run
use std::thread;

const N_THREADS: usize = 8;
let chunk = pos.len().div_ceil(N_THREADS);

thread::scope(|s| {
    for ((p, v), e) in pos.chunks_mut(chunk)
        .zip(vel.chunks(chunk))
        .zip(energy.chunks_mut(chunk))
    {
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

`chunks_mut` is the standard library's slice splitter. Each thread receives its own `&mut [T]`; the borrow checker is satisfied; no `Mutex` is required.

## Exercise 2 - Speedup at scale

| N      |  1 thread |  4 threads |  8 threads |
|-------:|----------:|-----------:|-----------:|
|  100 K |    0.3 ms |    0.10 ms |    0.08 ms |
|    1 M |    3.0 ms |    0.9 ms  |     0.6 ms |
|   10 M |     30 ms |     12 ms  |     12 ms  |

At 10M, the working set is 200 MB, well past L3. The loop is memory-bandwidth bound; adding threads stops helping past 4 cores. At 1M (in L3), 8 threads gives ≈ 5× speedup.

## Exercise 3 - Spatial partition

After [§28](28_sort_for_locality.md)'s spatial sort, creatures in the same region are adjacent in memory. Assigning each thread a region means the cache lines a thread loads are the cache lines that thread uses - no cross-thread cache traffic.

For systems with neighbour reads (`next_event`'s collision check), spatial partitioning is roughly 10-30 % faster than entity-range partitioning at scale, depending on neighbour density.

## Exercise 4 - Workload-weighted partition

A naive partition with 1M creatures and 100K active gives some threads all the work and others none. A weighted partition divides the *active* set:

```rust,no_run
let active: Vec<u32> = /* ids of active creatures */;
let active_per_thread = active.len().div_ceil(N_THREADS);

thread::scope(|s| {
    for chunk in active.chunks(active_per_thread) {
        s.spawn(move || drive_active(chunk, /* ... */));
    }
});
```

Each thread gets ≈ 12 500 active creatures. Cost-per-thread is balanced; total time ≈ `total_active_work / N_THREADS`. The naive version would have one thread doing all the active work → speedup ≈ 1×.

## Exercise 5 - `rayon::par_chunks_mut`

```rust,no_run
use rayon::prelude::*;

pos.par_chunks_mut(chunk_size)
   .zip(vel.par_chunks(chunk_size))
   .zip(energy.par_chunks_mut(chunk_size))
   .for_each(|((p, v), e)| {
       for i in 0..p.len() {
           p[i].0 += v[i].0 * dt;
           p[i].1 += v[i].1 * dt;
           e[i]   -= burn * dt;
       }
   });
```

Rayon's work-stealing scheduler handles unbalanced workloads automatically: a thread that finishes its chunk early steals work from a slower thread. For uniform work, performance matches manual `chunks_mut`; for unbalanced work, it can outperform.

The dependency cost: rayon brings in a small ecosystem (~50 KB binary impact, a global thread pool, a few transitive crates). For most simulators this is a clear win; for embedded targets or fully reproducible builds it may not be. The §41 from-scratch rule applies: write the manual `thread::scope` version first, then decide.
