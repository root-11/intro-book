# 39 - System of systems

<p align="center"><img src="../covers/phase_system_of_systems.jpg" alt="System of systems phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 39](../../concepts/glossary.md#39--system-of-systems).*

The trunk so far has assumed every system runs every tick and completes within the tick budget. That covers most of what the simulator does - motion, EBP dispatch, cleanup, persistence - and the surrounding chapters earned the assumption. But the assumption is not universal. Practical simulators have at least three classes of work that do not fit it.

- **Optimisation.** A scheduler choosing which tasks each warehouse robot should take next. A combat AI choosing a counter-strategy. A constraint solver finding a feasible plan. These can take seconds or minutes; they cannot fit in a 33 ms tick.
- **Search.** The nearest-task scan for a warehouse operator. A path-finder over a large map. A neighbour query in a million-creature world. Even with [§28](28_proximity.md)'s spatial binning, some searches genuinely take longer than one tick can afford.
- **Out-of-process work.** A game AI evolving its strategy on a separate thread. A pricing model running on a remote server. A precomputation handed off to a worker pool. The simulator never blocks waiting; results arrive when they arrive.

This chapter names the three patterns that cover these cases without breaking any of the trunk's previous rules. They are not new architecture. They are the trunk's existing rules, applied to a wider set of cadences.

The unifying principle: **a system has a cadence, and the cadence does not have to be one tick.** A system can run every tick (motion). It can run every N ticks (the spatial compaction from [§28](28_proximity.md)/[§24](24_append_only_and_recycling.md), which reorders the columns by cell every few dozen ticks). It can have a *deadline* and return its best current answer when the deadline arrives. It can be *suspended and resumed* across ticks, with its progress part of its state. It can be *out-of-loop* entirely, communicating with the simulator only through the queue. The DAG generalises naturally: edges still represent dependencies, but some dependencies wait for promises rather than synchronous returns.

## Anytime algorithms

An *anytime* algorithm produces a valid answer at any time after it has started. The longer it runs, the better the answer. CP-SAT, Monte Carlo Tree Search, evolutionary algorithms, simulated annealing, branch-and-bound - all are anytime. They have a common shape: maintain a *best so far*; refine it as long as time permits; return *best so far* when the budget runs out.

For the simulator, the system call looks synchronous from the trunk's perspective:

```rust,no_run
fn plan_route(world: &World, deadline: Instant) -> Route {
    let mut best = greedy_route(world);
    while Instant::now() < deadline {
        let candidate = improve(&best, world);
        if score(&candidate) > score(&best) { best = candidate; }
    }
    best
}
```

The deadline is the budget. The algorithm respects it. Quality is a function of how much time was available - at 5 ms it is mediocre but valid; at 50 ms it is good; at 500 ms it is near-optimal. The simulator can give it whatever budget the tick allows and never get blocked.

This is [§4](04_cost_and_budget.md) applied to a long computation: the budget is named explicitly, and the algorithm honours it. The student who has internalised the budget calculus already knows how to design these algorithms; the only new vocabulary is the *anytime* contract.

> [!NOTE]
> **Soft real-time, not hard.** The deadline here is a *budget*, not a *guarantee*. An anytime algorithm honours its deadline by returning the best answer it has when the deadline arrives; if that answer is mediocre, the system degrades gracefully but still ships a frame. That is *soft* real-time: a missed deadline costs quality, not correctness. *Hard* real-time is a different discipline. When a missed deadline is a fault - an avionics control loop, a surgical robot, motor control, an AMR's emergency stop that fails to fire in time - you need worst-case execution time (WCET) analysis, bounded jitter, no allocation and no syscall in the inner loop, and a scheduler that can enforce deadlines (`SCHED_DEADLINE`, priority-inversion avoidance). The trunk's tick budget is soft by construction. Building a hard-real-time controller on these ideas is real work the book does not do; for that frontier, start with the WCET and `SCHED_DEADLINE` literature.

## Time-sliced computation

Some work cannot be made anytime - there is no "best partial answer" until the work is complete. A spatial search that has examined 20 % of the cells has a 20 % chance of having found the answer; otherwise it has nothing useful to report. For these, the pattern is *time-slicing*: divide the work across many ticks, with the system's *progress* as part of its persistent state.

```rust,no_run
struct SpatialSearch {
    target_px: f32, target_py: f32,
    cursor:     usize,            // next cell to examine
    best:       Option<(u32, f32)>, // (creature_id, distance) so far
}

fn step_search(s: &mut SpatialSearch, world: &World, max_cells: usize) {
    let end = (s.cursor + max_cells).min(world.cells.len());
    for cell in s.cursor..end {
        for &id in &world.cells[cell] {
            let i = id as usize;
            let d = distance(world.creatures.px[i], world.creatures.py[i], s.target_px, s.target_py);
            if s.best.map_or(true, |(_, prev)| d < prev) {
                s.best = Some((id, d));
            }
        }
    }
    s.cursor = end;
}
```

