//! Reference for Part II - "Where SoA does not pay", project C: the spreadsheet recalc.
//!
//! This is where the arc pays off. A spreadsheet cell holds a *formula* - an expression
//! tree over other cells (project A, used as the per-cell evaluator) - and the cells
//! form a *dependency DAG*. Recalculation is a topological sort of that DAG: exactly
//! "the program is a topological sort of who-reads-what-who-wrote" (book node 14), now
//! executable. Dirty propagation, which project B did down a tree, now runs through a
//! graph, and the tree's contiguous-subtree shortcut is gone.
//!
//! Three recalc strategies are measured:
//!
//! - Full: recompute every cell in topological order. O(N), branchless.
//! - Cone: recompute only the transitive dependents of what changed.
//! - Cone + cutoff: the same, but stop propagating at any cell whose recomputed value
//!   did not actually change. "Validation is cheaper than recompute": you do not push a
//!   change that did not happen.
//!
//! The nature of the change matters, and it is domain-specific. In a scenegraph (B) a
//! frame's movers scatter at random across the tree. A spreadsheet cannot produce a
//! random scatter: the UI gives you a single-cell edit or a contiguous fill-down, and
//! the dirty set is then the *dependency cone* of that edit, its size and shape fixed
//! by the formula topology. So the crossover here is swept by a fill-down of k cells -
//! a real edit - not by sampling random cells.
//!
//! Run:    cargo run --release
//! Tests:  cargo test --release

use std::cmp::Reverse;
use std::collections::BinaryHeap;
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
// A cell's formula: an expression tree (project A) whose leaves can be other
// cells, plus the two aggregate shapes a real sheet lives on - a range Sum (the
// pivot total) and a range Max (the kind of aggregate early cutoff bites on).
// ============================================================================

#[derive(Clone)]
enum Expr {
    Const(f64),
    Cell(u32),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Sum(u32, u32), // sum of values[start .. start + len]
    Max(u32, u32), // max of values[start .. start + len]
}

fn eval(e: &Expr, values: &[f64]) -> f64 {
    match e {
        Expr::Const(c) => *c,
        Expr::Cell(i) => values[*i as usize],
        Expr::Add(l, r) => eval(l, values) + eval(r, values),
        Expr::Mul(l, r) => eval(l, values) * eval(r, values),
        Expr::Sum(start, len) => {
            let mut acc = 0.0;
            for v in &values[*start as usize..(*start + *len) as usize] {
                acc += *v;
            }
            acc
        }
        Expr::Max(start, len) => {
            let mut m = f64::NEG_INFINITY;
            for v in &values[*start as usize..(*start + *len) as usize] {
                if *v > m {
                    m = *v;
                }
            }
            m
        }
    }
}

/// The cells this formula reads. Aggregates read their whole range, so a column total
/// over R rows contributes R dependency edges - the price of an aggregate.
fn deps_of(e: &Expr, out: &mut Vec<u32>) {
    match e {
        Expr::Const(_) => {}
        Expr::Cell(i) => out.push(*i),
        Expr::Add(l, r) | Expr::Mul(l, r) => {
            deps_of(l, out);
            deps_of(r, out);
        }
        Expr::Sum(start, len) | Expr::Max(start, len) => {
            for i in *start..*start + *len {
                out.push(i);
            }
        }
    }
}

// ============================================================================
// The sheet. Cells are stored flat in topological order (id order is a valid
// recompute order, because every formula references only lower ids - the same
// trick B used with pre-order). `dependents` is the reverse graph in CSR form:
// the cells that read cell d are dependents[dep_offsets[d] .. dep_offsets[d+1]].
// ============================================================================

#[derive(Clone)]
struct Sheet {
    formula: Vec<Expr>,
    values: Vec<f64>,
    dep_offsets: Vec<u32>,
    dependents: Vec<u32>,
}

impl Sheet {
    fn len(&self) -> usize {
        self.formula.len()
    }

    fn dependents_of(&self, id: u32) -> &[u32] {
        let lo = self.dep_offsets[id as usize] as usize;
        let hi = self.dep_offsets[id as usize + 1] as usize;
        &self.dependents[lo..hi]
    }

