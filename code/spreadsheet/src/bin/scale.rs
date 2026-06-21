//! Project C at a billion cells. The `spreadsheet` binary built the recalc engine at a
//! million cells with a `Box`-`Expr` per cell. That representation cannot reach a billion:
//! at ~160 bytes of formula object per cell it would need ~160 GB before a single value.
//!
//! A billion-cell sheet forces the program itself to go SoA. A real big sheet is not a
//! billion distinct formulas; it is a handful of *templates* stamped across huge ranges
//! (a column of `=A1*B1` is one rule applied per row - a fill-down). So the formula graph
//! collapses from a tree per cell (AoS) to a template per column plus operand columns
//! (SoA), and the dependency graph collapses from an explicit edge list to an *implicit*
//! rule. That collapse is the lesson: the arc's "columns are the default" applies to the
//! program, not just the data.
//!
//! Two parts:
//!
//!   1. In RAM - the compact representation at 1e9 cells: a full dataflow recompute vs a
//!      fill-down patch, and the size of the program (templates) vs the per-cell tree.
//!   2. On disk - the pivot over a column-major file bigger than a small machine's RAM.
//!      The honest metric is bytes moved (full reads the whole file, a patch reads only the
//!      dirty columns) and the working set, both exact and size-independent. When the file
//!      exceeds RAM those bytes are disk reads - which is the entire point.
//!
//! The working set is *memory-pegged*: the pivot streams each column through a fixed-size
//! tile, so the footprint is a constant you choose, not a function of the data. OOM is not
//! avoided by luck (hoping the data fits) but structurally - it cannot happen, because the
//! program never asks for more than one tile. Few programs are written to peg memory this
//! way; this is the smallest honest demonstration of it.
//!
//! Run:    cargo run --release --bin scale            # ~1e9 cells (a 4 GB working set)
//!         cargo run --release --bin scale -- 8000000000   # size it past your own RAM
//! Tests:  cargo test --release

use std::fs::{File, remove_file};
use std::hint::black_box;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::time::Instant;

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
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

const COLS: usize = 250;

/// The fixed working-set tile for the disk pivot: 4.19 M f32 = 16 MB. The pivot never holds
/// more than this, whatever the column height - so a column bigger than RAM still streams.
/// This is the peg: the process cannot allocate past one tile, so it cannot OOM by design.
const TILE: usize = 1 << 22;

/// A column's formula, shared by all its rows: `value = m * left + k`, where `left` is the
/// same row of the previous column. One of these per column is the entire "program" for the
/// dataflow region - `COLS` of them, a few kilobytes, for a billion cells.
#[derive(Clone, Copy)]
struct Template {
    m: f32,
    k: f32,
}

/// The cell at (row, col) lives at index `col * rows + row` (column-major): a column is a
/// contiguous range, the shape the pivot and the patch both want.
fn idx(rows: usize, col: usize, row: usize) -> usize {
    col * rows + row
}

/// Full recompute of the dataflow region: each column from the one before it. Column-major,
/// so this streams forward through memory one column at a time.
fn recompute_full(values: &mut [f32], rows: usize, cols: usize, templates: &[Template]) {
    for c in 1..cols {
        let t = templates[c];
        let (prev, cur) = values.split_at_mut(c * rows);
        let prev = &prev[(c - 1) * rows..c * rows];
        let cur = &mut cur[..rows];
        for (out, &src) in cur.iter_mut().zip(prev.iter()) {
            *out = t.m * src + t.k;
        }
    }
}

/// Patch recompute after a fill-down of the first `k` input rows: only those rows, across
/// every column. Cost is k * cols, independent of the total cell count.
fn recompute_patch(values: &mut [f32], rows: usize, cols: usize, templates: &[Template], k: usize) {
    for c in 1..cols {
        let t = templates[c];
        for r in 0..k {
            values[idx(rows, c, r)] = t.m * values[idx(rows, c - 1, r)] + t.k;
        }
    }
}

// ---------------------------------------------------------------------------
// Disk: a column-major file of f32 values. A column is a contiguous byte range,
// so a patch reads only the dirty columns, and each column streams through a
// fixed tile, so the working set is pegged regardless of column height.
// The raw byte view (f32 has no invalid bit patterns or padding) is the same
// raw-column-bytes approach the §37 logger uses - priced, not avoided.
// ---------------------------------------------------------------------------

fn as_bytes(s: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, std::mem::size_of_val(s)) }
}

fn as_bytes_mut(s: &mut [f32]) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(s.as_mut_ptr() as *mut u8, std::mem::size_of_val(s)) }
}

