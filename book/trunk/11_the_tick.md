# 11 - The tick

<p align="center"><img src="../covers/phase_time_passes.jpg" alt="Time & passes phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 11](../../concepts/glossary.md#11---the-tick).*

A program's life has a shape:

- **Start-up** - initialisation. Tables are allocated, inputs are opened, the RNG is seeded, the world reaches a known state.
- **Steps** - ticks of the clock in a simulation, turns in a card game, event handlers in a server. The repeating unit of forward motion.
- **Save and load** - the in-memory state is preserved to disk so a future run can resume from where this one left off. Optional, but if you want it, it lives here.
- **Exit** - resources are returned to the kernel. Memory, file handles, sockets, lockfiles. Failure to do this cleanly is called a *memory leak* (or a stale lock, or a broken socket).

This section is about the step. The step is where the time budget binds, where the [systems](13_system_as_function.md) - the functions that read and write the world's tables - run in the order set by their [DAG](14_systems_compose_into_a_dag.md), where determinism either holds or breaks. The other phases are real and important - the book returns to save and load when persistence is named at [§36](36_persistence_is_serialization.md), and exit is mostly the operating system's job - but the inner step is what makes or breaks every other property the book builds on.

Each step is a *tick*. State at the start of a tick is read; state at the end is written; nothing is half-updated mid-tick. Even an interactive program - a card game waiting for the next move, a text editor waiting for a keystroke - is a tick loop, just with an external trigger driving it. A program that does a single pass over a file and exits is a degenerate tick loop with one tick: it has the same start-of-tick / end-of-tick contract, just with N=1.

Ticks come in two natural shapes.

A **time-driven** tick fires at a fixed rate. The simulator from [`code/sim/SPEC.md`](../../code/sim/SPEC.md) runs at 30 Hz: one tick every 33 ms. The loop wakes up, advances every system by one step, sleeps until the next tick. Most simulations, games, control loops, audio engines, and animation systems are time-driven. The rate is a contract with the rest of the world: at this rate, output appears.

A **turn-based** tick fires when an event arrives. A card game ticks when a player makes a move. A chess engine ticks when its opponent moves. A discrete-event simulator ticks at the timestamp of the next pending event, however far in the future that is. The clock advances *with* the events, not under them. Turn-based ticks have no fixed rate; their pace is set by the input stream.

Both are ticks. The difference is what triggers the next pass:

```rust,no_run
// time-driven
use std::time::{Duration, Instant};
const TICK: Duration = Duration::from_millis(33);

loop {
    let start = Instant::now();
    run_all_systems(&mut world);
    let elapsed = start.elapsed();
    if elapsed < TICK {
        std::thread::sleep(TICK - elapsed);
    }
}
```

```rust,no_run
// turn-based
loop {
    let event = wait_for_next_event();
    apply_event(&mut world, event);
}
```

The §0 simulator runs time-driven. The card game from §5 ran turn-based - every card you dealt was one tick. Both are valid; both fit the same framework.

Within each tick, the systems run in an order specified by the system DAG ([§14](14_systems_compose_into_a_dag.md)'s topic). Each tick has a *budget*: 33 ms at 30 Hz, the ms-per-move in a card game played at human speed. The budget binds the design: at 30 Hz with 1 000 000 creatures, each motion update has 33 nanoseconds, which only fits if the data layout cooperates ([§4](04_cost_and_budget.md) made this precise).

When a tick runs long, the budget has a visible failure mode: the frame is *dropped*. The loop wakes late, the rate sags below its contract, and downstream something stutters. That visibility is worth instrumenting from the first day, because it is the cheapest operations tool you will ever have. Time each tick, and when it overruns the budget, *raise* in development so a regression stops the build, and *warn and count* in production so a degraded run is recorded rather than silent. The count of late ticks is the first number an operator reads when asked whether the thing is keeping up. The book returns to this in Part II as the front of the operations toolkit; here it is just a habit, that a tick which can blow its budget should be able to say so.

A subtle pitfall worth naming. Mixing turn-based and time-driven thinking in the same loop produces *drift*: the turn-based subsystem's pace bleeds into the time-driven subsystem's budget. The fix is to keep the two cleanly separated - typically, one outer loop and the other as an event source feeding it.

A tick is the unit of forward motion in any program that has forward motion. The next sections name what *fits* in one tick, in what order, and what does not.

## Exercises

You will need a minimal Rust project for these. `cargo new tick_lab` is enough.

1. **A 30 Hz time-driven loop.** Write a `main` that loops at 30 Hz. Each iteration, print the elapsed time since program start. Sleep between ticks to maintain the rate. Run it for 10 seconds. Did you actually get 300 iterations?
2. **The naive sleep mistake.** Replace your sleep logic with `std::thread::sleep(Duration::from_millis(33))` (no measurement). Run for 30 seconds. Does the program drift over time? Why?
3. **Dropped frames.** Inside the loop, sleep for 50 ms - longer than the budget. The loop is now running at 20 Hz; it has *missed frames*. Print a warning when this happens, and keep a running count of late ticks - that count is the first number an operator reads when asked whether the loop is keeping up.
4. **A turn-based loop.** Write a tiny REPL: print `> `, read a line, print `you said: <line>`. Each line is one tick. Run it. Note that the loop has no fixed rate - its pace is your typing.
5. **Mixing the two.** Modify exercise 4 so that, while waiting for input, the program also prints the current second once per second. (Hint: spawn a thread, use a non-blocking read, or interleave with timeouts.) Note how mixing the two patterns adds complexity quickly.
6. *(stretch)* **A discrete-event tick loop.** Maintain a `Vec<(f64, String)>` of `(timestamp, message)` events. Pop the smallest-timestamp event, advance a "simulation clock" to that timestamp, print the message, repeat until the queue is empty. This is the structure of a discrete-event simulator and a preview of [§12](12_event_time_vs_tick_time.md).

Reference notes in [11_the_tick_solutions.md](11_the_tick_solutions.md).

## What's next

Exercise 6 hints at the next section. The clock can live on the events themselves, independent of how often the loop fires. [§12 - Event time is separate from tick time](12_event_time_vs_tick_time.md) names that separation.
