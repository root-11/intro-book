# Solutions: 7 — Structure of arrays (SoA)

## Exercise 1 — Build both layouts

```rust
struct Card { suit: u8, rank: u8, location: u8 }

// SoA
let (suits, ranks, locations) = new_deck();

// AoS
let cards: Vec<Card> = (0..52)
    .map(|i| Card {
        suit: suits[i],
        rank: ranks[i],
        location: locations[i],
    })
    .collect();
```

Note `std::mem::size_of::<Card>()` is 3 in theory but Rust may pad to 4 for alignment unless you `#[repr(packed)]` (which has its own hazards). For the timing exercises below, that 3-vs-4 detail does not change the qualitative result.

## Exercises 2-3 — Counting and timing

```rust
fn count_held_soa(locations: &[u8], player: u8) -> usize {
    let mut n = 0;
    for &l in locations { if l == player { n += 1; } }
    n
}

fn count_held_aos(cards: &[Card], player: u8) -> usize {
    let mut n = 0;
    for c in cards { if c.location == player { n += 1; } }
    n
}
```

Both compile to tight loops. The SoA loop reads one byte per iteration; the AoS loop reads sizeof(Card) bytes per iteration even though it only inspects one field. For 10,000 entries the AoS loop is roughly 3-4× slower; for 1,000,000 the gap widens further as the AoS working set spills more cache levels.

## Exercise 4 — The cliff

At 1,000,000 entries: SoA is 1 MB (fits L2 on most chips); AoS at 4 bytes/Card is 4 MB (out of L2, into L3). At 10,000,000 SoA still fits L3; AoS does not. Each cache transition is a sharp slowdown for AoS while SoA continues at near-L2 speed.

## Exercise 5 — Hot/cold

Add `nickname: [u8; 16]` to `Card`. Now `size_of::<Card>() ≥ 19` (will pad to 20 or 24 for alignment). The AoS count loop reads 24 bytes per element while SoA still reads 1. The ratio is no longer ~3-4× — it is ~20×. This is the hot/cold split waiting to happen, named in [§26](26_hot_cold_splits.md).

## Exercise 6 — When AoS wins

```rust
fn touch_all_fields_aos(cards: &mut [Card], i: usize) {
    cards[i].suit = (cards[i].suit + 1) % 4;
    cards[i].rank = (cards[i].rank + 1) % 13;
    cards[i].location = 0;
}

fn touch_all_fields_soa(suits: &mut [u8], ranks: &mut [u8], locations: &mut [u8], i: usize) {
    suits[i]     = (suits[i] + 1) % 4;
    ranks[i]     = (ranks[i] + 1) % 13;
    locations[i] = 0;
}
```

For a *single* card, AoS touches one cache line; SoA touches three (one per column). For a million cards iterated in order, both layouts stream their data through cache and the difference is small — but the SoA version uses 3× the cache lines per iteration. This is the reverse of exercise 2 and is the case where AoS may win.

In practice this case is rarer than it sounds. Most systems read or write only a subset of fields per pass; the situations where every field is touched together usually live at the *boundary* (deserialise the row, write it to disk) rather than in the inner loop.

## Exercise 7 — `SoaDeck` sketch

```rust
struct SoaDeck {
    suits:     Vec<u8>,
    ranks:     Vec<u8>,
    locations: Vec<u8>,
}

impl SoaDeck {
    fn reorder(&mut self, order: &[usize]) {
        self.suits     = order.iter().map(|&i| self.suits[i]).collect();
        self.ranks     = order.iter().map(|&i| self.ranks[i]).collect();
        self.locations = order.iter().map(|&i| self.locations[i]).collect();
    }
    // Read accessors; no per-column mutators exposed.
}
```

What you gain: the only way to reorder is through `reorder`, which takes all columns at once — alignment is enforced by the type system. What you lose: a system that wants to mutate just `locations` cannot do so without going through the wrapping struct or breaking the encapsulation. You have moved the alignment discipline from a code-review concern to a type-system concern, at the cost of some flexibility. This is mechanism-vs-policy ([§40](40_mechanism_vs_policy.md)) — choose where the rule lives.
