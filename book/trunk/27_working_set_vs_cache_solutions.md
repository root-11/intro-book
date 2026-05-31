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
fn motion_bench(pos: &mut [(f32, f32)], vel: &[(f32, f32)], energy: &[f32], dt: f32) {
    for i in 0..pos.len() {
        pos[i].0 += vel[i].0 * dt;
        pos[i].1 += vel[i].1 * dt;
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

## Exercise 3 - Reduce the working set

After hot/cold splitting, motion's working set drops from 20 B to (still 20 B in this case, since we only kept the hot fields). If the original loop also read `birth_t` (8 B unnecessarily), the split drops from 28 B to 20 B - a roughly 30 % shrink, pushing the cliff outward by ~30 % in N.

## Exercise 4 - A wider field

Switching `energy: f32` → `energy: f64` adds 4 B per row → 24 B total. For a fixed L3 size, the maximum N that fits drops by ~17 %. The cliff moves inward.

## Exercise 5 - Random vs sequential

Sequential RAM access: ~3-10 ns per element (prefetcher helping). Random RAM access: ~50-200 ns per element. The cliff drops by 10-50× depending on hardware. The same algorithm, two access patterns; orders of magnitude apart.

## Exercise 6 - The L1 sweet spot

L1 is 32 KB. 32 KB / 20 B per row ≈ 1 600 rows fills L1. At 75 % fill, ~1 200 rows. Run motion at N = 1 200 and at N = 10 000. The 1 200 case stays in L1 throughout; the 10 000 case spills.

Typical: 1 200 → 0.2 ns/elem; 10 000 → 0.5 ns/elem. About 2.5× faster for the L1-resident loop. The exact ratio depends on how aggressive the compiler is at vectorising - both loops should auto-vectorise to AVX2 or AVX-512.
