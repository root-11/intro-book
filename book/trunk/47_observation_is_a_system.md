# 47 - Observation is a read-only system

[§46](46_log_survives_power_loss.md) made the log survive the stop: the system comes back from a crash to a world that existed. The next thing the missing human took with them is softer and just as fatal - knowing what the system is *doing*. [§13](13_system_as_function.md) said the data is visible: `print!` any column and look. That is true and useful, and it is a *debugger's* answer - you, at your desk, world paused, stepping through one moment. At 2 AM the world is not paused, you are not at your desk, and there is no `print!` you can add to a process that is already running and already wrong.

The reflex is to reach for a logging library and bolt strings onto the hot path. Resist it. Observability is not a thing you sprinkle on a system; *it is a system*, in the exact sense the book has used the word since [§13](13_system_as_function.md). A function over tables. Its read-set is the world; its write-set is a small set of tables it owns and nothing else touches. Everything built for simulation systems applies to it unchanged, and that reuse is the whole trick.

Two properties fall out of "it is a read-only system," and both are the line between observability that works and observability that lies.

**It cannot perturb what it measures.** A system whose write-set is disjoint from the world's tables ([§31](31_disjoint_writes_parallelize.md)) cannot change the world by reading it. A metrics system that only appends to its own table leaves the simulation bit-identical to a run with no metrics at all - provably, by the same write-set discipline that lets systems parallelise. This is the observer effect, designed out: contrast a debugger that mutates state to inspect it, or a profiler that rewrites the hot path. Read-only is not a courtesy here; it is what makes the measurement trustworthy.

**It must be cheap, or the cost becomes the measurement.** Reading columns is a sequential scan, the cheapest pass there is ([§7](07_structure_of_arrays.md), [§27](27_working_set_vs_cache.md)). A per-tick metrics sample is a handful of reductions over columns that are already hot. Keep it to a read, a reduce, and an append and it disappears against the tick budget ([§4](04_cost_and_budget.md)). Let it grow teeth - a sort to find a median, a full extra pass - and it starts heating the water it is trying to take the temperature of: the timing it reports is now the timing *with the thermometer in it*.

The three views are all that one read-only-system shape, pointed three ways.

**Metrics** - aggregate numbers over time. A system that each tick (or every N) reduces the world's columns to a row of scalars - population, mean energy, tick duration, queue depths - and appends it to a metrics table. That table is a time series: the same SoA columns, indexed by tick. "How fast is the population changing" becomes a column you read, not a number you wish you had sampled.

**Traces** - one thing across many systems. The [§37](37_log_is_world.md) log already records per-entity events; a trace is the log filtered to one `rid` across a tick, or one unit of work across the [§35](35_boundary_is_the_queue.md) boundary when a trace id rides along with it. Tracing is a query, not new machinery.

**Structured logs** - typed events you can ask questions of. Not `print!("creature {id} died")` but the event itself (the [§37](37_log_is_world.md) record), queryable: every `DIE` for creature 17, every tick where population fell more than ten percent. A string is for a human reading one line; a structured event is for a system reading a million.

**Alerting** is the fourth view, and again just a system: read-set the metrics table, output a threshold crossing. "Population reached zero." "Tick budget exceeded for a hundred ticks." The thing that wakes you is a read-only system reading a table that another read-only system wrote.

One inversion of the [§46](46_log_survives_power_loss.md) rule, stated so it is not missed. The log is **lossless** because the world depends on it: drop a record and the world is wrong, so you pay the `fsync` and you do not acknowledge until it is durable. Metrics are **lossy by choice** because the world does *not* depend on them. The pipeline that ships metrics across the boundary runs behind the queue ([§35](35_boundary_is_the_queue.md), the [§37](37_log_is_world.md) revolver), and if the sink is slow or down you **drop samples; you never stall a tick to emit one**. Backpressure on observability degrades observability, not the system. The §46 acknowledgement rule still binds anything you *claim* delivered - a billing counter is not lossy - but the default for a metric is fire-and-forget: a missing sample is a gap in a chart, a stalled tick is an outage. Never trade the system's progress for a measurement of it.

The leverage lands at 2 AM. You do not add observability during the incident; the read-only systems were already running, costing nothing, writing the history that answers the question before you knew to ask it. The difference between an outage and a glance is whether the three numbers you need are already a table you can read.

The exclusion, named: observability is not debugging. A debugger stops the world and inspects one instant at full fidelity; observability never stops the world and records its whole history at low fidelity. You reach for the debugger at your desk, for observability when you cannot. Neither replaces the other.

## Measurements

The observer's cost is a sequential read plus a reduction plus an append - the cheapest pass there is ([§7](07_structure_of_arrays.md), [§27](27_working_set_vs_cache.md)) - so the claim is that a per-tick metrics system is ~free against the tick budget; the exercise measures the tick with it on and off. The correctness claim is non-perturbation: the world hash is identical with metrics on and off (disjoint write-set), a pass/fail test, not a number. A four-machine overhead table follows once the metrics system is a specimen, as in `code/logger`.

## Exercises

1. **A metrics system.** Add a system that every N ticks reduces the world's columns to one row - population, mean energy, min/max energy, tick duration - and appends it to a `metrics` table. The table is a time series with the same SoA shape as the world.
2. **Prove it is read-only.** Hash the world after 1 000 ticks with the metrics system running, and again with it removed. The hashes must match ([§31](31_disjoint_writes_parallelize.md)): a disjoint write-set cannot perturb the world. If they differ, find the column the observer wrote that it should not have.
3. **Measure the thermometer.** Time the tick with metrics on and off; show the cost is ~free. Then make the metrics system sort the energy column to report a median every tick, and watch the reported tick time climb - the measurement now changes the thing measured. Replace the sort with a streaming estimate and recover the budget.
4. **Trace one creature.** Query the [§37](37_log_is_world.md) log for a single `rid` across 100 ticks and reconstruct its life: born, ate, became hungry, died. Note that this needs no new storage - it is a filter over the log you already keep.
5. **Ask a question logs cannot answer as strings.** Find every tick where the population fell by more than ten percent. Do it over the structured event table; then argue why the same query over `print!` text output is grep-and-pray, not a query.
6. **An alert is a system.** Add a system whose read-set is the `metrics` table and whose output fires when population hits zero, or when tick duration exceeds the budget for T consecutive ticks. The pager is one more read-only system.
7. **Behind the queue.** Ship the metrics out to a file or socket through the [§35](35_boundary_is_the_queue.md) queue. Pause the sink mid-run. Show the tick rate is unaffected and samples are dropped, not stalled - observability degraded, system intact.
8. *(stretch)* **The guaranteed metric.** For a counter you must not lose - a billing total, an audit count - apply the [§46](46_log_survives_power_loss.md) rule: do not advance the "reported" watermark until the sink confirms the batch durable, and on restart resend from the watermark. Contrast its cost with the fire-and-forget default, and decide per metric which one it is.

Reference notes in [47_observation_is_a_system_solutions.md](47_observation_is_a_system_solutions.md).

## What's next

The system now survives the stop and reports what it is doing. The next unattended failure is quieter than either: it gives a *different answer* on a different machine. [§48](48_reductions_dont_parallelize_freely.md) takes the determinism the first act earned ([§16](16_determinism_by_order.md)) into the place it most easily breaks - a parallel reduction whose result depends on the core count - so that "same seed, same world" survives the move from your laptop to the server you have never seen.
