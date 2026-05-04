# Solutions: 25 — Ownership of tables

## Exercise 1 — Identify the writers

| table              | one writer       | notes                                       |
|--------------------|------------------|---------------------------------------------|
| `creatures`        | `cleanup`        | every other system writes via `to_remove`/`to_insert` |
| `food`             | `cleanup`        | written via `to_remove(food)` from apply_eat, plus `to_insert(food)` from food_spawn |
| `food_spawner`     | `food_spawn` (or admin) | constant-quantity in practice         |
| `pending_event`    | `next_event`     | rebuilt every tick                          |
| `eaten`, `born`, `dead` | `apply_eat`, `apply_reproduce`, `apply_starve` | one writer per log |
| `hungry`           | `classify_hunger` | the system that decides hunger              |
| `to_remove`        | many              | side table; cleanup reads-and-clears        |
| `to_insert`        | many              | side table; cleanup reads-and-clears        |

The `to_remove` / `to_insert` "many writers" is allowed because each system writes only its own pushes; nobody reads the side table until cleanup.

## Exercise 2 — Constructed violation

```rust,no_run
// Two systems both write energy directly. Don't do this.
fn apply_eat_bad(food: &[Food], pending: &[Event], energy: &mut [f32]) {
    for ev in pending {
        if ev.kind == EAT { energy[ev.creature as usize] += 1.0; }
    }
}

fn apply_decay_bad(energy: &mut [f32], dt: f32) {
    for e in energy { *e -= 0.1 * dt; }
}
```

Run sequentially: correct, but order matters. Run in parallel via `std::thread::scope`:

```rust,no_run
std::thread::scope(|s| {
    s.spawn(|| apply_eat_bad(&food, &pending, &mut energy));
    s.spawn(|| apply_decay_bad(&mut energy, dt));
});
```

Rust's borrow checker rejects the code: `&mut energy` cannot be borrowed twice. The language refuses to compile the violation.

## Exercise 3 — Refactor

Add a side buffer:

```rust,no_run
fn apply_eat(food: &[Food], pending: &[Event], energy_delta: &mut Vec<(usize, f32)>) {
    for ev in pending { if ev.kind == EAT { energy_delta.push((ev.creature as usize, 1.0)); } }
}

fn apply_decay(energy_delta: &mut Vec<(usize, f32)>, count: usize, dt: f32) {
    for i in 0..count { energy_delta.push((i, -0.1 * dt)); }
}

fn apply_energy(energy: &mut [f32], deltas: &[(usize, f32)]) {
    for &(i, d) in deltas { energy[i] += d; }
}
```

Now `apply_eat` and `apply_decay` write to disjoint slices of `energy_delta` (use `Vec::extend_from_slice` from per-thread buffers, then merge). The single writer of `energy` is `apply_energy`. The rule holds.

## Exercise 4 — InspectionSystem

```rust,no_run
struct WorldSnapshot {
    creature_count: usize,
    food_count: usize,
    population_alive: usize,
    energy_avg: f32,
}

fn inspect(world: &World) -> WorldSnapshot {
    WorldSnapshot {
        creature_count: world.creatures.len(),
        food_count: world.food.len(),
        population_alive: world.id_to_slot.iter().filter(|&&s| s != INVALID).count(),
        energy_avg: if world.energy.is_empty() { 0.0 } else {
            world.energy.iter().sum::<f32>() / world.energy.len() as f32
        },
    }
}
```

`fn(&World) -> Snapshot` — read-only; no `&mut` anywhere. The system can run alongside any other system without violating ownership; multiple parallel readers are fine.

## Exercise 5 — Borrow checker

```rust,ignore
let mut a = vec![1, 2, 3];
let r1: &mut Vec<i32> = &mut a;
let r2: &mut Vec<i32> = &mut a; // ERROR
```

```
error[E0499]: cannot borrow `a` as mutable more than once at a time
```

The error is the language enforcing the architecture. Two `&mut` borrows of the same `Vec` cannot coexist. By choosing `&mut [T]` everywhere our systems take their write-set, the compiler enforces single-writer ownership at compile time.

## Exercise 6 — Audit

The audit should find:

- Every direct mutation of `creatures` happens in `cleanup`. If it happens elsewhere, mark the location and refactor.
- Every direct mutation of `food` happens in `cleanup`. Same rule.
- Every system has a clearly declared write-set, expressed as `&mut` parameters in its signature.
- No system holds a `&mut World`. Such a signature would allow it to write *any* table, violating the rule.

If the audit passes, the simulator is ready for parallelism ([§31](31_disjoint_writes_parallelize.md)) without any further refactoring. If it fails, the failure points are the only places that need fixing.
