# 22 — Mutations buffer; cleanup is batched

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 22](../../concepts/glossary.md#22--mutations-buffer-cleanup-is-batched).*

<p align="center"><img src="../illustrations/engineer_fuel.jpg" alt="Engineer-fuel coffee, mouse soldering — work buffered on the bench, applied in a batch" style="max-height: 300px; max-width: 100%;"></p>

This rule has been forward-referenced through ten chapters. Time to make it concrete.

Mutations during a tick do not apply immediately; they queue, and a single cleanup pass applies them all at the tick boundary. The shape:

```rust,no_run
struct CleanupBuffer {
    to_remove: Vec<u32>,           // creature ids to delete
    to_insert: Vec<CreatureRow>,   // new creature rows to add
}
```

During the tick, every system that wants to delete pushes to `to_remove`. Every system that wants to add pushes to `to_insert`. No system mutates the live tables.

At the end of the tick, one system runs:

```rust,no_run
fn cleanup(world: &mut World, buffer: &mut CleanupBuffer) {
    // 1. Delete all queued removals via swap_remove.
    for &id in &buffer.to_remove {
        let slot = world.id_to_slot[id as usize] as usize;
        for col in world.columns_mut() {
            col.swap_remove(slot);
        }
        // (Update id_to_slot — covered in §23.)
    }
    buffer.to_remove.clear();

    // 2. Append all queued inserts.
    for row in buffer.to_insert.drain(..) {
        world.append(row);
    }
}
```

Two passes, both bulk operations. The world is in a fully consistent state at the end.

**What this fixes.**

The iteration-corruption problem from [§21](21_swap_remove.md) goes away because swap_remove never runs while any system is iterating. By the time cleanup runs, every system has finished. There is no concurrent iteration to confuse.

The race-condition problem from concurrent mutation goes away. Two systems may both want to remove a creature; both push to `to_remove`; cleanup deduplicates (or is idempotent in the rare case of double-removal). Neither system needs to coordinate.

The composition problem from [§14](14_systems_compose_into_a_dag.md) goes away. Systems read consistent snapshots; they read the world *as it was at tick start*, not the world *as some other system half-rewrote it*.

**What it costs.**

Every mutation is one extra row pushed to a side table. For a simulator with 1 000 deaths and 500 reproductions per tick, that is 1 500 rows of bookkeeping — a few thousand bytes, completely negligible against the cost of running the systems themselves.

The cleanup pass is one additional system in the DAG. It is empty (no work) when no mutations are queued; it iterates `to_remove` and `to_insert` when there are. The system is wired in once and never removed.

**What it does not fix.**

Two systems may both push the *same* id to `to_remove` if they independently detect the same death condition. Cleanup either deduplicates (a small set check) or is robust to double-removal (a `swap_remove` on a slot whose id is no longer there is a no-op if you check). Most simulators dedupe via a small `HashSet` at cleanup time.

The order of removals vs insertions inside cleanup matters: deletions first, then insertions. If you insert first, an inserted row might land in a slot you are about to delete. Deleting first frees up slots that subsequent inserts can reuse — though slot recycling is its own decision ([§24](24_append_only_and_recycling.md)).

The pattern itself is universal. Database transactions buffer writes and commit at the boundary. Graphics pipelines render to a back buffer and swap. Version-controlled file systems collect changes and commit. They all solve the same problem: how do you let many independent operations modify shared state without stepping on each other? The answer is always the same — accumulate, then apply atomically.

## Exercises

1. **Implement the side tables.** Add `to_remove: Vec<u32>` and `to_insert: Vec<CreatureRow>` to your simulator's world struct. They are empty at the start of every tick.
2. **Push from `apply_starve`.** Modify your starvation system to push to `to_remove` instead of calling `swap_remove`. Verify the system no longer mutates `creatures`.
3. **Push from `apply_reproduce`.** Modify reproduction to push offspring rows to `to_insert`. Verify reproduction no longer mutates `creatures`.
4. **Implement cleanup.** Write the cleanup system. Apply removals first, then insertions. Run a tick with both kinds of mutations; verify the world is consistent after.
5. **The dedup question.** Two systems push id 42 to `to_remove`. Run cleanup naively (no dedup). What happens? Now add a small dedup pass at cleanup. Does the result change?
6. **Tick-delayed visibility.** A creature inserted in tick 5 (via `to_insert`) does not appear in `creatures` during tick 5's systems — only at the end, in cleanup. Verify by adding an `age_in_ticks` column that increments at the end of each tick; the new creature's value starts at 0 in tick 6, not tick 5.
7. *(stretch)* **A graphics pipeline analogy.** A rendering pipeline draws to a "back buffer" while the "front buffer" is being displayed. At the boundary of one frame to the next, the buffers swap. Argue why this is the same pattern as `to_remove` / `to_insert` plus `cleanup`.

Reference notes in [22_mutations_buffer_solutions.md](22_mutations_buffer_solutions.md).

## What's next

[§23 — Index maps](23_index_maps.md) is the missing piece for swap_remove to be useful: a parallel data structure that tracks where every id currently lives.
