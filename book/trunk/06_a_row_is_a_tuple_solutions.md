# Solutions: 6 - A row is a tuple

## Exercise 1 - Print row 17

```rust
fn row(suits: &[u8], ranks: &[u8], locations: &[u8], i: usize) -> (u8, u8, u8) {
    (suits[i], ranks[i], locations[i])
}

let (suits, ranks, locations) = new_deck();
let (s, r, l) = row(&suits, &ranks, &locations, 17);
println!("row 17: suit={s} rank={r} location={l}");
```

The function does not look up by id; it looks up by *slot*. With a fresh deck the slot 17 holds a stable card, but as soon as the deck is sorted or rearranged, the same call returns a different card. That is the §9 lesson; here we only ask the slot what it holds *right now*.

## Exercise 2 - Mishandle the alignment

```rust
// `suits` is already sorted by construction, so `suits.sort()` would change nothing.
// Sort `ranks` in isolation to actually reorder a column:
ranks.sort();
for i in [5, 17] {
    let (s, r, l) = row(&suits, &ranks, &locations, i);
    println!("row {i}: suit={s} rank={r} location={l}");
}
```

After `ranks.sort()`, slot 5 still reads suit 0 and location 0 - those columns were untouched, so they are card 5's - but its rank is now 1, dragged in from a different card. The row no longer describes any real card. Slot 17 reads suit 1, rank 4, location 0, exactly as before: the four rank-4 cards occupy slots 16 through 19 both before and after the sort, so this one slot survives by coincidence. Note that only one column moved, so a broken row mixes *two* cards, not three; the real danger is that inspecting a single lucky slot like 17 would tell you nothing is wrong.

## Exercise 3 - Lockstep sort

```rust
let mut order: Vec<usize> = (0..52).collect();
order.sort_by_key(|&i| ranks[i]);
suits     = order.iter().map(|&i| suits[i]).collect();
ranks     = order.iter().map(|&i| ranks[i]).collect();
locations = order.iter().map(|&i| locations[i]).collect();
```

Every column is rebuilt through the *same* `order`, so whichever card lands in slot 5 or 17 brings its suit, rank, and location with it. The slot's three fields agree on one card again. The fix is structural: one order vector, applied to every column, can never produce the mismatch from exercise 2.

## Exercises 4-6 - Sketches

**Exercise 4.** `dealt_at` is just a fourth column. Add it to the lockstep sort. Spot-check by setting `dealt_at[7] = 42` before the sort, then verifying that after the sort `dealt_at[new_slot]` is still 42 *for that same card* (find it via the id column from §10).

**Exercise 5.** The `reorder_deck` function takes `&mut` references to all four columns plus `&[usize]` order. Inside, it does the four `iter().map(...).collect()` lines. The contract in the comment: "any reorder of the deck must use this function. Direct calls to `Vec::sort()` or `Vec::swap()` on individual columns are forbidden, even if they happen to compile."

**Exercise 6.** A natural-key query like `is_ace_of_spades(s, r)` reads only `suits[i]` and `ranks[i]` without caring what `locations[i]` says. The `locations` column can be reordered independently and the query remains correct - provided `suits` and `ranks` stay aligned with each other. Two-of-three alignment is sometimes acceptable; full alignment is the only state in which *all* queries are valid. Reasoning about partial alignment is fragile and rarely worth the complexity.
