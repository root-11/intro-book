# 41 - Deferred abstraction

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 41](../../concepts/glossary.md#41--deferred-abstraction).*

The instinct most programmers acquire from training is *abstract early*. See a case; imagine the second case; design an interface that handles both. The early abstraction feels tidy. It also breaks down the moment the third or fourth case turns out not to fit.

The data-oriented discipline is the opposite. *Write the concrete case three times before extracting anything.* Then look at the three concrete versions and ask whether the abstraction that fits all three is obvious. Often it is, and the extraction is mechanical. Sometimes it is not - the three cases share less than expected, and the right move is to leave them concrete.

Walk through the failure mode. You write the simulator's `motion` system. You can already see motion would also apply to food drift, particle effects, projectile trajectories. The instinct says: design a generic `Movable` interface. The discipline says: don't yet. Write motion. Move on.

When the second case arrives - say, food drift - you write it concretely. Maybe it shares 80 % of motion's structure. Maybe only 60 %. You see this clearly because both versions exist as concrete code, not as imagined cases.

When the third case arrives, look at all three. Now the shared structure is *measured*, not imagined. If the abstraction is obvious, extract it. If the three cases share only a vague shape, leave them. A bad abstraction is more expensive than three concrete versions of similar code.

The cost saving is in the *avoided* abstractions. A library of premature interfaces - `Movable`, `Mortal`, `Hungry`, `Reproductive` trait hierarchies - is a library of code-shaped scar tissue. Each interface fits some of its uses well and others poorly. The misfits add casts, downcasts, defaults, and special cases. Concrete code has none of these.

The Rust ecosystem demonstrates deferred abstraction repeatedly. `std::iter::Iterator` is the abstraction over many concrete iteration patterns; it earned its place because the concrete patterns existed first and were obviously shared. `serde` is the abstraction over serialisation; it earned its place because every serialisation library was writing the same boilerplate before serde existed. These abstractions feel inevitable because they are *generalizations* of patterns the community had already written by hand many times.

<p align="center"><img src="../illustrations/tip_simplify_full.jpg" alt="Break complex problems into smaller parts. Simplicity leads to clarity." style="max-height: 300px; max-width: 100%;"></p>

The discipline is structural, not stylistic. *Generalize when you can see the shape, not before.* The book's own through-line uses it. The simulator was built one concrete piece at a time. The DAG was named after the systems were built, not before. The trunk vocabulary is the generalization of patterns that actually emerged.

A useful test: after extracting an abstraction, can the abstraction handle a *fourth* case without a special branch? If yes, the abstraction is real. If no - if the abstraction grew an `if-else` for the fourth case - the abstraction was wrong, and the fourth case is the case showing it.

The connection to the next chapter is concrete. A third-party library is somebody else's abstraction - one they generalized from *their* concrete cases. If your three concrete cases match theirs, the library fits and adopting it saves real work. If they do not, the library is friction at every use. [§42](42_you_can_only_fix_what_you_wrote.md) develops this into the dependency-pricing discipline.

## Exercises

1. **Find a too-early abstraction.** Look at code you have written. Find a generic function or trait that has fewer than three concrete uses. Could it be inlined? Often the answer is yes; the abstraction was speculative.
2. **Three concrete versions.** Write `filter_creatures_by_hunger`, `filter_creatures_by_age`, `filter_creatures_by_location`. Three independent functions. Look at them. Is there an obvious shared abstraction?
3. **Resist extraction.** Even with an obvious abstraction in exercise 2, ask: do the three concrete versions read more clearly *as concrete versions*? In some cases yes - a four-line specific function is more legible than a generic `filter_by` with a closure.
4. **Add a fourth case.** Suppose you also want `filter_creatures_by_proximity_to_food`. Does this fit the abstraction from exercise 2? If yes, the abstraction holds. If no (the proximity calculation needs `food`, which the others do not), the abstraction was a tight fit, and the fourth case requires either a new abstraction or a different concrete shape.
5. *(stretch)* **A library audit.** Look at one Rust crate you have used. Identify the abstractions it offers. For each, ask: does it match three or more concrete cases that came before it, or is it an abstraction of one case generalised on speculation? The answer says whether the crate is a real generalization or a guess.

Reference notes in [41_compression_oriented_solutions.md](41_compression_oriented_solutions.md).

## What's next

[§42 - You can only fix what you wrote](42_you_can_only_fix_what_you_wrote.md) extends deferred abstraction to dependencies: every crate is somebody else's abstraction; adopting it is a bet that their generalization matches yours.
