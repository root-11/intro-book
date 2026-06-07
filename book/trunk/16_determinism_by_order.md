# 16 - Determinism by order

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 16](../../concepts/glossary.md#16--determinism-by-order).*

<p align="center"><img src="../illustrations/monte_carlo.jpg" alt="Monte Carlo estimate of π - same seed, same answer, every run" style="max-height: 300px; max-width: 100%;"></p>

A program is *deterministic* if the same inputs and the same execution produce the same outputs, every time. Sounds obvious. It is not - most modern programs are *not* deterministic by default. Threads run in OS-scheduled order. Hash maps may iterate in randomised order. The system clock differs by run. `rand::thread_rng()` differs by process.

In an ECS architecture, determinism is structural. Same world state at tick start + same system order + same inputs (events, RNG seed) = same world state at tick end. Bit-identical. Every time.

This is not a quality goal; it is a precondition for almost everything the book builds on:

- **Replay.** The world is the log decoded ([§37](37_log_is_world.md)). Replay reconstructs world state by re-running the inputs through the same system sequence. Without determinism, replay is impossible.
- **Testing.** A property test fixes an RNG seed and asserts the simulator behaves identically across runs. Without determinism, every test is flaky.
- **Distributed simulation.** Multiple machines run identical copies of the world. Without determinism, they drift apart by tick 1.
- **Debugging.** A bug at tick 4783 should appear at tick 4783 every run. Without determinism, debugging real-time bugs becomes guesswork.

The recipe for determinism is simple: forbid every source of non-determinism in the inner systems.

- **No `HashMap` in iteration order.** Use `Vec` or `BTreeMap`, which iterate in deterministic order.
- **No system clock.** Get time from input events, not from `Instant::now()`. Time is a value passed *into* the system, not read from the OS.
- **One RNG, seeded.** A single `Rng` with a fixed seed, used in a defined order. Each system that needs randomness reads from it in DAG order.
- **No threads inside a system.** A system runs single-threaded internally. Parallelism happens *between* systems with disjoint write-sets ([§31](31_disjoint_writes_parallelize.md)), not inside one system.
- **Buffered mutations.** [§15](15_state_changes_between_ticks.md)'s rule: mutations apply at tick boundaries, not mid-tick.

These rules are restrictive. They are also the price of every benefit listed above. Most modern programs decline to pay this price and accept the costs - flaky tests, unreproducible bugs, divergent distributed simulation. The book pays the price.

The cost of determinism is not absolute. *Within* a system, the implementation is free to use whatever it likes - SIMD intrinsics, branch hints, compile-time tricks - as long as the inputs and outputs are bit-identical to what the abstract specification demands. The discipline is at the system boundary: between systems, everything must be reproducible.

> [!NOTE]
> **Parallel reductions are the exception.** "Bit-identical" is easy to lose the moment a reduction runs in parallel. Floating-point addition is not associative: `(a + b) + c` is not always `a + (b + c)` in the last bits. A parallel sum that splits the data across threads and merges the partials adds in a different grouping than a serial sum, so the *same seed with a different thread count* can produce different bits. That is the canary: if your world hashes diverge only when you change core count, suspect a parallel floating-point reduction. Two escape hatches keep determinism. Fix the reduction order: always fold the partials in a defined sequence, independent of how many threads produced them. Or accumulate in integers: scale to fixed-point, sum exactly, scale back. Integer accumulation is exact by construction; fixed-order floating-point is reproducible but still rounds. This is the determinism gotcha behind the replay-across-heterogeneous-hardware claim above - drop it and "same seed, same answer" quietly stops being true across machines with different core counts.

A test for determinism is concrete. Run the simulator twice with the same seed, the same input event log, the same system order. After 1 000 ticks, hash the entire world state. If the hashes match, you are deterministic. If they do not, find the system whose output first differs, and trace the source of variability. Often: a `HashMap`, a system clock, a thread.

A simulator that is deterministic is also a simulator that *can be tested*. Once that property holds, every other quality goal - performance, parallelism, distribution - becomes safe to optimise toward. Without determinism, every optimisation is a coin flip.

The full payoff of determinism arrives at the *save and load* phase named in [§11](11_the_tick.md). The simulator can be paused, its tables serialised to disk, reloaded later, and resumed - and the result must be indistinguishable from a run that never paused. The mechanics arrive in [§36 - Persistence is table serialization](36_persistence_is_serialization.md): a snapshot is the world's tables written as a stream of `(entity, key, value)` triples - the same shape they have in memory. Combined with the input event log, replay is structural - read the snapshot, replay events through the same DAG with the same seed, you reconstruct the world at any later tick exactly. Determinism (this section), serialization ([§36](36_persistence_is_serialization.md)), and log-as-world ([§37](37_log_is_world.md)) are the three legs of replay.

## Exercises

1. **Hash the world.** Write a function that takes the simulator state and produces a `u64` hash by feeding every column through `std::hash::Hasher`. Use this to compare world states across runs.
2. **Two identical runs.** Run the simulator twice with the same RNG seed. Hash the world at tick 100. Are they equal?
3. **Introduce non-determinism deliberately.** Replace your seeded RNG with `rand::thread_rng()` (or wallclock-seeded). Run twice. Show the hashes differ.
4. **Find the culprit.** Suppose your hashes differ. Hash the world after each system in the DAG. Identify which system's output first differs, and what source of non-determinism it pulls from.
5. **`HashMap` in iteration order.** Build a `HashMap<u32, f32>` of 10 entries and iterate it twice within one program. Print the order each time. Are they the same? Try with `BTreeMap`. Try across two runs of the same program.
6. **Time as input.** Refactor a system that uses `Instant::now()` to instead take `current_time: f64` as a parameter. The system is now deterministic; the source of `current_time` is the only place non-determinism can enter.
7. *(stretch)* **A property test.** Hand-roll a simple property test: generate 100 random seeds. For each, run the simulator for 100 ticks. Hash the resulting world. Verify that the same seed always produces the same hash, and that different seeds usually produce different hashes.

Reference notes in [16_determinism_by_order_solutions.md](16_determinism_by_order_solutions.md).

## What's next

You have closed Time & passes. The next phase is *Existence-based processing*, starting with [§17 - Presence replaces flags](17_presence_replaces_flags.md). The simulator's hunger and starvation systems are about to lose their booleans.
