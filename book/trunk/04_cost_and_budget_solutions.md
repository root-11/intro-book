# Solutions: 4 - Cost is layout, and you have a budget

## Exercise 1 - Picking rates

| system | plausible rate | budget |
|---|---|---|
| card game | turn-based; budget per move maybe 100 ms (responsive feel) | - |
| real-time strategy game | 30-60 Hz | 17-33 ms |
| market data feed | 1 kHz to 100 kHz | 10 µs to 1 ms |
| embedded sensor controller | 1-10 kHz | 100 µs to 1 ms |
| web API endpoint user is waiting for | rate is per-request; budget ~50-200 ms is "fast" | - |
| offline batch over 1B rows | not real-time; budget set by total time, e.g. "complete in under 1 hour" |

The lesson is: every system has a rate, even ones not described as "real-time". Naming it makes the budget visible.

## Exercise 2 - Count an operation

`HashMap::get` on 1M entries: typically 50-150 ns. Pick the middle: 100 ns = 0.1 µs. In a 30 Hz tick (33 333 µs) you can fit 33 333 / 0.1 ≈ 330 000 lookups. In a 1 kHz tick (1 000 µs) you can fit 10 000.

If your "for each entity, look up something" loop has 1 000 000 entities, neither tick budget fits - you must restructure (a sorted index, a join, a column lookup) or accept a slower rate.

## Exercise 3 - The layout difference

`Vec<u64>` sum: ~1 ns/elem.
`HashMap<u32, u64>` sum: ~50-100 ns/elem.

50-100× difference for the same total work. Most of it goes to memory: hash maps have one cache line per bucket, the buckets aren't sequential, the hash itself touches more bytes per access. Big-O is the same; constant factor decides.

## Exercise 4 - The cliff

For a typical desktop with 1 MB L2:

- 100 000 `u64` = 800 KB → fits in L2 → ~1 ns/elem.
- 1 000 000 `u64` = 8 MB → spills L2 into L3 → ~2-4 ns/elem.

Compute the fraction of the budget by ratio. At 30 Hz (33 333 µs), 1M elements at 1 ns each is 1000 µs ≈ 3% of the budget. At 4 ns each, 4000 µs ≈ 12% of the budget. The cliff is real.

## Exercise 5 - Working backwards

60 Hz tick: 16 666 µs = 16 666 000 ns.
100 000 entities × one cache line each. A cache line takes ~1-3 ns to load if sequential (L2/L3) or ~20-100 ns if random (RAM).

Sequential: 100 000 × 2 ns = 200 µs = 1.2% of the tick. Lots of headroom.
Random RAM: 100 000 × 50 ns = 5 000 µs = 30% of the tick. Tight but possible.
Random pointer-chase to scattered allocations: 100 000 × 100 ns = 10 000 µs = 60% of the tick. One inner loop, sixty percent. No headroom for anything else.

The lesson: ask whether the access is sequential before estimating.

## Exercise 6 - A bad design

The classic: a graph of `Box<Node>` allocated by many small calls to `Box::new`, then iterated by following pointers. Each node is a separate heap allocation, scattered across the heap by the allocator. A million-node "linked structure" is a million RAM round-trips per traversal: 100 ns × 10⁶ = 100 ms - three full 30 Hz ticks for one traversal.

The same data laid out as a `Vec<Node>` (or, better, as SoA: `Vec<u32>` of values plus `Vec<u32>` of next-indices) traverses in 1-2 ms - fifty times faster, same algorithm, different layout. This is the whole book's premise in one number.

## Exercise 7 - Find your CPU's TDP

Typical 2026 ranges:

- AMD Ryzen 9 mobile (e.g. 7940HS, 8945HS): cTDP 35-54 W (configurable).
- AMD Ryzen 9 desktop (e.g. 9950X): TDP 170 W; PPT (sustained) up to 230 W.
- Intel Core i9-13900H (mobile): PL1 45 W, PL2 115 W.
- Apple M3 Pro: roughly 25-35 W under sustained load.

A "TDP" number is the *sustained* envelope; *burst* (PL2 on Intel, PPT on AMD) can run 1.5-3× higher for tens of seconds before thermal/PPT limits clamp it back. For a sim that runs continuously, the sustained number is the budget that matters.

## Exercise 8 - Battery budget

50 Wh ÷ 8 W = 6.25 hours.
50 Wh ÷ 14 W = ~3.57 hours.

The 75% rise in average draw (8→14 W) cuts battery life by 43%. A layout change that pushes loads to RAM is not a footnote on the time budget; it is a roughly halving of how long the laptop runs on one charge.

## Exercise 9 - Measure delta power

The workload generator at `code/measurement/src/bin/power_loop.rs` takes one argument (`sequential` or `random`) and runs the chosen workload in a tight loop for 45 s - long enough to outlast a 30 s `perf stat` window comfortably. Build once with `cargo build --release --bin power_loop` inside `code/measurement/`, then run from a terminal:

