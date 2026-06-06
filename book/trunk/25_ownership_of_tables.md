# 25 - Ownership of tables

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 25](../../concepts/glossary.md#25--ownership-of-tables).*

<p align="center"><img src="../illustrations/dag_planning_checklist.jpg" alt="One plan, one writer - PLAN, ANALYZE, DESIGN, BUILD, TEST, IMPROVE" style="max-height: 300px; max-width: 100%;"></p>

Every table has exactly one writer.

The rule is small. Its consequences are everything.

> [!NOTE]
> **"Ownership" here means the right to write.** Rust already gives *ownership* a precise meaning: who holds a value and is responsible for dropping it. This chapter uses the word for something narrower and related - which single system is allowed to *mutate* a table, the guarantee Rust's `&mut T` expresses at the type level. The overlap is not an accident. Both senses exist to remove ambiguity about who may change a thing, and that is the weight the word carries here: when exactly one writer can touch a table, its contents at any tick are a function of the inputs alone, not of who reached it first. That is what makes the world deterministic. Read "ownership of a table" as "write-ownership" throughout.

**Why it works.** A row is a tuple ([§6](06_a_row_is_a_tuple.md)) - its fields are aligned by index. A table's columns must be modified together to maintain alignment. A single writer guarantees this: only one place in the code mutates the table, so only one place can violate alignment, so testing one place is enough.

A table with two writers has two places where alignment can be violated. If they run concurrently, alignment is violated nondeterministically. If they run sequentially, the order matters and must be specified. Either way, the cost of getting it right grows superlinearly with the number of writers.

**The disciplines that depend on it.** All of these need single-writer ownership to work:

- **[§31 - Disjoint write-sets parallelize freely](31_disjoint_writes_parallelize.md).** Two systems with disjoint write-sets can run on different threads. The rule guarantees no shared mutation.
- **[§22 - Mutations buffer](22_mutations_buffer.md).** A side-table writer (cleanup) is the *only* writer of `creatures`. All other systems push to `to_remove` and `to_insert`, which they own.
- **[§43 - Tests are systems](43_tests_are_systems.md).** A test system reads everything and writes nothing. The ownership rule is what guarantees its reads see consistent state.
- **The InspectionSystem pattern.** A debug inspector holds read-only references to every table. Read-only access composes with single-writer ownership to make races structurally impossible.

**What the rule looks like in practice.**

```rust,no_run
fn motion(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], dt: f32) { /* writes px, py */ }

fn next_event(px: &[f32], py: &[f32], food: &[Food], pending: &mut [Event]) {
    /* reads px, py, food; writes pending_event */
}

fn apply_eat(pending: &[Event], food: &[Food],
             to_remove: &mut Vec<u32>, energy: &mut [f32]) {
    /* reads pending, food; writes to_remove and energy */
}
```

For each table, exactly one writer is allowed:

- `px`, `py`: written only by `motion`.
- `pending_event`: written only by `next_event`.
- `to_remove`, `to_insert`: written by *many* systems, but each system writes only its own queued mutations; no one reads them until cleanup.
- `creatures`, `food`: written only by `cleanup`, which materialises every other system's queued changes.

Multiple systems may *contribute* to a table by pushing to its side buffer; the actual single writer is cleanup. The architecture preserves the rule even as many systems propose mutations.

**The borrow checker enforces this.** Rust's `&mut [T]` is the type-level expression of single-writer ownership: only one mutable reference can exist at a time. The borrow checker rejects code that violates it. The data-oriented discipline of single-writer-per-table is what Rust's ownership model is *for*; the language enforces what the architecture demands.

**Bugs that arise from violations.** Two systems writing the same column produce inconsistent state. The bug is usually *intermittent* (depends on schedule), *silent* (no error reported, just bad data), and *late-binding* (manifests far from the cause). They are among the hardest bugs in any concurrent system. The single-writer rule eliminates them by construction.

The rule applies recursively. A view table whose entries are derived from another table inherits the ownership rule: a `hungry: Vec<u32>` is owned by the system that classifies hunger; no other system writes to it.

This is the rule that closes Memory & lifecycle. Without it, the buffering, swap_remove, index maps, and slot recycling are all unsafe in any concurrent or parallel context. With it, everything composes.

## Exercises

1. **Identify the writers.** For each table in your simulator (`creatures`, `food`, `food_spawner`, `pending_event`, `eaten`, `born`, `dead`, `hungry`, `to_remove`, `to_insert`), name the *one* system that writes it. If you find a table with two writers, the rule is violated - investigate.
2. **A constructed violation.** Write two systems that both update `creature.energy` directly (not via `to_remove`/`to_insert`). Run them in sequence; observe correct results. Run them in parallel via `std::thread::scope`; either Rust's borrow checker rejects the code, or you observe a race.
3. **Refactor.** For one of the violations from exercise 1 (or 2), introduce a *buffer* table that one system writes and the other reads. The two systems are now writer-disjoint.
4. **Build an InspectionSystem.** Write a system that takes `&` (immutable) references to every table and returns a `WorldSnapshot` struct. Run it after every tick. The system is read-only and never violates the rule.
5. **Borrow checker.** Try to write code where two systems hold `&mut` to the same `Vec`. Rust refuses. Note the exact error message - this is the language enforcing the architecture.
6. *(stretch)* **The cleanup system as canonical writer.** In your simulator, audit: every mutation of `creatures`, `food`, etc. flows through cleanup. Every other system writes only to `to_remove`, `to_insert`, or its own outputs. Verify the audit holds for the simulator end-to-end.

Reference notes in [25_ownership_of_tables_solutions.md](25_ownership_of_tables_solutions.md).

## What's next

You have closed Memory & lifecycle. The simulator's machinery is now complete: it can grow, shrink, recycle, parallelise, and replay. The next phase is *Scale*, starting with [§26 - Hot/cold splits](26_hot_cold_splits.md). The simulator's per-tick cost goes under the microscope.
