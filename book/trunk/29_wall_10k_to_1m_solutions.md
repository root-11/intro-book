# Solutions: 29 — The wall at 10K → 1M

## Exercise 1-2 — Calibration and scale-up

Run the simulator at 10K for 1000 ticks: typical wall-clock ~1–3 s.

Run at 1M for 100 ticks (same total entity-ticks): expect ~10–30 s if the simulator is well-tuned, ~100–300 s if it has unaddressed walls.

The ratio is the diagnostic. Anything above ~15× indicates that constant-factor walls are binding.

## Exercise 3 — Profile

`cargo flamegraph` produces a `flamegraph.svg`. The wide frames at the top of the graph are the hottest functions. Common offenders at the 1M boundary:

- `<Vec as Extend>::extend` — uncapped reallocations
- `core::iter::any` over a `Vec<u32>` — linear scan that should be an indexed lookup
- `std::collections::HashMap::iter` — non-deterministic, slow at scale
- `core::fmt::Write` — `println!` in the hot path

## Exercise 4 — Pre-size `to_insert`

```rust,no_run
let estimated_max = creatures.len() / 50; // 2% reproduction rate, with margin
let to_insert: Vec<CreatureRow> = Vec::with_capacity(estimated_max);
```

Re-profile: the `Vec::extend` frames should shrink dramatically. A typical fix removes 5–15 % of total wall time.

## Exercise 5 — Hot/cold split

Apply §26's split. Re-profile. Cache-miss counters (visible in `perf stat -e cache-misses`) should drop by ~30–50 %. Wall-clock for motion drops by a similar fraction.

## Exercise 6 — Index maps

Replace `hungry.iter().any(|&id| id == target)` with `id_to_slot[target] != INVALID && hungry_membership[target]`. A function that was `O(N)` per call is now `O(1)`.

For a system that asks the question 100K times per tick at 1M creatures, this is the difference between 100 s and 0.005 s per tick.

## Exercise 7 — Find one new wall

Open-ended. Common discoveries the first time a reader runs this exercise:

- A `Vec<Box<T>>` somewhere in the code, costing one allocation per element.
- A `clone()` inside a hot loop where a `&` would do.
- A `String::from(...)` in a logging path that runs millions of times.
- A `HashMap::contains_key` where a `Vec<bool>` mask would be O(1) and 100× faster.

In each case, the fix is a one-line change once the wall is found. The challenge is finding the wall, not removing it.
