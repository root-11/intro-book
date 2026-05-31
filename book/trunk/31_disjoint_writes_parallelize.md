# 31 - Disjoint write-sets parallelize freely

<p align="center"><img src="../covers/phase_concurrency.jpg" alt="Concurrency phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 31](../../concepts/glossary.md#31--disjoint-write-sets-parallelize-freely).*

Two systems can run in parallel if and only if their write-sets do not overlap. That is the rule. It is small. It is what node 25's single-writer ownership buys you.

Concretely: in the simulator's tick, `motion` writes `creature.pos` and `creature.energy`; `food_spawn` writes `food`. Their write-sets are disjoint. They can run on two different threads with no coordination - no locks, no atomics, no message-passing. The data layout makes the parallelism free.

```rust,no_run
use std::thread;

thread::scope(|s| {
    s.spawn(|| motion(&mut hot.pos, &hot.vel, &mut hot.energy, dt));
    s.spawn(|| food_spawn(&food_spawner, &mut food));
});
// both threads have completed before scope() returns
```

`std::thread::scope` is the Rust idiom that proves at compile time the two threads finish before the surrounding state is touched. The borrow checker enforces the disjoint-writes rule: if you tried to spawn two threads each holding `&mut hot.pos`, the code would not compile.

The same shape works at finer grain. The simulator's three appliers (`apply_eat`, `apply_reproduce`, `apply_starve`) all read `pending_event` and write disjoint things - `apply_eat` writes `food`, `to_remove`; `apply_reproduce` writes `to_insert`; `apply_starve` writes `to_remove`. Two of the three write the same table (`to_remove`). To parallelise them, give each its own *segment* of `to_remove` (one per thread), then merge at cleanup. The merge is `Vec::extend_from_slice` or equivalent - O(N) in the merged total, free relative to the work that produced it.

Three things this rule does for you:

**No locks.** A lock is a tax paid by every reader and writer of the locked thing. With single-writer ownership, locks are unnecessary; with disjoint write-sets, they remain unnecessary at the parallel boundary. The simulator at this scale has zero `Mutex`, zero `RwLock`, zero `Atomic*` in its inner systems.

**Speedup is structural, not promised.** N threads with disjoint work give N× speedup, modulo memory-bandwidth limits. That ceiling is real - at 50 GB/s of DDR5 bandwidth, eight threads cannot all do bandwidth-bound work in parallel; one thread saturates the bus. But for compute-bound work or for cache-resident loops, the speedup is close to N.

**Tools without ceremony.** The Rust ecosystem's standard parallelism crate is `rayon`, which provides `par_iter` and `par_chunks_mut` for parallel iteration. With disjoint writes by construction, `rayon::join` and `par_iter_mut` work without changing the simulator's design - they are conveniences over `std::thread::scope`, not new architectures.

The single-writer rule (§25) was the precondition. Disjoint write-sets is the rule applied across systems. Together, parallelism becomes a scheduling decision, not a design decision.

## Exercises

You will need a multi-core machine. Most desktops and laptops qualify.

1. **Two parallel systems.** Wrap `motion` and `food_spawn` in `std::thread::scope`. Run a tick. Verify both completed and the world state is the expected combination.
2. **Time the speedup.** Run the same two systems serially. Run them in parallel via `thread::scope`. Compare. Speedup should be close to 2× when both systems are individually expensive; less if one dominates.
3. **A failing case.** Try to run `motion` and `apply_eat` in parallel. Both write `creature.energy`. Rust's borrow checker rejects the code. Note the error message - that is the architecture being enforced by the compiler.
4. **`rayon::join`.** Replace `thread::scope` with `rayon::join((|| motion(...), || food_spawn(...)))`. Confirm the same behaviour. Adding rayon to `Cargo.toml` is a §42 dependency-pricing decision in miniature: read what the crate gives you, decide consciously.
5. **Per-thread segments.** Split `to_remove` into 8 thread-local `Vec<u32>`s. Run 8 threads of `apply_starve`, each producing its own segment. Merge at the end. Verify the merge produces the same result as a single-threaded run.
6. *(stretch)* **Find the bandwidth ceiling.** Time motion at 1, 2, 4, 8 threads on a 1M-creature world. Plot speedup vs thread count. The plot is roughly linear up to the memory-bandwidth limit, then flat.

Reference notes in [31_disjoint_writes_parallelize_solutions.md](31_disjoint_writes_parallelize_solutions.md).

## What's next

[§32 - Partition, don't lock](32_partition_dont_lock.md) takes the next step: when one system *must* write a single table from multiple threads, you split the table, not the access.
