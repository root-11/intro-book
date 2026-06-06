# 38 - Storage systems: bandwidth and IOPS

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 38](../../concepts/glossary.md#38--storage-systems-bandwidth-and-iops).*

A *storage system* is the part of the program that crosses the boundary into something that holds bytes for longer than RAM does. Disk, network, distributed file system, message queue, message broker - all are storage systems. They differ in technology; they share a cost model.

The cost has two dimensions.

**Bandwidth** - bytes per second. How fast bytes can move through the storage system. NVMe SSD is roughly 3-7 GB/s read, 2-5 GB/s write. SATA SSD: ~500 MB/s. Spinning HDD: 100-200 MB/s sequential. Gigabit network: 100 MB/s. 10 Gbit network: 1 GB/s. SQLite on local NVMe: 200-500 MB/s for bulk inserts.

**IOPS** - operations per second. How many separate read/write operations the storage system can complete per second. NVMe: 100K-1M random IOPS; sequential IOPS counts are much higher (the underlying flash can stream). SATA SSD: 50-100K IOPS. HDD: 100-200 IOPS (limited by seek time). Network connection: bounded by latency × concurrency.

A workload's cost is bounded by *both*. A 1 MB sequential read on NVMe is one IOP and ~250 µs of bandwidth time. A million 1-byte random reads is a million IOPs and ~10 seconds of latency time. Same total bytes, three orders of magnitude apart.

The [§22](22_mutations_buffer.md) batched-cleanup pattern at [§30](30_streaming_wall.md)'s streaming scale gathers many small mutations into one large write. This converts a high-IOPS, low-bandwidth workload (1000 separate writes per tick) into a low-IOPS, bandwidth-friendly one (one batched write per tick)<sup>1</sup>. The pattern is the natural fit for storage systems where IOPS is the binding constraint.

<p align="center"><img src="../illustrations/power_supply_components.jpg" alt="Storage systems have bandwidth and IOPS - counted like power and current" style="max-height: 300px; max-width: 100%;"></p>

Three concrete examples worth keeping in mind:

**SQLite.** On local NVMe, SQLite handles ~50 K row inserts per second using one-by-one `INSERT` statements; ~500 K-1 M per second using prepared statements with batched transactions; ~5 M per second using `INSERT INTO ... SELECT FROM ...` over an in-memory table. The simlog exporter at [`science/simlog/logger.py`](../simlog/logger.py) uses the last form. The same database, three orders of magnitude in throughput, depending on whether the workload pushes IOPS or bandwidth.

**Network sockets.** A round-trip to a server is bounded by latency: ~0.1 ms LAN, ~10-100 ms internet, ~1 ms data centre. Each round-trip is one IOP from the workload's perspective. Bandwidth is not the binding constraint until the response is many KB. The §22 pattern at this scale: batch many requests into one round-trip.

**Distributed file systems.** S3, EFS, CephFS - bandwidth scales with concurrency (many parallel reads from many objects = high aggregate bandwidth) but per-object IOPS is low (one operation per request). Workloads that want sequential bandwidth fan out across many objects; workloads that want low latency on small reads do not fit this storage system.

The lesson: when adding a storage system to the simulator, measure both bandwidth *and* IOPS *of your workload* - not just the system's spec sheet. A 7 GB/s NVMe drive limited to 100 K IOPS is bottlenecked at ~30 KB per IOP for random workloads. Below that block size, IOPS bind.

The §4 budget framing applies here too. A 30 Hz tick has 33 ms of budget. A 100 µs disk read costs 0.3 % of the budget. Ten of them cost 3 %. A hundred cost 30 % - already a third of the tick. Bound the I/O per tick, batch where possible, and treat every cross-boundary operation as a real cost in the same ledger as cache misses and arithmetic.

The simulator inside the boundary is a pure function. The storage system at the boundary is the function's connection to durable reality. The cost of that connection is the bandwidth × IOPS budget; the discipline is the batching pattern; the architecture is the queue.

## Measurements

Batching many small writes into one trades a high-IOPS workload for a bandwidth-friendly one; how much it buys depends on the per-write overhead of the path (the 2012 laptop's slow per-write syscall path is the outlier). Full output: `code/README.md`.

| # | measurement | Ryzen 9 (modern) | i7-3610QM (2012) | i3-5010U (2015) | Pi 4 |
|---|---|---|---|---|---|
| 1 | batched vs unbatched write | 38x | 256x | 30x | 14x |

## Exercises

1. **Measure your bandwidth.** On Linux: `dd if=/dev/zero of=/tmp/test bs=1M count=1024 oflag=direct` measures sequential write. Note your number.
2. **Measure your IOPS.** Time 10 000 separate `File::write` calls of 4 KB each, with `sync_all()` after the loop. Compute IOPS as `10_000 / time_in_seconds`. Compare to the spec sheet.
3. **Batched vs unbatched.** Write 1 000 000 rows of 32 bytes each to a file: first as 1 000 000 separate writes; then as one bulk write. Compare times. The batched version is much faster - measured 14-256× across the four reference machines (`batched_write`), the spread driven by filesystem and write buffering. With an `fsync` per write (a durable log) the gap widens by orders of magnitude; the exercise here measures buffered writes, which is the floor.
4. **SQLite throughput.** Insert 1 000 000 rows into a SQLite table: first as separate `INSERT` statements; then in a single transaction; then via one `INSERT INTO ... VALUES (...)` with all rows. Note the three orders of magnitude.
5. **Compute your tick budget.** At 30 Hz with 1 000 mutations per tick, what is the largest acceptable per-mutation I/O cost? Below NVMe latency, you are fine; above it, you must batch.
6. *(stretch)* **A second storage system.** If you have a network filesystem handy (NFS, SSHFS), repeat exercise 3 against a remote file. Note the latency-vs-bandwidth tradeoff. The IOPS limit is your bandwidth-delay product divided by IO size.

Reference notes in [38_storage_systems_solutions.md](38_storage_systems_solutions.md).

## What's next

You have closed I/O & persistence. The simulator can now talk to durable storage and external systems without sacrificing determinism or layout discipline. The next phase is *System of systems*, starting with [§39 - System of systems](39_system_of_systems.md): the patterns for work that does not fit the standard tick model - long-running optimisation, time-sliced search, out-of-loop computation. After that, *Discipline* (§40-§43) closes the book with the design rules that keep the simulator working over time.
