# Solutions: 14 — Systems compose into a DAG

## Exercise 1 — Draw the DAG

Drawing it by hand from the read-sets and write-sets in `code/sim/SPEC.md` reproduces the diagram in the chapter. Forks (after `next_event`) and joins (before `cleanup`) are the structural fingerprints of parallel-friendly stages.

## Exercise 2 — Spot the cycle

The proposed change creates:

- `apply_starve` writes `food`
- `food_spawn` reads `food`, writes `food`
- `next_event` reads `food`, writes `pending_event`
- `apply_starve` reads `pending_event`

So `apply_starve → food_spawn → next_event → apply_starve` — a cycle.

Three ways to break it:

1. **Buffer the write.** `apply_starve` pushes to `food_to_drop` (a side table); a separate next-tick `food_drop` system applies it.
2. **Reorder the policy.** Move "food appears where creatures died" out of the inner loop entirely — make food spawn pre-emptive, decoupled from death events.
3. **Accept the latency.** Allow `apply_starve` to write to a *next-tick* `food` buffer that `food_spawn` reads next tick. The food appears one tick late.

The first is the standard fix: introduce a side table, defer the cross-system write to the next tick.

## Exercise 3 — Topological sort

```
A writes X
B reads X, writes Y
C reads X, writes Z
D reads Y and Z, writes W
```

Dependencies: A → B, A → C, B → D, C → D. B and C are at the same DAG level: they share a read of X but their write-sets (Y and Z) are disjoint, so they can run in parallel. Valid orders include `A, B, C, D` and `A, C, B, D`. Both are correct.

## Exercise 4 — Compose two systems

```rust,no_run
fn tick(world: &mut World, dt: f32) {
    motion(&mut world.pos, &world.vel, dt);
    next_event(&world.pos, &world.food, &mut world.pending_event);
}
```

The order is forced: `motion` writes `pos`, `next_event` reads `pos`. Reverse the order and `next_event` reads stale positions.

## Exercise 5 — Add cleanup

```rust,no_run
fn tick(world: &mut World, dt: f32) {
    motion(&mut world.pos, &world.vel, dt);
    next_event(&world.pos, &world.food, &mut world.pending_event);
    cleanup(&mut world.creatures, &mut world.to_remove, &mut world.to_insert);
}
```

`cleanup` reads `to_remove` and `to_insert`, writes `creatures`. Neither `motion` nor `next_event` touches those tables, so `cleanup` runs at the end with no DAG conflicts.

## Exercise 6 — A query planner

A SQL plan for `SELECT u.name FROM users u JOIN orders o ON u.id = o.user_id WHERE o.amount > 100` decomposes into:

1. `scan(orders)` (operation)
2. `filter(amount > 100)` (filter)
3. `join(users, filtered_orders)` (operation, two-input)
4. `project(name)` (operation)

Each is a system with a read-set and a write-set. The plan is a DAG. The simulator's tick is the same shape with the *systems* in `code/sim/SPEC.md` substituting for relational operators. A compiled SQL plan and a simulator tick are isomorphic structures running at different cadences.
