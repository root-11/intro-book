# 20 — Empty tables are free

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 20](../../concepts/glossary.md#20--empty-tables-are-free).*

<p align="center"><img src="../illustrations/tip_visualize_full.jpg" alt="Visualize the problem — the diagram of an empty table is free" style="max-height: 300px; max-width: 100%;"></p>

If a presence table is empty, the system that iterates it does nothing. No rows, no work. This is the consequence of [§19](19_ebp_dispatch.md) at the limit, and it is the property that lets the simulator scale gracefully under shifting state.

Concretely: a 1 000 000-creature simulation that has zero hungry creatures right now spends *zero* cycles in `drive_hunger`. The system is wired into the DAG, runs every tick, takes a `&[u32]` slice of `hungry` of length 0, iterates zero times, returns. The overhead is one function call and one slice creation — measured in nanoseconds, not milliseconds.

This is not "fast in the empty case as an optimisation". It is "*free* in the empty case as a structural consequence". The flag-based version runs through the entire creature table even when all flags are `false`, paying full memory bandwidth to discover that no work is needed. The EBP version is told there is no work by the simple fact of an empty table.

The effect compounds across many states. A simulation with twenty possible behaviours, each represented as a presence table, pays for the fraction of creatures actually exhibiting each behaviour. Most ticks, most tables are nearly empty. The total work is proportional to the *sum of active rows across all tables*, not to *population × number of behaviours*. For a sparsely active world this is one or two orders of magnitude cheaper than the equivalent flag-based design.

A subtle case worth naming: an *empty system* is not the same thing as a *missing system*. A `drive_hunger` system that iterates an empty `hungry` is still in the DAG, still scheduled, still part of the program's contract. It is just doing zero rows of work this tick. Removing it from the DAG entirely would change the contract. EBP gives you cheap idle systems, not absent ones.

This property has implications for design.

**Activity-based costs.** A simulator's per-tick cost is set by what is *active*, not by what *exists*. A million dormant creatures cost nothing to ignore. Only behaving creatures consume budget. Most simulators in production rely on this — game worlds with hundreds of thousands of NPCs but only a few in active play, training simulations with millions of agents but few in critical phases, control systems with thousands of sensors but few in alarmed state.

**Structural sparsity.** The world is encouraged to be in mostly-resting states. Designs that scatter activity across many small presence tables (lots of cheap idle systems) outperform designs that concentrate activity in a single big "active creatures" flag. The data-oriented mindset is to multiply states (`hungry`, `sleepy`, `mating`, `fighting`, ...) rather than gate behaviour through one master switch.

**Persistence is also activity-based.** A snapshot of an empty `hungry` table is one row in the schema and zero rows of data. A snapshot of an `is_hungry: Vec<bool>` of length 1 000 000 is 1 MB regardless of how many flags are set. Backups, replication, and replay all benefit from the same property.

The flag-based mind sees idle objects as "still present, just inactive". The data-oriented mind sees idle objects as *not in the table*. The difference is one of cost: the former pays for what exists; the latter pays for what is happening.

## Exercises

1. **Time the empty case.** With your simulator from [§19](19_ebp_dispatch.md), run a tick where `hungry` is empty. Time `drive_hunger`. It should be in the nanoseconds range — function call plus slice creation, no inner loop.
2. **Time the same case in flag form.** Run the flag version of `drive_hunger` against a 1 000 000-creature world where all `is_hungry` are `false`. Time it. Should be milliseconds — full table walked.
3. **The cost-per-active-creature plot.** Run the EBP simulator with `hungry.len()` ranging over 0, 100, 1 000, 10 000, 100 000, 1 000 000. Time `drive_hunger` at each. Plot. The line is roughly linear, starting at near-zero.
4. **Add four more states.** Add `sleepy`, `mating`, `fighting`, `idle` as presence tables, each with its own driver system. Run a tick where most tables are empty (most creatures are in `idle`, say). Confirm the per-tick cost is roughly the cost of the `idle` driver only.
5. **Activity histogram.** At each tick, log `(tick, table_name, len)` for every presence table. After 1000 ticks, plot `len` over time. The plot is the simulator's *activity profile*; flat lines mean the world is at rest, bumps mean events are firing.
6. *(stretch)* **Idle systems removed?** Argue why removing an empty system from the DAG (rather than running it with zero work) is the wrong move. Hint: it changes the system DAG, breaks determinism if the table is non-empty next tick, and adds dynamic scheduling cost that exceeds the empty-loop overhead.

Reference notes in [20_empty_tables_are_free_solutions.md](20_empty_tables_are_free_solutions.md).

## What's next

You have closed Existence-based processing. The next phase is *Memory & lifecycle*, starting with [§21 — `swap_remove`](21_swap_remove.md). The simulator is about to start making structural changes to its tables — births and deaths, in production volumes — and the lifecycle phase makes those cheap.
