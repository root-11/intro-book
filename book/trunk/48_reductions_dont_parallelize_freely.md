# 48 - Reductions don't parallelize freely

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 48](../../concepts/glossary.md#48---reductions-dont-parallelize-freely).*

[§31](31_disjoint_writes_parallelize.md) earned a strong claim: systems with disjoint write-sets parallelise freely, with no locks and no coordination. [§16](16_determinism_by_order.md) earned another: same seed, same system order, same world, every run. Both are true. Put them under one stress the first act never applied - *a different number of cores* - and a seam opens between them. The world that hashed identically on your four-core laptop hashes differently on the thirty-two-core server. Same code, same seed, same log. Different machine, different world.

This is the worst class of bug, because it passes. It passes every test you ran, because you ran them on one machine with one core count. It surfaces only after the move to the hardware you have never seen - the unattended server of [§46](46_log_survives_power_loss.md) and [§47](47_observation_is_a_system.md), where you cannot attach a debugger and the only symptom is that two nodes that should agree do not. The determinism survived everything except the deployment.

The cause is one fact and one consequence.

**Floating-point addition is not associative.** `(a + b) + c` is not always `a + (b + c)` in the last bits. This is not a bug in your code or your hardware; it is IEEE-754 working as specified. Each addition rounds its result to fit the mantissa, and rounding depends on the magnitudes being added. Change the grouping and you change which intermediate values get rounded, and the final bits move.

**A parallel reduction groups by core count.** Split a sum of a million values across four threads and you add four partials, each a sum of 250 000 in some order, then combine the four. Across eight threads you add eight partials of 125 000. Serially you add all million in index order. Three different groupings, three different roundings, three different results in the low bits. The reduction's output is a function of how many threads computed it. Same data, same seed, more cores, different number.

And it does not stay in the low bits. That last-bit difference is an input to the next tick. A simulation is a feedback loop; small differences amplify. Over enough ticks two worlds that started one ULP apart are visibly, structurally different - different creatures alive, different population. The [§16](16_determinism_by_order.md) world-hash that was the bedrock of replay, distribution, and testing now depends on `nproc`. "The log is the world" ([§37](37_log_is_world.md)) quietly acquired an asterisk: *on the same core count*. Distribution - two nodes converging from one log - breaks outright.

The canary is precise: **hashes stable when you fix the core count, unstable when you change it.** If your world hashes match locally and diverge in CI, and the CI box has a different `nproc`, suspect a parallel floating-point reduction before anything else.

The fix is not "stop parallelising." [§31](31_disjoint_writes_parallelize.md) still holds: the per-element work parallelises freely. The reduction is the one place where parallel work meets a single shared result, and that is the only place order leaks back in. So you isolate the non-determinism to the combine step and make *that* deterministic. **Determinism is a property of the combine, not the compute.** Two ways to buy it.

**Fix the reduction order.** Choose a *fixed* number of partitions, independent of the thread count - say sixty-four - and reduce each into a slot indexed by its partition id. The threads share those fixed partitions however the scheduler likes; the partials land in id order regardless of which thread computed which, and a single serial fold walks the slots in id order. The grouping is now defined by the fixed partition count, not by how many threads ran. The easy mistake - and it is the obvious one - is to make the partition count *equal* the thread count, giving each thread its own partition: that changes the grouping right back, and the result still moves with `nproc`. The number of partials must be fixed, not the number of threads. The expensive per-element work still runs on all cores; only the fold over a handful of partials is serial, and a handful is cheap. You keep the [§31](31_disjoint_writes_parallelize.md) speedup and recover the [§16](16_determinism_by_order.md) guarantee - the result still rounds per addition, but it rounds the *same way every time, on every machine*.

**Accumulate in integers.** Integer addition *is* associative: exact, order-independent, identical on one core or sixty-four. Scale each value to a fixed-point integer, sum exactly in any order, scale back at the end. There is no rounding to reorder because there is no rounding until the final scale-back. The price is range management - you choose the fixed-point scale, and you must not overflow the integer - so it fits best where the quantity is bounded and its precision is known, like a sum of energies. Integer accumulation is deterministic by construction; fixed-order floating-point is deterministic by discipline. Where you can bound the range, integers are the stronger guarantee.

Neither is exotic, and the choice is usually easy: retrofit fixed-order onto an existing float pipeline; reach for fixed-point when the reduction is core to correctness and the range is known.

One forward note. Threads are not the only reducer that reorders. A SIMD sum adds in lanes; a GPU reduction adds in a tree across thousands of lanes. Both reorder, both diverge, and both take the same two fixes. The discipline you build here is the precondition for crossing to a vector unit or an accelerator at all - the heterogeneous-compute chapter inherits it directly.

The exclusion, named: this is about *reproducibility*, not *accuracy*. A fixed-order or integer reduction is not more correct in the numerical-analysis sense - it does not get you closer to the true sum - it gets you the *same* answer every time, which is what replay, distribution, and a passing test across machines require. If you need accuracy too, that is compensated summation, a separate technique layered on top.

## Measurements

The divergence is a demonstration, not a benchmark. Measured (`reduction_divergence`, one million harmonic values), the low bits of the parallel float sum change with the thread count:

| threads | racy (partition = threads) | fixed-order (64 partitions) | integer (i128) |
|---|---|---|---|
| 1 | `…1df0d6` | `…1df271` | `…025a920c` |
| 2 | `…1df2a6` | `…1df271` | `…025a920c` |
| 4 | `…1df2c6` | `…1df271` | `…025a920c` |
| 8 | `…1df234` | `…1df271` | `…025a920c` |

The racy column is a different result at every thread count; the fixed-order and integer columns are bit-identical across all four. (The fixed-order value differs from the racy one-thread value in the low bits, because a 64-partition grouping rounds differently from index order - it is *reproducible*, not more accurate, exactly the exclusion named below.) The cost of the fix is the serial fold over the partition count - a few dozen values against the parallel work it guards, a rounding error on the [§31](31_disjoint_writes_parallelize.md) speedup. This is a single-machine reproduction; cross-machine numbers are pending, but the divergence and the two fixes are machine-independent facts (IEEE-754 non-associativity and integer associativity), not measurements that vary by box.

The simulator gives the complementary evidence: `forage` parallelised across one to eight threads is bit-identical to serial (measured, `forage_scaling`), precisely because it is a per-element map with *no* reduction across targets - the safe case. Add a global energy sum each tick (exercise 2) and you are in the trap.

## Exercises

1. **Make it diverge.** Sum a float column of one million values in parallel at 1, 2, 4, and 8 threads. Hash each result. Show the hashes differ, and that they are each *stable* on repeated runs at a fixed thread count. The bug is real and it is reproducible per core count.
2. **Compound it.** Feed that reduction into the simulator (say, a global energy normalisation each tick). Run 1 000 ticks at two different thread counts from the same seed. Hash the worlds. Watch a last-bit difference become a different population.
3. **Fix the order.** Reduce per partition into a fixed-id slot, then fold the slots serially in id order. Re-run exercise 1: the hashes are now identical across all thread counts. Time the serial fold and show it is negligible against the parallel work.
4. **Accumulate in integers.** Scale the energies to fixed-point `i64`, sum exactly, scale back. Show the result is identical across thread counts *and* across summation orders. Find the scale where overflow begins, and the scale where precision loss begins.
5. **Replay across core counts.** Replay one committed log ([§46](46_log_survives_power_loss.md)) at two thread counts. Bit-identical world only with a fixed-order or integer reduction. This is the [§37](37_log_is_world.md) distribution claim, made true on heterogeneous hardware.
6. **The canary test.** Write a CI check that runs the simulator at two core counts and asserts equal world hashes. It fails today; make it pass; keep it - it is the regression guard that catches the next racy reduction before a server does.
7. *(stretch)* **The same bug on a vector unit.** Replace the threaded reduction with a SIMD horizontal sum. Show it reorders and diverges just as a thread pool does, and that the fixed-order and integer fixes both still apply. Note what this implies for the GPU chapter ahead.

Reference notes in [48_reductions_dont_parallelize_freely_solutions.md](48_reductions_dont_parallelize_freely_solutions.md).

## What's next

Three of the four unattended questions are answered: the system survives the stop ([§46](46_log_survives_power_loss.md)), reports what it is doing ([§47](47_observation_is_a_system.md)), and gives the same answer on every machine. The last one is the hardest deadline of all. [§49](49_worst_case_is_the_only_case.md) takes on *hard real-time* - where a missed deadline is not a dropped frame but a fault - and marks the line between the soft budgets the trunk taught ([§4](04_cost_and_budget.md), [§39](39_system_of_systems.md)) and the worst-case-execution-time discipline a control loop demands.
