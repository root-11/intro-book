# 4 — Cost is layout — and you have a budget

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 4](../../concepts/glossary.md#4--cost-is-layout--and-you-have-a-budget).*

A program runs at some *target rate*. A game runs at 30 Hz or 60 Hz; an audio loop at 48 kHz; a control loop at 1 kHz; an interactive shell at "as fast as a human can type". The target rate sets a *budget* — the time available for one tick of work.

|     Target rate | Budget per tick |
|----------------:|----------------:|
|           30 Hz |          33 ms  |
|           60 Hz |          17 ms  |
|         1000 Hz |           1 ms  |
|       1 000 000 |        1 µs     |

Every operation the program does in one tick spends from that budget. Operations have very different costs: the arithmetic is virtually free, an L1 read is around 1 ns, an L3 read is around 10 ns, a RAM read is around 100 ns, a disk read is around 100 µs, a network round-trip is around 100 ms. A 30 Hz program spending one disk read per tick has lost a third of its budget on one operation.

> [!NOTE]
>
> Three regimes are worth naming, because the rest of the book references them. A loop is **compute-bound** when its cost is dominated by arithmetic — typically when the data fits in L1 and the inner instructions are heavy (dot products, transcendentals, integer divides). It is **bandwidth-bound** when its cost is dominated by how fast the memory subsystem can deliver bytes — typically when the working set is bigger than L3 *but* the access pattern is sequential, so the prefetcher can fill lines ahead of demand. It is **latency-bound** when its cost is dominated by individual memory round-trips — typically when the access pattern is random, so the prefetcher cannot help. The three regimes have very different time budgets and very different power profiles. A sequential `Vec<u64>` sum on a modern desktop is bandwidth-bound at ~50 GB/s, roughly 0.15 ns per element. The same `Vec` accessed by random index is latency-bound at one full RAM round-trip per element, roughly 50-100 ns per element — three orders of magnitude slower, despite the same arithmetic. The lesson of node 4 is that complexity-class reasoning cannot tell these regimes apart, but they are the difference between a program that meets its tick budget and one that does not.

The unit of accounting is **time** — microseconds for most real-time work, nanoseconds for tight inner loops. A 30 Hz tick has 33 ms (33 000 µs) of budget; a 1 kHz tick has 1 000 µs; a 1 MHz tick has 1 µs. When a teacher asks you "what does this function cost?", they are asking how many microseconds it takes. A function that costs 100 µs out of a 33 000 µs budget is fine — about 0.3% of the tick. The same function in a 1 000 µs budget is 10% of the tick. The same function in a 1 µs budget does not exist; there is no room for it.

Cost is also *layout*. The same algorithm that costs 100 µs on a sequential `Vec` may cost 5 ms on a hash map of the same size, because the loads scatter. Two programs with the same big-O complexity can differ by an order of magnitude on the same hardware, just because of where their data sits.

This gives you a design rule. *Decide your target rate before you decide anything else.* That sets the budget. Then when you choose data structures, ask whether the resulting working set fits in cache; ask how many memory loads per row your inner loop does; ask whether any single operation in the loop dominates the budget. Most decisions become forced once the budget is named.

The reverse direction is also useful. If you find yourself wanting to *add* something to the inner loop — a database query, a HashMap lookup, an allocation — count its cost in microseconds against the budget. Often the answer is "this single addition uses 80% of my tick", and the right move is not to optimise it but to lift it out of the inner loop entirely.

<p align="center"><img src="../illustrations/ohms_law.jpg" alt="Ohm's Law: V = I·R" style="max-height: 300px; max-width: 100%;"></p>

The shape of this thinking is familiar to engineers in other domains. An electrical engineer designs a circuit by counting milliamps against a current budget. A structural engineer counts kilonewtons against a load budget. The data-oriented programmer counts memory loads and microseconds against a tick budget. *Good design is measured in millivolts and microamps* — and in nanoseconds and microseconds.

> [!NOTE]
>
> *Time is one budget. Power is another.* Cache hits are energetically nearly free — the data is already next to the arithmetic units. Cache misses fire up the memory controller, the bus drivers, sometimes a DRAM refresh; that is where the watts go. A loop that fits in L2 spends most of its time on cheap arithmetic; a loop that pointer-chases through RAM spends most of its time *waiting*, and during the waiting the CPU drops clocks and the chip stays cool. The same SoA-and-sequential-access discipline that fits the time budget also fits a power budget. For embedded, mobile, control, and battery-powered work, power is the *primary* budget; time is downstream of it. The "millivolts and microamps" line above is literal, not metaphor.

## Exercises

1. **Pick your rates.** For each of these systems, name a plausible target rate and the resulting per-tick budget: a card game; a real-time strategy game; a market data feed; an embedded sensor controller; a web API endpoint a user is waiting for; an offline batch job that processes a billion rows.
2. **Count an operation.** Time a single `HashMap::get` on a map of 1 000 000 entries. Note its cost in microseconds. How many can you fit in a 30 Hz tick (33 ms)? In a 1 kHz tick (1 ms)?
3. **The layout difference.** Sum 1 000 000 `u64`s in a `Vec<u64>`. Sum 1 000 000 `u64`s in a `HashMap<u32, u64>`. Both are O(N). What is the per-element time difference (in nanoseconds)? Where did it go?
4. **The cliff.** With your numbers from [§1 exercise 4](01_the_machine_model.md#exercises), pick a `Vec` size that just fits in L2 and one that just doesn't. Time a sum loop at each size. The cliff is real.
5. **Working backwards from the budget.** You target 60 Hz; your inner loop runs over 100 000 entities; each entity touches one cache line. Estimate the cost of the loop in microseconds and compare to your 60 Hz budget (16 666 µs). Where is your headroom?
6. **A bad design.** Construct a design that is "obviously fast" by big-O reasoning but blows the 30 Hz budget on a million entities. (Hint: object-graph traversal with one heap allocation per node is a classic.)
7. **Find your CPU's TDP.** Look up your CPU's rated thermal design power on the manufacturer's spec sheet, or read it locally on Linux with `sudo dmidecode -t processor | grep -i 'power\|TDP'`. Note the value. TDP is what the chip can dissipate sustained without thermal throttling — burst can be 1.5-2× higher for tens of seconds; sustained settles back to TDP.
8. **Battery budget.** A typical laptop battery holds about 50 Wh. Your simulator runs at 30 Hz and draws an average of 8 W (mostly memory bandwidth on the inner loop). How many hours of simulation does a full charge buy? If a layout change pushes more loads to RAM and raises the average draw to 14 W, how many hours then? Express the cost of the layout change as a percentage of battery life.
9. **Measure delta power.** A ready-made workload generator lives at `code/measurement/`. In one terminal: `cargo run --release --bin power_loop -- sequential` (then in a second run: `... -- random`). In another terminal, while the loop is running: `sudo perf stat -a -e power/energy-pkg/ -- sleep 30` reads the package-energy counter over 30 seconds. Run the perf command three times — idle, sequential, random — and write the joules down. Convert each to average watts. The random-access run should draw more watts than the sequential one, which should draw more than idle.

   While you are there: from `power_loop`'s iteration count, compute your sequential read bandwidth — `iterations × 10⁷ × 8 / 45` gives bytes per second — and compare to the published peak of your DDR generation. If you get within a factor of two of peak, your inner loop is *bandwidth-bound* (the regime named in the prose). The `random` mode's iteration count, divided into wall time, gives your effective per-element latency in nanoseconds; that is the *latency-bound* regime.
10. *(stretch)* **Joules per access.** Approximate energies per memory read: L1 hit ≈ 0.1 nJ, L2 ≈ 1 nJ, RAM ≈ 30 nJ (rough; published numbers vary by chip and process). Estimate the total energy of summing 10⁷ `u64`s sequentially (mostly prefetched, near-L1 cost) versus by random indices (mostly RAM misses). Convert both to milliwatt-hours and express as a fraction of a 50 Wh battery. The absolute numbers are tiny; the *ratio* is what your battery life and your data-centre electricity bill care about.

Reference notes in [04_cost_and_budget_solutions.md](04_cost_and_budget_solutions.md).

## What's next

You now have the machine model (§1), the data widths (§2), the table primitive (§3), and the budget calculus (§4). The next section is the conceptual heart of the book: [§5 — Identity is an integer](05_identity_is_an_integer.md). The card game is waiting.
