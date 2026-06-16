//! reduction_divergence - the specimen behind §48. A parallel floating-point reduction's result
//! depends on the thread count, because float addition is not associative and the partition
//! grouping changes with the number of threads. Then the two fixes, measured.
//!
//!     cargo run --release --bin reduction_divergence
//!
//! Sceptic's note this specimen settles: "fix the reduction order" buys core-count independence
//! ONLY if the partition count is fixed *independent of thread count*. Partitioning by thread
//! count (the obvious thing) still changes the grouping and still diverges. The specimen shows
//! both, so the chapter can state the precise version.

use std::thread;

const N: usize = 1_000_000;

// Values spanning magnitudes so the grouping order actually moves the low bits (harmonic series
// is the textbook case: 1, 1/2, 1/3, ... summed in different groupings rounds differently).
fn values() -> Vec<f64> {
    (0..N).map(|i| 1.0 / (i as f64 + 1.0)).collect()
}

// Serial sum in index order - the reference grouping.
fn serial(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |a, &x| a + x)
}

// RACY: partition into `threads` contiguous chunks (so the grouping = thread count), sum each in
// order, fold the partials in id order. Deterministic for a fixed thread count, but the grouping -
// and so the result - changes with the thread count.
fn racy(v: &[f64], threads: usize) -> f64 {
    let chunk = N.div_ceil(threads);
    let partials: Vec<f64> = thread::scope(|s| {
        let handles: Vec<_> = v.chunks(chunk).map(|c| s.spawn(|| serial(c))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    partials.iter().fold(0.0, |a, &x| a + x) // fold partials in id order
}

// FIXED ORDER: a FIXED number of partitions (here 64), independent of the thread count. The 64
// partials run in parallel (the OS schedules them across whatever cores exist), but the partials
// array is in partition-id order and the fold is in id order, so the grouping - and the result -
// is the same no matter how many cores ran it.
fn fixed_order(v: &[f64], parts: usize) -> f64 {
    let chunk = N.div_ceil(parts);
    let partials: Vec<f64> = thread::scope(|s| {
        let handles: Vec<_> = v.chunks(chunk).map(|c| s.spawn(|| serial(c))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    partials.iter().fold(0.0, |a, &x| a + x) // fold in partition-id order
}

// INTEGER: scale to fixed-point i128, sum exactly (integer add is associative), scale back.
fn integer(v: &[f64], threads: usize, scale: f64) -> f64 {
    let chunk = N.div_ceil(threads);
    let partials: Vec<i128> = thread::scope(|s| {
        let handles: Vec<_> = v
            .chunks(chunk)
            .map(|c| s.spawn(move || c.iter().map(|&x| (x * scale) as i128).sum::<i128>()))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    partials.iter().sum::<i128>() as f64 / scale
}

fn main() {
    let v = values();
    let s = serial(&v);
    println!("§48 specimen - parallel float reduction, N = {N}, harmonic values\n");
    println!("serial (index order):  {:.17e}  bits={:016x}\n", s, s.to_bits());

    println!("{:>7}  {:>18}  {:>18}  {:>18}", "threads", "racy(parts=thr)", "fixed-order(64)", "integer(i128)");
    for t in [1usize, 2, 4, 8] {
        let r = racy(&v, t);
        let f = fixed_order(&v, 64);
        let i = integer(&v, t, 1e9);
        println!("{:>7}  {:016x}  {:016x}  {:016x}", t, r.to_bits(), f.to_bits(), i.to_bits());
    }
    println!("\nRACY bits change with thread count - the world hashes differ across core counts.");
    println!("FIXED-ORDER bits are identical across thread counts (the partition count is fixed at 64,");
    println!("NOT equal to the thread count - that is the precise version of the fix).");
    println!("INTEGER bits are identical across thread counts and any grouping (associative).");
}
