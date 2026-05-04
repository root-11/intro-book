# 30 — Moving beyond the wall

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 30](../../concepts/glossary.md#30--the-wall-at-1m--streaming).*

<p align="center"><img src="../illustrations/microcontroller_loop.jpg" alt="Read / process / update under tight resource budget" style="max-height: 300px; max-width: 100%;"></p>

At 100 million creatures with 24 bytes of hot data each, the working set is 2.4 GB. At a billion, 24 GB. Most desktops have 16–64 GB of RAM. The simulator can no longer hold its world *and* its history *and* the OS *and* whatever else *and* operate at speed.

The fix is *streaming*: only the relevant slice of the world is in memory at any one time; the rest lives on disk and is read on demand.

The shape:

```rust,no_run
struct StreamingWorld {
    in_memory: Window,    // a small contiguous range of recent state
    disk:      Archive,   // the rest, append-only on disk
}
```

A *window* of recent state lives in memory, indexed for cheap query. Older state lives on disk in append-only chunks; it is read into the window when a query needs it.

This pattern shows up wherever this scale matters:

- **Time-series databases** (Prometheus, InfluxDB): recent metrics in RAM; older series compressed and disk-resident.
- **Game replay systems**: the last 30 seconds replayable from a memory ring; the full match streamed from a server.
- **Event-sourced systems**: recent state cached; the full event log on disk; replay reconstructs.
- **Database write-ahead logs**: append to log; flush to data files; the data files become disk-resident; recent log + memory hold the active set.

For the simulator, streaming entails three architectural shifts:

**The log is the canonical state.** The world's tables are derivable from the log. If the log is complete and durable, every other in-memory representation is reconstructible. This is the structural framing of [§37 — The log is the world](37_log_is_world.md): the log is not a record of state, it *is* the state.

**Persistence is serialisation of tables.** A snapshot is the world's current SoA, written as a stream of (entity, key, value) triples — the same shape it has in memory. Recovery is reading the triples back. There is no separate domain model; serialisation is *transposition*, not *translation*. This is [§36](36_persistence_is_serialization.md).

**Storage is a cost like any other.** Reading from disk costs bandwidth and IOPS, just as reading from RAM costs cache-line loads. Storage systems with bandwidth (bytes per second) and IOPS (operations per second) limits must be counted against the tick budget. SQLite, network sockets, distributed file systems — all are storage systems with their own cost profiles. This is [§38](38_storage_systems.md).

**Cleanup amortises the write cost.** The cleanup system you built in [§22](22_mutations_buffer.md) already batches in-memory mutations to avoid mid-tick races. At streaming scale, the same pattern earns its keep again, for a second reason: it batches *disk* writes. Without batching, 10 000 individual mutations per tick would mean 10 000 disk writes — at 100 µs per write, a full second of I/O per tick, far over budget. With cleanup, those 10 000 mutations become one durable batch per tick: a handful of disk pages flushed sequentially to the log. One syscall, one trip through the block layer, one (or a few) DMA transfers — versus 10 000 of each. The cost is amortised across the batch, not paid per row. The mechanics — page cache, vectored I/O, fsync semantics — belong to [§38](38_storage_systems.md); the gradient is what matters here. The architecture you assembled in §22 was already the streaming architecture in miniature; this section just lets you spell it out at scale.

The simulator at streaming scale is no longer a process running in memory; it is a *pipeline* between a memory window and a durable log, with the systems running on whatever slice of the world is currently mounted. Every read might fault to disk; every write is buffered into the next cleanup's batch.

The transition from in-memory to streaming is the largest architectural shift in the book. Below this wall, the simulator is a single-process program with its working state in RAM. Above it, the simulator is closer to a database with its working state on disk and a small in-memory hot path. The techniques are different; the discipline is the same — layout, working set, ownership, determinism — applied at a different scale.

This wall is where most projects either re-architect or quietly accept slower-than-target performance. The book points at the wall and names the techniques; it does not pretend the techniques are free.

## Exercises

1. **Compute your streaming threshold.** Estimate your simulator's per-creature footprint at full SoA. Divide your machine's RAM (the half you can spare for the simulator) by that footprint. The result is roughly the N at which the simulator hits the streaming wall.
2. **Predict the cost.** A disk read is ~100 µs (NVMe SSD), ~200–500 µs (SATA SSD), or ~10 ms (spinning disk). At a 33 ms tick budget, how many disk reads can a tick afford? How many might a system want to make?
3. **Snapshot a small world.** Write a function that serialises your simulator's current state to a single file (one file, no schema gymnastics, just write the columns). Read it back into a fresh world. Confirm the simulator continues running indistinguishably.
4. **A windowed log.** Implement an append-only log with a fixed in-memory window. Older entries go to disk; new entries always go to memory. Verify queries inside the window are fast; queries outside the window pay the disk cost.
5. **Log-as-world.** With the windowed log from exercise 4, reconstruct creature state at an earlier tick by replaying the log over the most recent snapshot whose tick is ≤ the requested one. Compare query speed to the in-memory case.
6. *(stretch)* **Document your bound.** Write down, for your simulator, the largest N you can run while staying inside a 33 ms tick budget. Include footprint, cache regime, and any disk-bound cost. Above this N, the simulator needs the streaming architecture.

Reference notes in [30_streaming_wall_solutions.md](30_streaming_wall_solutions.md).

## What's next

You have closed Scale. The next phase is *Concurrency*, starting with [§31 — Disjoint write-sets parallelize freely](31_disjoint_writes_parallelize.md). The simulator is about to start running on more than one thread.