```sh
# Terminal 1: pick a mode
cargo run --release --bin power_loop -- sequential
# (or: cargo run --release --bin power_loop -- random)
```

```sh
# Terminal 2: measure during a fresh start of the workload
sudo perf stat -a -e power/energy-pkg/ -- sleep 30
```

For the idle reading, run the perf command with no workload running. Permissions: `sudo` is usually required; `sudo sysctl kernel.perf_event_paranoid=0` is the alternative if you don't want to keep typing the password.

### Reading the `power_loop` output

The binary prints something like:

```
sequential: summing 10000000 u64 elements in order for 45s
done: 29131 iterations in 45.000658338s - sum = 291310000000 ...
```

From the iteration count you can compute throughput:

```
elements per second = iterations × N / wall time
                    = 29131 × 10_000_000 / 45 s
                    = 6.47 × 10⁹ elements/s

read bandwidth      = elements/s × 8 bytes
                    = 52 GB/s

ns per element      = wall time / total elements
                    = 45 × 10⁹ ns / (29131 × 10⁷)
                    = 0.15 ns/element
```

52 GB/s is close to the practical peak of dual-channel DDR5-5600 (~60 GB/s sustained from a typical workload). The loop is **bandwidth-bound**: the CPU can consume bytes faster than the memory subsystem can deliver them, so the prefetcher and SIMD are saturating the channel. There is essentially no slack left in the sequential read path on this hardware.

The random-access run will show a very different number. Expect the iteration count to drop by a factor of 50-300, because each element costs a full RAM round-trip (~50-100 ns) instead of being delivered in a sequential stream (~0.15 ns).

### Reading the `perf stat` output

`perf stat -a -e power/energy-pkg/ -- sleep 30` prints something like:

```
       56.58 Joules power/energy-pkg/
   30.001766291 seconds time elapsed
```

Compute average watts:

```
average watts = joules / seconds = 56.58 / 30 = 1.89 W
```

That is the package power for the whole 30-second window, including all process activity on the machine. The per-process number is rarely what you want; the package-level number is what your battery, cooling fan, and electricity bill see.

### Typical numbers

- **Idle (trimmed Linux install)**: 2-8 W. A Ryzen 9 mobile on Arch Linux with no background work reports 1.89 W in one of this book's draft sessions - exceptional but real. The cores are spending most of the window in deep C-states.
- **Sequential `power_loop`**: idle + 5-10 W. Bandwidth-bound: memory controller and SIMD units are working hard but the CPU also drops clocks during the prefetcher's brief refills. High utilisation but not thermally maxed.
- **Random `power_loop`**: idle + 10-25 W. Memory subsystem is working similarly hard, but now CPU stalls on every access cannot be filled by the prefetcher. Stalls do not save power if the rest of the chip stays active - clocks remain elevated while waiting for lines.

For perspective, the chip's cTDP is 35-54 W. That is the *sustained* envelope under heavy load, not a soft cap; bursts can briefly exceed it.

### The lesson

Sequential vs random over the *same* data, the *same* size, the *same* arithmetic:

- **~300× difference** in time per element (0.15 ns vs ~50 ns)
- **~2-3× difference** in instantaneous watts
- **~600-1000× difference** in joules per element processed

Notice that the energy ratio is the *product* of the time ratio and the power ratio. Energy is power × time; when a layout choice slows things down *and* draws more watts, the two effects compound multiplicatively. A workload that is 300× slower and 3× more power-hungry is 900× more energy-expensive - *not* 303×. Slow workloads pay twice: once in elapsed seconds, once in watts per second.

Layout-aware programming is power-aware programming. The bandwidth-bound path keeps the chip cool and fast; the latency-bound path keeps it hot and slow. Two paths, one chip, one decision: where does the data sit?

## Exercise 10 - Joules per access

10⁷ sequential `u64` reads: each line carries 8 elements, so 1.25 × 10⁶ line loads. Most are served from the prefetcher's pipeline (effectively L1-priced after the first miss in a stream), so use ~0.5 nJ per element on average. Total: 10⁷ × 0.5 nJ = **5 mJ**.

10⁷ random `u64` reads: every access is a fresh L3-or-RAM miss. Use ~30 nJ per element. Total: 10⁷ × 30 nJ = **300 mJ**.

Ratio: **60×**.

As a fraction of a 50 Wh battery (50 × 3600 = 180,000 J):

- Sequential: 5 × 10⁻³ J / 180,000 J ≈ **2.8 × 10⁻⁸** of a charge - negligible per run.
- Random: 300 × 10⁻³ J / 180,000 J ≈ **1.7 × 10⁻⁶** of a charge - still small per run.

The absolute numbers are tiny; the *ratio* is what scales. Run that loop ten million times a day across a fleet of devices and the choice of layout becomes the difference between a noticeable cooling-fan hum and a silent machine - and, at data-centre scale, a measurable line item on the electricity bill.
