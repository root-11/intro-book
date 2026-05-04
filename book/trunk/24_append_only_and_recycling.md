# 24 — Append-only and recycling

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 24](../../concepts/glossary.md#24--append-only-and-recycling).*

When a row is removed from a table, its slot is freed. There are two strategies for what happens to that slot.

**Append-only.** Old slots stay valid forever. The table grows monotonically. New rows always go to the end.

**Recycling.** Freed slots are reused. The table's length stays bounded. New rows go into freed slots before the table grows.

Each is correct; they have very different access patterns and costs.

**Append-only.** Use when:

- *History matters.* The simulator's `eaten`, `born`, `dead` logs from `code/sim/SPEC.md` are all append-only — they record what happened. Removed entries would be lost history.
- *Old references must remain valid forever.* Some pointer-into-table designs assume the table never shrinks.
- *Total volume is bounded by elapsed time, not by population.* A 30-second 30 Hz simulation produces at most 900 frames; an append-only frame log is at most 900 rows. No need to recycle.

The cost is monotonic memory growth. A long-running simulator with append-only `eaten` accumulates millions of rows over hours. Mitigations:

1. Periodic snapshot + truncate (the log is replaced by a recent slice).
2. Tiered storage — recent in memory, older streamed to disk ([§30](30_streaming_wall.md)).
3. Just accept the memory, if the run is short.

**Recycling.** Use when:

- *Steady-state size is small even though total inserted is large.* The simulator's `creatures` table at 100 000 alive with 100 000 deaths and 100 000 births per second — net flow zero, but total ever issued grows linearly. Recycling keeps memory bounded.
- *Memory matters.* Recycling caps the table at the high-water mark of live rows.

The cost is reference-stability complications. A new row in a recycled slot has the same slot as a previous, removed row. Code holding an old slot reference would silently dereference the new row. The fix is generational ids: each slot has a generation counter that increments on every recycle. References hold `(id, gen)`; dereference checks the generation. A stale reference fails its check.

A slot allocator looks like:

```rust,no_run
struct SlotPool {
    free_slots: Vec<u32>,  // freed slots awaiting reuse
    next_slot:  u32,       // high-water mark; the next never-used slot
    gen:        Vec<u32>,  // generation per slot
}

impl SlotPool {
    fn allocate(&mut self) -> (u32, u32) {
        let slot = self.free_slots.pop().unwrap_or_else(|| {
            let s = self.next_slot;
            self.next_slot += 1;
            self.gen.push(0);
            s
        });
        let g = self.gen[slot as usize];
        (slot, g)
    }

    fn free(&mut self, slot: u32) {
        self.gen[slot as usize] += 1;
        self.free_slots.push(slot);
    }
}
```

`allocate` pops a freed slot if any are available, otherwise grows. `free` bumps the generation and adds the slot back to the free list. Stale references (with the *old* generation) cannot dereference the recycled row.

**Choosing between them.** Match the strategy to the table's role:

| table              | strategy   | reason                           |
|--------------------|------------|----------------------------------|
| `creatures`        | recycling  | bounded population               |
| `eaten`            | append-only | history record                  |
| `born`             | append-only | history record                  |
| `dead`             | append-only | history record                  |
| `pending_event`    | recycling  | rebuilt every tick               |
| `food`             | recycling  | bounded                          |
| `food_spawner`     | constant   | no removals                      |

Mixing strategies in one simulator is normal. The discipline is to be explicit about which table is which, and apply the right machinery to each.

## Exercises

1. **Two append-only logs.** Implement `eaten` and `born` as append-only `Vec`s. After 1 000 ticks, examine the log lengths and verify they grow monotonically.
2. **A recycling pool.** Implement the `SlotPool` above. Allocate 1 000 slots, free 500, allocate 500 more, observe the slot indices. Did the pool reuse the freed slots, or grow?
3. **Stale reference detection.** Allocate a slot with `(slot, gen=0)`. Free it. Allocate a new row in the same slot — its gen is 1. Try to dereference the old `(slot, 0)`. The check fails; the reference is recognised as stale.
4. **Switch creatures to append-only.** Run the simulator with `creatures` as append-only (no recycling). Run for 10 000 ticks with steady birth and death. Plot the table's length over time. It grows monotonically; memory increases without bound.
5. **Switch eaten to recycling.** Run with `eaten` recycled. After 100 ticks, all "what did this creature eat at tick 50" queries fail because the rows were reused. The history is gone.

Reference notes in [24_append_only_and_recycling_solutions.md](24_append_only_and_recycling_solutions.md).

## What's next

[§25 — Ownership of tables](25_ownership_of_tables.md) is the rule that makes every other discipline in the phase work: each table has exactly one writer.
