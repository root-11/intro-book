# 7 — Structure of arrays (SoA)

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 7](../../concepts/glossary.md#7--structure-of-arrays-soa).*

<p align="center"><img src="../illustrations/ecs_banner.jpg" alt="Three mice: ENTITY, COMPONENT, SYSTEMS — naming the layout that splits an entity into component columns" style="max-height: 300px; max-width: 100%;"></p>

Your deck has three `Vec`s: `suits`, `ranks`, `locations`. Each field lives in its own array, indexed by entity. This layout is called *Structure of Arrays* — SoA. The opposite layout — a single `Vec<Card>` where each element is a struct holding all three fields — is called *Array of Structs* — AoS. They are different choices about *where the same data lives*.

```rust,no_run
// SoA: three columns, indexed in lockstep
let suits:     Vec<u8> = vec![/* 52 */];
let ranks:     Vec<u8> = vec![/* 52 */];
let locations: Vec<u8> = vec![/* 52 */];

// AoS: one column of structs
struct Card { suit: u8, rank: u8, location: u8 }
let cards: Vec<Card> = vec![/* 52 */];
```

Most programmers reach for AoS by default because it groups "related" data together. The trouble is that in a real loop "related" is whatever the inner loop reads, not whatever the data model says belongs together. A system that counts cards in player 1's hand reads only `locations` — it does not need suits or ranks at all. With SoA, that loop reads exactly 52 bytes from `locations`. With AoS, the loop reads all three bytes of each `Card` (because they live next to each other in memory and arrive on the same cache line) and ignores two of them — three times the memory traffic for the same answer.

At 52 cards the difference is invisible. At one million creatures with six fields each, the difference is the difference between a 30 Hz simulation and a 5 Hz one. The motion system in §1's simulator reads only `pos`, `vel`, and `energy` — three of six creature fields. With SoA it reads three sequential streams of exactly the bytes it needs. With AoS it reads all six fields of every creature, paying twice the memory bandwidth for half the data it actually wants.

This is the bandwidth-bound regime named in §4. SoA keeps the inner loop's working set small; AoS bloats it with fields the loop ignores. At cache-spilling sizes (any working set bigger than L3) the bloat becomes the dominant cost.

SoA is therefore the default in this book. AoS is sometimes the right choice — for example when every system reads every field, or when N is so small the cache line is dominated by per-row overhead either way. But this is a tradeoff to *earn* by measurement, not to assume by habit. Write SoA first; switch to AoS only when a benchmark forces you to.

## Exercises

You will need a stopwatch (`std::time::Instant`) for some of these.

1. **Build both layouts.** Take your §5 deck and add an AoS twin: a `Vec<Card>` of 52 entries, where `Card { suit: u8, rank: u8, location: u8 }`. Build both and verify they hold the same logical content.
2. **Count cards in a player's hand, both ways.** Write `fn count_held_soa(locations: &[u8], player: u8) -> usize` and `fn count_held_aos(cards: &[Card], player: u8) -> usize`. Confirm they return the same number on the same deck.
3. **Time the count at 10,000 entries.** Make `Vec<u8>` and `Vec<Card>` of length 10,000 (replicate the deck 192-fold, or fill arbitrarily). Time each `count_held_*` function. Note the ratio.
4. **Scale to 1,000,000 entries.** Repeat at length 1,000,000. The SoA version reads 1 MB; the AoS version reads 3 MB (assuming `size_of::<Card>() == 3` plus padding). On most chips L2 fits one but not the other. Note where the cliff appears.
5. **The hot/cold case.** Extend the row with a 16-byte `nickname: [u8; 16]`. Rebuild both. Now AoS reads 19+ bytes per element while SoA still reads 1. Time the count again. The gap should widen sharply.

> [!NOTE]
> How sharp depends on your memory hierarchy. Measured ratios at N=10M: ~2× on machines with generous L3 (modern desktops, mid-2010s Intel laptops), ~6× on a Raspberry Pi 4 (no L3, narrow LPDDR4 channel). The principle is the same; the slope of the cliff scales with how badly the AoS row blows the cache budget.

6. **A case where AoS wins.** Write a function that updates *every* field of one specific card. SoA writes to three different lines; AoS writes to one. For the case "update every field of every card" (rare in practice), AoS may even tie or win. Time it and discuss.
7. *(stretch)* **A from-scratch `SoaDeck` struct.** Wrap the three (or four) columns in one struct that owns them all. Provide `fn reorder(&mut self, order: &[usize])` as the only public mutator. What do you gain in correctness? What do you lose in flexibility?

Reference notes in [07_structure_of_arrays_solutions.md](07_structure_of_arrays_solutions.md).

## What's next

[§8 — Where there's one, there's many](08_where_theres_one_theres_many.md) is the universalising principle. The deck taught it implicitly; the next section names it.
