//! proximity - evidence for the §28 reframe: proximity is a property of
//! position, computed where position already lives, not a bolt-on index.
//!
//!     cargo run --release --bin proximity
//!
//! Part A (encounter queries). For N agents in a 2D box, answer "how many
//! others are within radius r of each agent" three ways:
//!   1. all-pairs   - each agent tests every other.            O(N^2)
//!   2. bolt-on hash - HashMap<cell, Vec<u32>>, the reflex "spatial index":
//!                     per-cell heap allocation, pointer-chased buckets.
//!   3. dense bin   - compute each agent's cell, counting-sort the indices
//!                     into one CSR bucket array (offsets + items), then read
//!                     a 3x3 block of contiguous ranges. No per-cell alloc, no
//!                     maintained structure; regenerated from the position
//!                     stream each tick. This is the work the motion system
//!                     already has the data to do.
//!
//! Part B (coordination / the pack-leader). Swarm cohesion: steer each agent
//! toward the group it belongs to.
//!   1. all-pairs   - each agent averages the other N-1 positions.   O(N^2)
//!   2. leader      - one pass computes the centroid; each agent reads that
//!                    single value and steers toward it.             O(N)

use std::collections::HashMap;
use std::hint::black_box;
use std::time::Instant;

const SIZE: f32 = 1000.0; // world is SIZE x SIZE
const R: f32 = 4.0;       // interaction radius; cell size = R
const GX: usize = (SIZE / R) as usize; // cells per axis
const NCELLS: usize = GX * GX;

#[inline(always)]
fn cell_of(x: f32, y: f32) -> usize {
    let cx = (x / R) as usize;
    let cy = (y / R) as usize;
    let cx = cx.min(GX - 1);
    let cy = cy.min(GX - 1);
    cy * GX + cx
}

// Deterministic scatter of N points across the box.
fn positions(n: usize) -> (Vec<f32>, Vec<f32>) {
    let mut px = Vec::with_capacity(n);
    let mut py = Vec::with_capacity(n);
    for i in 0..n {
        let a = (i.wrapping_mul(2_654_435_761) >> 8) as u32;
        let b = (i.wrapping_mul(40_503) ^ 0x9E3779B9) as u32;
        px.push((a % 100_000) as f32 / 100_000.0 * SIZE);
        py.push((b % 100_000) as f32 / 100_000.0 * SIZE);
    }
    (px, py)
}

fn near(px: &[f32], py: &[f32], i: usize, j: usize) -> bool {
    let dx = px[i] - px[j];
    let dy = py[i] - py[j];
    dx * dx + dy * dy <= R * R
}

// ---- 1. all-pairs ----
#[inline(never)]
fn count_all_pairs(px: &[f32], py: &[f32]) -> u64 {
    let n = px.len();
    let mut total = 0u64;
    for i in 0..n {
        for j in 0..n {
            if i != j && near(px, py, i, j) {
                total += 1;
            }
        }
    }
    total
}

// ---- 2. bolt-on hash map ----
fn count_hash(px: &[f32], py: &[f32]) -> (u64, f64, f64) {
    let n = px.len();
    let t0 = Instant::now();
    let mut grid: HashMap<usize, Vec<u32>> = HashMap::new();
    for i in 0..n {
        grid.entry(cell_of(px[i], py[i])).or_default().push(i as u32);
    }
    let build = t0.elapsed().as_nanos() as f64;

    let t0 = Instant::now();
    let mut total = 0u64;
    for i in 0..n {
        let cx = ((px[i] / R) as usize).min(GX - 1) as isize;
        let cy = ((py[i] / R) as usize).min(GX - 1) as isize;
        for ny in (cy - 1)..=(cy + 1) {
            for nx in (cx - 1)..=(cx + 1) {
                if nx < 0 || ny < 0 || nx >= GX as isize || ny >= GX as isize {
                    continue;
                }
                let c = ny as usize * GX + nx as usize;
                if let Some(bucket) = grid.get(&c) {
                    for &j in bucket {
                        let j = j as usize;
                        if i != j && near(px, py, i, j) {
                            total += 1;
                        }
                    }
                }
            }
        }
    }
    let query = t0.elapsed().as_nanos() as f64;
    (total, build, query)
}

