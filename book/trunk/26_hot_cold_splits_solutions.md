# Solutions: 26 — Hot/cold splits

## Exercise 1 — Audit access patterns

For the simulator's eight systems, the field accesses look roughly like this:

| system            | reads                                | writes               |
|-------------------|--------------------------------------|----------------------|
| `motion`          | `pos`, `vel`, `energy`               | `pos`, `energy`      |
| `food_spawn`      | `food_spawner.region`                | `food` (via insert)  |
| `next_event`      | `pos`, `food.pos`, `creature.energy` | `pending_event`      |
| `apply_eat`       | `pending_event`, `food.value`        | `to_remove`, `energy`|
| `apply_reproduce` | `pending_event`, `pos`, `energy`     | `to_insert`          |
| `apply_starve`    | `pending_event`, `id`                | `to_remove`          |
| `cleanup`         | `to_remove`, `to_insert`, `id`, `gen`| every column         |
| `inspect`         | every column                         | (nothing)            |

Hot fields (read by motion, next_event, apply_eat, apply_reproduce, apply_starve every tick): `pos`, `vel`, `energy`. Cold: `birth_t`, `id`, `gen` (cleanup and inspect only).

## Exercise 2 — Build the split

```rust,no_run
struct CreatureHot {
    pos:    Vec<(f32, f32)>,
    vel:    Vec<(f32, f32)>,
    energy: Vec<f32>,
}

struct CreatureCold {
    birth_t: Vec<f64>,
    id:      Vec<u32>,
    gen:     Vec<u32>,
}

fn append(hot: &mut CreatureHot, cold: &mut CreatureCold, row: CreatureRow) {
    hot.pos.push(row.pos);
    hot.vel.push(row.vel);
    hot.energy.push(row.energy);
    cold.birth_t.push(row.birth_t);
    cold.id.push(row.id);
    cold.gen.push(row.gen);
}
```

Both tables share the slot index. `hot.pos[17]` and `cold.id[17]` describe the same creature.

## Exercise 3 — Time motion at 1M

Pre-split: motion's per-tick cost ≈ 3 ns/elem × 1M = 3 ms. Post-split: ≈ 1.5 ns/elem × 1M = 1.5 ms. The factor of 2 is roughly the bandwidth saved by not reading `birth_t`, `id`, `gen` on each iteration.

## Exercise 4 — Cleanup must touch both

```rust,no_run
fn delete_creature(hot: &mut CreatureHot, cold: &mut CreatureCold, slot: usize) {
    hot.pos.swap_remove(slot);
    hot.vel.swap_remove(slot);
    hot.energy.swap_remove(slot);
    cold.birth_t.swap_remove(slot);
    cold.id.swap_remove(slot);
    cold.gen.swap_remove(slot);
}
```

Six `swap_remove` calls instead of three. Still O(6) per delete; the cost is unchanged. Alignment is preserved across both tables because the same slot is removed in lockstep.

## Exercise 5 — A bad split

If `energy` is moved to `creature_cold`, motion's loop now misses cache on every read of `energy` — a cache line per row instead of one cache line per several rows. The bandwidth saved on `birth_t` is dwarfed by the bandwidth lost on `energy`. Motion gets ~1.3× *slower*, not faster.

The lesson: which fields are hot is decided by the inner loops, not by the data model.

## Exercise 6 — The all-fields case

A serialiser reads every field. With the split, it reads two tables instead of one — the cost of the second `Vec` traversal plus the cost of the second range of cache lines. About 5–10% overhead vs the unsplit version.

This is fine. The serialiser does not run every tick; it runs at snapshot points. The hot path runs every tick and pays the much larger savings. Average-case cost goes down even though the worst-case cost goes up slightly.
