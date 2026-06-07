# Solutions: 48 - Reductions don't parallelize freely

## Exercise 1 - Make it diverge

```rust
fn parallel_sum(xs: &[f32], threads: usize) -> f32 {
    let chunk = xs.len().div_ceil(threads);
    std::thread::scope(|s| {
        let hs: Vec<_> = xs.chunks(chunk)
            .map(|c| s.spawn(move || c.iter().sum::<f32>()))
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).sum()
    })
}
```

Sum a million `f32`s at 1, 2, 4, 8 threads and the bit patterns of the results differ, because each thread count groups the additions differently and floating-point rounds at each step. Each result is *stable* on repeated runs at a fixed thread count - the non-determinism is across core counts, not across runs. The bug is real and reproducible, which is exactly why it slips through a test suite run on one machine.

## Exercise 2 - Compound it

Use that sum for a per-tick energy normalisation and run 1000 ticks from one seed at two thread counts:

```rust
let total = parallel_sum(&w.energy, threads);
for e in &mut w.energy { *e /= total; }   // the divergent total feeds every creature
```

Hash the worlds. They differ, and the difference is no longer in the last bit - the feedback loop has amplified one ULP into a different population. This is the [§16](16_determinism_by_order.md) contract broken by a single reduction.

## Exercise 3 - Fix the order

Reduce each partition into a fixed slot, then fold serially in slot order:

```rust
let partials: Vec<f32> = /* one per partition, computed in parallel */;
let total: f32 = partials.iter().sum();   // serial fold, fixed order, every machine
```

Re-run exercise 1: the hashes are now identical across all thread counts, because the grouping is defined by partition id, not by thread count. Time the serial fold - it is a sum over the *partition count* (a few to a few dozen values), negligible against the parallel per-element work it guards. You keep the [§31](31_disjoint_writes_parallelize.md) speedup and recover determinism.

## Exercise 4 - Accumulate in integers

```rust
const SCALE: i64 = 1 << 20;                       // fixed-point: ~6 decimal digits
let total_fixed: i64 = xs.iter()                  // any order, any thread count, identical
    .map(|&x| (x as f64 * SCALE as f64) as i64)
    .sum();
let total = total_fixed as f64 / SCALE as f64;
```

Integer addition is associative, so the result is identical across thread counts *and* across summation orders - no fixed-order discipline required. Find the boundaries: overflow begins when `count * max_value * SCALE` exceeds `i64::MAX` (lower the scale or widen to `i128`); precision loss begins when `SCALE` is too small to represent the smallest meaningful difference. Bounded quantities like energy fit comfortably between the two.

## Exercise 5 - Replay across core counts

```rust
let a = replay(snapshot.clone(), &committed_log, /*threads=*/ 4);
let b = replay(snapshot.clone(), &committed_log, /*threads=*/ 64);
assert_eq!(hash_world(&a), hash_world(&b));   // passes only with a deterministic reduction
```

With a racy reduction this fails and the [§37](37_log_is_world.md) "distribution is structural" claim is false on heterogeneous hardware - two nodes from the same log diverge. With the fixed-order or integer reduction it passes, and replay across machines with different core counts is bit-identical, as [§16](16_determinism_by_order.md) promised.

## Exercise 6 - The canary test

```rust
#[test]
fn deterministic_across_core_counts() {
    let seed = 42;
    let a = run_sim(seed, /*threads=*/ 1);
    let b = run_sim(seed, /*threads=*/ num_cpus::get());
    assert_eq!(hash_world(&a), hash_world(&b));
}
```

It fails today (a racy reduction is somewhere in the tick), passes once the reductions are fixed, and stays in CI as the guard that catches the next one. It is cheap, it is deterministic, and it fails on the developer's machine instead of in production on a box with a different `nproc`.

## Exercise 7 - The same bug on a vector unit

A SIMD horizontal sum accumulates into lanes and folds the lanes at the end - a different grouping again, so the result differs from a scalar sum and from a sum with a different lane count. The fix is the same: reduce into a fixed number of lane-accumulators and fold them in a defined order, or accumulate in integer lanes. The implication for the heterogeneous-compute chapter is direct - a GPU reduction is a tree across thousands of lanes, the most reordered reduction of all, and it takes the same two fixes. Determinism is a property of the combine wherever the combine runs.
