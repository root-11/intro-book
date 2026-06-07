# Solutions: 47 - Observation is a read-only system

## Exercise 1 - A metrics system

A system whose read-set is the world and whose write-set is one `metrics` table:

```rust
fn metrics_system(w: &World, m: &mut Metrics) {
    if w.tick % N != 0 { return; }
    m.tick.push(w.tick);
    m.population.push(w.energy.len() as u32);
    m.mean_energy.push(w.energy.iter().sum::<f32>() / w.energy.len() as f32);
    m.min_energy.push(w.energy.iter().copied().fold(f32::INFINITY, f32::min));
}
```

`Metrics` is SoA like everything else - parallel columns indexed by sample. It is a time series you can plot, query, or serialise with the [§36](36_persistence_is_serialization.md) machinery unchanged.

## Exercise 2 - Prove it is read-only

```rust
let a = run(world.clone(), 1000, &[motion, hunger, cleanup, metrics_system]);
let b = run(world.clone(), 1000, &[motion, hunger, cleanup]); // no metrics
assert_eq!(hash_world(&a), hash_world(&b));
```

The hashes match because `metrics_system` writes only `Metrics`, which is disjoint from the world's tables ([§31](31_disjoint_writes_parallelize.md)). A mismatch means the observer wrote a world column it should only have read - the most common slip is "normalising" or "clamping" a value while sampling it. Read, never write.

## Exercise 3 - Measure the thermometer

Time the tick with and without `metrics_system`: a read-plus-reduce-plus-append over hot columns is lost in the noise of the tick. Now make it compute a true median:

```rust
let mut e = w.energy.clone(); e.sort_by(|a,b| a.partial_cmp(b).unwrap()); // anti-pattern: bad!
m.median_energy.push(e[e.len()/2]);
```

The clone-and-sort is O(n log n) per tick and the reported tick time climbs visibly - the measurement is now changing what it measures. Replace it with a streaming estimate (a P-square quantile, or a coarse histogram updated in one pass) and the budget comes back. The lesson: an observer that is not cheap is not honest.

## Exercise 4 - Trace one creature

```rust
let life: Vec<&Event> = log.iter().filter(|e| e.rid == 17).collect();
```

No new storage: the trace is a filter over the [§37](37_log_is_world.md) log you already keep. The result reads as a sentence - born at tick 4, ate at 9 and 14, became hungry at 20, died at 31. Tracing across the [§35](35_boundary_is_the_queue.md) boundary is the same filter once a trace id rides with the work unit.

## Exercise 5 - Ask a question logs cannot answer as strings

```rust
for w in metrics.population.windows(2) {
    if (w[0] - w[1]) as f32 / w[0] as f32 > 0.10 { report(/* tick */); }
}
```

Over the structured `metrics` table this is a window scan. Over `print!` text it is `grep` plus a parser plus hope: the data is trapped in strings formatted for a human reading one line, not a program reading a million. A string is a dead end; a typed event is a column you can query.

## Exercise 6 - An alert is a system

```rust
fn alert_system(m: &Metrics, a: &mut Alerts) {
    if *m.population.last().unwrap() == 0 { a.fire("population extinct"); }
    if m.tick_ms.iter().rev().take(T).all(|&ms| ms > BUDGET) { a.fire("over budget"); }
}
```

Read-set the `metrics` table, write-set the `alerts` table. The pager is one more read-only system in the same DAG ([§14](14_systems_compose_into_a_dag.md)); nothing about it is special.

## Exercise 7 - Behind the queue

Hand metrics to a writer thread over a channel ([§35](35_boundary_is_the_queue.md), the [§37](37_log_is_world.md) revolver):

```rust
match tx.try_send(sample) {        // never blocks the tick
    Ok(()) => {}
    Err(TrySendError::Full(_)) => { dropped += 1; } // degrade observability, not the sim
}
```

Pause the sink and the tick rate is unchanged; the bounded channel fills and `try_send` drops samples. The chart gets gaps; the simulation does not stall. Contrast with a blocking `send`, which would stall the tick the moment the sink falls behind - trading the system's progress for a measurement of it.

## Exercise 8 - The guaranteed metric

For a counter you must not lose (a billing total, an audit count), apply the [§46](46_log_survives_power_loss.md) rule:

```rust
// advance the durable watermark only after the sink confirms the batch durable
let acked = sink.send_durable(&batch)?;   // returns once the far side fsync'd
watermark = acked.last_offset;            // persist watermark with the world
// on restart: resend everything after `watermark`
```

This is observability that has crossed from lossy to lossless, and it costs what durability always costs: a round-trip and a watermark to persist. Decide it per metric - fire-and-forget for a dashboard gauge, watermarked-and-resent for money. Most metrics are the former; the few that are the latter are not really metrics, they are log records wearing a metric's clothes.
