# 36 — Persistence is table serialization

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 36](../../concepts/glossary.md#36--persistence-is-table-serialization).*

<p align="center"><img src="../illustrations/mathematics_describes.jpg" alt="Mathematics describes, models, implements — persistence captures the world that worked" style="max-height: 300px; max-width: 100%;"></p>

The simulator pauses. The world is in memory: six columns of `creatures` (`pos`, `vel`, `energy`, `birth_t`, `id`, `gen`), a `food` table, presence tables (`hungry`, `dead`, etc.), the index map (`id_to_slot`), and the cleanup buffers. To pause durably, all of this must be written to disk; to resume, all of this must be read back.

The instinct the OOP world brings: design a "persistence format" with a schema, marshalling logic, version handling, and a translation layer between in-memory objects and on-disk records. This is wrong on the data-oriented side. There is no translation. There is only *transposition*.

A snapshot is the columns, written sequentially. A recovery is the columns, read sequentially. The on-disk format is the same shape as memory.

```rust,no_run
fn snapshot(world: &World, path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;

    // Header: tick, schema version.
    f.write_all(&world.tick.to_le_bytes())?;
    f.write_all(&SCHEMA_VERSION.to_le_bytes())?;

    // Each column: [length: u32][raw bytes...]
    write_column(&mut f, &world.pos)?;
    write_column(&mut f, &world.vel)?;
    write_column(&mut f, &world.energy)?;
    write_column(&mut f, &world.birth_t)?;
    write_column(&mut f, &world.id)?;
    write_column(&mut f, &world.gen)?;

    // Presence tables: same shape, append.
    write_column(&mut f, &world.hungry)?;
    // ... etc.

    Ok(())
}
```

Recovery is the inverse: read the bytes back into `Vec`s. No type conversion, no field mapping, no schema discrimination at the row level. The file is exactly what the memory was; the memory is exactly what the file is.

The savings are concrete:

**No schema design.** The schema is whatever the columns are. Schema documentation is the column declarations.

**No object marshalling.** No `serialize()` per row, no `deserialize()` per row. The `Vec` is written as bytes; bytes are read as a `Vec`. At 1M creatures × 24 bytes hot, the snapshot is 24 MB; writing it is one bulk write — ~5–10 ms on NVMe.

**No translation bugs.** ORMs are a famous source of subtle correctness issues — fields renamed, types coerced, edge cases mishandled. Here, the in-memory and on-disk forms are bit-identical; the load is `read_bytes_into_vec` and that is all.

**Deterministic recovery.** A snapshot taken in a deterministic simulator round-trips exactly. The hashed world after `snapshot → load` is identical to the hashed world before.

What it does *not* save you from:

**Platform versioning.** Three things can break a snapshot across environments: the *schema* changed (you added a column or renamed a type), the *byte order* differs (you saved on a little-endian machine and loaded on a big-endian one), or the *OS conventions* differ (line endings, native type widths, path conventions in any string fields). All three have the same fix. Write a small header into every snapshot — schema version, endianness, OS — and at load time, if any field differs from the current platform's value, run the matching migration. The migrations are one-directional (newer code reads older snapshots, x86 code reads big-endian snapshots, Linux code reads macOS snapshots) and they are the only translations in the system. Most simulators target a single architecture and OS, write the header fields anyway, and skip the migrations until they are needed; the mechanism is there from day one, the cost is the bytes for the header.

**Compression.** Raw bytes are rarely compact at runtime — many fields are sparse or small, and compression is sometimes worth a few milliseconds at snapshot time to save tens of milliseconds on the disk side. Apply only after measurement.

The pattern shows up everywhere this scale matters. Write-ahead logs in databases, save-game files in games, checkpoint files in HPC, frame snapshots in video editing. They all dodge the ORM trap by writing the columns directly.

The §0/§1 simulator's snapshot is roughly twenty-five lines of Rust per direction. The OOP equivalent — define a `CreatureRecord`, derive `Serialize`/`Deserialize`, walk the world serialising one creature at a time — is ten times the code, slower at runtime, and prone to the translation bugs the column-direct version cannot have.

## Exercises

1. **Snapshot the world.** Implement a `snapshot` function for your simulator. Save to `snapshot.bin`. Note the file size: it should match `bytes per column × N` for hot tables, plus headers.
2. **Load the snapshot.** Implement the inverse. Load `snapshot.bin` into a fresh `World`. Verify by running the simulator from the loaded state and comparing the hash to the original at the same tick.
3. **The OOP comparison.** Define a `CreatureRecord` struct and write a per-row serialiser via `serde_json` or `bincode`. Time it against the column snapshot at 1M creatures. The per-row version is typically 5–50× slower.
4. **Schema versioning.** Add a new column (`hunger_buildup: f32`) to the simulator. Make the snapshot reader handle both old and new versions: old snapshots get the new column zero-filled; new snapshots get loaded directly. Verify both round-trip cleanly.
5. *(stretch)* **Memory-mapped snapshot.** Use `memmap2` to map the snapshot file directly into memory. The Vec's pointer is the file's memory; loading is zero-copy. Compare load times for a 24 MB snapshot.

Reference notes in [36_persistence_is_serialization_solutions.md](36_persistence_is_serialization_solutions.md).

## What's next

[§37 — The log is the world](37_log_is_world.md) makes the structural argument explicit: the log of events and the world's tables share a shape; one is a projection of the other.
