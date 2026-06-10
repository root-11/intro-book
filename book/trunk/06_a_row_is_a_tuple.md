# 6 - A row is a tuple

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 6](../../concepts/glossary.md#6---a-row-is-a-tuple).*

<p align="center"><img src="../illustrations/cad_bearing.jpg" alt="A bearing's dimensioned drawing names every field" style="max-height: 300px; max-width: 100%;"></p>

In §5 you built a deck of 52 cards as three `Vec`s. The card at index 17 is the triple `(suits[17], ranks[17], locations[17])`. Together those three values are *the row*. There is no `Card` struct. The row exists *implicitly* in the alignment: the same index, used in every column, recovers all the data about one card.

This is what we call a *row* throughout the rest of the book - a coherent set of values that belong to the same entity. In a `creature` table the row is `(pos[i], vel[i], energy[i], birth_t[i], id[i], generation[i])`. In a `food` table it is `(pos[i], value[i], id[i])`. The fields belong to the same entity by virtue of all sharing index `i`. There is no struct holding them; there is only the discipline that whatever index `i` you used to read one column, you also use to read every other column of the same table.

The cost of implicit binding is that you must *keep the indices aligned*. If you sort `ranks` without also sorting `suits` and `locations`, you corrupt the column: the rank stored at a slot is no longer the rank of the card whose suit and location sit beside it. The deck still has 52 entries in 52 slots, but the slots no longer describe coherent cards. This is not a hypothetical bug; [§9](09_sort_breaks_indices.md) will produce it deliberately so you can feel the consequences. The structural fix in this book is simple: every operation that reorders any column of a table must reorder *all* columns of that table together.

The discipline that makes alignment maintainable is **single-writer-per-column**. If only one system writes to `locations`, and that system writes consistently, alignment is never violated. Multiple writers to the same column race against each other and produce inconsistent rows. This is what node 25 (one writer, many readers) enforces: each table is written by exactly one system, and a row is a tuple precisely because that one writer kept all its columns in step.

A row is a tuple - assembled from columns indexed by the same entity, kept aligned by discipline rather than by any container holding it together.

## Exercises

These extend your `src/main.rs` from §5.

1. **Print row 17.** Write `fn row(suits: &[u8], ranks: &[u8], locations: &[u8], i: usize) -> (u8, u8, u8)`. Use it to print the suit, rank, and location of card 17.
2. **Mishandle the alignment.** Sorting `suits` here would do nothing - the deck was built with suits already in order, so `suits.sort()` is a no-op that quietly hides the trap. Sort *only* `ranks` instead (`ranks.sort()`, no order vector), leaving `suits` and `locations` untouched. Print rows 5 and 17. Row 5 is plainly broken: its suit and location still belong to card 5, but its rank was overwritten by another card's. Row 17 looks fine, because slot 17 happens to hold rank 4 both before and after the sort. The column is corrupted either way; the surviving slot only proves that one spot-check is not a test.
3. **Lockstep sort.** Reset the deck. Now sort all three columns *together* by rank, using an order vector (the technique from §10): build `order`, sort it by `ranks[i]`, then rebuild every column through that one order. Print rows 5 and 17 again. Whatever card now sits in a slot, its suit, rank, and location moved there together, so the row is coherent again.
4. **Add a fourth column.** Add `let mut dealt_at: Vec<u32> = vec![u32::MAX; 52];` (when a card is dealt, write the current tick number into `dealt_at[i]`). Modify your lockstep sort to also reorder this column. Verify by spot-check that a row is still consistent after a sort.
5. **The single-writer rule.** Write `fn reorder_deck(suits: &mut Vec<u8>, ranks: &mut Vec<u8>, locations: &mut Vec<u8>, dealt_at: &mut Vec<u32>, order: &[usize])`. This function is the *only* one that should ever reorder any column of the deck. Document that contract in a comment above the function.
6. *(stretch)* **When alignment is moot.** A query that uses only `(suits[i], ranks[i])` to identify a card - for instance, "is this the Ace of Spades?" - does not depend on `locations` or `dealt_at`. Write such a query. The natural-key view from §5's strong form means this query survives reorderings of unrelated columns; only `suits` and `ranks` need to be aligned with each other.

Reference notes in [06_a_row_is_a_tuple_solutions.md](06_a_row_is_a_tuple_solutions.md).

## What's next

[§7 - Structure of arrays (SoA)](07_structure_of_arrays.md) names the layout choice you have been making implicitly: each field its own column. The next section defends that choice against its alternative.
