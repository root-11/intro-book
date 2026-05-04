# 34 — Order is the contract

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 34](../../concepts/glossary.md#34--order-is-the-contract).*

<p align="center"><img src="../illustrations/monte_carlo.jpg" alt="Monte Carlo simulation — reproducibility is the contract under concurrency" style="max-height: 300px; max-width: 100%;"></p>

§31, §32, and §33 unlocked parallelism. The natural temptation is to run *everything* in parallel — let the OS scheduler decide which system runs when, fan systems out across all available cores, push throughput up. This is wrong.

The system DAG ([§14](14_systems_compose_into_a_dag.md)) is the *contract* for the simulator's behaviour. Two systems with overlapping write-sets must run in a defined order. Two systems on the same DAG level may run in parallel — but they must both *complete* before any system that reads their outputs begins. Parallelism is allowed inside a step; it is never allowed across steps.

The reason is determinism ([§16](16_determinism_by_order.md)). Same inputs + same system order = same outputs. If `apply_eat`, `apply_reproduce`, and `apply_starve` run in undefined order — say, the first one to finish gets to write `to_remove` first — then `cleanup` sees a different `to_remove` ordering on different runs, and the world state at the end of the tick is non-reproducible. Replay breaks. Tests become flaky. Distributed simulation drifts apart.

The schedule looks like:

```text
                  ┌── apply_eat ──┐
                  │               │
   next_event ────┼── apply_repro ┼─→ cleanup → inspect
                  │               │
                  └── apply_starve┘
```

`next_event` runs first (its writes are needed by all three appliers). The three appliers run in parallel — their writes are disjoint (or partitioned into thread-local segments, [§31](31_disjoint_writes_parallelize.md)). `cleanup` runs after all three finish, never before any of them. `inspect` runs last.

The schedule is fixed by the DAG. Parallelism happens *within* the structure the DAG permits, not around it.

Two specific anti-patterns to avoid:

**The "let the OS decide" anti-pattern.** Spawning every system as a thread and letting them race is fast in the wrong way. Some runs will produce one result; some will produce another. The bug is intermittent, the cause is hard to find, and "fixing" it with locks reintroduces the costs §31-§33 worked to avoid.

**The "early start" anti-pattern.** Starting a system before its prerequisites have finished — even if the data "looks ready" — is a bet that the schedule will not change. The bet often pays off in practice, until the day a buffer fills slightly later than usual and the world's state shifts in ways no test caught. Wait for the explicit completion of every prerequisite.

The discipline is enforced by a *scheduler*. A scheduler executes systems in topological order, parallelising those at the same DAG level, joining at every level boundary. Most production ECS engines (Bevy's `World::run_schedule`, Unity DOTS's `JobHandle.Complete`) implement exactly this. The pattern is the same as a parallel `make`: build dependencies in order, build independents in parallel, never start before prerequisites have finished.

A useful test: *can you replay a tick to bit-identical output?* If yes, your scheduler respects the contract. If no, it does not — somewhere a system runs in undefined order, and the bug will surface in the worst possible debugging window.

This rule closes Concurrency. The simulator can now use every core on the machine without sacrificing the determinism that §16 guaranteed. The DAG is both the parallel schedule *and* the deterministic execution order; one document, two readings.

## Exercises

1. **Build the schedule.** Write a `tick()` that runs `next_event`, then a parallel block of the three appliers (using `thread::scope` plus per-thread `to_remove` segments), then `cleanup`, then `inspect`. Verify the boundaries: `cleanup` must not start before all three appliers complete.
2. **Test for determinism.** Run the simulator twice with the same seed. Hash the world after 100 ticks. The hashes must be identical even though the appliers ran in parallel.
3. **Break the contract.** Construct a schedule where `cleanup` starts before `apply_starve` finishes (e.g. via an `unsafe` shared buffer). Run twice. Hashes should differ — sometimes. The bug's intermittency is the lesson.
4. **Find your level boundaries.** Sketch your simulator's full DAG. Identify each *level* (set of systems with no transitive dependency on each other). Each level is a parallel batch; each boundary is a sync.
5. *(stretch)* **A minimal scheduler.** Write a function that takes a list of `(name, read_set, write_set)` and produces a `Vec<Vec<&str>>` — the systems grouped by level. The scheduler is just a topological sort plus level grouping. Around 50 lines of Rust.

Reference notes in [34_order_is_the_contract_solutions.md](34_order_is_the_contract_solutions.md).

## What's next

You have closed Concurrency. The simulator now runs on multiple cores without losing determinism. The next phase is *I/O & persistence*, starting with [§35 — The boundary is the queue](35_boundary_is_the_queue.md). The simulator is about to begin talking to the world outside its tick.
