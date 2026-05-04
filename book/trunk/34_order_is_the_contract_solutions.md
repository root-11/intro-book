# Solutions: 34 — Order is the contract

## Exercise 1 — Build the schedule

```rust,no_run
fn tick(world: &mut World, dt: f32) {
    next_event(&world.pos, &world.food, &mut world.pending);

    // Per-thread to_remove segments to avoid the appliers' shared write.
    let mut seg_eat:    Vec<u32> = Vec::new();
    let mut seg_repro:  Vec<CreatureRow> = Vec::new();
    let mut seg_starve: Vec<u32> = Vec::new();

    thread::scope(|s| {
        s.spawn(|| apply_eat(&world.pending, &world.food, &mut seg_eat, /* energy partition */));
        s.spawn(|| apply_reproduce(&world.pending, &world.energy, &mut seg_repro));
        s.spawn(|| apply_starve(&world.pending, &world.id, &mut seg_starve));
    });
    // All three appliers have completed before this line.

    world.to_remove.extend(seg_eat);
    world.to_remove.extend(seg_starve);
    world.to_insert.extend(seg_repro);

    cleanup(world);
    inspect(world);
}
```

The three appliers run in parallel; their write-sets are made disjoint by per-thread segments. `cleanup` runs after the scope returns, never before.

## Exercise 2 — Test for determinism

```rust,no_run
let mut w1 = init_world(0xCAFE);
let mut w2 = init_world(0xCAFE);

for _ in 0..100 { tick(&mut w1, 0.033); }
for _ in 0..100 { tick(&mut w2, 0.033); }

assert_eq!(hash_world(&w1), hash_world(&w2));
```

If the parallel boundaries are correct, the hashes match. The merge of `seg_eat`, `seg_starve`, and `seg_repro` happens in the same order on both runs (the `extend` calls are sequential after the scope), so `to_remove` and `to_insert` end up identical between runs.

## Exercise 3 — Break the contract

```rust,ignore
thread::scope(|s| {
    s.spawn(|| apply_eat(&world.pending, &world.food, &mut seg_eat, /* ... */));
    s.spawn(|| apply_reproduce(&world.pending, &world.energy, &mut seg_repro));
    s.spawn(|| apply_starve(&world.pending, &world.id, &mut seg_starve));
    s.spawn(|| cleanup(world)); // running concurrently with appliers
});
```

If you sidestep the borrow checker (e.g. via `unsafe` shared pointers), `cleanup` may start before the appliers have written `seg_*`. The two runs of the simulator produce different hashes — sometimes. Sometimes the same. The bug's *intermittency* is the lesson; intermittent bugs are the worst kind to debug, and the contract exists to prevent them.

## Exercise 4 — Level boundaries

For the simulator:

| level | systems                                       | reads                          | writes                          |
|------:|-----------------------------------------------|--------------------------------|---------------------------------|
|     0 | `food_spawn`                                  | `food_spawner`                 | `food`                          |
|     1 | `motion`                                      | `pos`, `vel`, `energy`, `food` | `pos`, `energy`                 |
|     2 | `next_event`                                  | `pos`, `food`                  | `pending`                       |
|     3 | `apply_eat`, `apply_reproduce`, `apply_starve` | `pending`                      | thread-local segments           |
|     4 | `cleanup`                                     | segments                       | `creatures`, `food`             |
|     5 | `inspect`                                     | everything                     | (nothing)                       |

Six levels. Within each level, parallelism. Between levels, sync. Total throughput is bounded by the slowest level's longest system.

## Exercise 5 — A minimal scheduler

```rust,no_run
use std::collections::HashMap;

struct SystemDecl {
    name: &'static str,
    reads:  Vec<&'static str>,
    writes: Vec<&'static str>,
}

fn schedule<'a>(systems: &'a [SystemDecl]) -> Vec<Vec<&'a str>> {
    let mut levels: Vec<Vec<&str>> = Vec::new();
    let mut placed: HashMap<&str, usize> = HashMap::new();

    for s in systems {
        // The earliest level we can run is one after every system that
        // wrote a table we read.
        let mut my_level = 0;
        for sr in &s.reads {
            for prior in systems.iter().filter(|p| p.writes.iter().any(|w| w == sr)) {
                if let Some(&l) = placed.get(prior.name) {
                    my_level = my_level.max(l + 1);
                }
            }
        }
        if levels.len() <= my_level {
            levels.resize(my_level + 1, Vec::new());
        }
        levels[my_level].push(s.name);
        placed.insert(s.name, my_level);
    }
    levels
}
```

Around 30 lines. Topological sort + level grouping. Real schedulers add work-stealing, priority, GPU dispatch, dynamic re-balancing — but the core *contract enforcement* is exactly this. If your scheduler produces the right `Vec<Vec<&str>>`, you have a correct parallel ECS executor.