    /// Edit a cell to a literal value. The UI's atom: this and a fill-down (a loop of
    /// these over a contiguous range) are the only dirty sets a spreadsheet can make.
    fn set_literal(&mut self, id: u32, v: f64) {
        self.formula[id as usize] = Expr::Const(v);
    }

    /// Full recompute: every cell, in id (topological) order. Each cell's inputs are at
    /// lower ids and already current, so this is a single branchless forward sweep.
    fn recompute_full(&mut self) {
        for id in 0..self.formula.len() {
            let v = eval(&self.formula[id], &self.values);
            self.values[id] = v;
        }
    }

    /// Recompute exactly the cells in `cone` (assumed sorted ascending = topological).
    /// The dirty cone is computed once when the edit happens (a real engine maintains
    /// it as cells are touched), so this times the recompute, not the marking.
    fn recompute_cone(&mut self, cone: &[u32]) {
        for &id in cone {
            let v = eval(&self.formula[id as usize], &self.values);
            self.values[id as usize] = v;
        }
    }

    /// Recompute with early cutoff: walk the dirty frontier in topological order, but
    /// only push a cell's dependents if its value actually changed. The marking is
    /// inseparable from the recompute here - that is the point - so it is timed whole.
    /// Returns the number of cells recomputed.
    fn recompute_cutoff(&mut self, edited: &[u32], in_queue: &mut [bool]) -> usize {
        in_queue.iter_mut().for_each(|b| *b = false);
        let mut heap: BinaryHeap<Reverse<u32>> = BinaryHeap::new();
        for &e in edited {
            if !in_queue[e as usize] {
                in_queue[e as usize] = true;
                heap.push(Reverse(e));
            }
        }
        let mut recomputed = 0usize;
        while let Some(Reverse(id)) = heap.pop() {
            in_queue[id as usize] = false;
            let old = self.values[id as usize].to_bits();
            let v = eval(&self.formula[id as usize], &self.values);
            self.values[id as usize] = v;
            recomputed += 1;
            if v.to_bits() != old {
                // The value changed: every cell that reads it is now stale.
                let lo = self.dep_offsets[id as usize] as usize;
                let hi = self.dep_offsets[id as usize + 1] as usize;
                for k in lo..hi {
                    let dep = self.dependents[k];
                    if !in_queue[dep as usize] {
                        in_queue[dep as usize] = true;
                        heap.push(Reverse(dep));
                    }
                }
            }
            // else: cutoff. The change was absorbed here; nothing downstream moved.
        }
        recomputed
    }

    /// The transitive dependents of `edited`, sorted ascending (topological). This is
    /// the cone a single edit or a fill-down dirties - never a random scatter.
    fn cone(&self, edited: &[u32]) -> Vec<u32> {
        let mut seen = vec![false; self.len()];
        let mut stack: Vec<u32> = Vec::new();
        for &e in edited {
            if !seen[e as usize] {
                seen[e as usize] = true;
                stack.push(e);
            }
        }
        let mut out = Vec::new();
        while let Some(id) = stack.pop() {
            out.push(id);
            for &dep in self.dependents_of(id) {
                if !seen[dep as usize] {
                    seen[dep as usize] = true;
                    stack.push(dep);
                }
            }
        }
        out.sort_unstable();
        out
    }
}

fn build_dependents(formula: &[Expr], n: usize) -> (Vec<u32>, Vec<u32>) {
    let mut counts = vec![0u32; n];
    let mut scratch = Vec::new();
    for cell in formula {
        scratch.clear();
        deps_of(cell, &mut scratch);
        for &d in &scratch {
            counts[d as usize] += 1;
        }
    }
    let mut offsets = vec![0u32; n + 1];
    for d in 0..n {
        offsets[d + 1] = offsets[d] + counts[d];
    }
    let mut dependents = vec![0u32; offsets[n] as usize];
    let mut cursor = offsets.clone();
    for (c, cell) in formula.iter().enumerate() {
        scratch.clear();
        deps_of(cell, &mut scratch);
        for &d in &scratch {
            dependents[cursor[d as usize] as usize] = c as u32;
            cursor[d as usize] += 1;
        }
    }
    (offsets, dependents)
}

