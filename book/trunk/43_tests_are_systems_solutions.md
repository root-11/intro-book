# Solutions: 43 - Tests are systems

## Exercise 1 - A test as a system

Add `no_creature_moves_too_far` to your simulator's DAG behind a `--test` flag:

```rust,no_run
if cfg.test_mode {
    let suspicious = no_creature_moves_too_far(
        &world.px_before, &world.py_before,
        &world.creatures.px, &world.creatures.py, MAX_STEP);
    assert!(suspicious.is_empty(), "{:?}", suspicious);
}
```

In live mode, the system is absent. In test mode, it runs every tick. Same code path; different schedule.

## Exercise 2 - Property test

```rust,no_run
let mut world = init_world(0xCAFE);
let initial = world.creatures.len();
for _ in 0..1000 {
    tick(&mut world, 0.033);
    assert!(world.creatures.len() <= 2 * initial);
}
```

Run twice. Both runs report identical assertion outcomes (because of [§16](16_determinism_by_order.md)). If the property fails, both runs fail at the same tick.

## Exercise 3 - Replay test

```rust,no_run
let recording = run_and_record(&mut world1, 100);
let mut world2 = init_world(seed);
for inputs in &recording {
    world2.in_queue.extend(inputs.iter().cloned());
    tick(&mut world2, /* recorded current_time */);
}
assert_eq!(hash_world(&world1), hash_world(&world2));
```

Replay and live run produce bit-identical states. The test is `assert_eq!`; the test fixture is the recorded queue.

## Exercise 4 - TDD a new system

Test first:

```rust,no_run
fn test_growth_slows_at_high_energy() {
    let mut world = init_one_creature_with_energy(100.0);
    let initial = world.creatures.size[0];
    for _ in 0..10 { tick(&mut world, 0.033); }
    let final_size = world.creatures.size[0];
    assert!(
        final_size - initial < HIGH_ENERGY_GROWTH_RATE * 10.0,
        "growth too fast at high energy"
    );
}
```

The test states what the system should do. Then write the system. Then watch the test pass. The order matters: writing the test first forces you to *specify* the behaviour before *implementing* it.

## Exercise 5 - InspectionSystem connection

Both:

- Read all relevant tables (`&` borrows everywhere)
- Have empty (or report-only) write-sets
- Run last in the DAG (after all mutations have settled)
- Produce reports for consumption outside the simulator

The only difference: an InspectionSystem reports state to a debug consumer (`pptop`, an IDE, a log). A test reports assertion results to a test runner. Same shape; different consumer.

## Exercise 6 - Test runner = simulator scheduler

The simulator's main:

```rust,no_run
fn main() {
    let mut world = init_world(seed);
    let scheduler = build_schedule(&[
        food_spawn,
        motion,
        next_event,
        apply_eat, apply_reproduce, apply_starve,
        cleanup,
        // inspect: present in --debug only
    ]);
    loop { scheduler.tick(&mut world); }
}
```

The test runner:

```rust,no_run
fn test_main() {
    let mut world = init_world(seed);
    let scheduler = build_schedule(&[
        food_spawn,
        motion,
        next_event,
        apply_eat, apply_reproduce, apply_starve,
        cleanup,
        check_no_creature_moves_too_far, // assertion system
        check_population_bounded,        // assertion system
        inspect,                         // and inspect
    ]);
    for _ in 0..1000 { scheduler.tick(&mut world); }
}
```

The two binaries differ in *which* systems they include. The scheduler, the world, and every system itself is the same code. Most of the binary is shared.
