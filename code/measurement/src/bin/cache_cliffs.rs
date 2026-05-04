//! cache_cliffs — sum a Vec<u64> at increasing sizes, print ns/element.
//! Used by §1 exercise 4 and §4 exercise 4.
//!
//!     cargo run --release --bin cache_cliffs
//!
//! Look for jumps in ns/element as the working set spills out of L1, L2, L3.

use std::io::Write;
use std::time::Instant;

const SIZES: &[usize] = &[
    1_000,        // ~8 KB     — L1
    10_000,       // ~80 KB    — L1/L2
    100_000,      // ~800 KB   — L2/L3
    1_000_000,    // ~8 MB     — L3/RAM
    10_000_000,   // ~80 MB    — RAM
    100_000_000,  // ~800 MB   — RAM
];

const TARGET_NS: u128 = 100_000_000; // run each size for at least 100 ms

#[inline(never)]
fn sum_seq(v: &[u64]) -> u64 {
    v.iter().sum()
}

fn main() {
    println!("{:>10} {:>11} {:>10} {:>14} {:>13}",
             "bytes", "elements", "iters", "total_ms", "ns/elem");
    for &n in SIZES {
        let v: Vec<u64> = (0..n as u64).collect();
        let mut iters = 1u32;
        let mut total_ns;
        loop {
            let t0 = Instant::now();
            let mut s = 0u64;
            for _ in 0..iters {
                // black_box the slice so the compiler can't hoist the sum out of the loop.
                s = s.wrapping_add(sum_seq(std::hint::black_box(&v[..])));
            }
            std::hint::black_box(s);
            total_ns = t0.elapsed().as_nanos();
            if total_ns >= TARGET_NS || iters >= (1 << 24) { break; }
            iters *= 2;
        }
        let ns_per_elem = total_ns as f64 / (iters as f64 * n as f64);
        println!("{:>10} {:>11} {:>10} {:>14.2} {:>13.3}",
                 human_size(n * 8), n, iters,
                 total_ns as f64 / 1_000_000.0,
                 ns_per_elem);
        std::io::stdout().flush().unwrap();
    }
}

fn human_size(bytes: usize) -> String {
    if      bytes >= (1 << 30) { format!("{:.1}GB", bytes as f64 / (1u64 << 30) as f64) }
    else if bytes >= (1 << 20) { format!("{:.1}MB", bytes as f64 / (1u64 << 20) as f64) }
    else if bytes >= (1 << 10) { format!("{:.1}KB", bytes as f64 / (1u64 << 10) as f64) }
    else                       { format!("{}B", bytes) }
}
