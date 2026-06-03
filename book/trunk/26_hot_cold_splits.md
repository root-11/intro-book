# 26 - Hot/cold splits

<p align="center"><img src="../covers/phase_scale.jpg" alt="Scale phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 26](../../concepts/glossary.md#26--hot-cold-splits).*

The simulator's `creature` table has six columns: `pos`, `vel`, `energy`, `birth_t`, `id`, `generation`. The motion system reads three of the six (`pos`, `vel`, `energy`). The starvation system reads only `energy`. The cleanup system reads `id` and `generation`. The births log reads `birth_t`. *No system reads all six*.

If the columns are stored together - same memory region, same prefetcher pulls - every load brings in fields the inner loop ignores. At cache-spilling sizes, the ignored fields cost real bandwidth.

The fix is a split: fields touched on the hot path go in one table; fields read rarely go in another. Two tables, same length, same id alignment.

```rust,no_run
struct CreatureHot {
    pos:    Vec<(f32, f32)>,    // motion, next_event, apply_eat
    vel:    Vec<(f32, f32)>,    // motion
    energy: Vec<f32>,            // motion, apply_eat, apply_starve
}

struct CreatureCold {
    birth_t: Vec<f64>,           // logging only
    id:      Vec<u32>,           // cleanup, id_to_slot maintenance
    generation:     Vec<u32>,           // cleanup
}
```

Motion reads only `CreatureHot`. Cleanup reads `CreatureCold`. The two systems' cache traffic does not overlap.

The bandwidth math: pre-split, motion's loop reads ~40 bytes per creature (the full row, prefetcher loads everything together). Post-split, motion reads 20 bytes (just `pos` + `vel` + `energy`). Half the bandwidth, which measured as ~2-2.5× faster wall-clock time at 1M creatures across the four reference machines (`code/README.md`).

The discipline carries cost. Two tables means two id-to-slot maps (or careful sharing of one). Cleanup must update both in lockstep when slots move. The split is a real architectural commitment - once made, every system that touches creatures must know which table it is touching.

When the split is wrong:

- **All-fields workloads.** A debug-inspect system that prints every field reads everything; the split adds overhead without reducing bandwidth.
- **Tiny rows.** If the full row is already 16-24 bytes (one or two fields per cache line), splitting a 4-byte field out adds more pointer traffic than it saves.
- **Frequently rebalancing.** If which fields are "hot" changes from tick to tick, a fixed split becomes unhelpful. Hot/cold is a static decision, made once for a given target workload.

The decision rests on measurement. Profile the simulator at the target size; identify the inner loop's actual touched fields; split accordingly. The split is earned by data, not by aesthetics.

A useful test: name the split *before* writing it. "I am moving `birth_t` into a cold table because no inner loop reads it" is a sound design choice. "I am moving `birth_t` into a cold table because that's how ECS engines do it" is not.

## Exercises

These extend the simulator's `creature` table from §0/§1.

1. **Audit access patterns.** For each system in your simulator, list which fields it reads and which it writes. Fields read every tick are hot; the rest are cold.
2. **Build the split.** Refactor `creature` into `creature_hot` and `creature_cold`. Both share the id allocator. Verify each row's fields stay aligned across the two tables.
3. **Time motion at 1M creatures.** Pre-split: time motion. Post-split: time motion. Compare. The post-split version should be ~2-2.5× faster.
4. **Cleanup must touch both.** Modify cleanup to `swap_remove` from both `creature_hot` and `creature_cold` when a creature dies. Verify alignment after.
5. **A bad split.** Construct a split where the wrong fields go cold (e.g. `energy` in cold). Time motion. The cost of the cache miss on `energy` should bury any savings elsewhere.
6. *(stretch)* **The all-fields case.** Write a system that reads every field (e.g. a serialiser). Time the split version. Discuss why the split's overhead is real here, and why this is a fine tradeoff: most ticks do not run this system.

Reference notes in [26_hot_cold_splits_solutions.md](26_hot_cold_splits_solutions.md).

## What's next

[§27 - Working set vs cache](27_working_set_vs_cache.md) puts numbers on the question this section was implicitly asking: how big *is* the inner loop's footprint, and what cache level does it fit in?
