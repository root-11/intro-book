//! motion_working_set - the simulator's real `motion` system, timed as the
//! working set grows out of cache. Backs the ns/elem ladder in §27 and the
//! hot/cold split ratio in §26.
//!
//!     cargo run --release --bin motion_working_set
//!
//! `motion` reads pos (f32x2), vel (f32x2), energy (f32) = 20 bytes/creature
//! and writes pos + energy. Three layouts are timed at each size:
//!
//!   SoA hot   - five parallel Vec<f32> columns (post-§26 split, 20 B touched).
//!   AoS full  - one Vec<Creature> where each row is the hot fields plus 20 B
//!               of cold padding (pre-§26 split, 40 B row pulled through cache).
//!   SoA random- the SoA loop visited through a shuffled index array, so each
//!               step is an unpredictable jump (the §27.44 random-vs-sequential
//!               contrast). The prefetcher cannot help.
//!
//! ns/elem is per creature per tick. Watch it climb as the columns spill L1 -> L2
//! -> L3 -> RAM, and watch AoS climb faster (it drags twice the bytes per row).

use std::io::Write;
use std::time::Instant;

const SIZES: &[usize] = &[
    10_000,      // ~200 KB hot  - L2
    1_000_000,   // ~20 MB hot   - L3/RAM
    10_000_000,  // ~200 MB hot  - RAM
];

const TARGET_NS: u128 = 100_000_000; // run each measurement for at least 100 ms
const DT: f32 = 0.016;
const BURN: f32 = 0.01;

/// Pre-split row: 20 hot bytes + 20 cold bytes = 40 bytes. Motion only reads the
/// hot fields, but the cache line drags the cold half along for the ride.
#[repr(C)]
#[derive(Clone, Copy)]
struct Creature {
    pos: [f32; 2],
    vel: [f32; 2],
    energy: f32,
    _cold: [u8; 20], // id, generation, kind, flags... untouched by motion
}

#[inline(never)]
fn motion_soa(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], e: &mut [f32]) {
    for i in 0..px.len() {
        px[i] += vx[i] * DT;
        py[i] += vy[i] * DT;
        e[i] -= (vx[i] * vx[i] + vy[i] * vy[i]).sqrt() * BURN;
    }
}

#[inline(never)]
fn motion_aos(cs: &mut [Creature]) {
    for c in cs.iter_mut() {
        c.pos[0] += c.vel[0] * DT;
        c.pos[1] += c.vel[1] * DT;
        c.energy -= (c.vel[0] * c.vel[0] + c.vel[1] * c.vel[1]).sqrt() * BURN;
    }
}

#[inline(never)]
fn motion_soa_random(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32],
                     e: &mut [f32], idx: &[u32]) {
    for &raw in idx {
        let i = raw as usize;
        px[i] += vx[i] * DT;
        py[i] += vy[i] * DT;
        e[i] -= (vx[i] * vx[i] + vy[i] * vy[i]).sqrt() * BURN;
    }
}

/// Run `f` until at least TARGET_NS has elapsed; return ns per creature.
fn bench(n: usize, mut f: impl FnMut()) -> f64 {
    let mut iters = 1u32;
    loop {
        let t0 = Instant::now();
        for _ in 0..iters { f(); }
        let ns = t0.elapsed().as_nanos();
        if ns >= TARGET_NS || iters >= (1 << 24) {
            return ns as f64 / (iters as f64 * n as f64);
        }
        iters *= 2;
    }
}

fn main() {
    println!("motion: reads 20 B/creature (pos+vel+energy), writes pos+energy");
    println!("{:>11} {:>10} {:>12} {:>12} {:>12}   {:>10} {:>10}",
             "creatures", "hot KB", "SoA ns/cr", "AoS ns/cr", "rand ns/cr",
             "AoS/SoA", "rand/seq");
    for &n in SIZES {
        // SoA columns.
        let mut px = vec![0.0f32; n];
        let mut py = vec![0.0f32; n];
        let vx: Vec<f32> = (0..n).map(|i| ((i % 7) as f32) - 3.0).collect();
        let vy: Vec<f32> = (0..n).map(|i| ((i % 5) as f32) - 2.0).collect();
        let mut e = vec![100.0f32; n];

        // AoS rows carrying the same hot data plus cold padding.
        let mut cs: Vec<Creature> = (0..n)
            .map(|i| Creature {
                pos: [0.0, 0.0],
                vel: [((i % 7) as f32) - 3.0, ((i % 5) as f32) - 2.0],
                energy: 100.0,
                _cold: [0; 20],
            })
            .collect();

        // Shuffled visit order for the random pass (LCG Fisher-Yates).
        let mut idx: Vec<u32> = (0..n as u32).collect();
        let mut s = 0x9E37_79B9_u64;
        let mut rng = || { s = s.wrapping_mul(6364136223846793005).wrapping_add(1); s };
        for i in (1..idx.len()).rev() {
            let j = (rng() % (i as u64 + 1)) as usize;
            idx.swap(i, j);
        }

        let soa = bench(n, || {
            motion_soa(std::hint::black_box(&mut px), &mut py, &vx, &vy, &mut e);
        });
        let aos = bench(n, || {
            motion_aos(std::hint::black_box(&mut cs));
        });
        let rand = bench(n, || {
            motion_soa_random(std::hint::black_box(&mut px), &mut py, &vx, &vy, &mut e, &idx);
        });

        // Defeat dead-code elimination on the mutated state.
        std::hint::black_box((&px, &py, &e, &cs));

        println!("{:>11} {:>10} {:>12.2} {:>12.2} {:>12.2}   {:>9.2}x {:>9.2}x",
                 n, n * 20 / 1024, soa, aos, rand, aos / soa, rand / soa);
        std::io::stdout().flush().unwrap();
    }
}
