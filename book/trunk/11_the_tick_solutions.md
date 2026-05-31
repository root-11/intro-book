# Solutions: 11 - The tick

## Exercise 1 - A 30 Hz time-driven loop

```rust,no_run
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(33); // ~30 Hz

fn main() {
    let program_start = Instant::now();
    let mut tick = 0u64;
    loop {
        let tick_start = Instant::now();
        println!("tick {tick} at {:?}", program_start.elapsed());
        tick += 1;
        if tick >= 300 { break; }

        let elapsed = tick_start.elapsed();
        if elapsed < TICK {
            std::thread::sleep(TICK - elapsed);
        }
    }
}
```

Expected: 300 iterations, total wall time ≈ 10 s. Sleeping the remainder of each tick keeps the rate stable; sleeping a fixed `33 ms` regardless of work would let drift accumulate.

## Exercise 2 - The naive sleep mistake

`thread::sleep(Duration::from_millis(33))` sleeps *33 ms in addition to* the work the loop did. If each iteration's work takes 5 ms, the total period is 38 ms = 26 Hz, not 30 Hz. Over 30 seconds the program runs ~780 iterations instead of 900. The drift is the work-per-tick, multiplied by the number of ticks.

## Exercise 3 - Dropped frames

Compare `tick_start.elapsed()` against `TICK`. If it exceeded `TICK`, the loop has missed a frame:

```rust,no_run
let work = tick_start.elapsed();
if work > TICK {
    eprintln!("missed frame: work {work:?} > tick {TICK:?}");
} else {
    std::thread::sleep(TICK - work);
}
```

A 50 ms sleep blows the 33 ms budget by 17 ms - every tick logs a missed-frame warning. Real simulators count and surface this metric: it is the most direct sign you have left your real-time budget.

## Exercise 4 - A turn-based loop

```rust,no_run
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        write!(stdout, "> ").unwrap();
        stdout.flush().unwrap();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap() == 0 { break; }
        println!("you said: {}", line.trim());
    }
}
```

The loop's pace is your typing speed, not the system clock. Each line is exactly one tick.

## Exercises 5-6 - Sketches

**Exercise 5.** Two clean approaches: spawn a thread that prints once per second using `mpsc::channel` with a one-second timeout, or use `mio`/`tokio` for non-blocking stdin. Either works; both add code that has nothing to do with the original logic. The lesson is in the friction.

**Exercise 6.** Maintain `events: Vec<(f64, String)>` sorted by timestamp (or use a `BinaryHeap`). Pop the smallest, advance `sim_time` to that timestamp, print, repeat. Wall-clock time and `sim_time` decouple completely - the loop processes events as fast as it can; `sim_time` advances in jumps determined by the data.
