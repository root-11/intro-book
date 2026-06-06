# Solutions: 23 - Index maps

## Exercise 1 - Build the map

```rust,no_run
const INVALID: u32 = u32::MAX;

// The entity table: one column per field, aligned by slot.
struct Creatures {
    px: Vec<f32>, py: Vec<f32>,
    vx: Vec<f32>, vy: Vec<f32>,
    energy: Vec<f32>,
    id: Vec<u32>,
}
impl Creatures {
    fn len(&self) -> usize { self.id.len() }
}

struct World {
    creatures: Creatures,
    id_to_slot: Vec<u32>, // length = high-water mark of ids ever issued
    next_id: u32,
}

// `CreatureRow` is a transient value: the fields of one new creature,
// scattered into the columns by `append`, never stored.
fn append(world: &mut World, c: CreatureRow) -> u32 {
    let id = world.next_id;
    world.next_id += 1;
    while world.id_to_slot.len() <= id as usize {
        world.id_to_slot.push(INVALID);
    }
    let slot = world.creatures.len() as u32;
    let cr = &mut world.creatures;
    cr.px.push(c.px); cr.py.push(c.py);
    cr.vx.push(c.vx); cr.vy.push(c.vy);
    cr.energy.push(c.energy);
    cr.id.push(id);
    world.id_to_slot[id as usize] = slot;
    id
}
```

The map grows lazily as new ids are issued. `INVALID` marks dead/never-used slots.

## Exercise 2 - Build the sparse set

The membership table is a *sparse set*: a `dense` list of the present slots (what the hot loop walks) plus a `sparse` index from slot to its position in `dense`, or `INVALID`. No per-creature boolean.

```rust,no_run
struct Subscription {
    dense:  Vec<u32>,   // present slots
    sparse: Vec<u32>,   // slot -> position in dense, or INVALID
}

impl Subscription {
    fn is_member(&self, i: u32) -> bool {
        let p = self.sparse[i as usize];
        p != INVALID && self.dense[p as usize] == i
    }
    fn subscribe(&mut self, i: u32) {
        if self.is_member(i) { return; }
        self.sparse[i as usize] = self.dense.len() as u32;
        self.dense.push(i);
    }
    fn unsubscribe(&mut self, i: u32) {
        let p = self.sparse[i as usize];
        if p == INVALID { return; }
        let moved = *self.dense.last().unwrap();
        self.dense.swap_remove(p as usize);
        self.sparse[moved as usize] = p;
        self.sparse[i as usize] = INVALID;
    }
}
```

`is_member` is O(1) - two array reads - against the §17 linear scan's hundreds of microseconds at 1 M. And unlike a `Vec<bool>` flag, the sparse index hands back the dense *position*, which is what makes `unsubscribe` O(1) too. It is the same index-map shape as `id_to_slot`, pointing into the membership table instead of into the columns.

## Exercise 3 - Maintain on swap_remove

```rust,no_run
// reindex one slot-keyed table after a row moves from `old` to `new`.
fn reindex_move(sub: &mut Subscription, old: u32, new: u32) {
    let p = sub.sparse[old as usize];
    if p == INVALID { return; }          // the moved creature was not a member
    sub.dense[p as usize] = new;
    sub.sparse[new as usize] = p;
    sub.sparse[old as usize] = INVALID;
}

fn delete_by_id(world: &mut World, id: u32) {
    // The dead creature was already unsubscribed from every table at death (§18).
    let slot = world.id_to_slot[id as usize] as usize;
    let cr = &mut world.creatures;
    let moved_id = *cr.id.last().unwrap(); // the row swap_remove relocates into `slot`
    cr.px.swap_remove(slot); cr.py.swap_remove(slot);
    cr.vx.swap_remove(slot); cr.vy.swap_remove(slot);
    cr.energy.swap_remove(slot); cr.id.swap_remove(slot);
    let moved_old_slot = world.creatures.len() as u32; // length AFTER the removes

    // id_to_slot: re-find the moved entity; retire the dead one.
    world.id_to_slot[moved_id as usize] = slot as u32;
    world.id_to_slot[id as usize] = INVALID;

    // every slot-keyed table: the survivor moved from `moved_old_slot` to `slot`.
    reindex_move(&mut world.hungry, moved_old_slot, slot as u32);
    // ... and any other subscription the survivor might be in.
}
```