// ============================================================================
// A realistic sheet: a rows x cols dataflow grid (column 0 are inputs, each later
// column derives from the one before it), an aggregate per column, and a grand
// aggregate over those. Stored COLUMN-MAJOR, so a column is a contiguous id range
// and the aggregates - the heavy, common operation - sweep sequentially. Row-wise
// edits then scatter across columns, which is the honest cost of that choice.
// Optionally a layer of `consumers` that all read the grand aggregate (a dashboard),
// used to show what early cutoff prunes.
// ============================================================================

#[derive(Clone, Copy)]
enum Agg {
    Sum,
    Max,
}

fn build_sheet(rows: usize, cols: usize, agg: Agg, consumers: usize, rng: &mut Lcg) -> Sheet {
    let r = rows as u32;
    let grid = rows * cols;
    let n = grid + cols + 1 + consumers;
    let mut formula = Vec::with_capacity(n);

    // The data grid, column-major: cell (row, c) has id = c*rows + row.
    for c in 0..cols {
        for _row in 0..rows {
            if c == 0 {
                formula.push(Expr::Const(rng.unit())); // an input literal
            } else {
                let left = ((c - 1) * rows + _row) as u32; // same row, previous column
                let m = 0.99 + 0.02 * rng.unit();
                let k = (rng.unit() - 0.5) * 0.1;
                formula.push(Expr::Add(
                    Box::new(Expr::Mul(
                        Box::new(Expr::Cell(left)),
                        Box::new(Expr::Const(m)),
                    )),
                    Box::new(Expr::Const(k)),
                ));
            }
        }
    }
    // One aggregate per column (a contiguous range), then the grand aggregate.
    for c in 0..cols {
        let start = (c * rows) as u32;
        formula.push(match agg {
            Agg::Sum => Expr::Sum(start, r),
            Agg::Max => Expr::Max(start, r),
        });
    }
    let agg_start = grid as u32;
    formula.push(match agg {
        Agg::Sum => Expr::Sum(agg_start, cols as u32),
        Agg::Max => Expr::Max(agg_start, cols as u32),
    });
    let grand = (grid + cols) as u32;
    // The dashboard: cells that all read the grand aggregate.
    for _ in 0..consumers {
        let w = 0.5 + rng.unit();
        formula.push(Expr::Mul(
            Box::new(Expr::Cell(grand)),
            Box::new(Expr::Const(w)),
        ));
    }
    assert_eq!(formula.len(), n);

    let values = vec![0.0; n];
    let (dep_offsets, dependents) = build_dependents(&formula, n);
    let mut sheet = Sheet {
        formula,
        values,
        dep_offsets,
        dependents,
    };
    sheet.recompute_full();
    sheet
}

// ============================================================================
// The pivot at scale (the bigger-than-RAM case, in miniature). A column-major
// value grid and its column sums. When a few inputs change, only the dirty
// columns are re-summed - and each is a contiguous patch, so a sheet too big for
// memory only needs the dirty columns resident.
// ============================================================================

fn pivot_full(grid: &[f64], rows: usize, cols: usize, out: &mut [f64]) {
    for c in 0..cols {
        let mut s = 0.0;
        for v in &grid[c * rows..c * rows + rows] {
            s += *v;
        }
        out[c] = s;
    }
}

fn pivot_patch(grid: &[f64], rows: usize, dirty_cols: &[usize], out: &mut [f64]) {
    for &c in dirty_cols {
        let mut s = 0.0;
        for v in &grid[c * rows..c * rows + rows] {
            s += *v;
        }
        out[c] = s;
    }
}

// ============================================================================
// Benchmark plumbing. Each strategy must start from the same stale state (values
// from before the edit, formulas from after), so we restore `values` from a
// snapshot before every timed sample. Restoring f64s is a cheap memcpy; what we
// never re-pay is the formula graph build.
// ============================================================================

fn median5(mut sample: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..5).map(|_| sample()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[2]
}

fn bench(sheet: &mut Sheet, snapshot: &[f64], mut run: impl FnMut(&mut Sheet)) -> f64 {
    median5(|| {
        sheet.values.copy_from_slice(snapshot);
        let t = Instant::now();
        run(sheet);
        black_box(sheet.values[sheet.len() - 1]);
        t.elapsed().as_nanos() as f64
    })
}

// ============================================================================
// main: the cone crossover (fill-down sweep), early cutoff, and the pivot patch.
// ============================================================================

