# Solutions: 46 - The log survives power loss

## Exercise 1 - Tear a log on purpose

Write `[len: u32][bytes]` records with no marker and no `fsync`, then truncate past the last sector:

```rust
let meta = std::fs::metadata("log.bin")?;
std::fs::OpenOptions::new().write(true).open("log.bin")?
    .set_len(meta.len() - 7)?; // land mid-record
```

A naive replay reads a `len`, then that many bytes. The torn tail does one of two bad things: `len` points past end of file (a decode error you at least *notice*), or `len` happens to be a small plausible value and you decode garbage as the final record and fold it into the world silently. The point is the second case: without a marker a replay cannot distinguish a complete log from a truncated one, so it cannot refuse the corruption.

## Exercise 2 - Add the commit marker

Frame each batch and trail it with a checksummed marker:

```rust
// batch bytes, then: [MAGIC: u32][batch_len: u32][crc32(batch): u32]
fn append_batch(f: &mut File, batch: &[u8]) -> io::Result<()> {
    f.write_all(batch)?;
    f.write_all(&MAGIC.to_le_bytes())?;
    f.write_all(&(batch.len() as u32).to_le_bytes())?;
    f.write_all(&crc32(batch).to_le_bytes())?;
    Ok(())
}
```

On replay, walk batch by batch; a trailing region whose marker is absent, whose `batch_len` overruns the file, or whose `crc32` does not match the preceding bytes is a torn tail. Stop there and discard it. Re-run exercise 1: recovery now yields the last *committed* batch, intact. The torn batch did not happen.

## Exercise 3 - Order the barrier

`fsync` the records, write the marker, `fsync` again:

```rust
f.write_all(batch)?; f.sync_data()?;     // records durable
f.write_all(&marker)?; f.sync_data()?;   // marker durable
```

Walk the crash points. Before the first `sync_data`: nothing is guaranteed durable, replay sees no complete batch, the batch did not happen - consistent. Between the two: records may be durable but the marker is not, so replay sees no marker and discards them - the batch did not happen - consistent. After the second: both durable, replay accepts - consistent. The *one* ordering that breaks is writing the marker before the records are durable: a crash there leaves a marker vouching for bytes that never landed, and replay accepts a batch built from garbage.

## Exercise 4 - Atomic snapshot

```rust
let tmp = "world.snap.tmp";
{ let mut s = File::create(tmp)?; s.write_all(&serialize(world))?; s.sync_all()?; }
std::fs::rename(tmp, "world.snap")?;       // atomic swap
File::open(dir)?.sync_all()?;              // make the rename itself durable
```

A `kill -9` at any instant leaves either `world.snap` (the previous complete snapshot) or `world.snap` plus an orphan `world.snap.tmp` you ignore on load. There is no instant where `world.snap` is half-written, because `rename` flips atomically. The directory `fsync` is the step everyone forgets: without it the rename can be lost even though the temp file was durable.

## Exercise 5 - Idempotent replay

Replay the committed `S..T` suffix onto the tick-S snapshot twice and hash:

```rust
let a = replay(load("S.snap")?, &log[s..t]);
let b = replay(load("S.snap")?, &log[s..t]);
assert_eq!(hash_world(&a), hash_world(&b));
```

They match because replay is deterministic re-application from a fixed starting world ([§16](16_determinism_by_order.md)): each committed event applies exactly once on the path from the snapshot forward, so "twice" is two independent runs of the same pure function, not double-application. If a hash differs, an event is reading wall-clock time, a `HashMap` iteration order, or its own prior output - find it and make it a function of the snapshot plus the log only.

## Exercise 6 - Recover to any tick

```rust
// after kill -9 mid-run:
let snap = load_latest_intact_snapshot()?;      // ex 4 guarantees it is whole
let world = replay(snap, &committed_suffix(&log, snap.tick)?); // ex 2 bounds the suffix
assert_eq!(hash_world(&world), live_hash_at(world.tick));
```

Recovery cost is bounded by the events since the last snapshot, which is why snapshot cadence is a tuning knob: frequent snapshots mean fast recovery and more write traffic. A divergent hash points at the first event that is not deterministic - the same hunt as exercise 5.

## Exercise 7 - The premature acknowledgement

```rust
// WRONG: ack before durable
append(&record)?; reply_ok(&sender);   // anti-pattern: bad!
f.sync_data()?;                          // crash before this loses the record
```

`kill -9` between `reply_ok` and `sync_data`: on recovery the record is gone, but the sender holds an "ok" for it and will never resend. The data the sender believes you accepted is lost. Move the acknowledgement after the marker is durable:

```rust
append(&record)?; write_marker()?; f.sync_data()?; reply_ok(&sender); // correct
```

Now a crash before the ack leaves the record absent *and* unacknowledged, so the sender retries it. The log and the sender always agree. This is the payment-processor rule made mechanical: the marker is the only honest place to say "done".

## Exercise 8 - Measure the barrier

`fsync` per record, per batch, and never:

```rust
// per record: write+sync_data each record  -> one fsync per record
// per batch:  write all, then one sync_data -> one fsync per tick
// none:       write all, no sync_data        -> buffered, not durable
```

The per-batch path reproduces the [§38](38_storage_systems.md) batched-vs-unbatched span (14-256x across the reference machines), and the durable-per-record path is worse still, because each record now pays a real `fsync` rather than a buffered write. The "none" path is the fastest and is not crash-safe - it is the floor, useful only to show what durability costs.

## Exercise 9 - Price the database

SQLite in WAL mode (`rusqlite`) gives you the checksummed write-ahead log, the atomic commit, and crash recovery for free, plus edge cases your version does not handle: group commit, partial-page tears, and the well-known fact that some consumer drives lie about `fsync`. What it costs you is a dependency (build size, a C library, less control over the on-disk format) and the loss of the column-direct layout you tuned in [§36](36_persistence_is_serialization.md). For a save-game, keep the hand-rolled log. For a system of record, buy SQLite and spend your attention elsewhere - now knowing exactly which guarantee you are paying for.
