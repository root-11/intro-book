# Solutions: 23 - Index maps

## Exercise 1 - Build the map

```rust,no_run
const INVALID: u32 = u32::MAX;

struct World {
    creatures: Vec<CreatureRow>,
    id_to_slot: Vec<u32>, // length = high-water mark of ids ever issued
    next_id: u32,
}

fn append(world: &mut World, mut row: CreatureRow) -> u32 {
    let id = world.next_id;
    world.next_id += 1;
    while world.id_to_slot.len() <= id as usize {
        world.id_to_slot.push(INVALID);
    }
    row.id = id;
    let slot = world.creatures.len() as u32;
    world.creatures.push(row);
    world.id_to_slot[id as usize] = slot;
    id
}
```

The map grows lazily as new ids are issued. `INVALID` marks dead/never-used slots.

## Exercise 2 - O(1) presence query

`hungry_membership` runs parallel to the creature columns: one `bool` per slot, set when a creature is classified hungry and cleared when it stops. `is_hungry` reaches it through `id_to_slot`, so it is two array reads: the id-to-slot map, then the membership column.

```rust,no_run
fn is_hungry(world: &World, id: u32) -> bool {
    let slot = world.id_to_slot[id as usize] as usize;
    world.hungry_membership[slot]
}
```

`is_hungry` is two array reads - a handful of nanoseconds. Compare with the linear scan over the `hungry` list from §17, which is hundreds of microseconds at 1 M creatures. Because `hungry_membership` is a per-slot column, it moves in lockstep with `creatures` on every `swap_remove` and every sort, exactly like the other columns.

## Exercise 3 - Maintain on swap_remove

```rust,no_run
fn delete_by_id(world: &mut World, id: u32) {
    let slot = world.id_to_slot[id as usize] as usize;
    // The last row is the one swap_remove will move into `slot`.
    let moved_id = world.creatures.last().unwrap().id;
    world.creatures.swap_remove(slot);
    world.id_to_slot[moved_id as usize] = slot as u32;
    world.id_to_slot[id as usize] = INVALID;
}
```

No branch. Grab the last row's id *before* the swap, because that is the row `swap_remove` relocates into `slot`. When the deleted row was already the last one, `moved_id == id`: the slot write is redundant and the `INVALID` write that follows corrects it. Three writes per delete; ~12 bytes; at ~12 GB/s memory bandwidth each delete is well under 10 ns of bandwidth cost.

## Exercise 4 - Time the difference

At 1 M creatures, the linear-scan presence check costs ~1 ms. The indexed version costs ~50 ns. Run 100 000 queries per tick:

- Linear: 100 000 × 1 ms = 100 seconds. Impossible.
- Indexed: 100 000 × 50 ns = 5 ms. Fits 30 Hz with margin.

The factor of N (a million) shows up in real wall time.

## Exercise 5 - Bandwidth cost of cleanup

1 000 deletes per tick × 12 bytes each = 12 KB written per tick. At ~12 GB/s memory bandwidth, that is ~1 µs. Compare to a 30 Hz budget of 33 ms: ~0.003 % of the tick. The cleanup pass is essentially free; the system can afford to run every tick without measurable cost.

## Exercise 6 - Sort-for-locality compatibility

```rust,no_run
fn sort_creatures_for_locality(world: &mut World) {
    let mut order: Vec<usize> = (0..world.creatures.len()).collect();
    order.sort_by_key(|&i| spatial_cell(world.creatures[i].pos));

    // Apply the permutation to creatures.
    let new_creatures: Vec<_> = order.iter().map(|&i| world.creatures[i].clone()).collect();
    world.creatures = new_creatures;

    // Rewrite id_to_slot.
    for (new_slot, row) in world.creatures.iter().enumerate() {
        world.id_to_slot[row.id as usize] = new_slot as u32;
    }
}
```

Every slot moves; the map is rewritten entirely. External references to ids continue to work; references to slots would not (which is why nobody holds slots - they hold ids).

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
