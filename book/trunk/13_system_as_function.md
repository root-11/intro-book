# 13 - A system is a function over tables

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 13](../../concepts/glossary.md#13--a-system-is-a-function-over-tables).*

<p align="center"><img src="../illustrations/differential_equations.jpg" alt="A mouse at the chalkboard - systems are functions of state" style="max-height: 300px; max-width: 100%;"></p>

A *system* is a function that reads from one or more tables and writes to one or more tables. It declares its inputs (the *read-set*) and its outputs (the *write-set*). It has no hidden state, no global side effects, no interaction with the outside world during a tick. The signature is the contract.

```rust,no_run
fn motion(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], dt: f32) {
    for i in 0..px.len() {
        px[i] += vx[i] * dt;
        py[i] += vy[i] * dt;
    }
}
```

Read-set: `vx`, `vy`, `dt`. Write-set: `px`, `py`. That is the entire contract. This system can run any time the velocity columns and `dt` are available and nothing else is writing the position columns.

Every system takes one of three shapes.

An **operation** is 1→1: every input row produces exactly one output row. `motion` is an operation: each creature's position is updated to its new position. Most update functions are operations.

A **filter** is 1→{0, 1}: every input row produces zero or one output rows. `apply_starve` (from `code/sim/SPEC.md`) is a filter: each creature with energy ≤ 0 produces an entry in `to_remove`; creatures with energy > 0 produce nothing.

An **emission** is 1→N: every input row produces zero or more output rows. `apply_reproduce` is an emission: a parent above the energy threshold produces two offspring (a 1→2 emission).

These three shapes are the same shapes a database query takes. `SELECT * FROM t WHERE p` is a filter, `SELECT a + b FROM t` is an operation, `SELECT explode(arr) FROM t` is an emission. A system is a database operation written in Rust against `Vec`s instead of SQL against tables.

The contract that the system has *no hidden state* is what makes systems compose. Two systems with disjoint write-sets can run in parallel without coordination ([§31](31_disjoint_writes_parallelize.md)). Two systems whose read-set and write-set form a chain must run in order ([§14](14_systems_compose_into_a_dag.md)). The contract is the basis for all of this.

Even *observability* is a system. A debug inspector is a system whose read-set is "all tables" and whose write-set is "nothing". It runs alongside the others, gathers data for inspection, and produces no side effects on the world. In production it is *absent*, not gated by a flag - the binary simply does not contain it.

A few patterns to watch for. A function that reads a table, writes to it, and reads it again in the same call is *not* a system - it has implicit ordering inside the body. Either split it into two systems with explicit ordering, or buffer the writes until the function exits. A function that takes `&mut World` and mutates whatever it likes is *not* a system - it has no declared write-set, and you cannot reason about it from its signature.

A system declares its inputs, declares its outputs, and does no more. That is the shape that lets every other discipline in the book work.

## Exercises

Use the deck from §5 or the §0 simulator skeleton; either provides enough tables.

1. **Identify the shape.** Classify each as operation, filter, or emission:
   - Squaring every entry in a `Vec<f32>`.
   - Filtering even integers from a `Vec<u32>`.
   - Splitting each string in `Vec<String>` into words, returning all words.
   - Computing the sum of a `Vec<u32>`.
2. **Write motion as a system.** With position columns `px, py: Vec<f32>` and velocity columns `vx, vy: Vec<f32>`, write `fn motion(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], dt: f32)`. Apply it to 100 creatures with random initial positions and velocities. Print the position of one creature across 10 ticks.
3. **Declare the contract.** Add doc comments to `motion` listing its read-set and write-set explicitly. The signature plus the doc comment is the system's contract.
4. **Write a filter.** With `energy: &[f32]`, write `fn starving(energy: &[f32]) -> Vec<usize>` returning the indices where `energy[i] <= 0`. This is the read-only first half of `apply_starve`.
5. **Write an emission.** With `parent_energy: &[f32]`, threshold `threshold: f32`, write `fn reproduce(parent_energy: &[f32], threshold: f32) -> Vec<(usize, f32)>` returning, for each parent above threshold, two `(parent_index, offspring_energy)` entries. This is a 1→2 emission.
6. **Observe non-systems.** Find a function in your previous work (or any tutorial) that mutates global state, writes to stdout in its body, or takes `&mut World`. Note what makes it not a system.
7. *(stretch)* **A test as a system.** Write `fn no_creature_moved_too_far(prev_px: &[f32], prev_py: &[f32], cur_px: &[f32], cur_py: &[f32]) -> Vec<usize>`, returning indices where the move was implausibly large. The "test" is just an inspection system reading the world.

Reference notes in [13_system_as_function_solutions.md](13_system_as_function_solutions.md).

## What's next

[§14 - Systems compose into a DAG](14_systems_compose_into_a_dag.md) takes the next step: when many systems run together, how do they fit?
