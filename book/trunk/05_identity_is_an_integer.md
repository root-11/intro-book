# 5 — Identity is an integer

<p align="center"><img src="../covers/phase_identity_structure.jpg" alt="Identity & structure phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 5](../../concepts/glossary.md#5--identity-is-an-integer).*

Hand a programmer fifty-two cards and tell them to write code that shuffles, sorts, and deals. Ask how long.

Most will start drawing classes — `Card`, `Deck`, `Hand`, `Player`, maybe a `Game` — and quote you four hours. They are being honest. The class hierarchy is real work. There will be constructors, copy semantics, and a vague unease about whether `Hand` should hold pointers or values, whether `Deck` owns its cards or borrows them, whether shuffling should mutate the deck or return a new one.

The whole problem fits in three lines. The way it fits is the lesson of this section.

A deck of cards has three pieces of information per card: its suit (♠ ♥ ♦ ♣), its rank (A, 2, ..., K), and its current location (in the deck, in someone's hand, in the discard pile). That is three columns. The deck itself is fifty-two rows.

In Rust:

```rust
let suits:     Vec<u8> = vec![ /* 52 entries: 0..4 */ ];
let ranks:     Vec<u8> = vec![ /* 52 entries: 0..13 */ ];
let locations: Vec<u8> = vec![ /* 52 entries: 0=deck, 1=hand1, ... */ ];
```

That is the deck. There is no `Card` struct. There is no `Deck` class. The card at index `17` has its suit at `suits[17]`, its rank at `ranks[17]`, and its current location at `locations[17]`. The card *is* the index.

Dealing a card from the deck to player 1 is one line:

```rust
locations[17] = 1; // card 17 is now in player 1's hand
```

Asking *what's in player 1's hand* is one loop:

```rust
let mut hand: Vec<usize> = Vec::new();
for i in 0..52 {
    if locations[i] == 1 {
        hand.push(i);
    }
}
```

Asking *how many cards are left in the deck* is one counter:

```rust
let mut count = 0u32;
for i in 0..52 {
    if locations[i] == 0 { count += 1; }
}
```

Shuffling — the move students expect to be hard — is shuffling the order of indices. `0..52` becomes `[7, 32, 1, 19, ...]`, and you read your way through the cards in that order:

```rust
let mut order: Vec<usize> = (0..52).collect();
fisher_yates(&mut order, &mut rng); // 5 lines, written below
```

Look at what just happened. Nothing about the cards changed. `suits[17]`, `ranks[17]`, and `locations[17]` are exactly the values they were before. The shuffle moved indices, not data.

Sorting works the same way. To sort by suit then rank, you sort the indices by `(suits[i], ranks[i])`:

```rust
order.sort_by_key(|&i| (suits[i], ranks[i]));
```

The cards do not move. Their identifiers are reordered.

That's the deck of cards in maybe twenty lines of Rust. It includes shuffle, sort, deal, and several queries. It is not a stylistic shortcut; it is what a deck of cards *is*. The OOP version's four hours of work was the cost of pretending a card was an object that owned its suit and rank, when actually a card is one number — an index — and its suit and rank are values stored in arrays at that index.

We call this **identity-is-an-integer**, and it is the precondition for every economy the rest of this book buys you. Persistence will work because tables are easy to serialise. Parallelism will work because indices are cheap to partition. Replay will work because a deck is just three arrays in a state. None of it works if you reach for `class Card`.

> [!NOTE]
>
> *The strong form, which we will return to later:* sometimes you do not even need the index. The pair `(suit, rank)` already uniquely identifies a playing card — there are only fifty-two such pairs. The index is a *surrogate key*; the pair is a *natural key*. For variable-quantity tables (creatures that come and go) you usually need a surrogate, because two creatures can be identical. For a constant-quantity 52-card deck, you do not.

## Exercises

The first time through, write everything from scratch in `src/main.rs`. Resist the urge to add a `Card` struct or helper methods. Three `Vec`s.

1. **Build the deck.** Write `fn new_deck() -> (Vec<u8>, Vec<u8>, Vec<u8>)` that returns the suits, ranks, and locations for a fresh, ordered deck (all 52 in `location 0 = deck`).
2. **Print a card.** Write `fn card_to_string(suit: u8, rank: u8) -> String` that returns strings like `"A♠"`, `"10♥"`, `"K♦"`. Use it to print the whole deck.
3. **Shuffle.** Write a tiny LCG random function (one-liner) and use it to implement Fisher-Yates on a `Vec<usize>`. Print the deck in shuffled order. Confirm by inspection that the `suits`, `ranks`, and `locations` arrays are unchanged.
4. **Sort by suit then rank.** Sort the `order` vector so suits come out grouped, ranks ascending within each suit. Print again. Once again, the deck arrays are unchanged.
5. **Deal a hand.** Move the first 5 cards from the deck (location 0) to player 1 (location 1). Print player 1's hand using `card_to_string`.
6. **Hand query.** Write `fn cards_held_by(locations: &[u8], player: u8) -> Vec<usize>` returning all card indices currently held by a given player.
7. **Count by location.** Write a function that returns counts grouped by location: how many in the deck, in each hand, in discard.
8. **Deal four hands.** Deal 5 cards to each of players 1, 2, 3, 4. Print all four hands.
9. *(stretch)* **Drop the index.** Rewrite `cards_held_by` to return `Vec<(u8, u8)>` of (suit, rank) pairs directly — no indices. What does this make easier? What does it make harder? (Hint: you cannot move the cards back to the deck without knowing which `i` they were.)
10. *(stretch)* **The sort hazard.** While player 1 is holding indices `[3, 17, 21, 28, 41]`, sort the deck arrays themselves (not just the order) by suit. What does player 1 think they hold now? This is the bug node 9 ("[sort breaks indices](../../concepts/dag.md)") was written for. Don't fix it yet — observe it.

Reference solutions for exercises 1-3 in [05_identity_is_an_integer_solutions.md](05_identity_is_an_integer_solutions.md). Solutions for the rest follow the same shape.

## What's next

Exercise 10 leaves you with a bug. The next section ([§9 — Sort breaks indices](09_sort_breaks_indices.md)) is the fix; it teaches you to keep a stable id alongside the position so external references survive reordering.
