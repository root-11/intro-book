//! false_sharing - eight threads increment eight counters. When the counters
//! share a cache line, the line ping-pongs between cores and the "parallel"
//! version runs no faster (often slower) than one thread. Pad each counter to
//! its own 64-byte line and the speedup reappears. Backs §33.
//!
//!     cargo run --release --bin false_sharing
//!
//! Three timings of the same total work (THREADS x ITERS increments):
//!   shared  - counters packed in one Vec<AtomicU64>; all in a few cache lines.
//!   padded  - each counter on its own 64-byte line (#[repr(align(64))]).
//!   single  - one thread does all the increments. The honest baseline.
//!
//! padded/single should approach THREADS (real scaling). shared/single is the
//! number that disappoints: often near 1x, sometimes worse than 1x (the
//! parallel run is slower than the serial one - true negative scaling).

use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const ITERS: u64 = 40_000_000;

#[repr(align(64))]
struct Padded(AtomicU64);

fn time_shared(threads: usize) -> Duration {
    let counters: Vec<AtomicU64> = (0..threads).map(|_| AtomicU64::new(0)).collect();
    let t0 = Instant::now();
    thread::scope(|s| {
        for c in &counters {
            s.spawn(move || {
                for _ in 0..ITERS { c.fetch_add(1, Ordering::Relaxed); }
            });
        }
    });
    std::hint::black_box(&counters);
    t0.elapsed()
}

fn time_padded(threads: usize) -> Duration {
    let counters: Vec<Padded> = (0..threads).map(|_| Padded(AtomicU64::new(0))).collect();
    let t0 = Instant::now();
    thread::scope(|s| {
        for c in &counters {
            s.spawn(move || {
                for _ in 0..ITERS { c.0.fetch_add(1, Ordering::Relaxed); }
            });
        }
    });
    std::hint::black_box(&counters);
    t0.elapsed()
}

/// One thread does the whole THREADS x ITERS workload. The baseline scaling is
/// measured against.
fn time_single(threads: usize) -> Duration {
    let c = AtomicU64::new(0);
    let t0 = Instant::now();
    for _ in 0..(threads as u64 * ITERS) {
        c.fetch_add(1, Ordering::Relaxed);
    }
    std::hint::black_box(&c);
    t0.elapsed()
}

// --- Naive partitioned reduction ----------------------------------------
// The realistic version of the bug: partition one input array into disjoint
// per-thread slices, fold each slice into a shared per-thread accumulator.
// The slices are disjoint, the work is balanced - and the accumulators sit
// packed in one array, so every fold writes a contended cache line.

const REDUCE_N: usize = 8_000_000; // input elements per thread

fn reduce_shared(threads: usize, input: &[u64]) -> Duration {
    let acc: Vec<AtomicU64> = (0..threads).map(|_| AtomicU64::new(0)).collect();
    let t0 = Instant::now();
    thread::scope(|s| {
        for (id, chunk) in input.chunks(REDUCE_N).enumerate() {
            let slot = &acc[id];
            s.spawn(move || {
                for &x in chunk { slot.fetch_add(x, Ordering::Relaxed); }
            });
        }
    });
    std::hint::black_box(&acc);
    t0.elapsed()
}

fn reduce_padded(threads: usize, input: &[u64]) -> Duration {
    let acc: Vec<Padded> = (0..threads).map(|_| Padded(AtomicU64::new(0))).collect();
    let t0 = Instant::now();
    thread::scope(|s| {
        for (id, chunk) in input.chunks(REDUCE_N).enumerate() {
            let slot = &acc[id].0;
            s.spawn(move || {
                for &x in chunk { slot.fetch_add(x, Ordering::Relaxed); }
            });
        }
    });
    std::hint::black_box(&acc);
    t0.elapsed()
}

fn reduce_single(input: &[u64]) -> Duration {
    let acc = AtomicU64::new(0);
    let t0 = Instant::now();
    for &x in input { acc.fetch_add(x, Ordering::Relaxed); }
    std::hint::black_box(&acc);
    t0.elapsed()
}

fn main() {
    let threads = thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(8);
    println!("false sharing: {threads} threads x {ITERS} increments each\n");

    let shared = time_shared(threads);
    let padded = time_padded(threads);
    let single = time_single(threads);

    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    println!("  {:<28} {:>10}", "single thread (baseline)", format!("{:.1} ms", ms(single)));
    println!("  {:<28} {:>10}  {:>6.2}x speedup", "padded (own cache line)",
             format!("{:.1} ms", ms(padded)), ms(single) / ms(padded));
    println!("  {:<28} {:>10}  {:>6.2}x speedup", "shared (false sharing)",
             format!("{:.1} ms", ms(shared)), ms(single) / ms(shared));
    println!();
    println!("  padding recovers {:.1}x of the {threads}x ideal.", ms(single) / ms(padded));
    if ms(shared) > ms(single) {
        println!("  false-shared parallel run is SLOWER than one thread - negative scaling.");
    } else {
        println!("  false-shared speedup is only {:.2}x despite {threads} cores.",
                 ms(single) / ms(shared));
    }

    // Partitioned reduction: the realistic "I did everything right" version.
    let input: Vec<u64> = (0..(threads * REDUCE_N) as u64).map(|i| i & 0xFF).collect();
    let r_single = reduce_single(&input);
    let r_shared = reduce_shared(threads, &input);
    let r_padded = reduce_padded(threads, &input);
    println!("\n  partitioned reduction ({} elements, {threads} disjoint slices):", input.len());
    println!("    {:<26} {:>10}", "single thread", format!("{:.1} ms", ms(r_single)));
    println!("    {:<26} {:>10}  {:>6.2}x", "packed accumulators",
             format!("{:.1} ms", ms(r_shared)), ms(r_single) / ms(r_shared));
    println!("    {:<26} {:>10}  {:>6.2}x", "padded accumulators",
             format!("{:.1} ms", ms(r_padded)), ms(r_single) / ms(r_padded));
}
