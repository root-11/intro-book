# Solutions: 36 - Persistence is table serialization

## Exercise 1 - Snapshot

```rust,no_run
const SCHEMA_VERSION: u16 = 1;

fn snapshot(world: &World, path: &Path) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    f.write_all(&world.tick.to_le_bytes())?;
    f.write_all(&SCHEMA_VERSION.to_le_bytes())?;

    let n = world.creatures.pos.len() as u32;
    f.write_all(&n.to_le_bytes())?;
    write_slice(&mut f, &world.creatures.pos)?;
    write_slice(&mut f, &world.creatures.vel)?;
    write_slice(&mut f, &world.creatures.energy)?;
    write_slice(&mut f, &world.creatures.id)?;
    Ok(())
}

fn write_slice<T>(f: &mut std::fs::File, v: &[T]) -> std::io::Result<()> {
    use std::io::Write;
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v))
    };
    f.write_all(bytes)
}
```

The `unsafe` is for direct byte access. In production, use `bytemuck::Pod` to get the same effect safely. For 1 M creatures, the snapshot is roughly 24 MB; writing it is one bulk syscall, ~5-10 ms on NVMe.

## Exercise 2 - Load

```rust,no_run
fn load(path: &Path) -> std::io::Result<World> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut tick_bytes = [0u8; 8];
    f.read_exact(&mut tick_bytes)?;
    let tick = u64::from_le_bytes(tick_bytes);

    let mut sv = [0u8; 2];
    f.read_exact(&mut sv)?;
    let _schema_version = u16::from_le_bytes(sv);

    let mut n_bytes = [0u8; 4];
    f.read_exact(&mut n_bytes)?;
    let n = u32::from_le_bytes(n_bytes) as usize;

    let mut world = World::new(tick);
    world.creatures.pos = read_vec::<(f32, f32)>(&mut f, n)?;
    // ... other columns ...
    Ok(world)
}
```

After load, `hash_world(&loaded)` matches `hash_world(&original)` byte for byte. Determinism + transposition = round-trip safety.

## Exercise 3 - OOP comparison

For 1 M creatures:

- Column snapshot via raw bytes: ~5-10 ms.
- Per-row `serde_json::to_writer`: ~500-1000 ms (text encoding, per-row overhead).
- Per-row `bincode::serialize_into`: ~50-100 ms (binary, per-row overhead).

The column-direct version is bound by sequential disk bandwidth. The per-row versions add CPU encoding cost on top.

## Exercise 4 - Schema versioning

```rust,no_run
let schema_version = u16::from_le_bytes(sv);
if schema_version >= 2 {
    world.creatures.hunger_buildup = read_vec::<f32>(&mut f, n)?;
} else {
    world.creatures.hunger_buildup = vec![0.0; n]; // default for older snapshots
}
```

Old snapshots (v1) lack the `hunger_buildup` column; the loader supplies zeros. New snapshots (v2) include it. Both round-trip cleanly. Version migration lives at load time, in one place; the rest of the simulator does not know about it.

## Exercise 5 - Memory-mapped snapshot

```rust,no_run
use memmap2::MmapOptions;

let f = std::fs::File::open(path)?;
let mmap = unsafe { MmapOptions::new().map(&f)? };
let bytes: &[u8] = &mmap;
// Parse columns directly from `bytes` - no copy.
```

For a 24 MB snapshot:

- Read-into-Vec: ~5 ms (one syscall + memcpy from kernel buffer to user heap).
- mmap: ~10 µs initial setup; access is page-faulted lazily.

If the simulator accesses the loaded data sequentially after load, mmap wins. If access is random, mmap pays page-fault costs each time. For load-once-then-stream patterns, mmap is the cleaner option.
