# Solutions: 30 - The wall at 1M → streaming

## Exercise 1 - Streaming threshold

Per-creature footprint at full SoA: hot ~24 B, cold ~16 B, plus presence flags ~8 B = ~48 B. Plus index maps and indices into derived tables: round to ~64 B per live creature.

For a desktop with 32 GB RAM, allocating 16 GB to the simulator: 16 × 10⁹ / 64 = 250 million creatures. Above ~250M, the simulator must start streaming.

In practice the threshold is lower because logs, snapshots, and OS overhead consume RAM too. A safe budget might be 50-100M before the streaming architecture is needed.

## Exercise 2 - Disk read cost

NVMe SSD: ~100 µs per read. A 33 ms tick budget is 33 000 µs / 100 µs ≈ 330 random disk reads. If a system would naturally make 10 000 disk reads per tick, the simulator is roughly 30× slower than budget unless reads are batched or sequential.

The fix is the same as for cache misses at the smaller scale: amortise the cost. Read a *page* of consecutive entries (4 KB = 64 cache lines = many rows) at one IOPS cost; touch all of them while the cost is paid.

## Exercise 3 - Snapshot

```rust,no_run
fn snapshot_world(world: &World, path: &Path) -> std::io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    use std::io::Write;
    let c = &world.creatures;
    f.write_all(&c.len().to_le_bytes())?;
    for &x in &c.px     { f.write_all(&x.to_le_bytes())?; }
    for &y in &c.py     { f.write_all(&y.to_le_bytes())?; }
    for &x in &c.vx     { f.write_all(&x.to_le_bytes())?; }
    for &y in &c.vy     { f.write_all(&y.to_le_bytes())?; }
    for &e in &c.energy { f.write_all(&e.to_le_bytes())?; }
    for &i in &c.id     { f.write_all(&i.to_le_bytes())?; }
    Ok(())
}

fn load_world(path: &Path) -> std::io::Result<World> {
    // Read the columns back in the same order. Sizes from the prefixed length.
    todo!()
}
```

After a snapshot + load round-trip, the simulator continues running indistinguishably *if and only if* the simulator is deterministic (§16). Determinism is the precondition for snapshot/load to mean what we expect.

## Exercise 4 - Windowed log

```rust,no_run
struct WindowedLog<E> {
    in_memory: std::collections::VecDeque<E>,
    capacity:  usize,
    disk:      std::fs::File,
}

impl<E: Encode> WindowedLog<E> {
    fn push(&mut self, e: E) {
        if self.in_memory.len() == self.capacity {
            // Evict oldest to disk, append new in memory.
            let oldest = self.in_memory.pop_front().unwrap();
            oldest.encode(&mut self.disk).unwrap();
        }
        self.in_memory.push_back(e);
    }
}
```

Queries inside the in-memory window are O(1) per entry. Queries past the window pay one disk seek (~100 µs) plus a sequential read for the requested span. The cost difference is the streaming wall.

## Exercise 5 - Log-as-world reconstruction

To recover state at tick T:

1. Find the most recent snapshot at tick S ≤ T.
2. Load the snapshot into a fresh world.
3. Replay log entries from S to T in order.

Replay speed depends on the events-per-tick rate. For most simulators, replay is 10-100× faster than original-tick rate (no rendering, no I/O outside the log read). Reconstructing 1000 ticks of history takes ~1 second of replay time.

## Exercise 6 - Document your bound

A well-tuned simulator at 30 Hz on a typical desktop:

- ~10 M creatures comfortable (RAM-resident, sub-budget).
- ~100 M creatures possible with narrow fields and the spatial compaction (RAM-resident, near-budget).
- ~1 B creatures requires streaming (working set exceeds RAM).
- ~10 B+ requires distributed simulation across multiple machines (covered in the monograph).

Each step is a different architecture, with the techniques in the previous step still applying. The book ends below the streaming wall; the monograph picks up above it.
