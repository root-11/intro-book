# Solutions: 28 — Sort for locality

## Exercise 1 — Compute spatial cells

```rust
fn spatial_cell(pos: (f32, f32), cell_size: f32) -> u32 {
    let x = (pos.0 / cell_size).floor() as i32 as u32 & 0xFFFF;
    let y = (pos.1 / cell_size).floor() as i32 as u32 & 0xFFFF;
    (x << 16) | y
}
```

For 1 000 random creatures in a 100 × 100 world with `cell_size = 10`:

```rust,no_run
let mut hist = std::collections::BTreeMap::new();
for &p in &pos {
    *hist.entry(spatial_cell(p, 10.0)).or_insert(0) += 1;
}
```

Output: roughly 100 cells, each holding ~10 creatures (uniform distribution). A skewed distribution would cluster.

## Exercise 2 — Sort by cell

```rust,no_run
fn sort_creatures_for_locality(world: &mut World, cell_size: f32) {
    let n = world.pos.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| spatial_cell(world.pos[i], cell_size));

    apply_permutation_inplace(&mut world.pos, &order);
    apply_permutation_inplace(&mut world.vel, &order);
    apply_permutation_inplace(&mut world.energy, &order);
    apply_permutation_inplace(&mut world.id, &order);
    // also: gen, birth_t

    // Rebuild id_to_slot
    for (new_slot, &id) in world.id.iter().enumerate() {
        world.id_to_slot[id as usize] = new_slot as u32;
    }
}
```

After the sort, `pos[0..10]` are all in the same cell (or a small number of adjacent cells). Spatial neighbours are now memory neighbours.

## Exercise 4 — Time `next_event`

A naive `next_event` for each creature scans the next 100 entries of `pos`:

Pre-sort: those 100 entries are random — random RAM access, ~50–100 ns per check, ~5 µs per creature. At 1M creatures, ~5 seconds per tick. Impossible.

Post-sort: those 100 entries are spatial neighbours — sequential reads, ~1–2 ns per check, ~150 ns per creature. At 1M creatures, ~150 ms per tick. Still over budget for 30 Hz, but ~30× faster.

The combination with the sort cadence (exercise 5) usually brings this in budget.

## Exercise 5 — Sort cadence

Sort cost at 1M: ~10 ms. Per-tick savings on `next_event` post-sort: ~500 ms (compared to pre-sort). One sort every 50 ticks amortises sort cost to 0.2 ms/tick — vastly cheaper than the savings.

If the world's positions barely change tick-to-tick, you can sort even less often. If positions change wildly, you need more frequent sorts. The right cadence is data-dependent and worth measuring.

## Exercise 6 — Z-order

A simple stripe packing puts cells with the same x in adjacent linear positions. A Z-order (Morton) curve interleaves x and y bits, so spatial neighbours in 2D are usually neighbours in the linear order — even across "stripe" boundaries.

A Morton encoder for 16-bit x, y:

```rust
fn morton_2d(x: u16, y: u16) -> u32 {
    let mut x = x as u32;
    let mut y = y as u32;
    x = (x | (x << 8)) & 0x00FF00FF;
    x = (x | (x << 4)) & 0x0F0F0F0F;
    x = (x | (x << 2)) & 0x33333333;
    x = (x | (x << 1)) & 0x55555555;
    y = (y | (y << 8)) & 0x00FF00FF;
    y = (y | (y << 4)) & 0x0F0F0F0F;
    y = (y | (y << 2)) & 0x33333333;
    y = (y | (y << 1)) & 0x55555555;
    x | (y << 1)
}
```

Z-order typically gives ~10–30 % better cache locality than stripe packing on 2D access patterns. The cost is a few more bit operations per row.
