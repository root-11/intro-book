//! swap_remove vs remove on a 1M Vec.
//! Used by §3 exercise 5.

use std::time::Instant;

const N: usize = 1_000_000;
const REMOVALS: usize = 1_000;

fn main() {
    // Vec::remove path — each removal shifts ~half the vector.
    let mut v: Vec<u32> = (0..N as u32).collect();
    let t0 = Instant::now();
    let mut sink = 0u32;
    for _ in 0..REMOVALS {
        let mid = v.len() / 2;
        sink = sink.wrapping_add(v.remove(mid));
    }
    std::hint::black_box(sink);
    let dt_remove = t0.elapsed();

    // Vec::swap_remove path — O(1) per removal.
    let mut v: Vec<u32> = (0..N as u32).collect();
    let t0 = Instant::now();
    let mut sink = 0u32;
    for _ in 0..REMOVALS {
        // black_box the index so the compiler can't constant-fold the loop.
        let mid = std::hint::black_box(v.len() / 2);
        sink = sink.wrapping_add(v.swap_remove(mid));
    }
    std::hint::black_box(sink);
    let dt_swap = t0.elapsed();

    let ns_remove_each = dt_remove.as_nanos() as f64 / REMOVALS as f64;
    let ns_swap_each   = dt_swap.as_nanos()   as f64 / REMOVALS as f64;
    println!("Vec::remove      ×{REMOVALS} on N={N}: {:>10.3} ms total ({:>9.0} ns/op)",
             dt_remove.as_secs_f64() * 1000.0, ns_remove_each);
    println!("Vec::swap_remove ×{REMOVALS} on N={N}: {:>10.3} ms total ({:>9.0} ns/op)",
             dt_swap.as_secs_f64() * 1000.0, ns_swap_each);
    println!("Ratio:                                    {:>10.0}x",
             ns_remove_each / ns_swap_each.max(1.0));
}
