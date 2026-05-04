# Solutions: 37 — The log is the world

## Exercise 1 — Log the simulator

```rust,no_run
struct Event {
    tick: u64,
    kind: u8,           // BORN, DIE, EAT, BECAME_HUNGRY, ...
    creature_id: u32,
    payload_a: u64,     // food_id, parent_id, ...
    payload_b: f32,     // energy_delta, ...
}

fn cleanup(world: &mut World, events: &mut Vec<Event>) {
    for &id in &world.to_remove {
        events.push(Event {
            tick: world.tick, kind: DIE,
            creature_id: id, payload_a: 0, payload_b: 0.0,
        });
        // ... apply removal ...
    }
    for row in &world.to_insert {
        events.push(Event {
            tick: world.tick, kind: BORN,
            creature_id: row.id, payload_a: row.parent_id as u64,
            payload_b: row.energy,
        });
        // ... apply insertion ...
    }
}
```

For a 100-creature simulator with steady birth and death, 100 ticks → roughly 200-1000 events depending on activity rate.

## Exercise 2 — Reconstruct from the log

```rust,no_run
fn replay(initial: &World, events: &[Event]) -> World {
    let mut w = initial.clone();
    for e in events {
        match e.kind {
            BORN => insert_creature(&mut w, e.creature_id, e.payload_b),
            DIE  => remove_creature(&mut w, e.creature_id),
            EAT  => apply_eat(&mut w, e.creature_id, e.payload_a as u32, e.payload_b),
            BECAME_HUNGRY => mark_hungry(&mut w, e.creature_id),
            // ...
        }
    }
    w
}
```

Run the simulator live for 100 ticks → world A. Run `replay(initial, &events)` → world B. `hash_world(&A) == hash_world(&B)`. The world is the log decoded.

## Exercise 3 — Save and load the log

The log is a `Vec<Event>` — same shape as any column-based table. Use [§36](36_persistence_is_serialization.md)'s column serialisation pattern. Save the log; load it; replay it onto the initial world; compare hashes.

## Exercise 4 — Snapshot + log

```rust,no_run
// At tick 0: snapshot.
snapshot(&world, "tick_0.snap")?;

// Run for 100 ticks, logging events.
let log = run_with_logging(&mut world, 100);

// Reconstruct any later tick T:
let snap_world = load("tick_0.snap")?;
let world_at_T = replay(&snap_world, &log[0..events_through_tick(T)]);
```

Snapshots are taken at convenient points; the log is appended continuously. Reconstruction at any T uses the most recent snapshot at S ≤ T plus the log slice from S to T.

## Exercise 5 — Triple-store form

```rust,no_run
struct Triple { rid: u32, key: u8, val: f64 }

let triples: Vec<Triple> = events.iter().flat_map(|e| match e.kind {
    DIE  => vec![Triple { rid: e.creature_id, key: KEY_DEAD, val: 1.0 }],
    BORN => vec![
        Triple { rid: e.creature_id, key: KEY_PARENT, val: e.payload_a as f64 },
        Triple { rid: e.creature_id, key: KEY_ENERGY, val: e.payload_b as f64 },
    ],
    // ...
}).collect();
```

For events with sparse fields (a `DIE` event uses only `creature_id`; an `EAT` event uses three fields), the triple-store form is 2-3× more compact because empty fields don't take space.

## Exercise 6 — A working specimen

[`science/simlog/logger.py`](../simlog/logger.py) implements the triple-store shape directly:

- `rids: Vec<u32>` — which entity (the row id)
- `keys: Vec<u16>` — which column (a numeric code)
- `vals: Vec<f64>` — the value, as 8 bytes (integers up to 2⁵³ round-trip exactly; strings are codebook-encoded to integers, then stored as the integer)

On read, these are densified into per-field `Vec`s plus presence masks. The same shape that was on disk is now in memory, ready for systems to iterate. The library does not need to know what an "event" is; it stores triples and lets the consumer interpret them. The §17/§37 structural pattern in working code.
