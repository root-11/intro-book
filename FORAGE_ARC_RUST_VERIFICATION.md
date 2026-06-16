# Forage arc: Rust verification (Phase 1 of porting the budget/scale/operability arc)

The Python edition (intro-py) developed the whole forage -> scale -> operability arc and its
"don't guess, measure" dogma. Before building the Rust chapters, the principal lessons were
re-run **in Rust and measured**, not assumed - because the editions diverge and some Python
receipts are language artifacts. Instrument: `code/measurement/src/bin/forage_scaling.rs`
(std-only, `std::thread::scope` for the parallel part - from-scratch-first, no rayon).

Measured on the dev box, 2026-06-15 (treat ratios as order-of-magnitude, re-run per machine):

## What holds (port the lesson AND the shape; re-measure the numbers)

- **H1 - binning is O(N) only at constant density.** Naive grid, fixed world: 7.6x then 10.8x
  per 3x population (quadratic). Constant density (world grown with N): 3.6x then 4.0x (linear).
  Identical conclusion to Python. The wall is geometric, not linguistic. (§28's density caveat.)
- **H2 - the per-cell representative restores O(N) even at fixed world.** 3.0x then 3.4x per 3x
  (linear), 1M targets in 29.7 ms. Collapsing each cell to one representative bounds the
  per-target candidate count at <=9 regardless of density. (§28's fix.)
- **H4 - slice-the-work parallelism is deterministic.** Single core == 8 threads bit-for-bit at
  every thread count (per-target argmin is order-free; no float reduction across targets). The
  determinism contract ports; the mechanism is cleaner than Python's (scoped threads over shared
  `&[f32]` and disjoint `&mut` chunks - no GIL, no `shared_memory`, no process plumbing). (§43/§16.)

## What diverges (the absence is itself the exhibit)

- **H3 - the cache-blocking win does NOT reproduce.** Whole 60.0 ms vs cache-blocked 59.3 ms =
  1.01x, where numpy got 1.4x. The numpy kernel built ~90M-entry candidate arrays and
  round-tripped them through RAM; blocking kept them in cache. The Rust loop keeps O(1) scratch
  per target and never materialises those temporaries, so there is no temporary-traffic wall to
  block. **The numpy temporary tax is a Python reality the compiler erases.** This is a teaching
  exhibit in its own right: the same algorithm, one runtime pays a memory tax the other does not.

## Cross-edition facts worth a sidebar

- **Rust is ~25x faster serial** (2M forage: ~58 ms vs numpy ~1450 ms). Same algorithm, no
  interpreter and no temporaries.
- **Rust's parallel scaling tops out lower** (6.0x at 8 threads vs numpy's blocked 9.4x), and the
  reason is instructive, not a defeat: the Rust kernel is so compute-light that it meets the
  memory-bandwidth ceiling sooner. Faster serial code parallelises *less*, because there is less
  work to hide the bus behind. (Connects to `scope_speedup.rs`'s existing note: bandwidth-bound
  passes show less than 2x; one thread can saturate the bus.)
- The lexsort-59% profiling beat (Python) has no Rust analogue: the Rust kernel does a per-target
  min in the loop, there is no sort to profile out. The profiling *lesson* (the hotspot is not
  where intuition points) still belongs in the Rust book, but it needs a different Rust receipt.

## Implication for the Rust book build (Phase 2)

The arc ports with the structure intact and most lessons holding. The chapters draw on the
verified receipts above:
- §28: density caveat (H1) + representative fix (H2), Rust numbers.
- §43: scale sweep as the envelope; the determinism test single == multi (H4).
- §4: budget-as-a-curve (re-measure the Rust framerate curve from a Rust sim tick - NOT yet
  built; the Rust `sim` crate is still spec-only, so the full-tick framerate is the next gap).
- Part II / staircase: the walls, detectors, levers, soft-vs-hard. The H3 divergence becomes a
  Rust-specific sidebar ("the temporary tax the compiler erases"), and the bandwidth ceiling is
  reached at a different point (H4) - re-measure, do not copy the Python staircase heights.

## Rust reference sim + framerate curve (gap closed 2026-06-15)

The `sim` crate is no longer spec-only: `code/sim/` now holds a faithful Rust port of the Python
reference (`src/lib.rs`), same systems / lifecycle / arena, std-only splitmix64 PRNG. It passes the
same gates as Python: determinism (§16, two runs bit-identical) and replay (§37, the log
reconstructs the live population). Dynamics match Python closely (grazers peak ~833 then go extinct,
grass blooms; Python was 0/831/0, Rust 0/833/0) despite a different PRNG - the model is faithfully
ported. `cargo run --release --bin sim [-- --check]`.

**The reference is the PAIR, and the diff is the extendability lesson** (Python kept this as
`sim1b.py` vs `sim2b.py`). Subscriptions live in a generic ORDERED registry (`Vec<Sub>`); `apply`
maintains every entry knowing none of their names. So the two species and the predator are two
bins sharing the lib:
- `src/bin/sim.rs` - grass + grazers.
- `src/bin/sim2.rs` - adds a predator that hunts grazers.

Diffing them is the lesson: the predator is one `register`, one `herd_move`, one `forage` edge
(predators eat grazers - the SAME write as grazers eating grass), and the founder count. `apply`,
every system, and the lib are untouched - a new trophic level is a subscription and a forage edge,
not surgery. Both bins pass the gates. The predator shows a trophic cascade: grazers peak 833->619,
grass blooms 17856->66560, predators peak 213. (Had `apply` hardcoded the two subscriptions - as an
early draft of the port did - the predator would have required editing the join, which would have
falsified the lesson; the registry is what keeps the diff honest.)

**Framerate curve** (`src/bin/scale_sweep.rs`, constant density, world = sqrt(N/0.4)):

| live | Rust tick | Rust Hz | Python Hz (same machine) |
|---|---|---|---|
| 62k | 5.0 ms | 201 | 60 |
| 187k | 15.9 ms | 63 | 21.5 |
| 623k | 60.3 ms | 16.6 | 4.5 |
| 1.9M | 216 ms | 4.6 | 1.0 |
| 6.2M | 887 ms | 1.1 | 0.2 |

Same shape (Hz ~ 1/N, O(N) tick), uniformly ~3-5x faster. The staircase heights shift right by
that factor: **30 Hz holds to ~390k live (Python ~125k); the 15 Hz tolerance to ~785k (Python
~250k).** The Rust staircase chapter uses these heights, not the Python ones - same curve, shifted
by the language.

Remaining for the staircase chapter (smaller gaps): the memory-footprint ceiling (64 B/slot is a
numpy-dtype fact; the Rust struct-of-Vecs layout must be measured) and the per-system breakdown at
scale (which system binds the Rust tick - a `scale_sweep` extension timing each system).
