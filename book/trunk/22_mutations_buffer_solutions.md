# Solutions: 22 - Mutations buffer; cleanup is batched

## Exercises 1-3 - Wire up the side tables

```rust,no_run
struct World {
    creatures: Creatures,   // the column set: px, py, vx, vy, energy, id (§23)
    to_remove: Vec<u32>,
    to_insert: Vec<CreatureRow>,
    id_to_slot: Vec<u32>,
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
    energy: &[f32], px: &[f32], py: &[f32],
    to_insert: &mut Vec<CreatureRow>,
    threshold: f32,
) {
    for i in 0..energy.len() {
        if energy[i] >= threshold {
            let half = energy[i] / 2.0;
            // Offspring inherit the parent's position; real ids are assigned at append (§23/§24).
            to_insert.push(CreatureRow { id: NEW_ID, px: px[i], py: py[i], vx: 0.0, vy: 0.0, energy: half });
            to_insert.push(CreatureRow { id: NEW_ID, px: px[i], py: py[i], vx: 0.0, vy: 0.0, energy: half });
        }
    }
}
```

(In practice, new ids come from the slot allocator from §24.)

## Exercise 4 - Implement cleanup

```rust,no_run
fn cleanup(world: &mut World) {
    // Removals first: swap_remove every column in lockstep.
    for id in world.to_remove.drain(..) {
        let slot = world.id_to_slot[id as usize] as usize;
        let cr = &mut world.creatures;
        let moved_id = *cr.id.last().unwrap();
        cr.px.swap_remove(slot); cr.py.swap_remove(slot);
        cr.vx.swap_remove(slot); cr.vy.swap_remove(slot);
        cr.energy.swap_remove(slot); cr.id.swap_remove(slot);
        world.id_to_slot[moved_id as usize] = slot as u32;
        world.id_to_slot[id as usize] = INVALID;
    }

    // Then insertions: scatter each new row into the columns.
    for c in world.to_insert.drain(..) {
        let slot = world.creatures.len() as u32;
        let cr = &mut world.creatures;
        cr.px.push(c.px); cr.py.push(c.py);
        cr.vx.push(c.vx); cr.vy.push(c.vy);
        cr.energy.push(c.energy);
        cr.id.push(c.id);
        world.id_to_slot[c.id as usize] = slot;
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

## Exercise 6 - Buffers keep their capacity

```rust,no_run
for tick in 0..100 {
    // ... systems push to to_remove / to_insert ...
    cleanup(&mut world); // drain(..) empties the buffers but keeps their capacity
    println!("{tick}: cap {} {}", world.to_remove.capacity(), world.to_insert.capacity());
}
```

`drain(..)` empties a `Vec` without freeing its buffer, so after the first few busy ticks `to_remove.capacity()` and `to_insert.capacity()` settle at the high-water mark and stop growing. Reusing the buffers means cleanup does zero allocation in steady state; a fresh `Vec` per tick would pay an allocation, and a later free, every tick on the hot path for no benefit. This is the [§4](04_cost_and_budget.md) budget again: the cheapest allocation is the one you already made.

## Exercise 7 - Graphics pipeline analogy

A renderer draws into a "back buffer" while the GPU is displaying the "front buffer". At vsync, the buffers swap (or the back buffer is presented). The display never sees a partially-drawn frame; the renderer never overwrites a frame mid-scan.

The simulator's tick is the same: systems write into `to_remove` and `to_insert` (the back buffer); cleanup applies them to the live tables (the front buffer); the next tick reads consistent state. The shape - accumulate, commit at the boundary - is universal.
