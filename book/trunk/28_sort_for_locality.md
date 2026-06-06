# 28 - Sort for locality

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 28](../../concepts/glossary.md#28--sort-for-locality).*

<p align="center"><img src="../illustrations/optimization.jpg" alt="Optimization: minimize f(x) - sorting for locality is reordering for cost" style="max-height: 300px; max-width: 100%;"></p>

In §9 you learned the sort-breaks-indices bug. In §10 you fixed it with stable ids. In §23 you made id-to-slot lookup O(1). With those three pieces in place, the simulator can now do something it could not before: rearrange its rows for locality.

The principle is simple. Rows accessed near each other in time should sit near each other in memory. Two creatures that interact (collide, query a neighbour, broadphase against each other) should land on adjacent cache lines.

The classic technique is a *spatial sort*. Each creature's position is hashed to a spatial cell; the creatures table is sorted by cell. Reading "all creatures in cell C" becomes a contiguous range read.

```rust,no_run
fn spatial_cell(px: f32, py: f32, cell_size: f32) -> u32 {
    let x = (px / cell_size).floor() as i32;
    let y = (py / cell_size).floor() as i32;
    // Pack (x, y) into a single u32 hash. (Z-order or Hilbert work too.)
    ((x as u32 & 0xFFFF) << 16) | (y as u32 & 0xFFFF)
}

fn sort_creatures_for_locality(world: &mut World, cell_size: f32) {
    let c = &world.creatures;
    let mut order: Vec<usize> = (0..c.len()).collect();
    order.sort_by_key(|&i| spatial_cell(c.px[i], c.py[i], cell_size));
    apply_permutation(world, &order); // reorders every column; rewrites id_to_slot
}
```

Two creatures in the same spatial cell are now adjacent in `px` and `py`. The next-event system, which checks every creature against its spatial neighbours, can stride through the position columns and read neighbours from the same cache line.

The cost is the sort itself. At 1M creatures, an O(N log N) sort of `u32` keys takes ~10 ms. Done every tick this is too expensive - but typically the sort is done every ~100 ticks (or when accumulated motion exceeds a threshold), amortising to ~0.1 ms per tick. The savings on the inner loop dwarf the cost.

Other sort orders pay off in different regimes:

- **Sort by id.** Stable across runs; nice for debugging; but no locality benefit unless ids correlate with access patterns.
- **Sort by access frequency.** Hot creatures first; cold last. Useful only when the inner loop respects the order.
- **Sort by behaviour.** All hungry creatures together; all sleepy together. Mostly redundant in a presence-based system, where the hungry-driver iterates `hungry` directly (§19).

Sort cadence is its own decision. Sorting every tick is wasted work if the world is mostly stationary. Sorting once at startup is wrong if the world drifts. Most simulators trigger a re-sort when accumulated motion since the last sort exceeds a fraction of the cell size.

The sort interacts with stable references (§10): rebuilding `id_to_slot` is part of the sort's cost, not a separate concern. Code outside the sort holds *ids*, not slots; the sort moves slots, the map keeps the ids correct.

This is the pattern Bevy, Unity DOTS, Unreal's Mass Entities, and most production ECS engines use under the hood. Locality is paid up front (one sort) and amortised over many cache-friendly inner loops.

## Exercises

1. **Compute spatial cells.** Write `fn spatial_cell(pos, cell_size) -> u32`. Apply it to a 1 000-creature world. Print the histogram of cells.
2. **Sort by cell.** Implement `sort_creatures_for_locality`. Run it. Verify: print `pos[0..10]` - these should be near-neighbour positions.
3. **Maintain `id_to_slot`.** Update `id_to_slot` during the sort. Verify a previously held id still resolves to the right creature.
4. **Time `next_event` before and after.** Write a `next_event` system that, for each creature, scans the next 100 entries of `pos` for collisions. Time it pre-sort vs post-sort. The post-sort version should be measurably faster.
5. **Sort cadence.** Run a 10-tick simulation, sorting every tick. Run the same simulation, sorting every 10 ticks. Compare total cost. Find the cadence where sort cost equals `next_event` savings.
6. *(stretch)* **Z-order curve.** Replace the simple `(x, y)` packing with a Z-order (Morton) hash. Compare `next_event` timings. Z-order keeps spatially close cells close in the linear order; it usually outperforms simple stripe packing.

Reference notes in [28_sort_for_locality_solutions.md](28_sort_for_locality_solutions.md).

## What's next

[§29 - The wall at 10K → 1M](29_wall_10k_to_1m.md) is where these techniques start to bind. Code that ran fine at 10K stops running fine at 1M; the chapter is about finding out where and why.
