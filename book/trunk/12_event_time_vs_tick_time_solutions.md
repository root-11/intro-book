# Solutions: 12 - Event time vs tick time

## Exercise 1 - A tiny event queue

```rust
fn main() {
    let mut state: u64 = 0xC0FFEE;
    let rand = |s: &mut u64| { *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); *s };

    let mut events: Vec<(f64, String)> = Vec::new();
    for i in 0..10 {
        let t = (rand(&mut state) >> 32) as f64 / u32::MAX as f64 * 10.0;
        events.push((t, format!("event #{i}")));
    }
    events.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    for (t, msg) in events {
        println!("[t={t:.4}] {msg}");
    }
}
```

The events come out timestamp-sorted, even though they were generated in arbitrary order. The sort is the entire trick.

## Exercise 2 - The wrong way

A 30 Hz counter advances by `1.0 / 30.0 ≈ 0.0333`. Asking it to fire an event at `t = 0.005` either fires the event at `t = 0.0333` (the first tick boundary that crosses 0.005) or skips it entirely. Either way, the model has lost 28 ms of resolution.

## Exercise 3 - The right way

Inside the 30 Hz loop:

```rust,no_run
let real_now = program_start.elapsed().as_secs_f64();
while events.first().map(|e| e.0 <= real_now).unwrap_or(false) {
    let (t, msg) = events.remove(0);
    println!("[event t={t:.6}] {msg}"); // applies at t, not at real_now
}
```

The event at `t = 0.005` fires inside whichever tick has `real_now >= 0.005` - the first one - and the printed `t` is `0.005`, not the tick boundary. The simulation time is what the data says.

## Exercise 4 - Sampling at different rates

Run the same event list through three loops at 30 Hz, 60 Hz, 1 Hz. The events fire at the same `t` values in all three runs. Only the wall-clock time at which they fire differs. The model is invariant under tick rate.

## Exercise 5 - Float and time

`f32` has ~7 significant decimal digits. At `t ≈ 1 hour = 3600 s`, the smallest distinguishable step is roughly `3600 / 10^7 = 0.00036 s = 360 µs`. At `t ≈ 1 day = 86400 s`, ~8.6 ms. At `t ≈ 1 year ≈ 3.15 × 10^7 s`, ~3.2 s - `f32` cannot represent millisecond resolution at year-scale.

`f64` has ~15-16 significant digits. At one year, the smallest step is microseconds. For any simulation longer than a few hours of real time at sub-millisecond resolution, use `f64` for timestamps.

## Exercise 6 - Budget-aware loop

```rust,no_run
let tick_start = Instant::now();
let budget = Duration::from_millis(25);
while events.first().map(|e| e.0 <= sim_now).unwrap_or(false) {
    if tick_start.elapsed() > budget { break; }
    let (t, msg) = events.remove(0);
    apply(msg);
}
```

Events that did not fit are still in the queue; they fire in the next tick. The model degrades gracefully under load instead of stalling the loop.
