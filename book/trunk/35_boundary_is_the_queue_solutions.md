# Solutions: 35 - The boundary is the queue

## Exercise 1 - Build the queues

```rust,no_run
struct InputEvent  { tick: u64, kind: u8, payload: u64 }
struct OutputEvent { tick: u64, kind: u8, payload: u64 }

struct World {
    // ... tables ...
    in_queue:  Vec<InputEvent>,
    out_queue: Vec<OutputEvent>,
}

fn tick(world: &mut World, current_time: f64) {
    let inputs: Vec<_> = world.in_queue.drain(..).collect();
    // pure transformation - no other I/O
    next_event(/* ... */);
    motion(/* ... */, current_time);
    // ... etc; outputs accumulated into world.out_queue
}
```

The two queues are the boundary. Inputs arrive via `in_queue.push()`. Outputs leave via `out_queue.drain()`. Inside the tick, only the inputs and outputs cross the seam.

## Exercise 2 - Refactor `Instant::now()`

Before: every system that needs time calls `Instant::now()` directly. Multiple systems → multiple non-deterministic readings.

After: the tick driver reads `Instant::now()` once, computes `current_time: f64`, passes it to every system as a parameter. The systems are pure functions of their inputs.

```rust,no_run
let now = Instant::now();
let current_time = (now - sim_start).as_secs_f64();
tick(&mut world, current_time);
```

Replay can substitute a recorded `current_time` instead of reading the wall clock. The simulator's behaviour is identical.

## Exercise 3 - Refactor `println!`

Before:
```rust,ignore
fn apply_starve(...) {
    if energy[i] <= 0.0 {
        println!("creature {} starved", id[i]);
    }
}
```

After:
```rust,no_run
fn apply_starve(..., out: &mut Vec<OutputEvent>) {
    if energy[i] <= 0.0 {
        out.push(OutputEvent { tick, kind: STARVED, payload: id[i] as u64 });
    }
}
```

The system pushes to `out_queue` instead of writing stdout. The tick driver reads the queue after the tick and prints whatever is there. Logging is now deterministic; tests assert on the queue.

## Exercise 4 - Replay test

```rust,no_run
let saved_inputs: Vec<Vec<InputEvent>> = run_and_record(&mut world1, 100);
let mut world2 = init_world(seed);
for inputs in saved_inputs {
    world2.in_queue.extend(inputs);
    tick(&mut world2, /* recorded current_time */);
}
assert_eq!(hash_world(&world1), hash_world(&world2));
```

If the boundary is respected, `world2` after replay matches `world1` after the live run. If they differ, somewhere a system reads outside the queue.

## Exercise 5 - Two simulators from one queue

Same structure as exercise 4, but feed two simulators (in parallel or sequentially) from the same recorded inputs. Hash both worlds at tick 100. They must match. If they don't, the difference traces back to one (or more) system reading outside the queue.

## Exercise 6 - Audit a real simulator

Common findings in production code:

- `tokio::time::Instant::now()` inside a request handler - pulls wall time into the per-request transform.
- `tracing::info!` with side-effecting log macros - couples the system to the tracing infrastructure.
- `tokio::fs::File::open` reads - couples the system to the filesystem.
- `env::var` calls - couples the system to the OS environment.
- `rand::thread_rng()` - pulls non-deterministic randomness into the per-tick transform.

Each is a place where determinism leaks. Each could be queue-ified.
