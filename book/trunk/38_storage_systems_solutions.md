# Solutions: 38 - Storage systems

## Exercise 1 - Bandwidth

```sh
$ dd if=/dev/zero of=/tmp/test bs=1M count=1024 oflag=direct
1073741824 bytes (1.1 GB) copied, 0.395 s, 2.7 GB/s
```

Typical numbers:

- NVMe SSD (PCIe 4.0): 3-7 GB/s sequential write
- SATA SSD: ~500 MB/s
- Spinning HDD: 100-200 MB/s
- USB 3 external: depends on the device, often 200-500 MB/s

`oflag=direct` bypasses the OS page cache, giving you the device's actual bandwidth, not what the page cache absorbs.

## Exercise 2 - IOPS

```rust,no_run
use std::io::Write;
use std::time::Instant;

let mut f = std::fs::File::create("/tmp/iops_test")?;
let buf = [0u8; 4096];
let n = 10_000;

let start = Instant::now();
for _ in 0..n {
    f.write_all(&buf)?;
}
f.sync_all()?; // important - without this, writes sit in the OS buffer
let elapsed = start.elapsed();
println!("IOPS: {:.0}", n as f64 / elapsed.as_secs_f64());
```

Typical numbers:

- NVMe: 50-200 K IOPS for 4 KB writes (the device may report higher random IOPS in benchmarks; sequential same-block writes hit different caches)
- SATA SSD: 50-100 K IOPS
- HDD: 100-200 IOPS

Without `sync_all`, the kernel buffers writes; the apparent IOPS is much higher than the device's actual rate. The actual disk-side IOPS is what `sync_all` exposes.

## Exercise 3 - Batched vs unbatched

```rust,no_run
// Unbatched: 1M writes
let mut f = std::fs::File::create("/tmp/unbatched")?;
for _ in 0..1_000_000 {
    f.write_all(&[0u8; 32])?;
}
f.sync_all()?;

// Batched: 1 write
let mut f = std::fs::File::create("/tmp/batched")?;
let big_buf = vec![0u8; 32 * 1_000_000];
f.write_all(&big_buf)?;
f.sync_all()?;
```

Typical results on NVMe:

- Unbatched: 5-30 seconds (1 M writes × IOPS limit)
- Batched: 50-100 ms (one ~30 MB write at sequential bandwidth)

100-500× faster. The exact ratio depends on the OS page cache's absorption behaviour; with `sync_all` to expose the actual disk-side cost, the gap is at the upper end.

## Exercise 4 - SQLite throughput

```rust,no_run
// Per-row INSERT (no transaction): ~50K rows/sec
for row in &rows {
    conn.execute("INSERT INTO t VALUES (?, ?, ?)", params![/* ... */])?;
}

// Single transaction: ~500K-1M rows/sec
conn.execute("BEGIN", [])?;
for row in &rows {
    conn.execute("INSERT INTO t VALUES (?, ?, ?)", params![/* ... */])?;
}
conn.execute("COMMIT", [])?;

// Bulk INSERT VALUES: ~5M rows/sec
let mut sql = String::from("INSERT INTO t VALUES ");
for row in &rows {
    sql.push_str(&format!("({}, {}, {}),", row.0, row.1, row.2));
}
sql.pop(); // trailing comma
conn.execute(&sql, [])?;
```

The IOPS dimension binds the per-row version (each `INSERT` is one disk operation when not in a transaction). The transaction version reduces per-row to one shared commit. The bulk-VALUES version reduces 1M operations to one - bandwidth-bound, not IOPS-bound.

## Exercise 5 - Tick budget

At 30 Hz: 33 ms / tick = 33 000 µs.

For 1000 mutations per tick:

- Per-mutation budget = 33 µs.
- NVMe latency = 100 µs (about 3× over budget *per mutation*).
- Batched: 1000 × 32 B = 32 KB, one ~5 µs write at NVMe sequential bandwidth (well under budget).

Unbatched mutations cannot fit a 30 Hz budget; batched ones easily can.

## Exercise 6 - A second storage system

For SSHFS at LAN latency (~0.5 ms RTT):

- Per-statement INSERT: 2 RTT minimum ≈ 1 ms = ~1000 IOPS max
- Single-transaction with 1 M rows: 2 RTT for the transaction (commit) + bandwidth for the data ≈ 100 ms total
- The IOPS limit is the bandwidth-delay product divided by IO size: at 1 Gbit/s × 0.5 ms = 64 KB in flight, so ~16 K IOPS max for 4 KB I/Os, ~1 K for 64 KB.

The pattern is the same: batching converts a high-latency, low-bandwidth workload into a sequential one bounded by bandwidth. On a network filesystem the latency penalty is much larger; the batching imperative is correspondingly stronger.