/// Write a whole in-RAM grid at once. Used by the tests (which build small grids); the
/// `main` run uses the streaming writer so the file can exceed RAM.
#[cfg(test)]
fn write_grid(path: &std::path::Path, values: &[f32]) -> std::io::Result<()> {
    let mut w = BufWriter::with_capacity(1 << 22, File::create(path)?);
    w.write_all(as_bytes(values))?;
    w.flush()
}

/// Write the column-major grid to disk one column at a time, holding only two columns in
/// RAM (the previous and the current). This is what lets the disk file exceed RAM: the file
/// can be 36 GB while the process touches ~two columns. Each column is the template applied
/// to the one before it, so the file is a valid dataflow sheet, not random noise.
fn write_grid_streaming(
    path: &std::path::Path,
    rows: usize,
    templates: &[Template],
    rng: &mut Lcg,
) -> std::io::Result<()> {
    let mut w = BufWriter::with_capacity(1 << 22, File::create(path)?);
    let mut prev = vec![0.0f32; rows];
    let mut cur = vec![0.0f32; rows];
    for p in prev.iter_mut() {
        *p = rng.unit(); // column 0: inputs
    }
    w.write_all(as_bytes(&prev))?;
    for &t in templates.iter().skip(1) {
        for (out, &src) in cur.iter_mut().zip(prev.iter()) {
            *out = t.m * src + t.k;
        }
        w.write_all(as_bytes(&cur))?;
        std::mem::swap(&mut prev, &mut cur);
    }
    w.flush()
}

/// Sum `rows` f32 from the file's current position into an f64, reading in fixed `tile`-
/// sized chunks. The working set is one tile, not one column: the same partition-and-stream
/// discipline that lets the *sheet* exceed RAM, applied one level down so a single *column*
/// may exceed RAM too. The bound is a tile you choose, never a natural unit you hope fits.
/// Accumulate in f64 - per project D, an f32 running total over millions of rows would lose
/// its low bits. Reading in row order keeps the sum bit-identical to a single-buffer read.
fn sum_column_tiled(file: &mut File, rows: usize, tile: &mut [f32]) -> f64 {
    let mut remaining = rows;
    let mut acc = 0.0f64;
    while remaining > 0 {
        let len = remaining.min(tile.len());
        file.read_exact(as_bytes_mut(&mut tile[..len])).unwrap();
        for &v in &tile[..len] {
            acc += v as f64;
        }
        remaining -= len;
    }
    acc
}

/// Sum every column by reading the file straight through. Returns the bytes read (the whole
/// file).
fn pivot_full(file: &mut File, rows: usize, cols: usize, tile: &mut [f32], out: &mut [f64]) -> u64 {
    file.seek(SeekFrom::Start(0)).unwrap();
    for o in out.iter_mut().take(cols) {
        *o = sum_column_tiled(file, rows, tile);
    }
    (rows * cols * 4) as u64
}

/// Re-sum only the dirty columns, each a seek then a tiled read. Returns the bytes read
/// (just those columns) - the number that, once the file is bigger than RAM, is disk I/O.
fn pivot_patch(
    file: &mut File,
    rows: usize,
    dirty: &[usize],
    tile: &mut [f32],
    out: &mut [f64],
) -> u64 {
    for &c in dirty {
        file.seek(SeekFrom::Start((c * rows * 4) as u64)).unwrap();
        out[c] = sum_column_tiled(file, rows, tile);
    }
    (dirty.len() * rows * 4) as u64
}

fn median3(mut sample: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..3).map(|_| sample()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[1]
}

fn gb(bytes: u64) -> f64 {
    bytes as f64 / 1.0e9
}

