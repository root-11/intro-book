//! l1_sweet_spot - motion's loop run tight at an L1-resident size vs an
//! L2-resident size, in repetition so the working set stays hot. Backs §27
//! exercise 6.
//!
//!     cargo run --release --bin l1_sweet_spot
//!
//! 20 bytes/creature. ~1200 creatures = 24 KB fills L1 (32 KB) to ~75%; 10,000
//! creatures = 200 KB lives in L2. Looped in tight repetition, the L1 size
//! should run at a lower ns/creature than the L2 size - but on a modern core
//! with deep prefetch the streaming gap is small, not the dramatic cliff that
//! random access shows. The honest number is whatever this prints.

use std::time::Instant;

const DT: f32 = 0.016;
const BURN: f32 = 0.01;
const REPS: usize = 200_000;

#[inline(never)]
fn motion(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], e: &mut [f32]) {
    for i in 0..px.len() {
        px[i] += vx[i] * DT;
        py[i] += vy[i] * DT;
        e[i] -= (vx[i] * vx[i] + vy[i] * vy[i]).sqrt() * BURN;
    }
}

fn run(n: usize) -> f64 {
    let mut px = vec![0.0f32; n];
    let mut py = vec![0.0f32; n];
    let vx: Vec<f32> = (0..n).map(|i| ((i % 7) as f32) - 3.0).collect();
    let vy: Vec<f32> = (0..n).map(|i| ((i % 5) as f32) - 2.0).collect();
    let mut e = vec![100.0f32; n];
    let t0 = Instant::now();
    for _ in 0..REPS {
        motion(std::hint::black_box(&mut px), &mut py, &vx, &vy, &mut e);
    }
    std::hint::black_box((&px, &py, &e));
    t0.elapsed().as_nanos() as f64 / (REPS as f64 * n as f64)
}

fn main() {
    let l1 = run(1_200);   // 24 KB - ~75% of a 32 KB L1
    let l2 = run(10_000);  // 200 KB - L2-resident
    println!("motion in tight repetition ({REPS} reps):");
    println!("  N=1,200  (24 KB, L1):  {l1:.3} ns/creature");
    println!("  N=10,000 (200 KB, L2): {l2:.3} ns/creature");
    println!("  L2/L1 ratio:           {:.2}x", l2 / l1);
}
