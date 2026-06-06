# 28 - Proximity is a property of position

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 28](../../concepts/glossary.md#28--proximity-is-a-property-of-position).*

<p align="center"><img src="../illustrations/optimization.jpg" alt="Optimization: minimize f(x) - proximity is a function of position, computed where position lives" style="max-height: 300px; max-width: 100%;"></p>

Creatures eat the food they encounter. So `next_event` has to answer, for every creature, *which food is within reach?* At §1's ten thousand that is a cheap scan. At §2's million it is a wall: comparing every creature to every food is O(C×F). Measured, even twenty thousand all-pairs neighbour tests cost ~270 ms - one frame's entire budget spent on a fraction of the world.

The reflex is to reach for a *spatial index*: a quadtree, a grid hash, a structure that lives beside the world, that you insert into and delete from as things move, that you query. It works. But stop and look at what it is: a second copy of information the world already holds - position - with its own maintenance budget, its own allocations, and pointer-chased buckets that miss cache on every hop.

Step back and ask what proximity *is*. It is a function of position. And position is already owned and streamed, every tick, by the motion system. The cell a creature falls in is one line - `cell = f(px, py)` - computed in the pass motion is already making, branchless, SIMD-friendly. The index was never necessary. The cell is a property you read off position.

**Bin, don't index.** Compute each creature's cell, then place the creatures into per-cell buckets with a counting sort: histogram the cells, prefix-sum into offsets, scatter the indices into one dense array. No `HashMap`, no per-cell allocation, no pointer-chasing - three linear passes over contiguous memory. A neighbour query reads the 3x3 block of cells around a point as contiguous ranges.

```rust,no_run
// cell id per creature - the SIMD-friendly byproduct of the position stream
let cell: Vec<u32> = (0..n).map(|i| cell_of(px[i], py[i])).collect();

// counting sort into a dense CSR bucket array: offsets[c]..offsets[c+1]
let mut offsets = vec![0u32; ncells + 1];
for &c in &cell { offsets[c as usize + 1] += 1; }
for c in 0..ncells { offsets[c + 1] += offsets[c]; }
let mut items = vec![0u32; n];
let mut cursor = offsets.clone();
for i in 0..n { let c = cell[i] as usize; items[cursor[c] as usize] = i as u32; cursor[c] += 1; }
```

**Measured** (`proximity`, 1M creatures): the dense bin answers the neighbour query in ~520 ms against the bolt-on `HashMap`'s ~1470 ms - about 2.8x - and its *build* is ~8x cheaper (3.7 ms vs 31 ms). The hash spends its time allocating buckets and chasing them; the dense bin streams.

The sharpest number is the build itself. Rebuilding the *entire* spatial structure from scratch costs 3.7 ms - **0.7% of the query it serves.** So the whole reason a bolt-on index exists, "don't pay to rebuild," is optimising under one percent of the work. You do not maintain proximity across ticks. You recompute it from the position stream each tick, for free, in the pass motion already makes. *Recompute from the stream* beats *maintain a structure*, and the old question of how often to re-sort the world simply evaporates: there is no kept structure to schedule.

**The gather still scatters, and that is [§26](26_subscription_tables.md)'s job.** Binning finds the *candidates* cheaply, but reading their positions jumps around the columns. Making that gather dense is the compaction from [§24](24_append_only_and_recycling.md)/[§26](26_subscription_tables.md): the same batch pass that reclaims dead slots can reorder the survivors *by cell* (a Z-order curve keeps neighbouring cells adjacent in memory), so a cell's creatures land on adjacent cache lines. That reorder is the GC's slow-cadence pass, not a separate spatial sort with its own knob. §28 says *which cell*; §26 makes *reading the cell* stream.

```rust,no_run
fn cell_of(px: f32, py: f32, cell_size: f32) -> u32 {
    let x = (px / cell_size).floor() as i32;
    let y = (py / cell_size).floor() as i32;
    // simple stripe pack; a Z-order (Morton) hash keeps 2D neighbours close in 1D
    ((x as u32 & 0xFFFF) << 16) | (y as u32 & 0xFFFF)
}
```

**The same lesson at the global scale: the pack-leader.** Swarming beasts look coordinated, but if every beast accounts for every other - cohesion, alignment, separation against all N - the cost is O(N²) (~240 ms at twenty thousand). The way the old games did it: put an abstract, invisible leader at the centre of the pack. The leader does the one expensive thing, deciding where the pack goes; each beast subscribes to the leader and steers relative to it. One centroid pass, every member reads one value: O(N), ~0.03 ms at the same twenty thousand - four orders of magnitude, and the gap grows with N. Lifelike swarm behaviour, no all-pairs accounting. The "who is near the group" question, like "who is near me," is answered by a single pass over position, not by a structure every agent maintains.

The meta-lesson is the one worth keeping. Twice now the cheap path was to refuse the obvious data structure - the `id_to_slot` hop in [§26](26_subscription_tables.md), the spatial index here - and instead let the system that already owns the data produce the answer in the pass it already makes. Ask what the problem *is* before reaching for a structure to make it fit. Proximity is position; position is already in hand.

## Exercises

1. **The all-pairs wall.** For N agents in a box, count neighbours within radius `r` by testing every pair. Time it at N = 1K, 10K, 20K. Confirm the O(N²) curve, and that 20K alone already exceeds a 30 Hz frame budget.
2. **Cell as a derived column.** Write `fn cell_of(px, py, cell_size) -> u32`. Compute the `cell` column for a 1M-creature world in the same loop that would update position. Note that it adds one cheap arithmetic op per creature to a pass you are already making.
3. **Dense binning.** Build the CSR bucket array (count, prefix-sum, scatter). Answer "neighbours within `r`" by reading the 3x3 cell block. Verify it gives the same counts as exercise 1.
4. **Bolt-on hash vs dense bin.** Build the same query with a `HashMap<cell, Vec<u32>>`. At 1M, time build and query for both. Reproduce the ~2.8x end-to-end gap; note where the hash spends its time (allocation, pointer-chasing) against the dense bin's contiguous streams.
5. **Recompute beats maintain.** Measure the dense bin's build as a fraction of its query. Confirm it is roughly 1%. Argue why maintaining a spatial index incrementally (to "save" the rebuild) optimises the wrong thing.
6. **The pack-leader.** Steer N agents toward the group two ways: each averaging the other N-1 positions (all-pairs), and each reading one centroid computed in a single pass. Time both; reproduce the O(N²) vs O(N) gap. Argue why the leader gives swarm-like behaviour without any agent knowing about any other.
7. *(stretch)* **Z-order and the compaction.** Replace the stripe pack with a Z-order (Morton) hash. Then order the [§24](24_append_only_and_recycling.md) compaction by cell and re-time the neighbour query's gather (§26). How much of the remaining query cost was the scattered gather?

Reference notes in [28_proximity_solutions.md](28_proximity_solutions.md).

## What's next

[§29 - The wall at 10K → 1M](29_wall_10k_to_1m.md) is where these techniques start to bind. Code that ran fine at 10K stops running fine at 1M; the chapter is about finding out where and why.
