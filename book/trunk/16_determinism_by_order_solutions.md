# Solutions: 16 — Determinism by order

## Exercise 1 — Hash the world

```rust
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

fn hash_world(world: &World) -> u64 {
    let mut h = DefaultHasher::new();
    world.creatures.len().hash(&mut h);
    for &x in &world.pos { x.0.to_bits().hash(&mut h); x.1.to_bits().hash(&mut h); }
    for &v in &world.vel { v.0.to_bits().hash(&mut h); v.1.to_bits().hash(&mut h); }
    for &e in &world.energy { e.to_bits().hash(&mut h); }
    h.finish()
}
```

Floats are hashed via `to_bits()` because `f32::hash` is not trait-implemented (a float can be NaN with multiple bit patterns; the language refuses to choose). `to_bits()` is bit-equality, which is what determinism requires.

## Exercise 2 — Two identical runs

```rust,no_run
let mut world1 = init_world(seed: 0xCAFE);
let mut world2 = init_world(seed: 0xCAFE);
for _ in 0..100 { tick(&mut world1); }
for _ in 0..100 { tick(&mut world2); }
assert_eq!(hash_world(&world1), hash_world(&world2)); // bit-identical
```

If the hashes match, your simulator is deterministic for this run length and seed. (Not a proof — just one data point — but a strong one.)

## Exercise 3 — Deliberate non-determinism

Replace `seeded_rng` with `thread_rng()` or a wall-clock-seeded RNG. Re-run. The hashes differ. The visible state of the world after 100 ticks is structurally the same shape but populated with different numbers.

## Exercise 4 — Find the culprit

Hash the world after every system. The first system whose post-hash differs between runs is the culprit. A few common sources:

- A system reads from a `HashMap` whose iteration order is randomised.
- A system reads `Instant::now()` or `SystemTime::now()`.
- A system spawns a thread; the thread's writes race with the main thread's.

Once located, the source is usually obvious. The fix is to remove the source — a deterministic alternative always exists.

## Exercise 5 — `HashMap` iteration order

`std::collections::HashMap` uses `RandomState` by default — its iteration order varies between processes (and sometimes within one process across rebuilds, depending on the Rust version). `BTreeMap` iterates in sorted-key order, deterministic across runs. For ECS use, prefer `Vec<(K, V)>` (sequential, deterministic, cache-friendly) over either.

## Exercise 6 — Time as input

```rust
// Before
fn motion(pos: &mut [(f32, f32)], vel: &[(f32, f32)]) {
    let dt = some_global_clock(); // non-deterministic
    /* ... */
}

// After
fn motion(pos: &mut [(f32, f32)], vel: &[(f32, f32)], dt: f32) {
    /* ... */
}
```

`dt` enters from the caller. The caller may compute it from `Instant::now()` (production) or read it from a recorded log (replay). The system itself does not know the difference.

## Exercise 7 — A property test

```rust,no_run
fn property(seed: u64) -> bool {
    let h1 = run_and_hash(seed);
    let h2 = run_and_hash(seed);
    h1 == h2
}

fn run_and_hash(seed: u64) -> u64 {
    let mut world = init_world(seed);
    for _ in 0..100 { tick(&mut world); }
    hash_world(&world)
}

for seed in 0..100u64 {
    assert!(property(seed), "non-deterministic at seed {seed}");
}
```

If any seed produces different hashes across runs, the simulator is non-deterministic. Different seeds usually (not always — the hash space has collisions) produce different hashes; that confirms the simulator is sensitive to its inputs, which is the dual property to determinism.
