# Solutions: 5 — Identity is an integer

Reference solutions for the exercises in [05_identity_is_an_integer.md](05_identity_is_an_integer.md). Try the exercises first.

## Exercise 1 — Build the deck

```rust
fn new_deck() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut suits = Vec::with_capacity(52);
    let mut ranks = Vec::with_capacity(52);
    let mut locations = Vec::with_capacity(52);
    for s in 0..4u8 {
        for r in 0..13u8 {
            suits.push(s);
            ranks.push(r);
            locations.push(0); // 0 = in the deck
        }
    }
    (suits, ranks, locations)
}
```

The order of insertion sets the index-to-card mapping. Spades fill indices 0-12, hearts 13-25, and so on. The Ace of Spades is at index 0; the King of Clubs at index 51.

`Vec::with_capacity(52)` is a small but honest gesture: the size is known up front, so we ask for exactly that much memory. No reallocation, no surprise. This is constant-quantity behaviour — node 27 will explain why it matters at a million.

## Exercise 2 — Print a card

```rust
const SUIT_CHARS: [&str; 4] = ["♠", "♥", "♦", "♣"];
const RANK_CHARS: [&str; 13] = [
    "A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K",
];

fn card_to_string(suit: u8, rank: u8) -> String {
    format!("{}{}", RANK_CHARS[rank as usize], SUIT_CHARS[suit as usize])
}

fn print_deck(suits: &[u8], ranks: &[u8]) {
    for i in 0..suits.len() {
        println!("{:>2}: {}", i, card_to_string(suits[i], ranks[i]));
    }
}
```

The `as usize` casts are because `Vec` and array indexing want `usize`. Choose `u8` for the columns because we're never going to have 256 suits or ranks; the smaller width saves memory and keeps more of the deck in L1.

## Exercise 3 — Shuffle

A tiny LCG, then Fisher-Yates over the index order:

```rust
// A Linear Congruential Generator. Not cryptographic. Fine for shuffling cards.
fn rand(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state >> 32
}

fn shuffle(n: usize, seed: u64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let mut state = seed;
    for i in (1..n).rev() {
        let j = (rand(&mut state) as usize) % (i + 1);
        order.swap(i, j);
    }
    order
}

fn print_deck_shuffled(suits: &[u8], ranks: &[u8], order: &[usize]) {
    for &i in order {
        println!("{}", card_to_string(suits[i], ranks[i]));
    }
}
```

The print function takes `suits`, `ranks`, and `order` as `&[u8]` and `&[usize]` slices — none of them are mutated. The cards stay where they are. Only the traversal changes.

> [!NOTE]
>
> A real shuffle wants a stronger RNG; for fifty-two cards an LCG is fine. The exercise is about the indices, not the entropy. When you have an excuse to use a real RNG, the [from-scratch test](../../concepts/dag.md) (node 40) applies: write the LCG version first, then read whatever crate you might reach for, and pick consciously.

## Exercises 4-8

Same shape. The pattern is:

- Whatever query you want is a `for` loop over an index range, asking the columns at each index.
- Whatever rearrangement you want is a permutation of the *order* vector, leaving the columns unchanged.
- A "move" — dealing, discarding — is a write to `locations[i]`, never a copy of the card.

If you find yourself constructing a `Card` struct to make exercise 8 cleaner, stop. The four hands together are simply a `Vec<u8>` of length 52 (the existing `locations` array) with values `0..5`. Printing each hand is `cards_held_by(&locations, p)` for `p in 1..=4`.

## Exercises 9-10

Both are bridges into the next sections.

- Exercise 9 (drop the index) is a preview of nodes 6 (row is a tuple) and the natural-key idea named in the strong-form note above. The (suit, rank) pair *is* the card; you don't need an integer to refer to it. But moving it back to the deck is now harder, because you've lost the slot reference.
- Exercise 10 (the sort hazard) is the bug that motivates [§9 — Sort breaks indices](09_sort_breaks_indices.md), which in turn motivates [§10 — Stable IDs and generations](10_stable_ids_and_generations.md). The bug shows up the moment you sort the data arrays themselves rather than the order vector. You need a stable name for a card that survives reordering — and that name is what node 10 introduces.
