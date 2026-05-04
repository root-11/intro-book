//! power_loop — sustain a workload while you measure package power in another terminal.
//!
//! Usage:
//!
//!     cargo run --release --bin power_loop -- <mode>
//!
//! Modes:
//!
//!     sequential   sum a Vec<u64> in order, looping for 45 s
//!     random       sum the same Vec by random index, looping for 45 s
//!
//! In a second terminal, run (during a fresh start of this program):
//!
//!     sudo perf stat -a -e power/energy-pkg/ -- sleep 30
//!
//! Divide the reported joules by 30 for the average package watts. Compare
//! across modes — and against an idle baseline. Used by §4's exercise 9.

use std::time::{Duration, Instant};

/// Working-set size in u64 elements. 10M × 8 B = 80 MB — bigger than any L3
/// on commodity hardware, so the loop is RAM-bound under both modes.
const N: usize = 10_000_000;

/// How long to sustain the workload. Long enough to outlast a 30 s `perf stat`
/// window with margin on either side.
const RUN_FOR: Duration = Duration::from_secs(45);

#[inline(never)]
fn sequential_sum(v: &[u64]) -> u64 {
    let mut sum = 0u64;
    for &x in v {
        sum = sum.wrapping_add(x);
    }
    sum
}

#[inline(never)]
fn random_access_sum(v: &[u64], indices: &[usize]) -> u64 {
    let mut sum = 0u64;
    for &i in indices {
        sum = sum.wrapping_add(v[i]);
    }
    sum
}

fn build_random_indices(n: usize) -> Vec<usize> {
    let mut state = 0xDEAD_BEEFu64;
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state as usize) % n
        })
        .collect()
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: power_loop <sequential|random>");
        std::process::exit(2);
    });

    let v: Vec<u64> = vec![1; N];
    let mut sum = 0u64;
    let mut iters = 0u64;
    let start = Instant::now();

    match mode.as_str() {
        "sequential" => {
            eprintln!("sequential: summing {N} u64 elements in order for {RUN_FOR:?}");
            while start.elapsed() < RUN_FOR {
                sum = sum.wrapping_add(sequential_sum(&v));
                iters += 1;
            }
        }
        "random" => {
            eprintln!("random: building indices...");
            let indices = build_random_indices(N);
            eprintln!("random: summing {N} u64 elements by random index for {RUN_FOR:?}");
            while start.elapsed() < RUN_FOR {
                sum = sum.wrapping_add(random_access_sum(&v, &indices));
                iters += 1;
            }
        }
        other => {
            eprintln!("unknown mode {other:?} (expected sequential|random)");
            std::process::exit(2);
        }
    }

    let elapsed = start.elapsed();
    eprintln!(
        "done: {iters} iterations in {elapsed:?} — sum = {sum} (kept to prevent dead-code elim)"
    );
}
