# 12 - Event time is separate from tick time

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 12](../../concepts/glossary.md#12---event-time-is-separate-from-tick-time).*

Most beginners assume the loop's frequency sets the model's time resolution. If the loop runs at 30 Hz, surely the model can only resolve events at 1/30 s = 33 ms? This is wrong, and the confusion costs many simulations their precision.

<p align="center"><img src="../illustrations/oscilloscope_sine.jpg" alt="An oscilloscope: sample rate is independent of signal frequency" style="max-height: 300px; max-width: 100%;"></p>

The tick rate is *how often the loop runs*. It says nothing about what the loop does inside one tick. Inside one tick, the loop can process events at arbitrary timestamps - microsecond, picosecond, whatever the data carries. The clock lives on the events, not on the loop.

Concretely: a 30 Hz loop receiving 1 000 events per tick, each with microsecond-precision timestamps, processes them in timestamp order - applying each event's effect with the precision the timestamp implies. Output to the rest of the world (rendering, logging, network) happens at 30 Hz, but the *physics inside* runs at microsecond resolution. The tick is a *sampling* rate; the events are the actual phenomena.

This is the model used by:

- **Discrete-event simulators** (queueing networks, traffic, supply chains): events fired at exact times.
- **Game replay systems** (rollback netcode, multiplayer): events arrive late but with their original timestamps.
- **Trade execution engines**: orders carry nanosecond timestamps; the loop processes them in order.
- **Logic simulators** in chip design: gate transitions at picosecond resolution; the simulator advances one transition at a time.

In each case, the tick rate of the host loop is irrelevant to the simulation's resolution. The data carries the time.

This separation is what makes the simulator's `pending_event` table possible. Each tick, the loop builds a list of events that should fire - collisions, eats, reproductions - each tagged with its predicted timestamp. The events fire in timestamp order regardless of which tick they were *predicted in*. A creature that "would have eaten 2 µs into the tick" has its eat applied at that exact moment, not at the start or end of the tick.

The pitfall is hard-coding the tick interval as the simulation's clock granularity. Code that says

```rust,ignore
creature.energy -= 1.0 / 30.0; // "one tick worth of fuel"
```

is conflating the two clocks. The right shape is

```rust,ignore
creature.energy -= elapsed_event_seconds * burn_rate;
```

using the actual elapsed event-time, not the tick interval.

Event time and tick time are decoupled because they answer different questions. Event time answers *when did this thing happen*. Tick time answers *when does the loop wake up*. The same model can be sampled at any tick rate the application needs - visualisation at 30 Hz, recording at 60 Hz, fast-forward replay at 1 kHz - without changing what the model means.

## Exercises

These extend the discrete-event loop from §11 exercise 6.

1. **A tiny event queue.** Use `Vec<(f64, String)>` and `Vec::sort_by`. Push 10 events with random timestamps in `[0, 10]` seconds. Pop them in order; print each as `[t=<sec>] <message>`. Verify the output is timestamp-sorted.
2. **The wrong way: tick-rate clock.** Run a 30 Hz loop. In each tick, advance a counter by `1.0 / 30.0`. Use this counter as your "simulation time". Try to fire an event at `t = 0.005 s` (5 ms). What happens? When does the event fire?
3. **The right way: timestamp on events.** Run the same 30 Hz loop, but each tick pop *all* events with timestamp ≤ current real time, applied in timestamp order. Fire an event at `t = 0.005 s`. Show that the event applies at exactly that time, not at the next tick boundary.
4. **Sampling at different rates.** Run the same model under a 30 Hz loop, then a 60 Hz loop, then a 1 Hz loop. The events should fire at the same simulation times in all three runs (down to whatever precision the loop allows).
5. **Float and time.** What's the smallest time step `f32` can represent for events at `t ≈ 1 hour`? At `t ≈ 1 day`? At `t ≈ 1 year`? When do you need `f64`? (See [§2](02_numbers_and_how_they_fit.md).)
6. *(stretch)* **A budget-aware loop.** Modify your 30 Hz loop: at the start of each tick, pop events until either (a) the queue is empty or (b) you have used 25 ms of the 33 ms budget. Defer remaining events to the next tick. This is the soft-real-time pattern used in interactive simulators.

Reference notes in [12_event_time_vs_tick_time_solutions.md](12_event_time_vs_tick_time_solutions.md).

## What's next

[§13 - A system is a function over tables](13_system_as_function.md) introduces the building block of every tick: the system. Read-set in, write-set out, no hidden state, no surprises.