Each call examines `max_cells` cells. The simulator runs `step_search` every tick (or every N ticks); progress accumulates in `cursor` and `best`; when `cursor` reaches the end, the search is complete and the result is delivered. From the simulator's perspective, the search is one system that takes its budget every tick until done.

This is [§15](15_state_changes_between_ticks.md) applied to a long computation: the system's *state at tick start* includes its in-progress work. The buffering rule that lets every system see consistent input also lets a system pick up where it left off.

## Out-of-loop computation

For work that is genuinely too large for *any* tick budget - a game AI re-planning its grand strategy, an offline machine-learning model, a remote optimisation service - the pattern is *out-of-loop*: the work runs on a separate thread, process, or machine, completely outside the simulator's tick. The simulator never blocks. When the work completes, its result enters the simulator through the input queue ([§35](35_boundary_is_the_queue.md)) like any other input event.

```rust,no_run
// Out-of-loop, on a worker thread:
fn ai_planner_thread(snapshot_rx: Receiver<WorldSnapshot>, result_tx: Sender<InputEvent>) {
    while let Ok(snapshot) = snapshot_rx.recv() {
        let strategy = compute_counter_strategy(&snapshot); // could take seconds
        let _ = result_tx.send(InputEvent::StrategyUpdate(strategy));
    }
}

// Inside the simulator's tick:
fn dispatch_ai(world: &World, snapshot_tx: &Sender<WorldSnapshot>) {
    if world.tick % 30 == 0 { // every second at 30 Hz
        let _ = snapshot_tx.try_send(snapshot_of(world));
    }
}
```

The simulator dispatches a snapshot every second; the AI thread chews on it; the strategy update lands in the input queue some time later. The strategy might be three ticks late, or three seconds late - the simulator does not know and does not care. The result is just one more input event; the queue mechanism is the same.

This is [§35](35_boundary_is_the_queue.md) applied to a long computation: anything that crosses the boundary takes its own time, and the queue absorbs the latency. The discipline is not to wait - never block the tick on an out-of-loop result.

## Hierarchical scheduling

Production simulators usually combine these patterns. Game engines run physics at 60 Hz (every-tick), AI at 5 Hz (every-12-ticks), save-game at 0.1 Hz (every-300-ticks), and a strategic planner out-of-loop on a worker. Industrial control loops run inner loops at 1 kHz and outer loops at 10 Hz. The DAG generalises: each system is annotated with its cadence; the scheduler runs each according to its frequency or trigger; the result is a *system of systems* - one architecture, many cadences.

The chapter is constructive: it names the three patterns and shows where each fits the simulator's existing structure. The next phase, *Discipline*, addresses what comes after: how to keep the architecture working as it ages, as people leave, as requirements change. *Making it work* is this chapter; *keeping it working* is the four chapters that follow.

## Exercises

1. **Audit cadence.** For each system in your simulator, name its cadence. Most are "every tick"; the ones that are not are candidates for the patterns in this chapter. Note any system whose work is currently capped or skipped because it would exceed the budget - these are unmet needs the patterns can serve.
2. **Anytime path-finder.** Implement `plan_route(world, deadline)` for one creature. The function returns the best path found within the deadline. With a 5 ms deadline, time how good the answers are; with 50 ms, how much better. Plot quality vs deadline.
3. **Time-sliced spatial search.** Implement `SpatialSearch` and `step_search` as in the prose. Run it across multiple ticks, advancing the cursor by a budget-bounded `max_cells` each tick. Verify the result is identical to a single-pass search done in one go.
4. **Out-of-loop AI.** Spawn a worker thread that receives world snapshots and returns strategy updates via channels. Dispatch a snapshot every second; let the worker take 5 seconds; observe that the simulator's tick rate is unaffected and the strategy update lands at the queue when ready.
5. **Mixed cadence.** Run your simulator with motion at every tick, the spatial compaction every 50 ticks, snapshot every 1000 ticks, and a (mock) AI thread updating strategy out-of-loop. Verify that determinism still holds: same seed plus same input queue produces identical hashes after 1000 ticks.
6. *(stretch)* **Anytime under varying budget.** Modify the path-finder so its caller passes the *remaining* tick budget each time. Some ticks have plenty of budget; some have very little. The path-finder still returns a valid answer in every case, and the answers improve when the budget allows. Plot quality over time as the simulator runs.

Reference notes in [39_system_of_systems_solutions.md](39_system_of_systems_solutions.md).

## What's next

[§40 - Mechanism vs policy](40_mechanism_vs_policy.md) opens *Discipline*: the rules that hold the architecture together over time. Where this chapter was about *making* the system work for problems that don't fit the standard tick, the next four chapters are about *keeping* it working as it ages.
