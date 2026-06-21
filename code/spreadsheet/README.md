# spreadsheet - "where SoA does not pay", project C

The payoff of the arc. A spreadsheet cell holds a *formula* - an expression tree over
other cells (project A, used as the per-cell evaluator) - and the cells form a
*dependency DAG*. Recalculation is a topological sort of that DAG: "the program is a
topological sort of who-reads-what-who-wrote" (book node 14), made executable. Dirty
propagation, which project B did down a tree, now runs through a graph, and the tree's
contiguous-subtree shortcut is gone.

```sh
cargo test --release   # cone and cutoff must match a full recompute, bit for bit
cargo run  --release   # the cone crossover, early cutoff, and the pivot patch
```

## The design

- A cell's formula is an `Expr` (A's tree) over `Cell` references, plus `Sum`/`Max` over
  a contiguous range - the aggregate a real sheet lives on.
- Cells are stored flat in **topological order** (every formula reads only lower ids, so
  id order is a valid recompute order - B's pre-order trick, generalized).
- The reverse graph (who reads cell d) is kept in CSR form for dirty propagation.
- Storage is **column-major**, so a column is a contiguous id range and the aggregates -
  the heavy, common operation - sweep sequentially. Row-wise edits then scatter across
  columns: the honest cost of optimizing the common case.

Three recalc strategies: **full** (every cell, topological order), **cone** (the
transitive dependents of the edit), and **cone + early cutoff** (stop propagating at any
cell whose recomputed value did not actually change - "validation is cheaper than
recompute").

## The lessons

1. **The nature of the change is domain-specific.** A scenegraph (B) scatters its movers
   at random; a spreadsheet cannot. The UI only makes a single-cell edit or a contiguous
   fill-down, and the dirty set is then the *dependency cone* of that edit, its size
   fixed by the formula topology. The crossover is swept by a fill-down of `k` cells - a
   real action - not by random sampling.
2. **"Incremental" does not make an aggregate incremental.** A single-cell edit's cone is
   tiny in cell *count*, but if it feeds a `SUM` over a column the recompute re-scans the
   whole column. One changed cell still costs O(column). That is the ceiling on the cone
   win below, and the reason real engines *delta-maintain* aggregates or patch them.
3. **Early cutoff is what tames a high-fan-out hub.** When an edit is absorbed by a `MAX`
   it never reaches the dashboard downstream. Dirty propagation alone would recompute all
   of it; cutoff stops at the unchanged hub. Validation beats recompute, measured.
4. **At scale, patch the dirty columns.** A pivot re-sums only the columns that changed,
   each a contiguous range - so a sheet too big for RAM needs only the dirty columns
   resident.

And the cliffhanger to project D: all of this rests on a pile of additions - the column
sums, the pivot totals - and floating-point addition is neither associative nor exact.
The totals are order-dependent, and at scale they are wrong. That is the next chapter.

## Reference run (single host, cross-machine pending)

Captured on a **Ryzen 9 270** (16 threads, 32 GB), rustc 1.94.0, `--release`, median of 5.
One box; the Pi 4 / i7 / i3 columns are not captured yet. Treat the shape as the claim.

### 1. Dirty-cone crossover, fill-down of `k` input cells

Sheet: 1,024,017 cells (64,000 x 16 grid + 16 column sums + grand total).
Full recompute = 9.91 ms.

| fill-down k | cone | cone ns | vs full |
|---:|---:|---:|---:|
| 1 | 0.0% | 608,692 | 16.3x |
| 100 | 0.2% | 620,644 | 16.0x |
| 1,000 | 1.6% | 749,877 | 13.2x |
| 5,000 | 7.8% | 1,334,424 | 7.4x |
| 20,000 | 31.3% | 3,533,980 | 2.8x |
| 40,000 | 62.5% | 6,489,165 | 1.5x |
| 64,000 | 100% | 10,034,056 | 0.99x |

Incremental wins until the fill-down covers most of the sheet. But note the **16x
ceiling** even for a single edited cell: the cone always includes the column `SUM`s, and
recomputing a sum re-reads its whole column, so a one-cell edit still pays O(column). The
cone is small in count and expensive in work - aggregates are not incremental by default.

### 2. Early cutoff (one sub-maximal edit under a `MAX` + 200,000-cell dashboard)

Sheet: 264,009 cells; the dashboard all reads the grand `MAX`.

| strategy | recomputed | ns |
|---|---:|---:|
| full | 264,009 | 1,714,957 |
| cone (no cutoff) | 200,017 | 1,167,641 |
| cone + early cutoff | 16 | 31,790 |

The edit changed a number but not the maximum, so cutoff stops after 16 cells and the
200,000-cell dashboard is never touched - 37x faster than the cone without cutoff, 54x
faster than full. Dirty propagation alone recomputes the lot; cutoff is what saves it.

### 3. Pivot patch (column-major grid: 10,000,000 cells = 100,000 x 100)

Full pivot (all columns) = 5.82 ms.

| dirty cols | patch ns | vs full |
|---:|---:|---:|
| 1 | 57,548 | 101x |
| 5 | 287,620 | 20.2x |
| 25 | 1,453,537 | 4.0x |
| 100 | 5,816,803 | 1.0x |

Linear in the number of dirty columns, each a contiguous patch. The bigger-than-RAM case
is the same shape: stream and re-sum only the dirty columns, never the whole grid.

## At a billion cells (`scale` binary)

A million cells hid the real lesson. The `main` binary stores a `Box`-`Expr` per cell;
at a billion cells that representation would need ~160 GB of formula objects before a
single value, and cannot be built. A billion-cell sheet forces the *program itself* to go
SoA: a real big sheet is not a billion distinct formulas, it is a handful of *templates*
stamped across huge ranges (a fill-down). So the formula graph collapses from a tree per
cell to a template per column, and the dependency graph from an explicit edge list to an
implicit rule.

```sh
cargo run --release --bin scale                 # 1e9 cells in RAM + a 4 GB disk pivot
cargo run --release --bin scale -- 9000000000   # a 36 GB pivot, past a 32 GB machine's RAM
```

The `scale` binary keeps the in-RAM representation fixed at 1e9 cells and streams the disk
file (two columns resident) so it can exceed RAM. The scratch file is written to the
current directory, **not** `/tmp`: `/tmp` is often a RAM-backed tmpfs, which would defeat
the whole point of leaving RAM.

The pivot streams each column through a fixed-size tile (16 MB), so the working set is
**memory-pegged**: a constant you choose, independent of the column height or the grid. OOM
is not avoided by hoping the data fits - it cannot happen, because the process never asks
for more than one tile. That is the discipline applied one level down: partitioning lets the
*sheet* exceed RAM, and tiling the column read lets a single *column* exceed RAM too.

### Reference run (Ryzen 9 270, 30 GB RAM, NVMe; rustc 1.94.0, `--release`)

The program, for 1e9 cells:

- compact representation: 250 column templates = **2,000 bytes**;
- the `Box`-`Expr` per cell would be **~160 GB** - cannot be allocated.

Dataflow recompute, 1e9 cells (4 GB f32, in RAM):

- full recompute: **220 ms**, 36 GB/s;
- a 1000-row fill-down patch (250,000 cells): **~50 us**, ~4400x less work.

Pivot, in-RAM-sized (4 GB file, fits in this box's RAM so the time is cache-resident):

- full pivot reads 4.0 GB in **929 ms**; patch (3 dirty columns) reads 48 MB in **12 ms**.

Pivot, **bigger than RAM** (36 GB file on a 30 GB machine - the real demonstration):

| operation | bytes moved | time |
|---|---:|---:|
| full pivot | 36.0 GB (the whole file) | **15,986 ms** |
| patch pivot (3 dirty columns) | 432 MB | **99 ms** |
| working set (either) | a fixed 16 MB tile | - |

This is the argument. Once the file exceeds RAM the full pivot is 16 seconds of genuine
disk reading (the same pivot was 929 ms when the 4 GB file fit in cache - the inflection,
measured), while the patch touches three columns in 99 ms. The footprint is a fixed 16 MB
tile in both cases, **not** the 144 MB column nor the 36 GB grid. The bytes moved and the
working set are exact at any size; only the *time* needs the file to exceed RAM to show its
teeth.

### Sizing it for your machine (RAM < problem < disk)

The point is to leave RAM, so use the math to pick the size for your system. Each GB of
RAM is 250 M f32 cells, so choose a cell count a bit above (your RAM in GB) x 250e6 - the
grid then exceeds RAM and the disk-bound time appears - while staying under your free disk.
RAM < problem < disk.

### Exercise: peg the whole pipeline

The pivot's read is pegged to a 16 MB tile, so it cannot OOM no matter how tall a column
grows. Most programs never peg memory deliberately; doing it is the lesson. Extend it:

1. **Peg the recompute, not just the read.** The dataflow recompute and the disk write still
   hold whole columns (`prev` and `cur`); peg them to a tile so the *generator* also has a
   constant footprint, and a single column may exceed RAM.
2. **Peg the patch write-back.** A real edit writes results back to disk; tile the write so
   updating a dirty column never materialises the whole column.
3. **Two-dimensional pegging.** When neither rows nor columns fit, the tile becomes a 2D
   block and the pivot a blocked reduction. Pick a block shape and show the footprint stays
   pegged while the sheet grows without bound.

The invariant to hold throughout: peak memory is a constant you set, never a function of the
data. That is how you make OOM structurally impossible instead of merely unlikely.
