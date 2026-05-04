# 35 — The boundary is the queue

<p align="center"><img src="../covers/phase_io_persistence.jpg" alt="I/O & persistence phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 35](../../concepts/glossary.md#35--the-boundary-is-the-queue).*

The simulator is a pure function. Given the world at tick start (`world_t`) and the inputs that arrived during the tick (`inputs_t`), it produces the world at tick end (`world_t+1`) and the outputs that should leave (`outputs_t`). Between those endpoints, no system touches the outside world. No system reads `Instant::now()`, sends a packet, writes to disk, or prints to stdout. Inside, the simulator is a transformation. Outside, it is a queue.

```text
   ┌─────────────────────────────┐
   │      Simulator (pure)       │
   │  ┌──────────────────────┐   │
   │  │     systems run      │   │
   │  │   on world_t state   │   │
   │  └──────────────────────┘   │
   │     ↑                  ↓    │
   │ inputs_t           outputs_t│
   └─────↑──────────────────↓────┘
         │                  │
   ┌─────────┐        ┌─────────┐
   │ in queue│        │out queue│
   └─────────┘        └─────────┘
        ↑                  ↓
   environment        environment
```

Inputs arrive on the in-queue: events with timestamps, food-spawn requests from the policy, network packets in a multiplayer simulator, user input events. They wait in the queue until the next tick consumes them.

Outputs leave on the out-queue: state-change events for the log (`eaten`, `born`, `dead`), rendering data for the visualiser, packets for peers, replication updates for distributed nodes. They wait in the queue after the tick produces them, until the storage system or transport layer ships them.

What happens *inside* the boundary: pure transformation. Systems read from `inputs_t` (which is just another table by the time the systems start), update the world's tables, queue mutations to `to_remove`/`to_insert`, and write to `outputs_t` (also just a table). No `Instant::now()`. No `println!`. No `File::open`. No `TcpStream::connect`. The inside is reproducible by construction; the outside is unpredictable, and the queue is the seam.

Why this matters:

**Determinism.** [§16](16_determinism_by_order.md)'s rule (same inputs + same order = same outputs) holds only if "inputs" is a complete description of the tick's environment. The queue *is* that complete description. Any system reading from outside the queue is a source of non-determinism the queue cannot capture.

**Replay.** Record the in-queue. Replay the tick from `world_t` with the recorded queue. Get bit-identical `world_t+1`. The queue is what makes replay possible.

**Testability.** A test fills the in-queue with a synthetic input, runs one tick, asserts on the out-queue. The test does not need to mock `File`, `TcpStream`, or the system clock; the queue interface is the only thing the simulator sees.

**Distribution.** A distributed simulator with multiple nodes communicates via queues — each node's out-queue feeds another node's in-queue. The queue interface is the same on a single machine and across a network. The simulator's design does not change.

**Auditability.** Every input that ever reached the simulator is in the in-queue's history. Every output is in the out-queue's history. The simulator's full external interface is two append-only logs.

The cleanup pattern from [§22](22_mutations_buffer.md) was the boundary at tick scope (mutations buffer, apply at boundary). The queue pattern at this scope is the same idea at run scope (I/O buffers, apply at the seam). The two compose: cleanup makes the tick atomic; the queue makes the run reproducible.

A useful test: can you run two simulators side-by-side from the same in-queue and get identical out-queues? If yes, the boundary holds. If no, somewhere a system reads the environment directly.

## Exercises

1. **Build the queues.** Add `in_queue: Vec<InputEvent>` and `out_queue: Vec<OutputEvent>` to your simulator. Both fill at tick boundaries.
2. **Refactor a system that reads time.** Find any system that uses `Instant::now()` directly. Refactor: take `current_time` as a parameter. The caller (the tick driver) reads `Instant::now()` once and passes it down. The system itself is now deterministic.
3. **Refactor a system that prints.** Find any system that calls `println!`. Refactor: push the message to `out_queue` instead. The caller reads the queue after the tick and writes whatever's there. Logging is now deterministic; tests can assert on the queue.
4. **Replay test.** Save the in-queue across a 100-tick run. Run the simulator a second time from the initial world state with the saved queue. Hash both worlds. They must match.
5. **Two simulators from one queue.** Run two simulators in parallel (or sequentially), feeding both from the same in-queue. After 100 ticks, hash both worlds. They must match. If they do not, somewhere a system reads from outside the queue.
6. *(stretch)* **Audit a real simulator.** Open any open-source simulator's tick function. Find every place it reads from the environment (clock, file, network, env vars). Each is a place where determinism leaks; each could be queue-ified.

Reference notes in [35_boundary_is_the_queue_solutions.md](35_boundary_is_the_queue_solutions.md).

## What's next

[§36 — Persistence is table serialization](36_persistence_is_serialization.md) takes the next step: when the simulator pauses and resumes, persistence is just writing the columns and reading them back. No translation, no impedance mismatch.
