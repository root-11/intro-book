# Solutions: 49 - The worst case is the only case

## Exercise 1 - Measure your jitter

```rust
let period = Duration::from_micros(1000);
let mut next = Instant::now();
let mut samples = Vec::with_capacity(2_000_000);
for _ in 0..2_000_000 {
    next += period;
    while Instant::now() < next {}           // busy-wait to the deadline
    samples.push(Instant::now().duration_since(next)); // lateness
}
```

Plot the distribution of lateness. The mean is tight - a microsecond or two - but the *tail* is not: the p99.9 and the max are tens to hundreds of times the mean on a stock desktop, driven by the OS scheduler, interrupts, and frequency changes, not by your code. The mean is a lie about a hard deadline; only the max tells the truth.

## Exercise 2 - Find the scheduler

Pin to an isolated core and raise the scheduling class:

```sh
# boot with isolcpus=3, then:
taskset -c 3 chrt -f 80 ./sim         # SCHED_FIFO priority 80 on core 3
```

```rust
// and lock pages so a fault never stalls the loop:
unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE); }
```

Re-measure: the tail shrinks markedly. You did not make the average tick faster - you made the *worst* tick smaller by removing the scheduler, the page fault, and the frequency change from the loop's path. That is the only quantity hard real-time scores.

## Exercise 3 - Hunt an unbounded operation

Audit one system for anything without a static bound: a `HashMap` insert that can trigger a rehash, a `Vec::push` that can reallocate, a `format!` that allocates, a `while` whose count depends on data. Each is a spike in the WCET even though it is invisible in the average.

```rust
let mut seen = HashMap::new();        // anti-pattern: bad! rehash is an unbounded spike
// bounded replacement: a pre-sized slot array indexed by entity id
let mut seen = vec![false; capacity];
```

Replace one and re-measure the tail: the spike at the rehash boundary disappears. Bounding the worst case, not speeding the average, is the work.

## Exercise 4 - Allocation is a spike

```rust
let _b = Box::new([0u8; 256]);   // anti-pattern: bad! one heap alloc per tick
```

Add this to the hot loop and the jitter tail grows - the allocator occasionally walks a free list or calls into the OS, and that occasionally lands in your worst-case tick. Remove it and the tail drops back. This confirms the book's no-per-tick-allocation discipline ([§7](07_structure_of_arrays.md) pre-sized columns, [§24](24_append_only_and_recycling.md) recycling) was buying tail latency all along, for free, while you thought you were only buying throughput.

## Exercise 5 - The cache is a worst case

Run one system over data that fits in L1, then over data that misses to RAM ([§27](27_working_set_vs_cache.md)). Do not compare the means - compare the *maxima*. The large working set's worst tick is far above its mean, because a cold miss is the worst case and the cache state is not under your control. A WCET cannot be read off an average-case benchmark; it needs either a measured-and-bounded worst case (with the cache cold, deliberately) or a static analysis that assumes every access misses. Average-case layout tuning makes the mean fast and tells you nothing about the ceiling.

## Exercise 6 - Soft, not hard - on purpose

Take the anytime system from [§39](39_system_of_systems.md) and try to write down its worst-case time. You cannot prove one: its runtime depends on how much work the deadline allowed, which depends on the rest of the tick, which depends on the data. It is *soft* real-time by construction - it degrades quality gracefully and that is its whole virtue - and it must therefore never sit in a control loop where a missed deadline is a fault. Knowing which deadlines a component may be trusted with is the deliverable; the honest answer here is "soft ones only."

## Exercise 7 - Priority inversion

Three threads sharing one lock - high, medium, low priority:

```text
low  takes the lock, then is preempted by medium (which never touches the lock)
high wakes, wants the lock, blocks behind low
medium runs freely, starving low, so low never releases, so high misses its deadline
```

Reproduce it with `SCHED_FIFO` priorities and a `Mutex` held briefly by `low` while `medium` spins. `high` misses. Then enable priority inheritance (a `PTHREAD_PRIO_INHERIT` mutex): `low` temporarily inherits `high`'s priority while holding the lock, runs ahead of `medium`, releases, and `high` makes its deadline. The mechanism that fixed it is priority inheritance - the lock lends its waiter's priority to whoever holds it.

## Exercise 8 - Degrade gracefully (the soft side)

The priority order is not arbitrary: shed in increasing order of how much the world depends on the work. The inspection system writes nothing the simulation reads, so it goes first and costs nothing but a stale dashboard. The GC's cadence can stretch because dead slots linger harmlessly for a few more ticks. Deferring reproduction is the strongest lever because it is *back-pressure*: the births are what grow the population that overran the budget, so deferring them reduces the next tick's load - the shed attacks the cause, and the system walks itself back under budget instead of fighting the symptom forever.

Two properties have to hold or the degradation is worse than the overrun. First, **integrity survives**: because mutation is buffered ([§22](22_mutations_buffer.md)) and committed at the tick boundary, a long or shed tick still applies a whole, consistent world - you observe a late world, never a torn one. Second, **the run still replays**: the shed must be a logged decision, not a branch on `if over_budget`, or the run stops being a function of its inputs and the [§37](37_log_is_world.md)/[§48](48_reductions_dont_parallelize_freely.md) replay guarantee evaporates - and that bug is invisible until the multi-hour run on the slower machine sheds differently and diverges. Log the decision; replay applies the identical shed.

Bounded staleness is the last check: each deferral has a fixed horizon (a system skipped this tick runs next tick; a region foraged every other tick is at most one tick stale), so the degraded world is never more than a known distance from the budget-met world, and it heals when the load drops. Soft real-time managed this way is not "it got slow"; it is "it chose, predictably and reversibly, what to slow."
