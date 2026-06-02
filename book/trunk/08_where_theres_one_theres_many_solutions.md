# Solutions: 8 - Where there's one, there's many

## Exercise 1 - The function over a slice

```rust
fn highest_rank_in_hand(hand: &[u32], ranks: &[u8]) -> Option<u8> {
    let mut best: Option<u8> = None;
    for &id in hand {
        let r = ranks[id as usize];
        best = match best {
            None => Some(r),
            Some(b) => Some(b.max(r)),
        };
    }
    best
}

let hand5: Vec<u32>   = vec![3, 17, 21, 28, 41];
let hand1: Vec<u32>   = vec![41];
let hand0: Vec<u32>   = vec![];

println!("{:?}", highest_rank_in_hand(&hand5, &ranks)); // Some(some rank)
println!("{:?}", highest_rank_in_hand(&hand1, &ranks)); // Some(ranks[41])
println!("{:?}", highest_rank_in_hand(&hand0, &ranks)); // None
```

One function. Three N values. The N = 1 and N = 0 cases are not special-cased; they fall out.

## Exercise 2 - Reverse the urge

```rust
fn face_cards(ranks: &[u8]) -> Vec<bool> {
    ranks.iter().map(|&r| r >= 10).collect() // 10 = J, 11 = Q, 12 = K (0-indexed)
}
```

Compared to a per-card `Card::is_face_card(&self) -> bool` plus a loop, the array version is shorter, more cache-friendly, and trivially vectorisable.

## Exercise 3 - The N = 0 case

`Option<u8>` returning `None` is the cleanest answer. A panic is hostile to callers (the empty case is a *valid* state of the world - a player just played their last card). A sentinel like `255` confuses the type. `Option` makes the absence visible at the type level.

## Exercise 4 - Singleton as trivial array

```rust
fn red_mask(suits: &[u8]) -> Vec<bool> {
    suits.iter().map(|&s| s < 2).collect() // suits 0,1 = hearts, diamonds
}

let one_suit = 0u8;
let is_red_one = red_mask(&[one_suit])[0]; // true
```

The singleton drops out as a one-element call. There is no separate `is_red(suit: u8) -> bool` function. If the call site is ergonomic enough you may write a thin wrapper for clarity, but the array version is the canonical implementation.

## Exercise 5 - From a tutorial

This is open-ended. The expected outcome: the rewritten array-first version is shorter, has fewer indirections, and answers cross-cutting queries (all face cards on the table; all spades in any hand) in one function call rather than a loop over methods.

A typical OOP card game tutorial weighs around 200-400 lines. The array-first rewrite of the same functionality usually lands at 80-150 lines, with the bulk of the savings coming from *not writing* getters, setters, copy semantics, and the various small accessors that an OOP `Card` accumulates.
