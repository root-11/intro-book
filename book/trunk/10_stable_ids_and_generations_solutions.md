# Solutions: 10 - Stable IDs and generations

Reference solutions for the exercises in [10_stable_ids_and_generations.md](10_stable_ids_and_generations.md). The deck exercises (1-3) are full; the rest are sketches.

## Exercise 1 - Add the id column

```rust
fn new_deck_with_ids() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u32>) {
    let mut suits     = Vec::with_capacity(52);
    let mut ranks     = Vec::with_capacity(52);
    let mut locations = Vec::with_capacity(52);
    for s in 0..4u8 {
        for r in 0..13u8 {
            suits.push(s);
            ranks.push(r);
            locations.push(0);
        }
    }
    let ids: Vec<u32> = (0..52).collect();
    (suits, ranks, locations, ids)
}

fn sort_deck_by_suit(
    suits: &mut Vec<u8>,
    ranks: &mut Vec<u8>,
    locations: &mut Vec<u8>,
    ids: &mut Vec<u32>,
) {
    let mut order: Vec<usize> = (0..suits.len()).collect();
    order.sort_by_key(|&i| suits[i]);
    *suits     = order.iter().map(|&i| suits[i]).collect();
    *ranks     = order.iter().map(|&i| ranks[i]).collect();
    *locations = order.iter().map(|&i| locations[i]).collect();
    *ids       = order.iter().map(|&i| ids[i]).collect();
}
```

The four columns are reordered in lockstep. Failing to reorder one of them is the bug.

## Exercise 2 - Find a card by id

```rust
fn slot_of(ids: &[u32], target: u32) -> Option<usize> {
    for i in 0..ids.len() {
        if ids[i] == target {
            return Some(i);
        }
    }
    None
}
```

The for-loop is intentional. Iterators with `position` do the same thing in one line; the loop version is the one to read first.

## Exercise 3 - Resolve the §9 bug

```rust
let (mut suits, mut ranks, mut locations, mut ids) = new_deck_with_ids();

// Player 1 holds *ids*, not slots
let player_1_hand: Vec<u32> = vec![3, 17, 21, 28, 41];

sort_deck_by_suit(&mut suits, &mut ranks, &mut locations, &mut ids);

for id in &player_1_hand {
    let slot = slot_of(&ids, *id).expect("card vanished");
    println!("{}", card_to_string(suits[slot], ranks[slot]));
}
```

The output is the same five cards as before the sort - different slots, same cards. The §9 bug is gone.

## Exercises 4-5 - Sketches

**Exercise 4.** `cards_held_by(locations, ids, player) -> Vec<u32>` walks the rows in lockstep and pushes `ids[i]` (not `i`) when `locations[i] == player`. Apply any rearrangement; the function still returns the same five ids. Using `slot_of` afterwards finds the cards.

**Exercise 5.** Take a `(id, gen)` reference *before* the swap-and-bump. After the operation, find the slot by id and read `gens[slot]`. The slot is the same; `gens[slot]` is `1` instead of `0`; the reference's `gen` is `0`; the dereference reports stale. The 52-card deck does not feel motivated yet - the simulator's `creature` table in §1 is where this stops feeling ceremonial.

## Exercise 6 - A tiny generational arena

The shape:

```rust
struct Creatures {
    pos:        Vec<f32>,
    gen:        Vec<u32>,
    id_to_slot: Vec<u32>, // id i -> current slot, or u32::MAX when removed
    free:       Vec<u32>, // slots awaiting reuse
    next_id:    u32,
}
```

`insert(pos)` either pops a slot from `free` (bumping `gen[slot]`) or pushes a new slot. `remove(slot)` pushes the slot into `free` and bumps `gen[slot]`. `get(slot, gen)` returns `Some(pos[slot])` only if `self.gen[slot] == gen`. The exercise is worth coding; the shape above is enough scaffolding.

## Exercise 7 - Comparing with `slotmap`

[`slotmap::SlotMap`](https://docs.rs/slotmap/) does the same thing with prettier ergonomics: keys pack `(slot, gen)` into a `u64`, the API uses `Index`/`IndexMut`, removals return the removed value, iterators are provided. None of these are *required* for the simulator; they are nice. Whether to adopt depends on whether you trust the crate to keep working ([§42](42_you_can_only_fix_what_you_wrote.md)). The from-scratch version above is small enough that you can fix it yourself if it ever breaks - which is the only reason to choose it over `slotmap`.
