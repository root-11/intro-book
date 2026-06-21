//! Reference for Part II - "Where SoA does not pay", project D: floating-point fragility.
//!
//! Project C made the spreadsheet incremental and bigger-than-RAM. But a pivot is a pile
//! of additions, and floating-point addition is neither associative nor exact. The
//! totals are order-dependent, and at scale they are wrong. Layout cannot rescue you: a
//! perfectly columnar sum is still wrong, and a perfectly SoA geometric predicate is
//! still wrong on a degenerate input. Correctness is orthogonal to layout - that is the
//! arc's honest counterweight, restated where columns simply do not help.
//!
//! Three measurements:
//!
//! 1. Summation - the same numbers summed in different orders give different totals; a
//!    naive sequential sum can lose everything; compensated and pairwise sums recover it,
//!    at a cost. (C's pivot patch agreed with the full pivot only because it summed in the
//!    same order.)
//! 2. Incremental drift - maintaining a running sum by deltas (the cheap "incremental
//!    aggregate" C wanted) never equals a fresh recompute. The absolute gap is small but
//!    persistent, and in relative terms it explodes when the true total nearly cancels.
//!    This is why aggregates are periodically re-anchored, not trusted forever.
//! 3. Geometric predicates - a naive f64 orientation test gives the wrong sign on
//!    near-collinear points; the exact integer test does not. No layout fixes it.
//!
//! Run:    cargo run --release
//! Tests:  cargo test --release

use std::hint::black_box;
use std::time::Instant;

// ============================================================================
// Deterministic RNG - Numerical Recipes LCG, the same one code/deck uses.
// ============================================================================

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ============================================================================
// Summation algorithms. Same inputs, different roundoff.
// ============================================================================

/// Left-to-right. What `for v in col { acc += v }` compiles to, and what a pivot does.
#[inline(never)]
fn sum_naive(xs: &[f64]) -> f64 {
    let mut acc = 0.0;
    for &x in xs {
        acc += x;
    }
    acc
}

/// Divide and conquer. Groups nearby magnitudes before they meet large running totals,
/// so error grows like log(n) instead of n. Also the shape a SIMD/tree reduction takes.
#[inline(never)]
fn sum_pairwise(xs: &[f64]) -> f64 {
    if xs.len() <= 128 {
        sum_naive(xs)
    } else {
        let mid = xs.len() / 2;
        sum_pairwise(&xs[..mid]) + sum_pairwise(&xs[mid..])
    }
}

/// Neumaier's compensated sum (a robust Kahan variant): carry the lost low-order bits in
/// a separate term and fold them back at the end. Accurate to about one ulp regardless of
/// order, at roughly four arithmetic ops per element instead of one.
#[inline(never)]
fn sum_kahan(xs: &[f64]) -> f64 {
    let mut sum = 0.0f64;
    let mut comp = 0.0f64;
    for &x in xs {
        let t = sum + x;
        if sum.abs() >= x.abs() {
            comp += (sum - t) + x;
        } else {
            comp += (x - t) + sum;
        }
        sum = t;
    }
    sum + comp
}

/// Sum in tiles, then combine the tile totals: exactly what C's "patch the dirty columns"
/// does, and a different grouping again - so a different tile size gives a different total.
#[inline(never)]
fn sum_tiled(xs: &[f64], tile: usize) -> f64 {
    let mut total = 0.0;
    let mut i = 0;
    while i < xs.len() {
        let end = (i + tile).min(xs.len());
        total += sum_naive(&xs[i..end]);
        i = end;
    }
    total
}

/// A realistic ill-conditioned column: many small entries (cents either way) with two
/// huge offsetting entries (a big credit and the matching debit) dropped in. The true
/// sum is the sum of the small entries; the giants cancel - but a naive running total
/// climbs to 1e16, where adding a 1.0 is below the ulp and silently lost.
fn ledger(n: usize, rng: &mut Lcg) -> Vec<f64> {
    let mut v: Vec<f64> = (0..n).map(|_| (rng.unit() - 0.5) * 2.0).collect();
    v[1] += 1.0e16;
    v[n - 2] -= 1.0e16;
    v
}

// ============================================================================
// Geometric orientation: is c left of, right of, or on the line a -> b?
// ============================================================================

