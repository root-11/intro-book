//! scope_speedup - two independent systems run serially vs in parallel via
//! std::thread::scope. Backs §31 exercise 2.
//!
//!     cargo run --release --bin scope_speedup
//!
//! Two compute-bound passes over disjoint arrays (a stand-in for `motion` and
//! `food_spawn`). Run back-to-back, then run together under thread::scope. With
//! both passes individually expensive and compute-bound, the speedup approaches
//! 2x. Bandwidth-bound passes would show less - one thread can saturate the bus.

use std::time::Instant;

const N: usize = 4_000_000;
const REPS: usize = 20;

/// Compute-bound work: a transcendental loop the optimiser cannot fold away.
#[inline(never)]
fn system(data: &mut [f32]) {
    for x in data.iter_mut() {
        let mut v = *x;
        for _ in 0..8 {
            v = (v * 1.000001 + 0.5).sin().cos().abs() + 0.1;
        }
        *x = v;
    }
}

fn main() {
    let mut a = vec![0.3f32; N];
    let mut b = vec![0.7f32; N];

    // Serial: both systems, one after the other.
    let t0 = Instant::now();
    for _ in 0..REPS {
        system(std::hint::black_box(&mut a));
        system(std::hint::black_box(&mut b));
    }
    let dt_serial = t0.elapsed();

    // Parallel: both systems at once, disjoint &mut proven safe by scope.
    let t0 = Instant::now();
    for _ in 0..REPS {
        std::thread::scope(|s| {
            s.spawn(|| system(std::hint::black_box(&mut a)));
            s.spawn(|| system(std::hint::black_box(&mut b)));
        });
    }
    let dt_parallel = t0.elapsed();

    std::hint::black_box((&a, &b));
    println!("two compute-bound systems, N={N} each, {REPS} reps");
    println!("  serial:   {:>8.1} ms", dt_serial.as_secs_f64() * 1000.0);
    println!("  parallel: {:>8.1} ms", dt_parallel.as_secs_f64() * 1000.0);
    println!("  speedup:  {:>8.2}x", dt_serial.as_secs_f64() / dt_parallel.as_secs_f64());
}
