# Solutions: 6 — A row is a tuple

## Exercise 1 — Print row 17

```rust
fn row(suits: &[u8], ranks: &[u8], locations: &[u8], i: usize) -> (u8, u8, u8) {
    (suits[i], ranks[i], locations[i])
}

let (suits, ranks, locations) = new_deck();
let (s, r, l) = row(&suits, &ranks, &locations, 17);
println!("row 17: suit={s} rank={r} location={l}");
```

The function does not look up by id; it looks up by *slot*. With a fresh deck the slot 17 holds a stable card, but as soon as the deck is sorted or rearranged, the same call returns a different card. That is the §9 lesson; here we only ask the slot what it holds *right now*.

## Exercise 2 — Mishandle the alignment

```rust
suits.sort();          // reorders only `suits`
let (s, r, l) = row(&suits, &ranks, &locations, 17);
println!("row 17 (corrupted): suit={s} rank={r} location={l}");
```

Slot 17 now holds: the suit that ended up at sorted-position 17 (probably one of the hearts), the rank that originally was at position 17 (5 of diamonds), and the location originally at 17 (still 0 = deck). Three fields from three different cards. This is the alignment violation in pure form.

## Exercise 3 — Lockstep sort

```rust
let mut order: Vec<usize> = (0..52).collect();
order.sort_by_key(|&i| suits[i]);
suits     = order.iter().map(|&i| suits[i]).collect();
ranks     = order.iter().map(|&i| ranks[i]).collect();
locations = order.iter().map(|&i| locations[i]).collect();
```

After the lockstep sort, slot 17 is whichever original card landed at sorted-position 17 — but whatever that card is, all three of its fields move together.

## Exercises 4-6 — Sketches

**Exercise 4.** `dealt_at` is just a fourth column. Add it to the lockstep sort. Spot-check by setting `dealt_at[7] = 42` before the sort, then verifying that after the sort `dealt_at[new_slot]` is still 42 *for that same card* (find it via the id column from §10).

**Exercise 5.** The `reorder_deck` function takes `&mut` references to all four columns plus `&[usize]` order. Inside, it does the four `iter().map(...).collect()` lines. The contract in the comment: "any reorder of the deck must use this function. Direct calls to `Vec::sort()` or `Vec::swap()` on individual columns are forbidden, even if they happen to compile."

**Exercise 6.** A natural-key query like `is_ace_of_spades(s, r)` reads only `suits[i]` and `ranks[i]` without caring what `locations[i]` says. The `locations` column can be reordered independently and the query remains correct — provided `suits` and `ranks` stay aligned with each other. Two-of-three alignment is sometimes acceptable; full alignment is the only state in which *all* queries are valid. Reasoning about partial alignment is fragile and rarely worth the complexity.