// ---- 3. dense binning (CSR via counting sort) ----
fn count_dense(px: &[f32], py: &[f32]) -> (u64, f64, f64) {
    let n = px.len();
    let t0 = Instant::now();
    // cell id per agent (the SIMD-friendly byproduct of the position stream)
    let cell: Vec<u32> = (0..n).map(|i| cell_of(px[i], py[i]) as u32).collect();
    // histogram -> prefix sum -> scatter, all dense, no allocation per cell
    let mut offsets = vec![0u32; NCELLS + 1];
    for &c in &cell {
        offsets[c as usize + 1] += 1;
    }
    for c in 0..NCELLS {
        offsets[c + 1] += offsets[c];
    }
    let mut items = vec![0u32; n];
    let mut cursor = offsets.clone();
    for i in 0..n {
        let c = cell[i] as usize;
        items[cursor[c] as usize] = i as u32;
        cursor[c] += 1;
    }
    let build = t0.elapsed().as_nanos() as f64;

    let t0 = Instant::now();
    let mut total = 0u64;
    for i in 0..n {
        let cx = ((px[i] / R) as usize).min(GX - 1) as isize;
        let cy = ((py[i] / R) as usize).min(GX - 1) as isize;
        for ny in (cy - 1)..=(cy + 1) {
            for nx in (cx - 1)..=(cx + 1) {
                if nx < 0 || ny < 0 || nx >= GX as isize || ny >= GX as isize {
                    continue;
                }
                let c = ny as usize * GX + nx as usize;
                let lo = offsets[c] as usize;
                let hi = offsets[c + 1] as usize;
                for &j in &items[lo..hi] {
                    let j = j as usize;
                    if i != j && near(px, py, i, j) {
                        total += 1;
                    }
                }
            }
        }
    }
    let query = t0.elapsed().as_nanos() as f64;
    (total, build, query)
}

fn ms(ns: f64) -> f64 {
    ns / 1_000_000.0
}

fn main() {
    println!("proximity - §28 evidence (R = {R}, grid {GX}x{GX} = {NCELLS} cells)\n");

    // ---- Part A: encounter queries ----
    println!("Part A (encounter queries): all-pairs vs bolt-on hash vs dense bin");

    let small = 20_000usize;
    let (px, py) = positions(small);
    let ap = {
        let t0 = Instant::now();
        let c = count_all_pairs(black_box(&px), black_box(&py));
        black_box(c);
        ms(t0.elapsed().as_nanos() as f64)
    };
    let (ch, hb, hq) = count_hash(&px, &py);
    let (cd, db, dq) = count_dense(&px, &py);
    assert_eq!(ch, cd, "hash and dense must agree");
    println!("  N = {small}:");
    println!("    all-pairs      {:>9.2} ms", ap);
    println!("    bolt-on hash   {:>9.2} ms  (build {:.2} + query {:.2})", ms(hb + hq), ms(hb), ms(hq));
    println!("    dense bin      {:>9.2} ms  (build {:.2} + query {:.2})", ms(db + dq), ms(db), ms(dq));

    let big = 1_000_000usize;
    let (px, py) = positions(big);
    // warm both
    for _ in 0..2 {
        let _ = count_hash(&px, &py);
        let _ = count_dense(&px, &py);
    }
    let (ch, hb, hq) = count_hash(&px, &py);
    let (cd, db, dq) = count_dense(&px, &py);
    assert_eq!(ch, cd, "hash and dense must agree");
    println!("  N = {big} (all-pairs would be ~{:.0}e9 tests, skipped):", (big as f64 * big as f64) / 1e9);
    println!("    bolt-on hash   {:>9.2} ms  (build {:.2} + query {:.2})", ms(hb + hq), ms(hb), ms(hq));
    println!("    dense bin      {:>9.2} ms  (build {:.2} + query {:.2})", ms(db + dq), ms(db), ms(dq));
    println!("    dense is {:.1}x faster end to end", (hb + hq) / (db + dq));

    // ---- Part B: coordination / pack-leader ----
    println!("\nPart B (coordination): all-pairs cohesion vs one leader/centroid");
    let n = 20_000usize;
    let (px, py) = positions(n);

    // all-pairs: each agent steers toward the average of every other agent.
    let t0 = Instant::now();
    let mut steer_x = vec![0.0f32; n];
    let mut steer_y = vec![0.0f32; n];
    for i in 0..n {
        let mut sx = 0.0f32;
        let mut sy = 0.0f32;
        for j in 0..n {
            if i != j {
                sx += px[j];
                sy += py[j];
            }
        }
        let inv = 1.0 / (n - 1) as f32;
        steer_x[i] = sx * inv - px[i];
        steer_y[i] = sy * inv - py[i];
    }
    black_box((&steer_x, &steer_y));
    let pairs = ms(t0.elapsed().as_nanos() as f64);

    // leader: one pass for the centroid; each agent reads that single value.
    let t0 = Instant::now();
    let mut cx = 0.0f32;
    let mut cy = 0.0f32;
    for i in 0..n {
        cx += px[i];
        cy += py[i];
    }
    cx /= n as f32;
    cy /= n as f32;
    for i in 0..n {
        steer_x[i] = cx - px[i];
        steer_y[i] = cy - py[i];
    }
    black_box((&steer_x, &steer_y, cx, cy));
    let leader = ms(t0.elapsed().as_nanos() as f64);

    println!("  N = {n}:");
    println!("    all-pairs cohesion {:>9.2} ms", pairs);
    println!("    one leader/centroid {:>8.3} ms", leader);
    println!("    leader is {:.0}x cheaper, and the gap grows linearly with N", pairs / leader);
}
