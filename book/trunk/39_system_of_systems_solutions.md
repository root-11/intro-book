# Solutions: 39 - System of systems

## Exercise 1 - Cadence audit

For a typical simulator the breakdown looks like:

| system          | cadence            | reason                              |
|-----------------|--------------------|-------------------------------------|
| motion          | every tick         | physics, the inner loop             |
| food_spawn      | every tick         | per-tick policy                     |
| next_event      | every tick         | event detection                     |
| apply_eat/repro/starve | every tick  | event consumption                   |
| cleanup         | every tick         | mutation commit                     |
| inspect         | every tick         | observation                         |
| sort-for-locality | every ~50 ticks  | amortised cost                      |
| snapshot        | every ~1000 ticks  | persistence checkpoint              |
| AI/strategy     | out-of-loop        | seconds-long computation            |
| route planning  | anytime, per-creature | budget-bounded path search       |
| spatial search  | time-sliced        | bounded `max_cells` per tick        |

The unmet-need column is what the chapter speaks to: any work currently being skipped or truncated is a candidate for one of the three patterns.

## Exercise 2 - Anytime path-finder

```rust,no_run
use std::time::Instant;

fn plan_route(world: &World, start: Pos, goal: Pos, deadline: Instant) -> Route {
    let mut best = greedy_route(world, start, goal);
    let mut iter = 0;
    while Instant::now() < deadline {
        let candidate = local_search_step(&best, world);
        if cost(&candidate) < cost(&best) {
            best = candidate;
        }
        iter += 1;
    }
    eprintln!("plan_route: {iter} iterations, cost {}", cost(&best));
    best
}
```

Typical numbers (illustrative; depends on the map):

- 1 ms deadline: ~10 iterations, ~50% optimal
- 5 ms: ~50 iterations, ~75% optimal
- 50 ms: ~500 iterations, ~95% optimal
- 500 ms: ~5000 iterations, ~99.5% optimal

The shape - diminishing returns over time - is generic to anytime algorithms. The deadline is the budget; quality scales with the budget; the simulator never waits past the deadline.

## Exercise 3 - Time-sliced spatial search

```rust,no_run
struct SpatialSearch {
    target_pos: (f32, f32),
    cursor:     usize,
    best:       Option<(u32, f32)>,
    done:       bool,
}

fn step_search(s: &mut SpatialSearch, world: &World, max_cells: usize) {
    let end = (s.cursor + max_cells).min(world.cells.len());
    for cell in s.cursor..end {
        for &id in &world.cells[cell] {
            let d = distance(world.pos[id as usize], s.target_pos);
            if s.best.map_or(true, |(_, prev)| d < prev) {
                s.best = Some((id, d));
            }
        }
    }
    s.cursor = end;
    if s.cursor == world.cells.len() {
        s.done = true;
    }
}
```

To verify: run the time-sliced version across `K` ticks with `max_cells = total_cells / K`. Compare with a single-pass search. The results must be bit-identical because both visit the same cells in the same order.

## Exercise 4 - Out-of-loop AI

```rust,no_run
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

fn spawn_ai(world_snapshot_rx: Receiver<WorldSnapshot>, ev_tx: Sender<InputEvent>) {
    thread::spawn(move || {
        while let Ok(snapshot) = world_snapshot_rx.recv() {
            let strategy = compute_counter_strategy(&snapshot); // takes seconds
            let _ = ev_tx.send(InputEvent::StrategyUpdate(strategy));
        }
    });
}
```

The simulator's tick:

```rust,no_run
if world.tick % 30 == 0 {
    let _ = world_snapshot_tx.try_send(snapshot_of(&world));
}
// Drain any AI results from the input queue
for ev in world.in_queue.drain(..) {
    apply_input(&mut world, ev);
}
```

Time the simulator's tick rate. With the AI computation taking 5 seconds and the simulator running at 30 Hz, the simulator should sustain its full 30 Hz throughout - the AI thread does not block the tick. The strategy update arrives 5 seconds after the snapshot was sent and lands in the input queue at the tick boundary it arrives at.

## Exercise 5 - Mixed cadence with determinism

The key insight: each cadence is itself deterministic if its trigger is deterministic. "Every 50 ticks" is a deterministic trigger (`if tick % 50 == 0`). An "out-of-loop AI" is harder - its results depend on wall-clock timing and may not be reproducible.

For a deterministic system with out-of-loop work, treat the AI's results as part of the input log. Replay re-feeds the same results at the same ticks they originally arrived. The simulator stays deterministic; the AI's computation is no longer in the loop, but its inputs are.

Test:

```rust,no_run
let mut w1 = init_world(0xCAFE);
let mut w2 = init_world(0xCAFE);
for tick in 0..1000 {
    w1.in_queue.extend(recorded_inputs[tick].iter().cloned());
    w2.in_queue.extend(recorded_inputs[tick].iter().cloned());
    tick_with_mixed_cadence(&mut w1, recorded_time[tick]);
    tick_with_mixed_cadence(&mut w2, recorded_time[tick]);
}
assert_eq!(hash_world(&w1), hash_world(&w2));
```

If determinism holds, the cadences compose; if not, an out-of-loop result is leaking non-determinism that the input queue did not capture.

## Exercise 6 - Anytime under varying budget

```rust,no_run
fn step(world: &mut World, plan_budget: Duration) {
    let deadline = Instant::now() + plan_budget;
    for creature_id in world.planning.iter() {
        let route = plan_route(world, world.pos[*creature_id as usize], world.goal[*creature_id as usize], deadline);
        world.routes[*creature_id as usize] = route;
    }
}
```

The remaining-tick budget is what is left after the higher-priority systems have run. Some ticks: 10 ms remaining, plenty for path-finding. Other ticks: 0.5 ms, only enough for greedy answers. The path-finder returns a valid path in both cases; quality varies; the simulator never blocks.

Plotting `route_quality(t)` over many ticks shows quality oscillating with budget, with steady-state quality reflecting the typical budget. The pattern is the simulator's *response* to load - when the system is busy, planning gets less time; when idle, more time. No system is ever starved or stalled.
