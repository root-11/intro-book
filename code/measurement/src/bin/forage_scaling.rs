//! forage_scaling - Rust reconstruction of the Python edition's forage arc, to VERIFY by
//! measurement which lessons port to Rust and which diverge. Backs the §28/§43/Part II reframe.
//!
//!     cargo run --release --bin forage_scaling
//!
//! Hypotheses under test (measure, do not assume):
//!   H1  binning is O(N) only at constant density; in a fixed world density grows with N and
//!       the grid is O(N^2) again.                              (geometric - expect HOLDS)
//!   H2  collapsing each cell to one representative restores O(N) even in a fixed world, by
//!       bounding the per-target candidate count at <=9.        (expect HOLDS)
//!   H3  the numpy cache-blocking win does NOT reproduce. The numpy kernel built ~90M-element
//!       candidate arrays and round-tripped them through RAM; blocking kept them in cache for
//!       1.4x. The Rust kernel keeps O(1) scratch per target - no intermediate arrays - so
//!       there is no temporary-traffic wall to block.           (expect DIVERGES: blocking ~nil)
//!   H4  slice-the-work parallelism scales without a GIL, and single == multi bit-for-bit
//!       (per-target argmin is order-free; no float reduction across targets).

use std::hint::black_box;
use std::time::Instant;

struct Grid {
    r: f32,
    gx: usize,
}
impl Grid {
    fn new(world: f32, r: f32) -> Self {
        Grid { r, gx: (world / r) as usize + 1 }
    }
    fn ncells(&self) -> usize {
        self.gx * self.gx
    }
    #[inline(always)]
    fn cell(&self, x: f32, y: f32) -> usize {
        let cx = ((x / self.r) as usize).min(self.gx - 1);
        let cy = ((y / self.r) as usize).min(self.gx - 1);
        cy * self.gx + cx
    }
}

// Deterministic scatter of n points across [0, world); `salt` separates foragers from targets.
fn positions(n: usize, world: f32, salt: usize) -> (Vec<f32>, Vec<f32>) {
    let mut px = Vec::with_capacity(n);
    let mut py = Vec::with_capacity(n);
    for i in 0..n {
        let i = i.wrapping_add(salt.wrapping_mul(0x9E37_79B9));
        let a = (i.wrapping_mul(2_654_435_761) >> 8) as u32;
        let b = (i.wrapping_mul(40_503) ^ 0x9E37_79B9) as u32;
        px.push((a % 100_000) as f32 / 100_000.0 * world);
        py.push((b % 100_000) as f32 / 100_000.0 * world);
    }
    (px, py)
}

// CSR of foragers grouped by cell (counting sort).
fn build_csr(g: &Grid, fx: &[f32], fy: &[f32]) -> (Vec<u32>, Vec<u32>) {
    let n = fx.len();
    let cells: Vec<u32> = (0..n).map(|i| g.cell(fx[i], fy[i]) as u32).collect();
    let mut offsets = vec![0u32; g.ncells() + 1];
    for &c in &cells {
        offsets[c as usize + 1] += 1;
    }
    for c in 0..g.ncells() {
        offsets[c + 1] += offsets[c];
    }
    let mut items = vec![0u32; n];
    let mut cur = offsets.clone();
    for i in 0..n {
        let c = cells[i] as usize;
        items[cur[c] as usize] = i as u32;
        cur[c] += 1;
    }
    (offsets, items)
}

// One representative forager per cell (last-write-wins, deterministic).
fn build_rep(g: &Grid, fx: &[f32], fy: &[f32]) -> Vec<i32> {
    let mut rep = vec![-1i32; g.ncells()];
    for f in 0..fx.len() {
        rep[g.cell(fx[f], fy[f])] = f as i32;
    }
    rep
}