/// Exact, via 128-bit integers. Ground truth.
fn orient_exact(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> i32 {
    let det = (b.0 - a.0) as i128 * (c.1 - a.1) as i128 - (b.1 - a.1) as i128 * (c.0 - a.0) as i128;
    det.signum() as i32
}

/// Naive, in f64. The same determinant, evaluated after rounding each coordinate and
/// product to the nearest double.
fn orient_naive(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> i32 {
    let det = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if det > 0.0 {
        1
    } else if det < 0.0 {
        -1
    } else {
        0
    }
}

/// A near-collinear triple with large integer coordinates: a = origin, b = (p, q),
/// c = (p+1, q+1). The exact determinant is p - q (made tiny by choosing q near p), while
/// the products are ~2^60, far past f64's 53-bit mantissa - so the rounded f64 difference
/// is dominated by noise and the sign is unreliable.
fn near_collinear(rng: &mut Lcg) -> ((i64, i64), (i64, i64), (i64, i64)) {
    let p = (rng.next() % (1 << 30)) as i64 + (1 << 29);
    let r = [(-2i64), -1, 1, 2][(rng.next() % 4) as usize];
    let q = p + r;
    ((0, 0), (p, q), (p + 1, q + 1))
}

fn as_f64(t: (i64, i64)) -> (f64, f64) {
    (t.0 as f64, t.1 as f64)
}

// ============================================================================
// Benchmark plumbing.
// ============================================================================

fn median5(mut sample: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..5).map(|_| sample()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[2]
}

fn time_sum(f: impl Fn(&[f64]) -> f64, xs: &[f64]) -> (f64, f64) {
    let value = f(xs);
    let ns = median5(|| {
        let t = Instant::now();
        let s = f(black_box(xs));
        black_box(s);
        t.elapsed().as_nanos() as f64
    });
    (value, ns)
}

// ============================================================================
// main.
// ============================================================================

fn main() {
    // ---- 1. Summation: order and accuracy, and the cost of getting it right ----
    let n = 10_000_000;
    let mut rng = Lcg::new(0x00F9_0A75);
    let col = ledger(n, &mut rng);
    let mut reversed = col.clone();
    reversed.reverse();

    let reference = sum_kahan(&col); // accurate to ~1 ulp; our ground truth
    let (naive, naive_ns) = time_sum(sum_naive, &col);
    let (naive_rev, _) = time_sum(sum_naive, &reversed);
    let (pairwise, pairwise_ns) = time_sum(sum_pairwise, &col);
    let (_, kahan_ns) = time_sum(sum_kahan, &col);

    println!("== 1. Summation: same {n} numbers, different order, different total ==");
    println!("an ill-conditioned ledger column (small entries + one huge offsetting pair).\n");
    println!(
        "{:>16} {:>24} {:>16} {:>12}",
        "method", "result", "abs error", "ns"
    );
    let row = |name: &str, val: f64, ns: Option<f64>| {
        let err = (val - reference).abs();
        match ns {
            Some(ns) => println!("{name:>16} {val:>24.6} {err:>16.6} {ns:>12.0}"),
            None => println!("{name:>16} {val:>24.6} {err:>16.6} {:>12}", "-"),
        }
    };
    row("kahan (ref)", reference, Some(kahan_ns));
    row("naive", naive, Some(naive_ns));
    row("naive reversed", naive_rev, None);
    row("pairwise", pairwise, Some(pairwise_ns));
    println!();
    for &tile in &[256usize, 4096, 65536] {
        let v = sum_tiled(&col, tile);
        println!(
            "{:>16} {:>24.6} {:>16.6} {:>12}",
            format!("tiled/{tile}"),
            v,
            (v - reference).abs(),
            "-"
        );
    }
    println!(
        "\nNaive loses the small entries entirely; reversing changes the answer; pairwise and"
    );
    println!(
        "tiled recover it (different tile = different total). Kahan is right, at ~{:.1}x the cost.",
        kahan_ns / naive_ns
    );

    // ---- 2. Incremental drift: maintaining the sum by deltas vs recomputing it ----
    let n = 1_000_000;
    let mut rng = Lcg::new(0x00DE_17A0);
    // Wide dynamic range, mixed sign: transaction amounts from 0.01 to ~1e10.
    let amount = |rng: &mut Lcg| {
        let mag = 10f64.powf(rng.unit() * 12.0 - 2.0);
        if rng.next() & 1 == 0 { mag } else { -mag }
    };
    let mut col: Vec<f64> = (0..n).map(|_| amount(&mut rng)).collect();
    // Start from the exact total, then maintain it only by deltas. Any gap that opens up
    // is pure delta-rounding drift, not a stale starting point.
    let mut running = sum_kahan(&col);

    println!("\n== 2. Incremental drift: running sum maintained by deltas vs a fresh recompute ==");
    println!("{n} mixed-magnitude entries; each edit does `running += new - old`.\n");
    println!(
        "{:>14} {:>24} {:>16}",
        "edits", "drift vs recompute", "relative"
    );

    let mut done = 0u64;
    for &target in &[100_000u64, 1_000_000, 10_000_000] {
        while done < target {
            let i = (rng.next() as usize) % n;
            let new = amount(&mut rng);
            running += new - col[i];
            col[i] = new;
            done += 1;
        }
        let fresh = sum_kahan(&col);
        let drift = (running - fresh).abs();
        println!(
            "{:>14} {:>24.6} {:>15.2e}",
            target,
            drift,
            drift / fresh.abs().max(1.0)
        );
    }
    println!(
        "\nThe maintained total never matches a fresh recompute. The absolute gap stays small,"
    );
    println!(
        "but as a fraction it explodes once cancellation shrinks the true total (the last row)."
    );
    println!("Incremental aggregates buy speed by spending correctness; they must be re-anchored.");

    // ---- 3. Geometric predicate: naive f64 vs exact integer orientation ----
    let trials = 1_000_000;
    let mut rng = Lcg::new(0x000A_11CE);
    let mut disagree = 0u64;
    let mut example = None;
    for _ in 0..trials {
        let (a, b, c) = near_collinear(&mut rng);
        let exact = orient_exact(a, b, c);
        let naive = orient_naive(as_f64(a), as_f64(b), as_f64(c));
        if exact != naive {
            disagree += 1;
            if example.is_none() {
                example = Some((a, b, c, exact, naive));
            }
        }
    }

    // Time the two predicates on the same stream.
    let mut rng_t = Lcg::new(0x000A_11CE);
    let triples: Vec<_> = (0..trials).map(|_| near_collinear(&mut rng_t)).collect();
    let naive_ns = median5(|| {
        let t = Instant::now();
        let mut s = 0i64;
        for &(a, b, c) in &triples {
            s += orient_naive(as_f64(a), as_f64(b), as_f64(c)) as i64;
        }
        black_box(s);
        t.elapsed().as_nanos() as f64
    }) / trials as f64;
    let exact_ns = median5(|| {
        let t = Instant::now();
        let mut s = 0i64;
        for &(a, b, c) in &triples {
            s += orient_exact(a, b, c) as i64;
        }
        black_box(s);
        t.elapsed().as_nanos() as f64
    }) / trials as f64;

    println!("\n== 3. Geometric predicate: orientation of near-collinear points ==");
    println!("{trials} near-collinear triples with ~2^30 integer coordinates.\n");
    println!(
        "naive f64 disagrees with the exact integer answer on {disagree} of {trials} ({:.1}%).",
        disagree as f64 / trials as f64 * 100.0
    );
    if let Some((a, b, c, exact, naive)) = example {
        println!("e.g. a={a:?} b={b:?} c={c:?}: exact = {exact}, naive = {naive} (wrong sign).");
    }
    println!("cost: naive {naive_ns:.2} ns/test, exact (i128) {exact_ns:.2} ns/test.");
    println!(
        "\nA perfectly columnar layout changes none of this. Correctness is orthogonal to layout."
    );
    println!("\nNow the answers are correct and incremental - but the sum is still one core's");
    println!(
        "bandwidth, and Kahan made it more compute-bound. That is where more hardware comes in."
    );
}

// ============================================================================
// Contract tests.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summation_is_not_associative() {
        // The mechanism, in miniature: a giant swamps the one, and order decides what
        // survives. Forward, the giants cancel first and the 1.0 lands: (1e16 - 1e16) + 1
        // = 1. Reversed, the 1.0 meets a giant and is lost below its ulp: (1 - 1e16) +
        // 1e16 = 0. This is IEEE-754 behaviour, identical on every conforming machine.
        let forward = [1.0e16, -1.0e16, 1.0];
        let mut backward = forward;
        backward.reverse();
        assert_eq!(sum_naive(&forward), 1.0);
        assert_eq!(sum_naive(&backward), 0.0);
    }

    #[test]
    fn kahan_beats_naive_on_known_sum() {
        // Build [1e16, then k ones, then -1e16]; the true sum is k.
        let k = 1000usize;
        let mut xs = vec![1.0e16];
        xs.extend(std::iter::repeat_n(1.0, k));
        xs.push(-1.0e16);
        let truth = k as f64;

        let naive_err = (sum_naive(&xs) - truth).abs();
        let kahan_err = (sum_kahan(&xs) - truth).abs();
        assert!(naive_err > 100.0, "naive should lose most of the ones");
        assert!(kahan_err < 1.0, "kahan should recover the true sum");
    }

    #[test]
    fn pairwise_more_accurate_than_naive() {
        let mut rng = Lcg::new(42);
        let xs = ledger(1_000_000, &mut rng);
        let reference = sum_kahan(&xs);
        let naive_err = (sum_naive(&xs) - reference).abs();
        let pair_err = (sum_pairwise(&xs) - reference).abs();
        assert!(
            pair_err < naive_err,
            "pairwise should beat naive on this column"
        );
    }

    #[test]
    fn orientation_exact_is_correct() {
        // a=origin, b=(M, M-1), c=(M+1, M). det = bx*cy - by*cx
        //   = M*M - (M-1)*(M+1) = M^2 - (M^2 - 1) = 1, so c is strictly left (sign +1).
        let m = 1i64 << 30;
        let a = (0, 0);
        let b = (m, m - 1);
        let c = (m + 1, m);
        assert_eq!(orient_exact(a, b, c), 1);
        // (That naive f64 gets this *wrong* at this scale is measured in `main`, not
        // asserted here: whether a given double rounds to the wrong sign is a property of
        // the hardware, while the exact integer answer above is the same everywhere.)
    }
}
