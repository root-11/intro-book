# 23 — Index maps

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 23](../../concepts/glossary.md#23--index-maps).*

<p align="center"><img src="../illustrations/linear_algebra.jpg" alt="Linear algebra: Ax = b — a lookup is a matrix-vector product" style="max-height: 300px; max-width: 100%;"></p>

The presence-replaces-flags substitution from [§17](17_presence_replaces_flags.md) had a sting in its tail. A presence query — "is creature 42 hungry?" — costs O(N) when implemented naively as `hungry.iter().any(|&x| x == 42)`. At 1 000 000 creatures, that is too slow for any system that needs to ask the question many times per tick.

The fix is a parallel data structure: an *index map* `id_to_slot: Vec<u32>` that maps every id to its current slot in the table. Lookup is now O(1):

```rust,no_run
const INVALID: u32 = u32::MAX;

fn slot_of(id_to_slot: &[u32], id: u32) -> Option<usize> {
    let slot = id_to_slot[id as usize];
    if slot == INVALID { None } else { Some(slot as usize) }
}
```

A sentinel value (`u32::MAX`) marks "no slot — this id does not have a current row". The `Option` return makes the missing case explicit.

**Maintenance.** The map must be updated whenever a row moves. The events that move rows:

- **`swap_remove`.** When slot `i` is removed by swapping the last row in, the row that was at `last` is now at `i`. Update `id_to_slot[that_row.id] = i`. Set `id_to_slot[deleted_row.id] = INVALID`.
- **Append.** When a new row is appended at slot `n`, set `id_to_slot[new_row.id] = n`.
- **Sort or shuffle.** When the table is reordered (for locality, [§28](28_sort_for_locality.md)), every slot moves. The full map is rewritten in lockstep with the sort.

The cleanup system from [§22](22_mutations_buffer.md) is the natural home for these updates. Every removal and every insertion goes through cleanup; cleanup keeps the map in step.

**Cost.** The map adds one `u32` per id ever issued, including ids that are currently dead but whose slots have not been recycled. For a simulator that issues a million ids over its lifetime but has 100 000 alive at any moment, the map is 4 MB. That is a real cost — bigger than the alive table itself if the table has narrow columns. Mitigations include:

- **Generational ids** ([§10](10_stable_ids_and_generations.md)) bound the map's size to the maximum live + recycled count, not the total ever issued.
- **A `HashMap<u32, u32>`** trades a constant-time lookup overhead for tighter memory; useful when ids are sparse.
- **A separate id allocator** that recycles dead ids, so the map's size matches the *high-water mark* of live ids.

For most simulators, the dense `Vec<u32>` is the right shape. It is one cache line per 16 ids; cleanup streams sequentially through it.

**The pattern in the wild.** Every ECS engine ships an index map. Bevy's `Entity` is a 64-bit handle whose unpacking is essentially a slot lookup with a generation check. `slotmap`'s `SlotMap` keeps an internal map. Database engines maintain index maps as B-trees over primary keys. The shape — id-to-slot lookup, maintained on every move — is universal.

Combined with [§10](10_stable_ids_and_generations.md)'s stable ids and [§24](24_append_only_and_recycling.md)'s slot recycling, the index map is the third piece of the *generational arena* — the canonical handle-based data structure in modern systems software.

## Exercises

1. **Build the map.** Add `id_to_slot: Vec<u32>` to your simulator. Initialise to `INVALID` for all ids. When a creature is appended at slot N, set `id_to_slot[id] = N`.
2. **O(1) presence query.** Add a parallel `hungry_membership: Vec<bool>` set to `true` when an id is in `hungry`. Now `is_hungry(id)` is two array lookups, both O(1).
3. **Maintain on swap_remove.** Modify your cleanup so that after `creatures.swap_remove(slot)`:
   - `id_to_slot[deleted_id] = INVALID`
   - `id_to_slot[moved_id] = slot` (the last row, now at `slot`)
4. **Time the difference.** Rerun the simulator at 1 M creatures, calling `is_hungry(random_id)` 100 000 times per tick. Compare the linear-scan version (§17) and the indexed version (§23). The ratio is roughly N — about a million.
5. **The bandwidth cost.** At 1 M ids, `id_to_slot` is 4 MB. Cleanup's update of the map writes ~12 bytes per swap_remove (delete row's slot, moved row's slot, plus bookkeeping). Compute the cleanup cost in microseconds for 1 000 deletes per tick; compare to the budget at 30 Hz.
6. **Sort-for-locality compatibility.** When `creatures` is sorted (a preview of [§28](28_sort_for_locality.md)), every slot moves. Rewrite `id_to_slot` in lockstep. Verify external references (held as ids) are still correct after the sort.
7. *(stretch)* **A from-scratch generational arena.** Combine [§10](10_stable_ids_and_generations.md)'s `gen: Vec<u32>`, [§22](22_mutations_buffer.md)'s deferred cleanup, and §23's `id_to_slot` map into a `SlotMap<T>` struct. Compare the shape with [`slotmap::SlotMap`](https://docs.rs/slotmap/) — same machinery, organised differently.

Reference notes in [23_index_maps_solutions.md](23_index_maps_solutions.md).

## What's next

[§24 — Append-only and recycling](24_append_only_and_recycling.md) names two strategies for what happens to a slot after it has been freed. The choice is decided by access pattern, not by taste.
