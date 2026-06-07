# 15 - State changes between ticks

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 15](../../concepts/glossary.md#15---state-changes-between-ticks).*

<p align="center"><img src="../illustrations/microcontroller_loop.jpg" alt="Init / while { read; process; update } - the visible tick loop" style="max-height: 300px; max-width: 100%;"></p>

Inside a tick, the *population* is frozen: no creature is born or dies mid-tick. Structural changes - insertions and removals - are *queued*, not applied, and committed in one atomic sweep at the tick boundary. Value updates are different: they flow along the DAG in tick order, each system reading its inputs as the upstream writers left them, never half-written.

This is the rule that makes the DAG from [§14](14_systems_compose_into_a_dag.md) actually work. The danger is a system reading another's half-finished work: if `next_event` began reading `pos` while `motion` were still writing it, half the creatures would have moved and half would not, and what `next_event` reads would no longer be well-defined. Two rules remove the danger. *Order*: a system runs only after the systems it reads from have *completed*, so it sees their finished output, never a partial write. *Frozen membership*: no system adds or removes a row mid-tick, so the *set* of creatures every system iterates is identical. Together they make the tick a clean function `world_t+1 = step(world_t, inputs_t)` - values move forward along the DAG, the population holds still, and `cleanup` commits the queued births and deaths only at the boundary.

Concretely: `apply_starve` does not call `creatures.swap_remove(slot)`. It calls `to_remove.push(creature_id)`. The `creatures` table is unchanged for the rest of the tick. After every system has run, `cleanup` consumes `to_remove` and `to_insert` together, applying every queued change in one sweep. *Now* the next tick begins with a consistent new world state.

This pattern is called *double buffering*: there is the committed world the systems read this tick, and the buffer of queued changes (`to_remove`, `to_insert`) that `cleanup` commits to produce the world the next tick reads. The pattern shows up everywhere - graphics frame buffers, database transactions, event-sourced systems. The rule is always the same: structural writes accumulate, then commit.

Two costs to absorb. First, every queued birth or death is one extra row pushed to a `to_remove` or `to_insert` table. Second, the cleanup pass is now its own system in the DAG. The benefit dwarfs the costs: every other system in the book composes cleanly, and parallelism becomes easy. With in-tick mutation, every parallel scheduling decision becomes a race condition. With buffered mutation, races are structurally impossible - disjoint write-sets are disjoint by construction.

A subtle case is *insertions*. A creature born during a tick (via `apply_reproduce`) does not appear in any system's read-set during that tick - it is in `to_insert`, not in `creatures`. The newborn lives its first life on the *next* tick. This is the right behaviour for almost every simulation: it gives every creature an equal first tick of life. The alternative - applying inserts mid-tick - is a closed-loop bug factory.

Within one system, the writes *can* be in-tick: a system that updates `pos` for every creature in a loop applies each write immediately, because the rest of the system is the only reader and the only writer. The buffering rule is between *systems*, not between iterations within one system. Inside a system, the writes are sequential; between systems, the writes are batched.

The shape that emerges is: read everything into local arrays at system entry; do work; write outputs to buffers at system exit; commit at tick boundary. It is the same shape as the audio engine's frame buffer, the database's transaction commit, and the version-controlled file system's commit-and-merge. They all solve the same problem: how do you read consistent state while the world is changing?

## Exercises

These build on the simulator skeleton. Your `to_remove: Vec<u32>` and `to_insert: Vec<CreatureRow>` should already exist.

1. **The bug.** Write a function that iterates `creatures` and calls `creatures.swap_remove(i)` whenever `energy[i] <= 0.0`. Run it on a 100-creature world where 30 are starving. What goes wrong? (Hint: skipped iterations, half the starvers survive.)
2. **The fix.** Rewrite the function to push the index into `to_remove` instead. After the loop completes, apply all removals in one pass. Verify all 30 starvers die.
3. **The cleanup pass.** Write `fn cleanup(world: &mut World, to_remove: &mut Vec<u32>, to_insert: &mut Vec<CreatureRow>)`. Apply removals first (using `swap_remove`), then insertions. Why this order, and not the other?
4. **Show two ticks.** Run the loop for two ticks. After tick 1, log the population. After tick 2, log it again. Confirm that creatures killed in tick 1's `apply_starve` *do not* appear in tick 2's input.
5. **Insertions are tick-delayed.** A creature reproduces in tick 5: parent in `creatures`, two offspring in `to_insert`. After cleanup, the offspring are in `creatures`. In tick 6 the offspring receive their first system pass. Confirm by adding an `age_in_ticks` column and watching offspring start at 0 in tick 6, not in tick 5.
6. *(stretch)* **A bad design that almost works.** Try to apply mutations in-tick *carefully* - collect dead creatures first, then process them in reverse-index order. Show one specific case where this still corrupts state. (Hint: a reproduction produces an offspring whose new index conflicts with an in-progress death.)

Reference notes in [15_state_changes_between_ticks_solutions.md](15_state_changes_between_ticks_solutions.md).

## What's next

[§16 - Determinism by order](16_determinism_by_order.md) is the property the buffering rule *guarantees*: same inputs, same system order, same outputs. Reproducibility is structural.