Two repairs per move, both O(1): `id_to_slot` re-finds the entity that relocated, and `reindex_move` rewrites that creature's slot wherever a subscription listed it. When the deleted row was already the last one, `moved_old_slot == slot` and `reindex_move` is a harmless no-op (the dead creature is no member). One `swap_remove` per column plus a handful of map writes - a few dozen bytes per delete, well under 10 ns at ~12 GB/s. This is the cost [§24](24_append_only_and_recycling.md) avoids entirely by not moving slots on death.

## Exercise 4 - Time the difference

At 1 M creatures, the linear-scan presence check costs ~1 ms. The indexed version costs ~50 ns. Run 100 000 queries per tick:

- Linear: 100 000 × 1 ms = 100 seconds. Impossible.
- Indexed: 100 000 × 50 ns = 5 ms. Fits 30 Hz with margin.

The factor of N (a million) shows up in real wall time.

## Exercise 5 - Bandwidth cost of cleanup

1 000 deletes per tick × 12 bytes each = 12 KB written per tick. At ~12 GB/s memory bandwidth, that is ~1 µs. Compare to a 30 Hz budget of 33 ms: ~0.003 % of the tick. The cleanup pass is essentially free; the system can afford to run every tick without measurable cost.

## Exercise 6 - Compaction compatibility

```rust,no_run
fn sort_creatures_for_locality(world: &mut World) {
    let n = world.creatures.len();
    let mut order: Vec<usize> = (0..n).collect();
    let cr = &world.creatures;
    order.sort_by_key(|&i| spatial_cell(cr.px[i], cr.py[i]));

    // Gather every column into the new order, in lockstep.
    let px = order.iter().map(|&i| cr.px[i]).collect();
    let py = order.iter().map(|&i| cr.py[i]).collect();
    let vx = order.iter().map(|&i| cr.vx[i]).collect();
    let vy = order.iter().map(|&i| cr.vy[i]).collect();
    let energy = order.iter().map(|&i| cr.energy[i]).collect();
    let id: Vec<u32> = order.iter().map(|&i| cr.id[i]).collect();
    world.creatures = Creatures { px, py, vx, vy, energy, id };

    // Rewrite id_to_slot: every slot moved.
    for (new_slot, &cid) in world.creatures.id.iter().enumerate() {
        world.id_to_slot[cid as usize] = new_slot as u32;
    }

    // Reindex every slot-keyed table through the same permutation.
    // order[new] = old, so invert it to map old slot -> new slot.
    let mut new_pos = vec![0u32; n];
    for (new, &old) in order.iter().enumerate() {
        new_pos[old] = new as u32;
    }
    reindex_subscription(&mut world.hungry, &new_pos);
    // ... and every other subscription.
}

fn reindex_subscription(sub: &mut Subscription, new_pos: &[u32]) {
    for s in sub.dense.iter_mut() { *s = new_pos[*s as usize]; }
    for p in sub.sparse.iter_mut() { *p = INVALID; }
    for (pos, &slot) in sub.dense.iter().enumerate() {
        sub.sparse[slot as usize] = pos as u32;
    }
}
```

Every slot moves, so *both* maps are rewritten: `id_to_slot`, so id-held references (a save, the network, the UI - [§26](26_subscription_tables.md)) still resolve; and each subscription's `dense`/`sparse`, so the slot-keyed memberships still point at the right creatures. Id references and slot references are each repaired by the sort, through their own map. Nothing holds a *bare* slot across the sort and expects it to survive - it survives only because cleanup rewrites it.

## Exercise 7 - From-scratch generational arena

```rust,no_run
struct SlotMap<T> {
    items: Vec<T>,
    generation:   Vec<u32>,
    free:  Vec<u32>,
}

impl<T: Clone + Default> SlotMap<T> {
    fn insert(&mut self, t: T) -> (u32, u32) {
        if let Some(slot) = self.free.pop() {
            self.items[slot as usize] = t;
            (slot, self.generation[slot as usize])
        } else {
            let slot = self.items.len() as u32;
            self.items.push(t);
            self.generation.push(0);
            (slot, 0)
        }
    }

    fn remove(&mut self, slot: u32) {
        self.generation[slot as usize] += 1;
        self.free.push(slot);
        self.items[slot as usize] = Default::default(); // optional
    }

    fn get(&self, slot: u32, generation: u32) -> Option<&T> {
        if self.generation[slot as usize] == generation { Some(&self.items[slot as usize]) } else { None }
    }
}
```

Compare with [`slotmap::SlotMap`](https://docs.rs/slotmap/) - the same machinery. The crate adds a packed key (slot + generation in one `u64`), an iterator API, and a `null()` sentinel. The shape is identical.
