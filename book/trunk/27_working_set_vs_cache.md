# 27 - Working set vs cache

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 27](../../concepts/glossary.md#27---working-set-vs-cache).*

<p align="center"><img src="../illustrations/bridge_clipboard.jpg" alt="Engineer mouse with clipboard - load capacity is what fits in the working set" style="max-height: 300px; max-width: 100%;"></p>

The *working set* of a loop is the data it touches per pass. The *cache hierarchy* (§1) is what holds that data. The two together decide the loop's speed.

Which cache level holds the working set decides the loop's speed. The numbers below are measured on the four reference machines (`code/README.md`), not theoretical - the modern desktop sits at the low end of each range, the Pi 4 at the high end. A flat streaming sum stays under ~0.5 ns/element in L1 (`cache_cliffs`). Motion's 20-byte loop, swept sequentially, measures ~0.3-4 ns/creature in L2 (10K creatures), ~0.4-10 ns in L3 (1M), and ~0.7-17 ns once it spills to RAM (10M)<sup>1</sup>. Sequential access stays bandwidth-bound and cheap on every machine; the expensive regime is *random* order, ~30-390 ns/creature at 10M<sup>3</sup>.

If you ran §1's exercises and exercise 2 below, you have your own machine's numbers. Treat the spread above as the envelope between slow and fast hardware, not an absolute.

Computing the working set is mechanical. Motion's inner loop reads `px, py: f32 = 8 bytes`, `vx, vy: f32 = 8 bytes`, `energy: f32 = 4 bytes`. Total: 20 bytes per creature. At N creatures, working set = 20 × N bytes.

| N           | working set | regime                    |
|-------------|-------------|---------------------------|
| 1 000       | 20 KB       | fits L1                   |
| 10 000      | 200 KB      | fits L2                   |
| 100 000     | 2 MB        | borderline L2/L3          |
| 1 000 000   | 20 MB       | fits L3, spills L2        |
| 10 000 000  | 200 MB      | spills L3, hits RAM       |

Sequential streaming barely shows the cliff: the prefetcher keeps motion bandwidth-bound, so per-creature time climbs only gently into RAM - roughly 0.3 ns at 10K rising to 0.7 ns at 10M on a modern desktop, 4 ns rising to 17 ns on a Pi 4. The steep cliff is in *random* order (exercise 5): at 10M creatures, random access costs ~30 ns/creature on the desktop and ~390 ns on the Pi. The per-machine ladder is `motion_working_set` in `code/measurement`; the numbers are in `code/README.md`.

This is what §4's "cliff" was about, made concrete for your simulator. The transition points are not magic - they are arithmetic over your cache sizes.

The working set is exactly the columns the loop reads, no more: SoA hands you that for free, because there is no wider row to trim ([§7](07_structure_of_arrays.md) again). From there, two levers push the cliff outward. A narrower field (§2) cuts the bytes per creature - dropping `energy` from `f64` to `f32` is 4 bytes per creature off the set. And a system that touches only a subset can subscribe (§26), cutting the *count* rather than the width. Motion reads every creature, so its lever is field width and access order, not subscription.

The implication is design discipline:

- Decide the target N before the schema. The schema must fit the cache that fits N.
- Audit the inner loops. Sum the bytes per row touched. Compare to your cache sizes.
- When you cross a transition, *measure* - do not assume. The prefetcher and the OS will sometimes save you, sometimes not.
- The narrowest field that holds the value (§2) is not aesthetic; it is the cliff's distance.

This is not premature optimisation. It is *layout-aware design* - making the schema fit the machine that will run it. A schema that ignores the cache works for small N and breaks at the scales the simulator was meant for.

## Measurements

Sequential motion stays cheap into RAM (the prefetcher keeps it bandwidth-bound); the cliff is in *random* order. The §27 "ns/element ladder" is really a random-access ladder. Full output: `code/README.md`.

| # | measurement | Ryzen 9 (modern) | i7-3610QM (2012) | i3-5010U (2015) | Pi 4 |
|---|---|---|---|---|---|
| 1 | motion sequential, ns/creature @ 1M | 0.44 | 1.70 | 3.30 | 10.05 |
| 2 | motion sequential, ns/creature @ 10M | 0.71 | 1.80 | 3.15 | 17.38 |
| 3 | motion random, ns/creature @ 10M | 31 | 80 | 84 | 392 |
| 4 | L1 vs L2, streaming motion | 1.02x | 1.19x | 1.20x | 1.08x |

## Exercises

1. **Compute your working sets.** For each system in your simulator, compute `bytes per row × N` for N = 1K, 10K, 100K, 1M, 10M. Note which cache level each falls into for your machine.
2. **Find your cliff.** Time motion at N = 1K, 10K, 100K, 1M, 10M. Plot ns-per-element against N. The transitions should match your cache sizes.
3. **The unused column costs nothing.** Add a `birth_t: f64` column that motion never reads. Recompute motion's working set and repeat exercise 2. The cliff should not move: in SoA a column a loop does not read sits in its own array, untouched, so it adds zero to that loop's working set. (In an array-of-structs world it would have widened every row and moved the cliff inward - the difference SoA buys you.)
4. **A wider field.** Change `energy: f32` to `energy: f64`. Recompute the working set. Repeat exercise 2. The cliff should move inward (closer to smaller N).
5. **Random vs sequential.** Repeat motion's loop with `for &i in random_indices` instead of `for i in 0..N`. At 10M creatures the per-element time rises by roughly 25-45× (random RAM access vs sequential). A single-pointer chase shows a wider gap; motion's is smaller because each creature amortises five columns.
6. *(stretch)* **The L1 sweet spot.** Find the N at which motion's working set fills L1 to roughly 75 %. Run the loop in tight repetition and compare to the closest L2-only neighbour. For *sequential* motion the difference is small - measured 1.0-1.2× across the four reference machines (`l1_sweet_spot`)<sup>4</sup>, because the loop is bandwidth-bound at both sizes and the prefetcher hides the L1/L2 boundary. The dramatic L1 win shows up when the access is random (exercise 5), not streaming.

Reference notes in [27_working_set_vs_cache_solutions.md](27_working_set_vs_cache_solutions.md).

## What's next

[§28 - Proximity is a property of position](28_proximity.md) answers the neighbour query the simulator has been deferring - and shows that the spatial index you reach for is one you never have to keep.
