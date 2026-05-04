# 1 — The machine model

<p align="center"><img src="../covers/phase_foundation.jpg" alt="Foundation phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 1](../../concepts/glossary.md#1--the-machine-model).*

Most explanations of "how a computer works" use a diagram with a CPU and a single big block called *memory*. The diagram is wrong. Memory is many things at different speeds, and which one your data sits in decides whether your program is fast or slow.

Inside the CPU there is **L1 cache** — small, sometimes only 32 KB per core, but a read from it costs about one nanosecond. Around it sits **L2** — a few hundred KB, around 3-4 ns. Then **L3** — measured in megabytes, around 10 ns. Outside the CPU sits **main memory (RAM)** — gigabytes, around 100 ns per read. The numbers vary by chip, but the *ratios* are stable: L1 is roughly a hundred times faster than RAM. Cache and RAM are the same kind of thing — bytes that the CPU reads — but they sit at very different distances from the arithmetic units.

When your code reads `vec[17]`, the CPU does not pull just byte 17. It pulls a whole 64-byte chunk — a *cache line* — and keeps that line in L1. The next read of `vec[18]` is then almost free. Reading sequentially through a `Vec` is fast because every line that gets loaded is mostly used before it gets evicted. Reading at random is slow because every read costs a fresh trip to RAM.

A pointer is an address in memory. Following one — `*ptr` — is one memory read at an address the CPU does not get to predict. If the address is in cache, the read is fast; if not, you wait the full ~100 ns. A program with many objects and many pointers between them is a program with many of those waits.

That asymmetry is the dominant fact about modern CPUs. The arithmetic — adding, multiplying, branching — is virtually free; the cost is *getting the data to the arithmetic*. A program that respects this is fast. A program that ignores it can be a hundred times slower than a program that does the same work, with the same number of additions, but in a layout the cache likes.

This is also what makes "complexity class" misleading on its own. An O(N log N) algorithm that hits the cache hard can outrun a "faster" O(N) algorithm that scatters reads across RAM. Big-O describes how cost grows with N; layout describes the constant factor that gets multiplied in. At the scales this book targets, the constant factor often wins.

You will *measure* this in the next two sections. The numbers above are nominal — the chip in front of you may be slightly faster or slightly slower, and the ratios are what matters. Once you have felt how big the gap is, the rest of the book's reasoning about layout, SoA, hot-cold splits, and parallelism follows naturally.

## Exercises

These exercises are calibrations. Run them on your machine and write the numbers down — the rest of the book references them.

1. **Look up your cache sizes.** On Linux, `lscpu | grep -i cache` lists L1d, L1i, L2, L3 per core. Write them down. (On macOS: `sysctl -a | grep cache`.) These are the budgets node 25 will hold you to later.
2. **Time a sequential sum.** Build a `Vec<u64>` of 100,000,000 elements (use `vec![1u64; 100_000_000]`), then time `vec.iter().sum::<u64>()`. Use `std::time::Instant`. Note the time per element in nanoseconds.
3. **Time a random-access sum.** Build the same `Vec<u64>`, plus a `Vec<usize>` of 100,000,000 random indices. Time the loop `let mut s = 0u64; for &i in &indices { s += vec[i]; }`. Compare with exercise 2.
4. **Find the cache cliffs.** Repeat exercise 2 at sizes 1K, 10K, 100K, 1M, 10M, 100M. Plot `time/element` (or just print it). Note the size at which it jumps — that's where you spilled out of L1, then L2, then L3.
5. **Pointer chasing.** Build a linked list of 1,000,000 `Box<Node>` where `Node { value: u64, next: Option<Box<Node>> }`. Time a sum that walks the list. Compare with the same sum on a `Vec<u64>` of the same length. The ratio is roughly the L1-to-RAM ratio.
6. *(stretch)* **Read your `lscpu` output to your benchmarks.** With your cache sizes from exercise 1 and your timings from exercise 4, identify which level of cache each size step is leaving. The transitions are not always clean — annotate where they are noisy.

Reference notes for these exercises in [01_the_machine_model_solutions.md](01_the_machine_model_solutions.md).

## What's next

The numbers you wrote down in exercise 1 and the cliffs you found in exercise 4 are the constants behind the whole book. [§2 — Numbers and how they fit](02_numbers_and_how_they_fit.md) takes the next step: how big is each unit of data, and how many fit in a cache line?