// Naive: each target scans EVERY forager in its 3x3 block. O(targets * foragers-per-cell).
fn forage_naive(
    g: &Grid, fx: &[f32], fy: &[f32], tx: &[f32], ty: &[f32],
    offsets: &[u32], items: &[u32], out: &mut [i32],
) {
    let r2 = g.r * g.r;
    for t in 0..tx.len() {
        let cx = ((tx[t] / g.r) as isize).min(g.gx as isize - 1);
        let cy = ((ty[t] / g.r) as isize).min(g.gx as isize - 1);
        let (mut bd, mut bf) = (f32::INFINITY, -1i32);
        for ny in (cy - 1)..=(cy + 1) {
            for nx in (cx - 1)..=(cx + 1) {
                if nx < 0 || ny < 0 || nx >= g.gx as isize || ny >= g.gx as isize {
                    continue;
                }
                let c = ny as usize * g.gx + nx as usize;
                for &f in &items[offsets[c] as usize..offsets[c + 1] as usize] {
                    let f = f as usize;
                    let dx = tx[t] - fx[f];
                    let dy = ty[t] - fy[f];
                    let d2 = dx * dx + dy * dy;
                    if d2 <= r2 && d2 < bd {
                        bd = d2;
                        bf = f as i32;
                    }
                }
            }
        }
        out[t] = bf;
    }
}

// Representative: each target scans the <=9 cell representatives. Bounded regardless of density.
// `out` is the slice for targets [lo, lo + out.len()); writes only its own slot (disjoint).
fn forage_rep(
    g: &Grid, fx: &[f32], fy: &[f32], tx: &[f32], ty: &[f32], rep: &[i32], lo: usize, out: &mut [i32],
) {
    let r2 = g.r * g.r;
    for k in 0..out.len() {
        let t = lo + k;
        let cx = ((tx[t] / g.r) as isize).min(g.gx as isize - 1);
        let cy = ((ty[t] / g.r) as isize).min(g.gx as isize - 1);
        let (mut bd, mut bf) = (f32::INFINITY, -1i32);
        for ny in (cy - 1)..=(cy + 1) {
            for nx in (cx - 1)..=(cx + 1) {
                if nx < 0 || ny < 0 || nx >= g.gx as isize || ny >= g.gx as isize {
                    continue;
                }
                let f = rep[ny as usize * g.gx + nx as usize];
                if f >= 0 {
                    let f = f as usize;
                    let dx = tx[t] - fx[f];
                    let dy = ty[t] - fy[f];
                    let d2 = dx * dx + dy * dy;
                    if d2 <= r2 && d2 < bd {
                        bd = d2;
                        bf = f as i32;
                    }
                }
            }
        }
        out[k] = bf;
    }
}

fn bench<F: FnMut()>(mut f: F) -> f64 {
    f(); // warm
    let mut best = f64::INFINITY;
    for _ in 0..3 {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_secs_f64());
    }
    best * 1000.0
}

fn split(n: usize) -> (usize, usize) {
    let nf = (n * 2 / 5).max(1); // 40% foragers
    (nf, n - nf)
}

