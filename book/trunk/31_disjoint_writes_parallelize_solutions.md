# Solutions: 31 — Disjoint write-sets parallelize freely

## Exercise 1 — Two parallel systems

```rust,no_run
std::thread::scope(|s| {
    s.spawn(|| motion(&mut hot.pos, &hot.vel, &mut hot.energy, dt));
    s.spawn(|| food_spawn(&food_spawner, &mut food));
});
```

Both systems write disjoint tables. The borrow checker is satisfied; the threads cannot interfere. After the scope returns, both threads have finished and the world is consistent.

## Exercise 2 — Time the speedup

At 1M creatures: motion alone ≈ 3 ms; food_spawn alone ≈ 0.1 ms. Serial total ≈ 3.1 ms. Parallel total ≈ 3 ms (food_spawn finishes first; motion dominates). Speedup is close to 1× because the workload is dominated by motion.

When both systems are individually expensive (e.g. food at 1M items as well), serial ≈ 6 ms, parallel ≈ 3.5 ms (memory bandwidth shared); speedup ≈ 1.7×.

## Exercise 3 — A failing case

```rust,ignore
std::thread::scope(|s| {
    s.spawn(|| motion(&mut hot.pos, &hot.vel, &mut hot.energy, dt));
    s.spawn(|| apply_eat(&pending, &food, &mut hot.energy));
});
```

Rust rejects:

```
error[E0524]: two closures require unique access to `hot.energy` at the same time
```

The architecture's safety is the language's safety. Compile-time, not run-time.

## Exercise 4 — `rayon::join`

```rust,no_run
use rayon::join;

join(
    || motion(&mut hot.pos, &hot.vel, &mut hot.energy, dt),
    || food_spawn(&food_spawner, &mut food),
);
```

Identical behaviour to `thread::scope` for two-system parallelism. rayon adds value at finer-grained parallelism (`par_iter`, work-stealing); for the simulator's two-system pattern, `join` is sufficient.

## Exercise 5 — Per-thread segments

```rust,no_run
const N: usize = 8;
let mut segments: Vec<Vec<u32>> = (0..N).map(|_| Vec::new()).collect();
let chunk = energy.len().div_ceil(N);

thread::scope(|s| {
    for (t, segment) in segments.iter_mut().enumerate() {
        let energy_chunk = &energy[t * chunk .. ((t+1) * chunk).min(energy.len())];
        let ids_chunk    = &ids[t * chunk    .. ((t+1) * chunk).min(ids.len())];
        s.spawn(move || apply_starve(energy_chunk, ids_chunk, segment));
    }
});

let to_remove: Vec<u32> = segments.into_iter().flatten().collect();
```

Each thread writes its own `Vec<u32>`. Merge at the end via `flatten`. The merge is O(total) — same cost as building the single-threaded vec, but distributed across threads.

## Exercise 6 — Bandwidth ceiling

| threads | speedup |
|--------:|--------:|
|       1 |    1.0× |
|       2 |    1.8× |
|       4 |    3.2× |
|       8 |    4.5× |

Above 4-6 threads, memory bandwidth becomes the bottleneck. The 8-core ceiling is around 5×, not 8×, because all cores pull from the same memory bus. Compute-bound work scales further; bandwidth-bound work hits this ceiling.

For your machine, the ceiling depends on the memory controller's throughput. DDR5-5600 dual-channel tops out around 60 GB/s sustained; eight cores doing 50 GB/s of bandwidth-bound work each would need 400 GB/s — they cannot.
