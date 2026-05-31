# 21 - `swap_remove`

<p align="center"><img src="../covers/phase_memory_lifecycle.jpg" alt="Memory & lifecycle phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 21](../../concepts/glossary.md#21--swap_remove).*

The presence-replaces-flags substitution from [§17](17_presence_replaces_flags.md) raised a problem we deferred. When a creature stops being hungry, you remove its id from `hungry`. When a creature dies, you remove its row from every table. *Removing rows from a `Vec` is expensive* - `vec.remove(i)` shifts every later row left by one, costing O(N).

For a 1 000 000-creature simulator with 1 000 deaths per tick, naive `remove` costs roughly 10⁹ moves per tick - a thousand times the budget of a 30 Hz simulation.

The fix is a built-in Rust method: `vec.swap_remove(i)`.

```rust
let mut v = vec![10, 20, 30, 40, 50];
let removed = v.swap_remove(1);
assert_eq!(removed, 20);
assert_eq!(v, vec![10, 50, 30, 40]); // 50 was moved into slot 1
```

The mechanism is small: take the last element, move it into the deleted slot, shrink the table by one. Two memory writes and a length decrement. O(1) regardless of N.

**Cost.** A 1 000 000-creature table with 1 000 swap_removes per tick costs ~6 000 memory writes (one per column, six columns) - about 50 nanoseconds. The naive `remove` would cost a thousand times more.

**Cost paid.** Order is sacrificed. If your code depended on rows being in any particular order, swap_remove reorders them. Two specific consequences:

- **Iteration corrupted.** If you iterate the table and call swap_remove during iteration, the slot you just visited now holds a different row, but your loop counter has moved past it. Half the rows after a swap_remove get skipped or revisited inconsistently.
- **External references break.** Any code holding a slot index into the table now refers to a different row. This is the same bug as [§9](09_sort_breaks_indices.md): rearrangement breaks slot-based references.

Both problems have fixes already named in the book. The iteration corruption is fixed by [§22 - Mutations buffer](22_mutations_buffer.md): swap_remove never runs during iteration; it runs during cleanup at the tick boundary, when no system is iterating. The external-reference problem is fixed by [§23 - Index maps](23_index_maps.md): an `id_to_slot` map is updated whenever a row moves, so id-based references survive.

This whole phase - Memory & lifecycle - only matters for *variable-quantity* tables. Constant-quantity tables like the 52-card deck never grow or shrink, never need swap_remove, never need any of the machinery in this phase. The card game ran for ten chapters without it. The simulator from §1 onward needs all of it, because creatures are born and die every tick.

To reuse the card-game milestone framing: the *constant vs variable* distinction is what determines whether a programmer reaches into the lifecycle toolbox at all. Once you have a table whose row count varies at runtime, every tool in this phase becomes load-bearing.

## Exercises

1. **Compare timings.** Build a `Vec<u64>` of length 1 000 000. Time 1 000 calls to `vec.remove(0)`. Time the same with `vec.swap_remove(0)`. The ratio is roughly N.
2. **Mid-table delete.** Build a `Vec<u64>` of length 1 000 000. Time 1 000 calls to `vec.remove(500_000)`. Time 1 000 calls to `vec.swap_remove(500_000)`. The naive version is about half as expensive as the front delete; the swap version is unchanged.
3. **The iteration hazard.** Build a `Vec<u64>` with values `0..100`. In a forward loop, iterate and call `vec.swap_remove(i)` whenever `vec[i] % 2 == 0`. Compare with the expected output (only odd values remaining). What did you actually get?
4. **The fix in one shape: iterate backwards.** Repeat exercise 3, but iterate `(0..v.len()).rev()`. Does it work now? Why does it work?
5. **The fix in another shape: deferred cleanup.** Repeat exercise 3, but instead of calling swap_remove inside the loop, push the index to `to_remove`. After the loop, drain `to_remove` (in reverse order) and apply swap_remove. This is the §22 pattern in miniature.
6. **Aligned swap_remove.** Build the simulator's six creature columns. Write `fn delete_creature(world: &mut World, slot: usize)` that calls `swap_remove(slot)` on every column in the same order. Verify all columns remain aligned after a sequence of deletes.
7. *(stretch)* **The bandwidth cost.** Compute the bytes moved by `vec.remove(0)` on a 1 GB `Vec`: roughly the whole 1 GB. Compute the same for `vec.swap_remove(0)`: roughly one element. The ratio is `N / 1`.

Reference notes in [21_swap_remove_solutions.md](21_swap_remove_solutions.md).

## What's next

[§22 - Mutations buffer; cleanup is batched](22_mutations_buffer.md) is the rule that makes swap_remove safe to use: it never runs while any system is iterating.
