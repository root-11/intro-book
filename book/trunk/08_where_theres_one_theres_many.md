# 8 - Where there's one, there's many

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 8](../../concepts/glossary.md#8---where-theres-one-theres-many).*

<p align="center"><img src="../illustrations/tip_simplify_full.jpg" alt="Break complex problems into smaller parts - the singleton special-cased away" style="max-height: 300px; max-width: 100%;"></p>

Code is written for the array. A function that operates on one entity is just the special case of N = 1; it does not need its own abstraction. A card game with 52 cards is three arrays - suit, rank, location - not 52 objects. A simulation with 100 creatures is six arrays of length 100, not 100 instances of `Creature`. The plural is the primary unit; the singular is the trivial case.

The pattern is simple. Write the array version first. The singleton drops out as a one-element slice. To shuffle one card you swap two indices in the `order` vector - same as shuffling the whole deck. To find the highest-rank card in player 1's hand you scan the (small) hand vector - same shape as scanning all 52. To deal one card you write one cell in `locations` - same shape as dealing many cells.

This stands against an instinct most programmers acquire from OOP: the urge to write `card.shuffle()` or `creature.update()` and then puzzle over how to do it for many. The puzzle does not exist when you write for arrays from the start. `shuffle(&mut deck)` is one function that works for any deck, including a deck of one. `update(&mut creatures)` is one function that works for any population, including a population of one.

A useful test: when you find yourself writing a method on a struct, ask *what does this look like over an array?* If the array version is shorter, drop the method. If the array version is the same length, keep the method as a function over a slice - `fn shuffle(deck: &mut Deck)`, not `impl Deck { fn shuffle(&mut self) }`. Either way, the singleton was never the right unit of code.

There is also a cost reason, though it does not bite at 52 cards. A method that runs on one entity at a time forces its caller to invoke it N times: N opaque calls the optimiser cannot fuse into a loop. A function over a slice is *one* call - the compiler sees the whole loop and can lift invariants, reorder, and vectorise it. Writing for the array keeps the work visible to the optimiser; writing for the singleton hides it. The bill for that hiding does not arrive until the simulator is walking a million rows a tick, and [§19](19_ebp_dispatch.md) measures it there. At deck scale this is a reason to prefer the array form, not yet a speed you can feel.

"Where there's one, there's many" is therefore not an architectural slogan but a daily practice. It costs nothing the first time. It costs everything the first time you forget.

## Exercises

These extend the deck again. The aim is to feel the array-first pattern in your fingertips before §5 turns into the rest of the book.

1. **The function over a slice.** Write `fn highest_rank_in_hand(hand: &[u32], ranks: &[u8]) -> Option<u8>` returning the highest rank held in the supplied set of card ids. Use it on a 5-card hand. Then use it on a 1-card hand. Then use it on an empty hand. Same function, three N values.
2. **Reverse the urge.** Given an OOP-style `Card::is_face_card(&self) -> bool`, rewrite it as `fn face_cards(ranks: &[u8]) -> Vec<bool>` - a function over the whole `ranks` array returning a parallel mask. Apply it to all 52 cards in one call.
3. **The N = 0 case.** What does `highest_rank_in_hand` do for an empty `hand`? Should it panic, return `None`, or return some sentinel? Pick one and justify.
4. **Predicate over a single value.** Suppose you want `is_red(suit: u8) -> bool` for a single card (suits 0 and 1 are hearts/diamonds). Write the array version `fn red_mask(suits: &[u8]) -> Vec<bool>` first. Then convince yourself the singleton case is `red_mask(&[suit])[0]` - the array version covers it.
5. *(stretch)* **From a tutorial.** Find any Rust tutorial that uses a `struct Card` with methods (`new`, `is_face`, `display`, etc.). Rewrite their full card game as three (or four) `Vec`s plus free functions. Compare line counts. Compare clarity. Compare what happens when you want to query "all face cards across the table" - one function call versus a loop over per-card method calls.

Reference notes in [08_where_theres_one_theres_many_solutions.md](08_where_theres_one_theres_many_solutions.md).

## What's next

You have closed Identity & structure. Cards behave; rows align; layouts are SoA; the singleton drops out. The next phase is *Time & passes*, starting with [§11 - The tick](11_the_tick.md). The ecosystem simulator from `code/sim/SPEC.md` is about to start running.
