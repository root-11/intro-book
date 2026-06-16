# 43 - Tests are systems; TDD from day one

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 43](../../concepts/glossary.md#43---tests-are-systems-tdd-from-day-one).*

<p align="center"><img src="../illustrations/dag_planning_checklist.jpg" alt="PLAN, ANALYZE, DESIGN, BUILD, TEST, IMPROVE - tests are part of the same loop, written first" style="max-height: 300px; max-width: 100%;"></p>

A test reads the world's state and asserts that some property holds. A system reads the world's state and writes a derived result. The two are structurally the same.

This is not a slogan. It is the structural fact that lets every other discipline in the book apply to tests without translation.

A test fixture is *the world at some tick*. A test is *a system whose write-set is empty*, or whose write-set is a small "report" table. A test runner is *the same scheduler that runs the simulator*, executing the test's read-set against the world.

```rust,no_run
fn no_creature_moves_too_far(
    px_before: &[f32], py_before: &[f32],
    px_after:  &[f32], py_after:  &[f32],
    max_step:  f32,
) -> Vec<(usize, f32)> {
    let mut suspicious = Vec::new();
    for i in 0..px_before.len() {
        let dx = px_after[i] - px_before[i];
        let dy = py_after[i] - py_before[i];
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > max_step {
            suspicious.push((i, dist));
        }
    }
    suspicious
}
```

This is a system. Read-set: `pos_before`, `pos_after`, `max_step`. Write-set: a report `Vec`. It runs over the simulator's tables. It asserts a property. It can run as part of the DAG (in test mode) or in production (as an inspection system). The same code path serves both uses.

Three benefits compound.

**Property tests over component arrays fall out.** A property test fixes an RNG seed, runs the simulator for N ticks, and asserts that some property holds at every tick. If the property is "no creature moves more than `max_step` per tick", the assertion is the system above. If it is "the population stays bounded", the assertion is `count(creatures) < bound`. Each is a system.

**Replay tests over event logs fall out.** A replay test loads a recorded log, runs the replayer, and compares the resulting world to a snapshot. The "test" is the comparison; the comparison is a system over both worlds' tables.

**Integration tests do not need mocks.** A mock exists because the test cannot exercise the real component. The boundary-as-queue rule from [§35](35_boundary_is_the_queue.md) means there are no external components inside the simulator - every external interaction goes through the queues. A test fills the in-queue with synthetic input, runs the simulator, asserts on the out-queue. No mocks; the test reads the same data the simulator reads.

The TDD-from-day-one piece is what makes this practical. From [§5](05_identity_is_an_integer.md) onward, every concept in the book is approached test-first. *What's the smallest case? What's the largest? What should the answer be for `u8`, for `u32`, for 10 000 agent ids?* The deck-game exercises start by asking "what should this return for a deck of 0 cards, of 1, of 52?" The simulator's exercises ask "what should population be after 100 ticks of zero food?" Tests come first; implementation follows.

The discipline pays off three ways:

- **Tests grow with the code.** Each new system has its tests as adjacent functions, sharing the same read/write conventions. A test refactor is no different from a system refactor.
- **Inspection and testing are the same code.** The InspectionSystem pattern from [§13](13_system_as_function.md) is identical to the test pattern: read-only access to all tables, output a report. In production, inspection is absent; in test, it is present and asserting. Same source code, different schedule.
- **Determinism makes tests trustworthy.** [§16](16_determinism_by_order.md)'s rule means tests are reproducible. A test that fails with seed `0xCAFE` fails with `0xCAFE` every time, on every machine. No flakiness.

## Tests are systems - and so is the budget

A test asserts a property of *logic* and passes or fails. Cost wants the same vigilance but cannot take that form: you cannot assert "this tick takes under 33 ms" as pass or fail, because a wall-clock number carries the machine, the scheduler, and the thermal state - run it twice and it disagrees with itself. The cost side is a *benchmark*, not a verdict. The analogy is still exact, `unit test : logic :: scale sweep : cost`: a scale sweep is a test-shaped system aimed at cost. Run each system across log-spaced scales, take the *minimum* of a few repetitions at each (the OS only ever adds time, so the minimum is the machine's floor with interference subtracted out), and watch where each curve crosses the budget. The system that crosses first is the binding constraint; improve it, re-sweep, watch the crossing move out. You characterise the envelope rather than assert a threshold - the one falsifiable, one-sided claim is that *even the unimpeded minimum exceeds the budget*, which is definitively too slow; everything above that floor is variance, read as a curve and not a red light.

Two habits keep the sweep honest, and both are where intuition lies. You do not know where the time goes - the hotspot is as often a sort you did not need as the arithmetic you expected - so you profile to find the binding line rather than guess it. And a benchmark that does not grow the way production grows reports a confident, precise, wrong number; scale it on the axis the system actually will, or it lies with a chart attached. The per-chapter measurements in this book are the baseline of that envelope: "the dense bin streams," "the representative holds linear" are not claims you trust once but curves you watch hold as the code changes. Measurement, made a tracked instrument rather than a one-time exhibit.

The book is closing.

Forty-two concepts; nine phases; one through-line simulator. The disciplines named in this last phase - mechanism vs policy, deferred abstraction, you-can-only-fix-what-you-wrote, tests-are-systems - are the rules that hold the rest together. They are not new architecture. They are how the architecture earlier chapters built stays maintainable.

A simulator that respects all forty-three nodes is one whose state is in tables, whose transformations are systems, whose tick is a pure function, whose history is a log, whose persistence is transposition, whose tests are systems, and whose dependencies are bets you took with your eyes open.

That is the data-oriented program. That is the book.

## Exercises

1. **A test as a system.** Take the `no_creature_moves_too_far` system from the prose. Add it to your simulator's DAG behind a `--test` flag. Run for 100 ticks. The system should report zero suspicious creatures.
2. **A property test.** Run the simulator for 1000 ticks with seed `0xCAFE`. Assert: `population <= 2 * initial_population`. Run twice with the same seed; both runs should report the same outcome (passing or failing at the same tick).
3. **A replay test.** Save the in-queue of a 100-tick run. Load it into a fresh simulator and replay. After 100 ticks, hash both worlds. They must match.
4. **TDD a new system.** Pick a piece of behaviour you have not built - say, "creatures with energy above 50 grow more slowly". Write the test first: what's the smallest case (one creature)? Largest (a million)? Then write the system. Confirm the test passes.
5. **The InspectionSystem connection.** Take the test from exercise 1 and the inspection-system idea from [§13](13_system_as_function.md). Argue why they are structurally identical - same read-set, same lack of write-set, same scheduling slot.
6. *(stretch)* **A test runner that *is* the simulator's scheduler.** Implement a tiny test runner whose only difference from the simulator's scheduler is *which* systems it includes in the DAG: production systems for live runs, test-and-inspection systems for test runs. The two binaries share most of their code; the difference is the systems list.
7. **The scale sweep (a test for cost).** Time one system across log-spaced scales (10K, 100K, 1M), taking the *minimum* of three repetitions at each. Lay your budget across the curve and find the scale where it crosses. Then make the same measurement lie: hold one input fixed while growing another so a hidden quantity (density, fan-out) stays constant, and watch the curve flatten into a falsely linear shape. State the axis a sweep must grow on for your system, and the one falsifiable claim a wall-clock number actually supports.

Reference notes in [43_tests_are_systems_solutions.md](43_tests_are_systems_solutions.md).

## What's next

You have closed the trunk. [§44 - What you have built](44_closure.md) looks back at the shape of what you built and opens the questions the book deliberately did not settle.