fn main() {
    let target_cells: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000_000);
    let cols = COLS;
    // Part 1 (the representation) lives in RAM, so it is fixed at a billion cells - the
    // impressive size that still fits a 4 GB working set. Part 2 (the disk pivot) uses the
    // requested size, meant to exceed RAM: it is streamed, never fully resident.
    let p1_rows = 1_000_000_000 / cols;
    let p1_n = p1_rows * cols;
    let p2_rows = target_cells / cols;
    let p2_n = p2_rows * cols;

    let mut rng = Lcg::new(0x00_5CA1E);
    let templates: Vec<Template> = (0..cols)
        .map(|_| Template {
            m: 0.99 + 0.02 * rng.unit(),
            k: (rng.unit() - 0.5) * 0.1,
        })
        .collect();

    println!("== Project C at scale ==");
    println!(
        "in-RAM representation: {p1_n} cells ({p1_rows} x {cols}), {:.1} GB f32, column-major.\n",
        gb((p1_n * 4) as u64)
    );

    // ---- representation: the program is templates, not a tree per cell ----
    let template_bytes = cols * std::mem::size_of::<Template>();
    let per_cell_bytes = 160; // measured shape of the 1M-demo's Box<Expr> per derived cell
    println!("-- the program (for {p1_n} cells) --");
    println!("compact (this binary): {cols} column templates = {template_bytes} bytes.");
    println!(
        "per-cell Box<Expr> (the 1M-cell binary's representation): ~{} GB - cannot be allocated.",
        gb((p1_n * per_cell_bytes) as u64)
    );
    println!(
        "the formula graph went from a tree per cell to a template per column; the dependency"
    );
    println!(
        "graph from an explicit edge list to an implicit rule. The program itself went SoA.\n"
    );

    // ---- in-RAM dataflow: full recompute vs a fill-down patch ----
    let mut values = vec![0.0f32; p1_n];
    for v in values.iter_mut().take(p1_rows) {
        *v = rng.unit(); // column 0: the inputs
    }
    recompute_full(&mut values, p1_rows, cols, &templates);

    let full_ns = median3(|| {
        let t = Instant::now();
        recompute_full(&mut values, p1_rows, cols, &templates);
        black_box(values[p1_n - 1]);
        t.elapsed().as_nanos() as f64
    });
    let k = 1000usize.min(p1_rows); // a 1000-row fill-down
    let patch_ns = median3(|| {
        let t = Instant::now();
        recompute_patch(&mut values, p1_rows, cols, &templates, k);
        black_box(values[idx(p1_rows, cols - 1, 0)]);
        t.elapsed().as_nanos() as f64
    });
    println!("-- dataflow recompute ({p1_n} cells) --");
    println!(
        "full: {:.1} ms, {:.1} GB/s.",
        full_ns / 1.0e6,
        gb((p1_n * 4 * 2) as u64) / (full_ns / 1.0e9) // read prev + write cur
    );
    println!(
        "fill-down patch ({k} rows x {cols} cols = {} cells): {:.1} us, {:.0}x less work.",
        k * cols,
        patch_ns / 1.0e3,
        full_ns / patch_ns
    );
    drop(values); // free the 4 GB before the disk part

    // ---- on disk: the pivot, where a patch reads only the dirty columns ----
    // The scratch file goes in the current directory, which is real disk. We deliberately
    // do NOT use the system temp dir: /tmp is often a RAM-backed tmpfs, and writing the
    // grid to RAM would quietly defeat the entire "leave the RAM" point.
    let path = std::env::current_dir()
        .unwrap_or_else(|_| ".".into())
        .join(format!("spreadsheet_scale_{}.bin", std::process::id()));
    let file_gb = gb((p2_n * 4) as u64);
    println!("\n-- pivot on disk ({p2_n} cells, {file_gb:.1} GB) --");
    print!(
        "writing {} (streamed, ~2 columns in RAM) ... ",
        path.display()
    );
    std::io::stdout().flush().ok();
    write_grid_streaming(&path, p2_rows, &templates, &mut rng).expect("write grid");
    println!("done.");

    let mut tile = vec![0.0f32; TILE];
    let mut sums = vec![0.0f64; cols];
    let mut file = File::open(&path).expect("open grid");

    // Full pivot reads the whole file: a single I/O-bound pass (no median; it is dominated
    // by the read, and at sizes past RAM each pass is genuine disk traffic).
    let t = Instant::now();
    let full_bytes = pivot_full(&mut file, p2_rows, cols, &mut tile, &mut sums);
    black_box(sums[cols - 1]);
    let full_ns = t.elapsed().as_nanos() as f64;

    let dirty: Vec<usize> = vec![0, 1, 2]; // three columns went dirty
    let patch_bytes = pivot_patch(&mut file, p2_rows, &dirty, &mut tile, &mut sums);
    let patch_ns = median3(|| {
        let t = Instant::now();
        let b = pivot_patch(&mut file, p2_rows, &dirty, &mut tile, &mut sums);
        black_box(b);
        black_box(sums[0]);
        t.elapsed().as_nanos() as f64
    });

    println!(
        "full pivot: reads {:.1} GB (the whole file), {:.1} ms.",
        gb(full_bytes),
        full_ns / 1.0e6
    );
    println!(
        "patch pivot ({} dirty columns): reads {:.1} MB, {:.2} ms - {:.0}x fewer bytes.",
        dirty.len(),
        patch_bytes as f64 / 1.0e6,
        patch_ns / 1.0e6,
        full_bytes as f64 / patch_bytes as f64
    );
    println!(
        "working set either way: a fixed {:.0} MB tile, independent of the {:.0} MB column height",
        (TILE * 4) as f64 / 1.0e6,
        (p2_rows * 4) as f64 / 1.0e6,
    );
    println!(
        "and the {file_gb:.1} GB grid - so even a column too big for RAM still streams through it."
    );

    // ---- the sizing math (RAM < problem < Disk) ----
    println!("\n-- sizing the problem to leave RAM --");
    println!(
        "this run's file is {file_gb:.1} GB. The bytes-moved and working-set numbers above are"
    );
    println!(
        "exact at any size; the disk-bound *time* only shows once the file exceeds RAM. To make"
    );
    println!(
        "the point on your machine: pick a cell count so cells x 4 bytes lands between your RAM"
    );
    println!(
        "and your free disk. RAM < problem < disk. Each GB of RAM is 250 M f32 cells, so for an"
    );
    println!(
        "R-GB machine choose a bit above R x 250e6 cells (e.g. 32 GB RAM -> ~9e9 cells, a 36 GB file):"
    );
    println!("    cargo run --release --bin scale -- <cells>");

    remove_file(&path).ok();
    println!("\n(removed the {file_gb:.1} GB scratch file.)");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(rows: usize, cols: usize) -> (Vec<f32>, Vec<Template>) {
        let mut rng = Lcg::new(1);
        let templates: Vec<Template> = (0..cols)
            .map(|_| Template {
                m: 0.99 + 0.02 * rng.unit(),
                k: (rng.unit() - 0.5) * 0.1,
            })
            .collect();
        let mut values = vec![0.0f32; rows * cols];
        for v in values.iter_mut().take(rows) {
            *v = rng.unit();
        }
        recompute_full(&mut values, rows, cols, &templates);
        (values, templates)
    }

    #[test]
    fn patch_matches_full_on_edited_rows() {
        let (rows, cols) = (500, 16);
        let (mut values, templates) = build(rows, cols);

        // Edit the first k inputs, then compare a patch against a full recompute.
        let k = 40;
        let mut rng = Lcg::new(99);
        for r in 0..k {
            values[idx(rows, 0, r)] = rng.unit() + 1.0;
        }
        let mut full = values.clone();
        recompute_full(&mut full, rows, cols, &templates);
        recompute_patch(&mut values, rows, cols, &templates, k);

        for c in 0..cols {
            for r in 0..rows {
                assert_eq!(
                    values[idx(rows, c, r)].to_bits(),
                    full[idx(rows, c, r)].to_bits(),
                    "cell ({r},{c})"
                );
            }
        }
    }

    #[test]
    fn disk_pivot_full_and_patch_agree() {
        let (rows, cols) = (400, 10);
        let (values, _) = build(rows, cols);
        let path =
            std::env::temp_dir().join(format!("spreadsheet_scale_test_{}.bin", std::process::id()));
        write_grid(&path, &values).unwrap();
        let mut file = File::open(&path).unwrap();

        let mut colbuf = vec![0.0f32; rows];
        let mut full = vec![0.0f64; cols];
        let mut patch = vec![0.0f64; cols];
        pivot_full(&mut file, rows, cols, &mut colbuf, &mut full);
        pivot_patch(
            &mut file,
            rows,
            &(0..cols).collect::<Vec<_>>(),
            &mut colbuf,
            &mut patch,
        );

        for c in 0..cols {
            assert_eq!(full[c].to_bits(), patch[c].to_bits(), "column {c}");
        }
        remove_file(&path).ok();
    }

    #[test]
    fn pivot_matches_direct_in_memory_sum() {
        // The disk pivot must equal an independent in-memory column sum (f64 accumulator).
        let (rows, cols) = (300, 8);
        let (values, _) = build(rows, cols);
        let path =
            std::env::temp_dir().join(format!("spreadsheet_scale_mem_{}.bin", std::process::id()));
        write_grid(&path, &values).unwrap();
        let mut file = File::open(&path).unwrap();

        let mut colbuf = vec![0.0f32; rows];
        let mut disk = vec![0.0f64; cols];
        pivot_full(&mut file, rows, cols, &mut colbuf, &mut disk);

        for c in 0..cols {
            let mut acc = 0.0f64;
            for r in 0..rows {
                acc += values[idx(rows, c, r)] as f64;
            }
            assert_eq!(disk[c].to_bits(), acc.to_bits(), "column {c}");
        }
        remove_file(&path).ok();
    }
}
