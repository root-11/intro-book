# 37 - The log is the world

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 37](../../concepts/glossary.md#37---the-log-is-the-world).*

<p align="center"><img src="../illustrations/model_real_world.jpg" alt="Model the real world - the log is the world reconstructed step by step" style="max-height: 300px; max-width: 100%;"></p>

[§36](36_persistence_is_serialization.md) said persistence is transposition: the in-memory tables are written as their bytes, read back as their bytes. This section makes the deeper structural claim. *The log is the world*, and the world is the log decoded.

In an event-sourced simulator, every state change is an event:

```text
(tick=42, kind=become_hungry, creature_id=17)
(tick=42, kind=eat,           creature_id=23, food_id=8, energy_delta=+5.0)
(tick=43, kind=reproduce,     parent_id=14, offspring_id=400, offspring_energy=2.5)
(tick=43, kind=die,           creature_id=89)
```

The log is a sequence of such events. The world's tables can be reconstructed from the log: start from an empty world (or a snapshot), replay events in order, and the resulting tables are bit-identical to the world the live simulator produced.

The structural fact: **the log and the world have the same shape**.

In memory a presence table like `hungry` is a list of *slots* ([§17](17_presence_replaces_flags.md)); in the log it is a stream of `become_hungry` and `stop_being_hungry` events keyed by the stable *creature id* - the boundary rule from [§26](26_subscription_tables.md), since a slot is meaningless once the world is reloaded into a different layout. Replaying that stream of (tick, creature_id) pairs reconstructs the membership.

A column `energy: Vec<f32>` is the result of starting from an empty `Vec` plus the events that wrote each entry. The log holds these writes; the column is the cumulative effect of replaying them.

In the most explicit form - the *triple-store* shape - the log is a sequence of `(rid, key, val)` triples:

- `rid` = which entity: the stable [id](10_stable_ids_and_generations.md), not the slot
- `key` = which cell: a code for `table.column` (e.g. `creatures.energy`)
- `val` = the value written there

Read one triple as a sentence: *entity `rid`, cell `table.column`, becomes `val`*. The key is best read as `table.column` - it names the table *and* the column, so `(rid, table.column)` is a fully-qualified address of one cell anywhere in the world. That `table.column` form is what makes the log uniform: every state change, in every table, is the same three fields, and replay is the mechanical `world.table.column[id_to_slot[rid]] = val` applied over the log in order. The codebook stores each distinct `table.column` string once and the per-event key as a small integer code, so the log never carries the string. (This is a write-ahead log: `table.column`, row-by-id, value.)

Three stable handles, one moving thing left out. The entity id is *identity* - it survives relocation and the save ([§26](26_subscription_tables.md)). The `table.column` is the *schema address* - stable as long as the schema is. The value is the write. The *slot* - the entity's momentary position in the columns - is never logged, because it is the one part that moves; replay re-derives it through `id_to_slot` ([§23](23_index_maps.md)). The triples form the log; transposed, they form the columns. Transposition is the only translation. There is no impedance mismatch because there is no model gap.

### A working specimen: `code/logger`

The crate [`code/logger`](https://github.com/root-11/intro-book/tree/main/code/logger) implements this triple-store shape directly, dependency-free. Its design is worth walking through, because it meets three problems that recur whenever a simulator wants to log everything.

**The IOPS problem → batching.** A naive event logger calls `write` once per event. At a million events per minute, that is millions of disk operations per minute - bound by IOPS, not bandwidth ([§38](38_storage_systems.md)). The disk's bandwidth sits mostly idle while it queues operations. The fix: collect events into an in-memory buffer; when the buffer fills, flush it as one large write. IOPS scales with "buffer flushes per second"; bandwidth absorbs the actual byte volume. Logging cost drops from disk-latency-bound to bandwidth-bound - typically 100-1000× faster.

**The redundancy problem → codebook and type inference.** Most fields in a simulator's event records repeat: the same kind code thousands of times, the same set of activity strings, the same handful of entity types. Storing each event's full payload wastes bytes. The fix: a *codebook* assigns each unique string a small integer code; the log stores the code, not the string. On read, the codebook reverses the mapping. The crate goes one step further with type inference - every value is stored as one `f64` (8 bytes), whether it began as an integer, a float, or a string code. Integers up to 2⁵³ round-trip exactly; the union format eliminates per-field type tags. With only the populated fields stored per record, a sparse log uses far less memory than dense column arrays.

**The write-blocking problem → the revolver.** If the foreground blocks while the disk flushes, the simulator pauses on every flush. The fix: two buffers cycle between the foreground and a background writer thread. When one fills, the foreground hands it to the writer over a channel and takes a recycled empty one back; the writer flushes it and recycles it. When the foreground outruns the writer, that empty-buffer channel becomes the backpressure. From the simulator's side, writing an event is a few pushes to a `Vec`, never a wait on disk - `std::sync::mpsc` does both the hand-off and the flow control.

The combined result: `log()` costs ~160 ns at 5 fields and ~310 ns at 11 on this machine, sub-microsecond and dominated by the codebook lookups and the `Vec` pushes (`cargo run --release --bin benchmark` times both widths; the number is on your machine, not on trust). The hot-path output is a sequence of raw little-endian column-byte chunks written sequentially by the background thread - the bytes on disk are the bytes in memory ([§36](36_persistence_is_serialization.md)), no `.npz`, no `serde`. Read-back rebuilds dense columns and presence masks (`to_arrays`), iterates decoded rows, and exports CSV; a SQLite export is left to a downstream converter, since SQLite is not in Rust's std and a crate would forfeit the crate's dependency-free property. The structural identity - log = world - holds across all these formats; what changes is the storage system at the boundary ([§38](38_storage_systems.md)).

The design is justified structurally, not by a microbenchmark: the sparse triple-store stores only what a record populates, the codebook deduplicates the strings, and the single `f64` value stream erases per-field type tags. None of the three is exotic; together they are the compact shape a sparse simulation log wants. Three views of the same idea are sketched in the stretch exercise below.

The library does not need to know what an "event" is. It stores triples; the consumer interprets them. That separation is what makes the same code serve as a simulation logger, an audit trail, and a replay source - three uses, one structural pattern.

Why this matters in practice:

**Replay is structural.** Snapshot + log = pause/resume. To recover the world at any tick T, load the most recent snapshot at tick S ≤ T, then replay the log from S to T. The cost is bounded by `T - S` events, which is small if snapshots are taken regularly.

**Auditability is free.** Every change in the world is in the log. To answer "why is creature 17 dead?", scan the log for events involving 17. The log is the system's complete history, in order.

**Testing is replay.** A test fixture is an initial world plus a log. A test is "replay this log; assert this property of the result". No mocks, no setup methods, no fixture builders.

**Distribution is structural.** Two nodes running identical code from the same log produce bit-identical worlds. Send the log; the worlds converge.

**The log is the system of record.** Snapshots are caches of the log's state; they exist for performance, not correctness. If snapshots are lost, the log can rebuild them. If the log is lost, no snapshot can recover events that have not been logged.

The discipline that makes this work is structural, not stylistic. Every state change in the simulator is logged before being applied. The cleanup pass ([§22](22_mutations_buffer.md)) is the natural place - it sees every mutation and can record each one as it commits. The [§38](38_storage_systems.md) storage system is the natural sink - log writes are sequential, batched, and amortised across the tick.

A simulator that respects this discipline is one whose history is the log, whose state is a projection of the log, and whose persistence is the log plus the most recent snapshot. Every other property the book has built - determinism, parallelism, EBP dispatch, snapshot serialisation - composes with this one.

## Exercises

1. **Log the simulator.** Add an `events: Vec<Event>` table to your world. Modify the cleanup pass to push one event per applied mutation. After 100 ticks, the log has roughly `active × ticks` events.
2. **Reconstruct from the log.** Write a `replay(initial: World, events: &[Event]) -> World` that applies each event in order. Verify: starting from an initial world and applying the log produces a world identical to the live simulator's output at the same tick.
3. **Save and load the log.** Persist the log via [§36](36_persistence_is_serialization.md)'s column serialisation. Reload. Replay. Confirm bit-identical state.
4. **Snapshot + log.** Save a snapshot at tick S; save the log from tick S onward. Reconstruct any tick T > S by loading the snapshot and replaying the log from S to T. Verify against the live simulator.
5. **The triple-store form.** Convert your `events` table to three parallel arrays: `rids: Vec<u32>`, `keys: Vec<u8>`, `vals: Vec<f64>`. Compare the storage size to the per-event-struct version. The triple-store form is typically 2-3× more compact for events with sparse fields.
6. *(stretch)* **A logger, three ways.** [`code/logger`](https://github.com/root-11/intro-book/tree/main/code/logger) is the crate form, built and benchmarked. Read it, then sketch the other two shapes and compare what each gains and loses:

   - **As a crate** (the built one). Read `code/logger/src/lib.rs`: a `log()` over `(name, Value)` records, an evolving codebook, a double-buffered writer thread, reusable across simulators behind a stable public API. Note what that public boundary costs versus the two forms below.
   - **As a module** inside your simulator. Same shape, but accessing the simulator's existing types (`Event`, `World`) directly without crossing a crate boundary. Less reusable, more efficient - no public API to keep stable.
   - **As an ECS system.** A logging system whose read-set is `to_remove`, `to_insert`, and any other commit-time tables, and whose write-set is the log buffer. It runs in the same DAG as `cleanup`, perhaps merged with it. The two halves of cleanup - committing mutations and logging them - become one system.

   Sketch the module and system forms; the crate form is already in `code/logger` to compare against. Weigh what each gains and loses: reusability, performance, ease of testing, distance from the simulator's other concerns.

Reference notes in [37_log_is_world_solutions.md](37_log_is_world_solutions.md).

## What's next

[§38 - Storage systems: bandwidth and IOPS](38_storage_systems.md) names the cost of crossing the I/O boundary in concrete terms. The log lives there; so does the snapshot; so does every external connection.