fn main() {
    println!("forage_scaling - Rust reconstruction of the Python forage arc (verify, don't assume)\n");
    let scales = [100_000usize, 300_000, 1_000_000];

    // H1: naive grid. Fixed world (density grows) vs constant density (world grows with N).
    println!("H1  naive grid: fixed world=100 (density ~N) vs constant density (world=sqrt(0.1N))");
    println!("    {:>9} {:>7} {:>9} {:>8}", "pop", "world", "ms", "growth/3x");
    for (tag, world_of) in [("fixed ", (|_n: usize| 100.0f32) as fn(usize) -> f32),
                            ("const ", (|n: usize| (0.1 * n as f32).sqrt()) as fn(usize) -> f32)] {
        let mut prev = 0.0;
        for (i, &n) in scales.iter().enumerate() {
            let (nf, nt) = split(n);
            let world = world_of(n);
            let g = Grid::new(world, 2.0);
            let (fx, fy) = positions(nf, world, 0);
            let (tx, ty) = positions(nt, world, 1);
            let (off, items) = build_csr(&g, &fx, &fy);
            let mut out = vec![-1i32; nt];
            let ms = bench(|| {
                forage_naive(&g, &fx, &fy, &tx, &ty, &off, &items, black_box(&mut out));
            });
            let gr = if i == 0 { 0.0 } else { ms / prev };
            prev = ms;
            println!("    {tag}{:>7} {:>7.0} {:>9.2} {:>7.1}x", n, world, ms, gr);
        }
    }

    // H2: representative, fixed world=100. Expect linear despite growing density.
    println!("\nH2  representative, fixed world=100 (the fix - O(N) even at fixed density):");
    println!("    {:>9} {:>9} {:>8}", "pop", "ms", "growth/3x");
    let mut prev = 0.0;
    for (i, &n) in scales.iter().enumerate() {
        let (nf, nt) = split(n);
        let g = Grid::new(100.0, 2.0);
        let (fx, fy) = positions(nf, 100.0, 0);
        let (tx, ty) = positions(nt, 100.0, 1);
        let rep = build_rep(&g, &fx, &fy);
        let mut out = vec![-1i32; nt];
        let ms = bench(|| forage_rep(&g, &fx, &fy, &tx, &ty, &rep, 0, black_box(&mut out)));
        let gr = if i == 0 { 0.0 } else { ms / prev };
        prev = ms;
        println!("    {:>9} {:>9.2} {:>7.1}x", n, ms, gr);
    }

    // H3: does cache-blocking help in Rust? (numpy got 1.4x; Rust has no temporaries to block.)
    println!("\nH3  representative at 2M, fixed world=100: whole vs cache-blocked (block=20k)");
    {
        let n = 2_000_000usize;
        let (nf, nt) = split(n);
        let g = Grid::new(100.0, 2.0);
        let (fx, fy) = positions(nf, 100.0, 0);
        let (tx, ty) = positions(nt, 100.0, 1);
        let rep = build_rep(&g, &fx, &fy);
        let mut out = vec![-1i32; nt];
        let whole = bench(|| forage_rep(&g, &fx, &fy, &tx, &ty, &rep, 0, black_box(&mut out)));
        let blocked = bench(|| {
            let block = 20_000;
            let mut lo = 0;
            while lo < nt {
                let hi = (lo + block).min(nt);
                forage_rep(&g, &fx, &fy, &tx, &ty, &rep, lo, &mut out[lo..hi]);
                lo = hi;
            }
            black_box(&out);
        });
        println!("    whole   {:>8.2} ms", whole);
        println!("    blocked {:>8.2} ms   ({:.2}x - numpy got 1.4x here)", blocked, whole / blocked);
    }

    // H4: slice-the-work parallelism + single == multi bit-for-bit.
    println!("\nH4  representative at 2M, fixed world=100: slice the work across threads");
    println!("    {:>7} {:>9} {:>8}   identical to serial", "threads", "ms", "speedup");
    {
        let n = 2_000_000usize;
        let (nf, nt) = split(n);
        let g = Grid::new(100.0, 2.0);
        let (fx, fy) = positions(nf, 100.0, 0);
        let (tx, ty) = positions(nt, 100.0, 1);
        let rep = build_rep(&g, &fx, &fy);

        let mut serial = vec![-1i32; nt];
        let s_ms = bench(|| forage_rep(&g, &fx, &fy, &tx, &ty, &rep, 0, black_box(&mut serial)));
        println!("    {:>7} {:>9.2} {:>7.1}x   -", "serial", s_ms, 1.0);

        for threads in [1usize, 2, 4, 8] {
            let mut out = vec![-1i32; nt];
            let ms = bench(|| {
                let chunk = (nt + threads - 1) / threads;
                let (g, fx, fy, tx, ty, rep) = (&g, &fx, &fy, &tx, &ty, &rep);
                std::thread::scope(|s| {
                    for (ti, oc) in out.chunks_mut(chunk).enumerate() {
                        let lo = ti * chunk;
                        s.spawn(move || forage_rep(g, fx, fy, tx, ty, rep, lo, oc));
                    }
                });
            });
            let ok = out == serial;
            println!("    {:>7} {:>9.2} {:>7.1}x   {}", threads, ms, s_ms / ms, ok);
            assert!(ok, "NON-DETERMINISM: {threads} threads != serial");
        }
    }
}
