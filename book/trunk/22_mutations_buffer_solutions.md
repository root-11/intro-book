# Solutions: 22 - Mutations buffer; cleanup is batched

## Exercises 1-3 - Wire up the side tables

```rust,no_run
struct World {
    creatures: Vec<CreatureRow>,   // simplified; really six columns
    to_remove: Vec<u32>,
    to_insert: Vec<CreatureRow>,
    // ...
}
```

`apply_starve` becomes:

```rust,no_run
fn apply_starve(energy: &[f32], ids: &[u32], to_remove: &mut Vec<u32>) {
    for i in 0..energy.len() {
        if energy[i] <= 0.0 { to_remove.push(ids[i]); }
    }
}
```

`apply_reproduce` becomes:

```rust,no_run
fn apply_reproduce(
    energy: &[f32], pos: &[Pos], ids: &[u32],
    to_insert: &mut Vec<CreatureRow>,
    threshold: f32,
) {
    for i in 0..energy.len() {
        if energy[i] >= threshold {
            let half = energy[i] / 2.0;
            to_insert.push(CreatureRow { id: NEW_ID, pos: pos[i], energy: half, /* ... */ });
            to_insert.push(CreatureRow { id: NEW_ID, pos: pos[i], energy: half, /* ... */ });
        }
    }
}
```

(In practice, new ids come from the slot allocator from §24.)

## Exercise 4 - Implement cleanup

```rust,no_run
fn cleanup(world: &mut World) {
    // Removals first.
    for id in world.to_remove.drain(..) {
        let slot = world.id_to_slot[id as usize] as usize;
        let moved_id = world.creatures.last().unwrap().id;
        world.creatures.swap_remove(slot);
        world.id_to_slot[moved_id as usize] = slot as u32;
        world.id_to_slot[id as usize] = INVALID;
    }

    // Then insertions.
    for row in world.to_insert.drain(..) {
        world.creatures.push(row);
        world.id_to_slot[row.id as usize] = (world.creatures.len() - 1) as u32;
    }
}
```

Removals first because freed slots are not reused (yet - that's §24's recycling). If you insert first, you may insert into a slot you are about to delete from.

## Exercise 5 - The dedup question

Without dedup, two systems pushing id 42 cause cleanup to call `swap_remove` twice on the same id. The first call removes the row. The second call attempts to look up `id_to_slot[42]`, finds `INVALID`, and... what? Either it panics, or it silently no-ops. Most simulators choose silent no-op via an early-return:

```rust,no_run
let slot = world.id_to_slot[id as usize];
if slot == INVALID { continue; }
```

With dedup (a `HashSet<u32>` collected before the cleanup loop), the second call is never made. Both approaches work; the no-op approach is cheaper for most simulators.

## Exercise 6 - Tick-delayed visibility

Add `age_in_ticks: Vec<u32>` to creatures. Set new rows to 0 in `to_insert`. After cleanup, increment every entry's `age_in_ticks` by 1.

A creature inserted in tick 5: enters cleanup at the end of tick 5, gets `age_in_ticks = 0`, then gets incremented to 1 by the end-of-tick increment. In tick 6 the creature has `age_in_ticks = 1`; it is the first tick where systems read it. The newborn never received tick 5's update.

## Exercise 7 - Graphics pipeline analogy

A renderer draws into a "back buffer" while the GPU is displaying the "front buffer". At vsync, the buffers swap (or the back buffer is presented). The display never sees a partially-drawn frame; the renderer never overwrites a frame mid-scan.

The simulator's tick is the same: systems write into `to_remove` and `to_insert` (the back buffer); cleanup applies them to the live tables (the front buffer); the next tick reads consistent state. The shape - accumulate, commit at the boundary - is universal.
