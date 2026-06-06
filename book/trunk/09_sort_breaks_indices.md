# 9 - Sort breaks indices

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 9](../../concepts/glossary.md#9--the-sort-breaks-indices).*

<p align="center"><img src="../illustrations/bridge_clipboard.jpg" alt="Engineer mouse with clipboard and F = ma - alignment is a structural property" style="max-height: 300px; max-width: 100%;"></p>

In [§5 - Identity is an integer](05_identity_is_an_integer.md), exercise 10 left you with a bug. Player 1 was holding the index list `[3, 17, 21, 28, 41]`. The dealer sorted the deck columns by suit. Player 1's hand was now wrong - the same indices, the same slots, but different cards.

That bug is the structural fact this section names. Sorting did not damage anything; the player's reference was never robust to begin with. An index points at a *slot*, not at a *thing*. When the slot's contents change, the index quietly changes meaning.

It is not only sorting. Any rearrangement does it: `swap_remove` (a O(1) deletion that moves the last row into the freed slot, coming in [§21](21_swap_remove.md)), reshuffling for locality ([§28](28_proximity.md)), compacting after a batch of deletions. The same index, the same array, the same line of code, now means a different card.

This is uncomfortable. In OOP you held a `Card` reference and the card stayed put because `Card` was a thing. In data-oriented code the card *is the slot*, and the slot does not have permanent meaning. The card you saved a reference to yesterday may be a different card today, if the deck has been touched.

There are two ways forward. The lazy one is to never rearrange the deck. That works for fifty-two cards, fails for ten thousand creatures, and becomes catastrophic for a million. The book is going to need rearrangements - sorting, deletion, compaction - at every scale beyond §0. So we need the other fix: a stable name that survives the slot it currently occupies.

That is what [§10 - Stable IDs and generations](10_stable_ids_and_generations.md) does. This section's only job is to make the *slot vs name* distinction concrete enough that §10's solution feels inevitable rather than ceremonial.

> [!NOTE]
>
> *Why feel the pain first?* Because the fix in §10 is small - one extra column - and small fixes only stick if the student knows what they fix. Reading "always store an id" without first feeling the bug produces students who add ids cargo-culted, then drop them when the codebase looks too cluttered. Reading it after watching player 1 lose their hand produces students who never drop them.

## Exercises

You should still have your `src/main.rs` from §5. These exercises extend it.

1. **Reproduce the bug.** With player 1 holding `[3, 17, 21, 28, 41]`, sort the deck *columns themselves* (`suits`, `ranks`, and `locations` in lockstep) by suit. Print player 1's hand using `card_to_string`. Confirm the cards have changed.
2. **A second rearrangement.** Instead of sorting, swap two cards' positions:
   ```rust
   suits.swap(3, 17);
   ranks.swap(3, 17);
   locations.swap(3, 17);
   ```
   Print player 1's hand again. Same bug shape, different cause.
3. **A third rearrangement.** Remove the card at slot 3 with `swap_remove(3)` on each column. Print player 1's hand. Note that the cards at slots `[17, 21, 28, 41]` are unchanged but slot 3 may now hold what was previously the last card; meanwhile slot 51 has silently been deleted.
4. **Quantify the breakage.** Write a function that takes the original `[3, 17, 21, 28, 41]` plus a freshly built deck, applies a Fisher-Yates shuffle to the deck columns themselves, and counts how many of the five references still point at the same `(suit, rank)` value. Run it 100 times. Roughly what fraction of references survive a random shuffle of the deck?
5. **A reference that *can* survive.** Without writing any new code - on paper - describe what kind of reference would survive a shuffle. (Hint: you already know. The card's `(suit, rank)` is unique to that card. The reference that survives is the one that does not depend on the slot.)
6. *(stretch)* **The cost of never rearranging.** Suppose you decide to *never* sort, swap, or remove from the deck columns, to avoid this bug forever. How would shuffling work? How would discarding a card work? Why does this not scale to ten thousand creatures?

Reference notes for these exercises in [09_sort_breaks_indices_solutions.md](09_sort_breaks_indices_solutions.md).

## What's next

Exercise 5 points at the answer; exercise 6 makes the never-rearrange option look bad. The real fix is to store identity *separately from position* - an `id` column that travels with the row across rearrangements, with a generation counter on top for variable-quantity tables. [§10 - Stable IDs and generations](10_stable_ids_and_generations.md) builds it.