fn main() {
    // ---- 1. Cone crossover, swept by a fill-down of k input cells ----
    let rows = 64_000;
    let cols = 16;
    let mut rng = Lcg::new(0x0005_DA7A_u64);
    let mut sheet = build_sheet(rows, cols, Agg::Sum, 0, &mut rng);
    let n = sheet.len();
    let snapshot = sheet.values.clone();
    let inputs_orig: Vec<f64> = (0..rows)
        .map(|i| match &sheet.formula[i] {
            Expr::Const(c) => *c,
            _ => unreachable!(),
        })
        .collect();

    let full = bench(&mut sheet, &snapshot, |s| s.recompute_full());

    println!("== 1. Dirty-cone crossover (fill-down of k input cells) ==");
    println!("sheet: {n} cells ({rows} x {cols} grid + {cols} column sums + grand total).");
    println!(
        "full recompute = {full:.0} ns. A fill-down is a contiguous column edit - a real UI action.\n"
    );
    println!(
        "{:>10} {:>10} {:>14} {:>10}",
        "fill-down k", "cone", "cone ns", "vs full"
    );

    for &k in &[1usize, 10, 100, 1_000, 5_000, 20_000, 40_000, 64_000] {
        let edited: Vec<u32> = (0..k as u32).collect();
        for &id in &edited {
            sheet.set_literal(id, inputs_orig[id as usize] + 0.5);
        }
        let cone = sheet.cone(&edited);
        let cone_pct = cone.len() as f64 / n as f64 * 100.0;
        let incr = bench(&mut sheet, &snapshot, |s| s.recompute_cone(&cone));
        println!(
            "{:>10} {:>9.1}% {:>14.0} {:>9.2}x",
            k,
            cone_pct,
            incr,
            full / incr
        );
        for &id in &edited {
            sheet.set_literal(id, inputs_orig[id as usize]); // revert for the next k
        }
    }
    println!(
        "\nThe dirty set is the cone of the edit, sized by topology - you cannot scatter it at random."
    );

    // ---- 2. Early cutoff: an edit absorbed by a MAX never reaches the dashboard ----
    let rows = 8_000;
    let cols = 8;
    let consumers = 200_000;
    let mut rng = Lcg::new(0x00C0_70FF_u64);
    let mut sheet = build_sheet(rows, cols, Agg::Max, consumers, &mut rng);
    let n = sheet.len();
    // Make row 0 the champion of every column, so other rows are sub-maximal.
    sheet.set_literal(0, 1.0e6);
    sheet.recompute_full();
    let snapshot = sheet.values.clone();

    // Edit a normal row's input to a new, still-sub-maximal value.
    let seed = 5u32; // an input cell in a non-champion row
    sheet.set_literal(seed, 2.5);
    let edited = [seed];
    let cone = sheet.cone(&edited);

    let mut in_queue = vec![false; n];
    let mut cutoff_count = 0usize;
    let full = bench(&mut sheet, &snapshot, |s| s.recompute_full());
    let cone_ns = bench(&mut sheet, &snapshot, |s| s.recompute_cone(&cone));
    let cutoff_ns = bench(&mut sheet, &snapshot, |s| {
        cutoff_count = s.recompute_cutoff(&edited, &mut in_queue);
    });

    println!(
        "\n== 2. Early cutoff (one sub-maximal edit under a MAX + {consumers}-cell dashboard) =="
    );
    println!("sheet: {n} cells; the dashboard all reads the grand MAX.\n");
    println!("{:>22} {:>12} {:>14}", "strategy", "recomputed", "ns");
    println!("{:>22} {:>12} {:>14.0}", "full", n, full);
    println!(
        "{:>22} {:>12} {:>14.0}",
        "cone (no cutoff)",
        cone.len(),
        cone_ns
    );
    println!(
        "{:>22} {:>12} {:>14.0}",
        "cone + early cutoff", cutoff_count, cutoff_ns
    );
    println!(
        "\nThe edit changed a number, but not the MAX - so cutoff stops there and the {consumers}-cell"
    );
    println!("dashboard is never touched. Dirty propagation alone would recompute all of it.");

    // ---- 3. Pivot patch: re-sum only the dirty columns of a big grid ----
    let prows = 100_000;
    let pcols = 100;
    let mut rng = Lcg::new(0x0007_1707_u64);
    let grid: Vec<f64> = (0..prows * pcols).map(|_| rng.unit()).collect();
    let mut sums = vec![0.0; pcols];

    let full_ns = median5(|| {
        let t = Instant::now();
        pivot_full(&grid, prows, pcols, &mut sums);
        black_box(sums[pcols - 1]);
        t.elapsed().as_nanos() as f64
    });

    println!(
        "\n== 3. Pivot patch (column-major grid: {} cells = {prows} x {pcols}) ==",
        prows * pcols
    );
    println!("full pivot (all columns) = {full_ns:.0} ns.\n");
    println!("{:>12} {:>14} {:>10}", "dirty cols", "patch ns", "vs full");
    for &d in &[1usize, 5, 25, 100] {
        let dirty: Vec<usize> = (0..d).collect();
        let patch_ns = median5(|| {
            let t = Instant::now();
            pivot_patch(&grid, prows, &dirty, &mut sums);
            black_box(sums[dirty[d - 1]]);
            t.elapsed().as_nanos() as f64
        });
        println!("{:>12} {:>14.0} {:>9.2}x", d, patch_ns, full_ns / patch_ns);
    }
    println!(
        "\nEach dirty column is a contiguous patch: a sheet too big for RAM needs only those resident."
    );
}

