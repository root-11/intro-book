# 27 - Working set vs cache

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 27](../../concepts/glossary.md#27--working-set-vs-cache).*

<p align="center"><img src="../illustrations/bridge_clipboard.jpg" alt="Engineer mouse with clipboard - load capacity is what fits in the working set" style="max-height: 300px; max-width: 100%;"></p>

The *working set* of a loop is the data it touches per pass. The *cache hierarchy* (§1) is what holds that data. The two together decide the loop's speed.

If the working set fits in L1 - typically 32 KB per core - the loop runs at near-arithmetic speed: ~0.1-0.5 ns per element. If it fits in L2 - typically 1-2 MB per core - it is ~0.5-2 ns. If it fits in L3 - typically 16-32 MB shared - it is ~1-5 ns. If it spills to RAM, sequential access drops to ~3-10 ns (prefetcher helping); random access drops to 50-200 ns (no prefetcher help).

These ranges are not theoretical. They are what your machine actually does, measured by §1's exercises. If you ran them, you have your numbers.

Computing the working set is mechanical. Motion's inner loop reads `pos: (f32, f32) = 8 bytes`, `vel: (f32, f32) = 8 bytes`, `energy: f32 = 4 bytes`. Total: 20 bytes per creature. At N creatures, working set = 20 × N bytes.

| N           | working set | regime                    |
|-------------|-------------|---------------------------|
| 1 000       | 20 KB       | fits L1                   |
| 10 000      | 200 KB      | fits L2                   |
| 100 000     | 2 MB        | borderline L2/L3          |
| 1 000 000   | 20 MB       | fits L3, spills L2        |
| 10 000 000  | 200 MB      | spills L3, hits RAM       |

Each transition costs roughly 3-5× in per-element time. At 10K, ~0.5 ns/elem. At 1M, ~3 ns/elem. At 10M, ~30 ns/elem (sequential).

This is what §4's "cliff" was about, made concrete for your simulator. The transition points are not magic - they are arithmetic over your cache sizes.

The hot/cold split (§26) shrinks the working set. Motion's working set went from 40 bytes per creature (full row) to 20 bytes (hot table only). This pushes the cliff outward by a factor of 2: a 2M-creature simulator now runs at L3-resident speeds instead of RAM-resident.

The implication is design discipline:

- Decide the target N before the schema. The schema must fit the cache that fits N.
- Audit the inner loops. Sum the bytes per row touched. Compare to your cache sizes.
- When you cross a transition, *measure* - do not assume. The prefetcher and the OS will sometimes save you, sometimes not.
- The narrowest field that holds the value (§2) is not aesthetic; it is the cliff's distance.

This is not premature optimisation. It is *layout-aware design* - making the schema fit the machine that will run it. A schema that ignores the cache works for small N and breaks at the scales the simulator was meant for.

## Exercises

1. **Compute your working sets.** For each system in your simulator, compute `bytes per row × N` for N = 1K, 10K, 100K, 1M, 10M. Note which cache level each falls into for your machine.
2. **Find your cliff.** Time motion at N = 1K, 10K, 100K, 1M, 10M. Plot ns-per-element against N. The transitions should match your cache sizes.
3. **Reduce the working set.** Apply hot/cold splits (§26) to push motion's footprint down. Repeat exercise 2. Did the cliff move?
4. **A wider field.** Change `energy: f32` to `energy: f64`. Recompute the working set. Repeat exercise 2. The cliff should move inward (closer to smaller N).
5. **Random vs sequential.** Repeat motion's loop with `for &i in random_indices` instead of `for i in 0..N`. The cliff drops by roughly a factor of 50-100 (random RAM access vs sequential).
6. *(stretch)* **The L1 sweet spot.** Find the N at which motion's working set fills L1 to roughly 75 %. Run the loop in tight repetition and compare to the closest L2-only neighbour. The L1-resident loop should be ~5-10× faster.

Reference notes in [27_working_set_vs_cache_solutions.md](27_working_set_vs_cache_solutions.md).

## What's next

[§28 - Sort for locality](28_sort_for_locality.md) puts the cache to work explicitly: rearrange your rows so accesses become more sequential.
