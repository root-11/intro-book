# 10 - Stable IDs and generations

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 10](../../concepts/glossary.md#10---stable-ids-and-generations).*

<p align="center"><img src="../illustrations/hard_hat_repeat.jpg" alt="MEASURE / CALCULATE / DESIGN / BUILD / REPEAT - generations cycle on a stable handle" style="max-height: 300px; max-width: 100%;"></p>

In [§9](09_sort_breaks_indices.md) you watched a player's reference go stale because they were holding *slots*, not *names*. The fix is to give each row a name - a stable identifier - that travels with the row when it moves.

A stable id is one extra column. For the deck:

```rust
let mut ids: Vec<u32> = (0..52).collect();
```

Now every card has both a *slot* (its current index in the columns) and an *id* (its name). When you sort the columns, you reorder `ids` in lockstep:

```rust
// sort by suit, taking ids along
let mut order: Vec<usize> = (0..52).collect();
order.sort_by_key(|&i| suits[i]);

let new_suits:     Vec<u8>  = order.iter().map(|&i| suits[i]).collect();
let new_ranks:     Vec<u8>  = order.iter().map(|&i| ranks[i]).collect();
let new_locations: Vec<u8>  = order.iter().map(|&i| locations[i]).collect();
let new_ids:       Vec<u32> = order.iter().map(|&i| ids[i]).collect();
```

The card with `id = 17` is still the same card - its suit, rank, and location are unchanged. It is just at a different *slot*.

To find a card by id, scan the `ids` column:

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

That is O(N), which is fine for a 52-card deck and slow for a million creatures. The fix - an `id_to_slot` map maintained on every rearrangement - is [§23 - Index maps](23_index_maps.md). For now the linear scan is honest pedagogy.

## Generations: when slots are reused

The deck is constant-quantity. Always 52 cards, never more, never less. The simple `id` column is enough.

For variable-quantity tables - creatures that are born and die, packets that arrive and are processed, sessions that come and go - slots get *reused*. A new creature is born in the slot that just held a dead one. Now imagine a player who held a reference to the dead creature: their reference points at the same slot with the same id, but the row at that location is a different creature.

One more column fixes it: a `generation` counter that increments every time a slot is recycled. A reference is now a pair `(id, generation)`. To dereference it, you check that the row's stored `generation` still matches the reference's `generation`. If it does, the reference is live. If it does not, the slot has been recycled since the reference was taken, and the dereference returns `None`.

```rust
struct CreatureRef {
    id:  u32,
    generation: u32,
}

fn get(creatures: &Creatures, r: CreatureRef) -> Option<usize> {
    let slot = creatures.id_to_slot.get(r.id as usize).copied()?;
    if creatures.generation[slot] == r.generation {
        Some(slot)
    } else {
        None
    }
}
```

This is the pattern called a *generational arena*. It is the single mechanism behind every "handle" type in every ECS engine: Bevy's `Entity`, `slotmap::SlotMap`, C++'s `entt::registry`. They differ in details - width of the id, packing into a `u64`, generation overflow handling - but the structural idea is the same: one column for identity, one for generation, a checked dereference.

That is enough machinery for the rest of the book to lean on. Sorting now works because the id column travels with the row. Deletion now works because the generation counter rejects stale references. Append-only and recycling tables ([§24](24_append_only_and_recycling.md)) are two policies on the same machinery.

> [!NOTE]
>
> *The strong form of [§5](05_identity_is_an_integer.md) still applies.* If your row has a natural key - `(suit, rank)`, `(date, ticker)`, `(species, position)` - you do not need a surrogate id. The card-game deck can be played without ids; the reference that survives is the `(suit, rank)` pair, because the data is unique by construction. Surrogate ids and generations earn their keep when the data has no natural unique tuple - which is most of the time once you start producing rows at runtime.

## Exercises

These extend the §5 deck once more, then take a step toward the simulator's variable-quantity case.

1. **Add the id column.** Add `let ids: Vec<u32> = (0..52).collect();` to your deck. Modify your sort so it reorders `ids` along with the other columns. Verify the original ids are still there, just in a new order.
2. **Find a card by id.** Implement `slot_of(ids: &[u32], target: u32) -> Option<usize>` as in the prose. Use it to look up the card with `id = 17` after a sort.
3. **Resolve the §9 bug.** With player 1 holding *ids* `[3, 17, 21, 28, 41]` (not slots), sort the deck. Use `slot_of` to translate ids to slots and print the hand. Confirm the cards are unchanged.
4. **Permutation-friendly hand query.** Rewrite `cards_held_by(locations, ids, player) -> Vec<u32>` to return *ids*, not slots. The player now holds names. Test by sorting the deck after a deal and confirming `cards_held_by` still returns the same five cards.
5. **A first generation counter.** Add `let mut generation: Vec<u32> = vec![0; 52];`. The 52-card deck does not actually recycle, but extend a small `swap_remove`-like operation: pop the last card from the deck (location 0), insert a "fresh" card at the freed slot, and bump that slot's `generation` by one. Take a `CreatureRef`-style `(id, generation)` reference *before* the operation. After the operation, look up the slot by id; check `generation[slot]` against the reference's `generation`. Confirm the dereference correctly reports stale.
6. *(stretch)* **A tiny generational arena.** Outside the deck, build a `Creatures` struct with `pos: Vec<f32>`, `generation: Vec<u32>`, plus `free: Vec<u32>` of slots awaiting reuse. Implement `insert(pos) -> (slot, generation)`, `remove(slot)`, and `get(slot, generation) -> Option<f32>`. Convince yourself by example that stale references cannot read a fresh creature's data.
7. *(stretch)* **Compare with `slotmap`.** Read [`slotmap::SlotMap::insert` and `get`](https://docs.rs/slotmap/latest/slotmap/). Identify which of your fields and operations correspond. What does `slotmap` add that you didn't need for the simulator? Decide consciously whether to adopt it. (This is the from-scratch-then-price-the-crate move from [§41 - Deferred abstraction](41_compression_oriented.md) and [§42 - You can only fix what you wrote](42_you_can_only_fix_what_you_wrote.md).)

Reference solutions for the deck exercises (1-5) in [10_stable_ids_and_generations_solutions.md](10_stable_ids_and_generations_solutions.md). The arena and `slotmap` exercises follow the same shape and are worth working without reference.

## What's next

You now have stable references. The next thing the simulator will need is to look up a row by id in O(1) rather than O(N) - an `id_to_slot` map maintained on every reordering. That is [§23 - Index maps](23_index_maps.md). It is one extra `Vec<u32>`, updated whenever the columns move.
