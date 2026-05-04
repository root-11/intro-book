# Solutions: 9 — Sort breaks indices

Reference notes for the exercises in [09_sort_breaks_indices.md](09_sort_breaks_indices.md). These exercises are mostly about *observing* the bug; there is little new code. Work them first — the lesson is felt, not read.

## Exercise 1 — Reproduce the bug

```rust
let (mut suits, mut ranks, mut locations) = new_deck();
let player_1_hand = vec![3, 17, 21, 28, 41];

// Sort the deck columns themselves (in lockstep) by suit.
let mut order: Vec<usize> = (0..52).collect();
order.sort_by_key(|&i| suits[i]);
suits     = order.iter().map(|&i| suits[i]).collect();
ranks     = order.iter().map(|&i| ranks[i]).collect();
locations = order.iter().map(|&i| locations[i]).collect();

for &i in &player_1_hand {
    println!("{}", card_to_string(suits[i], ranks[i]));
}
```

Compare what prints now with what printed before the sort. The slots `[3, 17, 21, 28, 41]` still exist; their *contents* moved.

## Exercise 4 — Quantify the breakage

After a uniform random shuffle, a fixed slot has a 1-in-52 chance of holding the same card it held before. Five references, five independent draws — expected survivors per shuffle is roughly `5/52 ≈ 0.10`. Most runs have *zero* references survive. The bug is total, not partial.

## Exercise 5 — A reference that can survive

The pair `(suit, rank)` is unique to each card. A reference that stores `(suit, rank)` rather than a slot index survives any rearrangement of the deck columns, because the pair is *natural* — it lives in the data itself, not in the slot. This is the strong form of [§5 — Identity is an integer](05_identity_is_an_integer.md).

The *price* of natural keys is paid in [§10](10_stable_ids_and_generations.md): returning `Vec<(u8, u8)>` from a query is fine for reading, but to *move* a card (deal, discard) you still need to know which slot to write to. Surrogate ids generalise to cases where the data has no natural unique tuple.

## Exercise 6 — The cost of never rearranging

Without rearranging:

- *Shuffling* must be implemented as a permutation in `order: Vec<usize>` while the deck columns themselves stay in their original layout. This is what §5 actually did. It works.
- *Discarding* means writing `locations[i] = DISCARD` rather than removing the row. Logical removal via a flag, not structural removal. The deck never shrinks.

For 52 cards the deck-never-shrinks rule is fine. For 10,000 creatures with steady birth and death the table grows without bound; every system scans past dead rows; cache traffic doubles, then quadruples. The fix in [§21 — `swap_remove`](21_swap_remove.md) needs the stable references that §10 introduces in order to be safe to call.
