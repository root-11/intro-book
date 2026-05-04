# Solutions: 20 — Empty tables are free

## Exercise 1 — Time the empty case

```rust,no_run
let hungry: Vec<u32> = Vec::new();
let id_to_slot: Vec<u32> = (0..1_000_000).collect();
let mut energy = vec![100.0f32; 1_000_000];

let start = Instant::now();
drive_hunger_ebp(&hungry, &id_to_slot, &mut energy, 0.033);
println!("{:?}", start.elapsed()); // typically 50-200 ns
```

Function call overhead plus slice creation. The for loop never executes. The overhead is in the noise of the measurement.

## Exercise 2 — Time the flag-only case

```rust,no_run
let is_hungry = vec![false; 1_000_000];
let mut energy = vec![100.0f32; 1_000_000];

let start = Instant::now();
drive_hunger_filtered(&is_hungry, &mut energy, 0.033);
println!("{:?}", start.elapsed()); // typically 1-2 ms
```

A million slots are walked even though every flag is false. The branch is correctly predicted (`if false` every time), but the cache lines for `is_hungry` are still loaded — that is the bandwidth cost. ~1 MB of bytes moved through L3 to RAM and back.

The ratio between the two cases is roughly 10⁴-10⁵: the empty EBP case is tens of thousands of times faster than the all-false flag case.

## Exercise 3 — Cost-per-active plot

|  hungry.len() | EBP time |
|--------------:|---------:|
|             0 |  ~100 ns |
|           100 |  ~500 ns |
|         1 000 |    ~2 µs |
|        10 000 |   ~20 µs |
|       100 000 |  ~200 µs |
|     1 000 000 |    ~3 ms |

(The 1M case is bandwidth-bound and may even cross to slower-than-filtered, since `id_to_slot` lookups become random reads at that scale.) The line is roughly linear with a fixed-cost intercept of ~100 ns; the slope is ~2 ns/active-row.

## Exercise 4 — Multi-state, mostly idle

In a tick where most creatures are in `idle` and the other tables are empty:

- `drive_idle(idle.len() = 999 000)` does the work.
- `drive_hunger(hungry.len() = 0)`: ~100 ns.
- `drive_sleep(sleepy.len() = 0)`: ~100 ns.
- `drive_mating(mating.len() = 0)`: ~100 ns.
- `drive_fighting(fighting.len() = 0)`: ~100 ns.

Total: roughly the cost of `drive_idle` plus 400 ns of overhead from four empty systems. The four idle systems contribute essentially nothing. A flag-based equivalent with five flag fields per creature would walk all five flags for every creature — five times the bandwidth, regardless of activity.

## Exercise 5 — Activity histogram

After 1000 ticks:

```text
tick   hungry  sleepy  mating  fighting  idle
0           0       0       0         0  1000
1          14       0       0         0   986
2          27       3       0         0   970
...
50         98      48      12         3   839
51         95      44      11         2   848
...
```

Plotted, the lines are mostly flat with small bumps at events (a wave of births, a famine, a reproductive bloom). The plot is the simulator's *vital signs*. Anomalies show up as sudden spikes; anomalies that do not show up here probably do not exist as state changes worth caring about.

## Exercise 6 — Idle systems removed?

Removing an empty system from the DAG looks superficially like a win — saves the function-call overhead — but is wrong on three fronts:

1. **Dynamic scheduling cost.** Adding/removing systems each tick requires the scheduler to reconsider the DAG. This is more expensive than running the empty system.
2. **Determinism break.** A system that disappears in tick N and reappears in tick N+1 has a different DAG between the two ticks; replay must reproduce both DAGs exactly, adding state to the replay system that has nothing to do with the simulation.
3. **Tick-boundary atomicity.** If the table fills *during* a tick (via `to_insert`, applied at cleanup), the system that processes it must already be in the schedule for next tick. Removing it would lose the work.

The fixed cost of an empty EBP system — single-digit microseconds at most — is far cheaper than any of these. Idle systems stay; tables empty out and fill up; the schedule does not move.