// ============================================================================
// Contract tests. Incremental recompute, with and without cutoff, must land on
// the same values as a full recompute, and the topological invariant the whole
// engine rests on (every formula reads only lower ids) must hold.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn values_equal(a: &[f64], b: &[f64]) {
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert_eq!(a[i].to_bits(), b[i].to_bits(), "cell {i}");
        }
    }

    #[test]
    fn id_order_is_topological() {
        let mut rng = Lcg::new(1);
        let sheet = build_sheet(200, 5, Agg::Sum, 100, &mut rng);
        let mut deps = Vec::new();
        for id in 0..sheet.len() {
            deps.clear();
            deps_of(&sheet.formula[id], &mut deps);
            for &d in &deps {
                assert!((d as usize) < id, "cell {id} reads later cell {d}");
            }
        }
    }

    #[test]
    fn cone_recompute_matches_full() {
        let mut rng = Lcg::new(2);
        let base = build_sheet(300, 6, Agg::Sum, 500, &mut rng);

        // A fill-down edit, then compare cone recompute against a full recompute.
        let edited: Vec<u32> = (0..50).collect();
        let mut edited_sheet = base.clone();
        for &id in &edited {
            edited_sheet.set_literal(id, 0.123 + id as f64);
        }

        let mut full = edited_sheet.clone();
        full.recompute_full();

        let cone = edited_sheet.cone(&edited);
        let mut incr = edited_sheet.clone();
        incr.recompute_cone(&cone);

        values_equal(&incr.values, &full.values);
    }

    #[test]
    fn cutoff_matches_full() {
        let mut rng = Lcg::new(3);
        let mut base = build_sheet(500, 8, Agg::Max, 1000, &mut rng);
        base.set_literal(0, 1.0e6); // champion row
        base.recompute_full();

        let seed = 7u32;
        base.set_literal(seed, 3.0); // sub-maximal edit

        let mut full = base.clone();
        full.recompute_full();

        let mut cut = base.clone();
        let mut in_queue = vec![false; cut.len()];
        let recomputed = cut.recompute_cutoff(&[seed], &mut in_queue);

        values_equal(&cut.values, &full.values);
        // Cutoff must have done far less work than a full recompute.
        assert!(
            recomputed < cut.len() / 10,
            "cutoff recomputed {recomputed} of {}",
            cut.len()
        );
    }

    #[test]
    fn pivot_patch_matches_full() {
        let mut rng = Lcg::new(4);
        let (rows, cols) = (1000, 12);
        let grid: Vec<f64> = (0..rows * cols).map(|_| rng.unit()).collect();
        let mut full = vec![0.0; cols];
        let mut patch = vec![0.0; cols];
        pivot_full(&grid, rows, cols, &mut full);
        let all: Vec<usize> = (0..cols).collect();
        pivot_patch(&grid, rows, &all, &mut patch);
        values_equal(&full, &patch);
    }
}
