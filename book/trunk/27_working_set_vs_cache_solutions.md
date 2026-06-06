# Solutions: 27 - Working set vs cache

## Exercise 1 - Compute working sets

For motion (reads `pos: 8B`, `vel: 8B`, `energy: 4B` = 20 B per row):

| N           | working set | regime                    |
|-------------|-------------|---------------------------|
| 1 000       | 20 KB       | fits L1                   |
| 10 000      | 200 KB      | fits L2                   |
| 100 000     | 2 MB        | borderline L2/L3          |
| 1 000 000   | 20 MB       | fits L3, spills L2        |
| 10 000 000  | 200 MB      | spills L3, hits RAM       |

For `apply_eat` (reads `pending: 24B`, `food: 8B`, `energy: 4B` ≈ 36 B):

A pending event is only active for one tick, so the working set is `36 × pending_count`, not `36 × N`. Even with 10K events per tick, the working set is 360 KB - comfortably L2.

## Exercise 2 - Find your cliff

Run the loop:

```rust,no_run
fn motion_bench(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], energy: &[f32], dt: f32) {
    for i in 0..px.len() {
        px[i] += vx[i] * dt;
        py[i] += vy[i] * dt;
        // (dummy use of energy to keep it in the working set)
        std::hint::black_box(energy[i]);
    }
}
```

Plot ns-per-element vs N. On a typical desktop:

- 1K: ~0.3 ns
- 10K: ~0.5 ns
- 100K: ~1 ns
- 1M: ~2.5 ns
- 10M: ~6 ns
- 100M: ~10 ns

The transitions at 100K (L2 boundary) and 10M (L3 boundary) are the visible cliffs.

## Exercise 3 - The unused column costs nothing

Adding `birth_t: f64` leaves motion's working set at 20 B per creature, and the cliff does not move. Motion reads `px, py, vx, vy, energy`; `birth_t` lives in its own array that the loop never touches, so it contributes nothing to the bytes streamed per pass. In an array-of-structs layout the same field would have widened every record from 20 B to 28 B and pulled the cliff inward by ~30 % in N. That gap is the SoA win (§7): the working set is the columns you read, never the columns you store.

## Exercise 4 - A wider field

Switching `energy: f32` → `energy: f64` adds 4 B per row → 24 B total. For a fixed L3 size, the maximum N that fits drops by ~17 %. The cliff moves inward.

## Exercise 5 - Random vs sequential

Sequential RAM access stays bandwidth-bound: motion's loop measures ~0.7-17 ns/creature at 10M (modern desktop to Pi 4, the prefetcher helping). Random RAM access is ~30-390 ns/creature - the prefetcher cannot help. The gap is ~25-45× depending on hardware (`motion_working_set`, `code/README.md`). The same algorithm, two access patterns; an order of magnitude or more apart.

## Exercise 6 - The L1 sweet spot

L1 is 32 KB. 32 KB / 20 B per row ≈ 1 600 rows fills L1. At 75 % fill, ~1 200 rows. Run motion at N = 1 200 and at N = 10 000. The 1 200 case stays in L1 throughout; the 10 000 case spills.

Typical: 1 200 → 0.2 ns/elem; 10 000 → 0.5 ns/elem. About 2.5× faster for the L1-resident loop. The exact ratio depends on how aggressive the compiler is at vectorising - both loops should auto-vectorise to AVX2 or AVX-512.
