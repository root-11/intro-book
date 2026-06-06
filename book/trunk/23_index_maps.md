# 23 - Index maps

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 23](../../concepts/glossary.md#23--index-maps).*

<p align="center"><img src="../illustrations/linear_algebra.jpg" alt="Linear algebra: Ax = b - a lookup is a matrix-vector product" style="max-height: 300px; max-width: 100%;"></p>

The slot-keyed tables from [§17](17_presence_replaces_flags.md) and [§19](19_ebp_dispatch.md) left two questions open, and [§21](21_swap_remove.md) added a third.

1. **Point membership.** "Is slot `i` in `hungry`?" costs O(N) when answered by scanning the table (`hungry.iter().any(|&s| s == i)`).
2. **Unsubscribe.** To `swap_remove` slot `i` out of `hungry` you first need its *position in the table* - the same O(N) scan.
3. **The moved slot.** When swap_remove relocates a row ([§21](21_swap_remove.md)), every slot-keyed table that listed the old position now points at the wrong creature.

All three are solved by one idea: an *index map* - a parallel array from a key to a position, with a sentinel for "absent". It appears twice in the simulator, the same shape pointing at two different things.

**Instance one: `id_to_slot`.** Maps a stable [entity](10_stable_ids_and_generations.md) to its current column slot. This is what re-finds a creature after a move, and what anything outside the columns (a save, the network, the UI - [§26](26_subscription_tables.md)) uses to turn an id back into a slot.

```rust,no_run
const INVALID: u32 = u32::MAX;

fn slot_of(id_to_slot: &[u32], entity: u32) -> Option<usize> {
    let slot = id_to_slot[entity as usize];
    if slot == INVALID { None } else { Some(slot as usize) }
}
```

A sentinel (`u32::MAX`) marks "no slot - this entity has no current row". The `Option` makes the missing case explicit.

**Instance two: the sparse set.** A membership table needs O(1) "is slot `i` present?" and O(1) unsubscribe, without a per-creature boolean - a boolean would be exactly the flag [§17](17_presence_replaces_flags.md) abolished, one byte per creature whether set or not. The structure is two arrays: a `dense: Vec<u32>` of the present slots (what the hot loop walks), and a `sparse: Vec<u32>` indexed by slot, holding each present slot's *position in `dense`*, or `INVALID`.

```rust,no_run
// is slot i present?   sparse[i] != INVALID
// subscribe(i):        sparse[i] = dense.len() as u32; dense.push(i);
// unsubscribe(i):      let p = sparse[i] as usize;
//                      let moved = *dense.last().unwrap();
//                      dense.swap_remove(p);
//                      sparse[moved as usize] = p as u32;
//                      sparse[i] = INVALID;
```

`sparse` stores positions and a sentinel, not booleans - it is the index-map pattern again, pointing into the membership table instead of into the columns. It answers "present?" *and* "where, so I can remove it in O(1)?", which a boolean could not. This pair, a dense list plus a sparse index, is the *sparse set* - the membership structure every ECS ships.

**Maintenance.** Both maps must be kept current whenever a row moves. Take the move that hurts most, swap_remove ([§21](21_swap_remove.md)): the last row, at slot `last`, backfills the freed slot `i`.

- **`id_to_slot`.** Set `id_to_slot[moved_entity] = i`; set `id_to_slot[deleted_entity] = INVALID`.
- **Every slot-keyed table.** Wherever a `dense` array listed slot `last`, rewrite it to `i` - a reindex through the move. With the sparse set this is O(1): the moved creature's position is `sparse[last]`, so `dense[sparse[last]] = i`, then `sparse[i] = sparse[last]; sparse[last] = INVALID` (when the moved creature was a member).
- **Append.** A new row at slot `n` sets `id_to_slot[new_entity] = n`.
- **Sort or shuffle.** Reordering for locality ([§28](28_sort_for_locality.md)) moves every slot; both maps are rewritten in lockstep with the new order.

The cleanup system from [§22](22_mutations_buffer.md) is the natural home for all of this. Every move goes through cleanup; cleanup keeps the maps in step. This is also why [§24](24_append_only_and_recycling.md) prefers not to move slots on death at all: every avoided move is a reindex never paid.

**Cost.** The map adds one `u32` per id ever issued, including ids that are currently dead but whose slots have not been recycled. For a simulator that issues a million ids over its lifetime but has 100 000 alive at any moment, the map is 4 MB. That is a real cost - bigger than the alive table itself if the table has narrow columns. Mitigations include:

- **Generational ids** ([§10](10_stable_ids_and_generations.md)) bound the map's size to the maximum live + recycled count, not the total ever issued.
- **A `HashMap<u32, u32>`** trades a constant-time lookup overhead for tighter memory; useful when ids are sparse.
- **A separate id allocator** that recycles dead ids, so the map's size matches the *high-water mark* of live ids.

For most simulators, the dense `Vec<u32>` is the right shape. It is one cache line per 16 ids; cleanup streams sequentially through it.

Each *maintained* membership table carries its own `sparse` array of the same magnitude (one `u32` per slot). So the cost is paid once for `id_to_slot` and once more per slot-keyed table kept incrementally. A table you instead *rebuild from scratch* each tick needs no sparse index at all - only its dense list, cleared and refilled. Rebuild when the membership churns almost completely each tick; maintain incrementally when it is stable and only a few entries change per tick. It is the same cost-versus-churn judgment as everywhere else in the book.

**The pattern in the wild.** Every ECS engine ships an index map. Bevy's `Entity` is a 64-bit handle whose unpacking is essentially a slot lookup with a generation check. `slotmap`'s `SlotMap` keeps an internal map. Database engines maintain index maps as B-trees over primary keys. The shape - id-to-slot lookup, maintained on every move - is universal.

Combined with [§10](10_stable_ids_and_generations.md)'s stable ids and [§24](24_append_only_and_recycling.md)'s slot recycling, the index map is the third piece of the *generational arena* - the canonical handle-based data structure in modern systems software.

## Exercises

1. **Build the map.** Add `id_to_slot: Vec<u32>` to your simulator. Initialise to `INVALID` for all ids. When a creature is appended at slot N, set `id_to_slot[id] = N`.
2. **Build the sparse set.** Give `hungry` a `sparse: Vec<u32>` alongside its dense list. Implement `subscribe(i)`, `unsubscribe(i)`, and `is_member(i)` - each O(1), no boolean. Confirm `is_member` always agrees with a linear scan of the dense list across a run of subscribes and unsubscribes.
3. **Maintain both maps on swap_remove.** Modify your cleanup so that after `creatures.swap_remove(slot)`:
   - `id_to_slot[deleted_entity] = INVALID`; `id_to_slot[moved_entity] = slot` (the last row, now at `slot`)
   - every slot-keyed table is reindexed: wherever it listed the moved row's old slot, rewrite it to `slot`. Verify `hungry` still lists exactly the hungry creatures after a sequence of deaths.
4. **Time the difference.** At 1 M creatures, call `is_member(random_slot)` 100 000 times per tick. Compare the linear scan of the dense list (§17) with the sparse-set lookup (§23). The ratio is roughly N - about a million.
5. **The bandwidth cost.** At 1 M ids, `id_to_slot` is 4 MB. Cleanup's update of the map writes ~12 bytes per swap_remove (delete row's slot, moved row's slot, plus bookkeeping). Compute the cleanup cost in microseconds for 1 000 deletes per tick; compare to the budget at 30 Hz.
6. **Sort-for-locality compatibility.** When `creatures` is sorted (a preview of [§28](28_sort_for_locality.md)), every slot moves. Rewrite `id_to_slot` *and* every slot-keyed table in lockstep with the new order. Verify both id-held references and slot-keyed memberships are still correct after the sort.
7. *(stretch)* **A from-scratch generational arena.** Combine [§10](10_stable_ids_and_generations.md)'s `generation: Vec<u32>`, [§22](22_mutations_buffer.md)'s deferred cleanup, and §23's `id_to_slot` map into a `SlotMap<T>` struct. Compare the shape with [`slotmap::SlotMap`](https://docs.rs/slotmap/) - same machinery, organised differently.

Reference notes in [23_index_maps_solutions.md](23_index_maps_solutions.md).

## What's next

[§24 - Append-only and recycling](24_append_only_and_recycling.md) names two strategies for what happens to a slot after it has been freed. The choice is decided by access pattern, not by taste.
