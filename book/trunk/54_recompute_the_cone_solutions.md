# Solutions: 54 - A spreadsheet is a dependency graph

Numbers below are the Ryzen 9 270 + NVMe figures from the [`spreadsheet`](https://github.com/root-11/intro-book/tree/main/code/spreadsheet) crate (the `scale` binary for the billion-cell run); cross-machine capture is pending, so treat the shape as the claim, not the digits.

## Exercise 1 - The cone by hand

The five-cell sheet: `A1=2`, `A2=3`, `B1=A1*A2`, `B2=B1+A1`, `T=B1+B2`. Edit `A1`:

```
A1  changed
B1  reads A1            -> stale
B2  reads B1 and A1     -> stale
T   reads B1 and B2     -> stale         order: B1, then B2, then T
A2  reads nothing changed -> still correct
```

Edit `A2` instead and only `B1`, `B2`, and `T` that descend from it go stale - but here `B1` reads `A2`, so the chain is the same three cells. The cones differ by which inputs *reach* a cell along the feeds-into edges, not by any layout position: `A2` feeds `B1` (and onward), while `A1` feeds both `B1` and `B2` directly. The cone is the reachable set, and it must be recomputed in topological order so each cell sees the fresh values of everything it reads.

## Exercise 2 - Recompute in order

```rust,no_run
// cells stored so every formula reads only lower ids: id order is a valid recompute order.
fn recompute_all(cells: &mut [Cell]) {
    for i in 0..cells.len() { cells[i].value = eval(&cells[i].formula, cells); }
}

// cone of an edit: transitive dependents via the reverse graph, recomputed in id order.
fn recompute_cone(cells: &mut [Cell], edited: usize, readers: &Csr) {
    let cone = reachable_from(edited, readers);   // ascending ids
    for &i in &cone { cells[i].value = eval(&cells[i].formula, cells); }
}
```

Storing cells in topological order makes the full recompute a single forward pass, and makes the cone recompute a walk of the reverse graph in ascending id order, which is automatically a valid recompute order. The cone result matches a full recompute exactly, because it recomputes precisely the cells a full pass would have changed and leaves the rest at values that were already correct.

## Exercise 3 - The fill-down crossover

Sweep a fill-down of `k` cells on a 1,024,017-cell sheet (full recompute = 9.91 ms):

| fill-down k | cone | cone vs full |
|---:|---:|---:|
| 1 | 0.0% | 16.3x |
| 1,000 | 1.6% | 13.2x |
| 5,000 | 7.8% | 7.4x |
| 20,000 | 31.3% | 2.8x |
| 40,000 | 62.5% | 1.5x |
| 64,000 | 100% | 0.99x |

The cone wins big while the fill-down is small and the win shrinks as it covers more of the sheet, until at a full-column fill-down the plain full recompute is marginally faster. You cannot drive this with a *random* dirty set, because no real edit produces one: a person types a single cell or drags a fill-down down a contiguous run, so the dirty set is always the cone of one of those, its size set by the formula topology rather than by chance.

## Exercise 4 - The sum that is not incremental

Note the **16x ceiling** in the table above, present even for a single edited cell. The reason is that the cone of any edit in that sheet includes the column `SUM`s, and recomputing a sum re-reads its entire column:

```rust,no_run
Formula::Sum(range) => { let mut acc = 0.0; for c in range.clone() { acc += cells[c].value; } acc }
```

A sum keeps no memory of its old value, so one changed cell forces it to add the whole column again - O(column), not O(1). The cone was tiny in cell *count* and huge in *work*. A sum cannot be patched by touching only what changed unless you keep a running total and maintain it by deltas (add the new value, subtract the old) - which is cheap, and which [§55](55_floating_point_fragility.md) shows drifts away from the truth.

## Exercise 5 - Early cutoff

A `MAX` over a column feeds a 200,000-cell dashboard; edit a cell to a value still below the maximum (264,009 cells total):

| strategy | recomputed | ns |
|---|---:|---:|
| full | 264,009 | 1,714,957 |
| cone, no cutoff | 200,017 | 1,167,641 |
| cone + early cutoff | 16 | 31,790 |

```rust,no_run
let new = eval(&cells[i].formula, cells);
if new == cells[i].value { continue; }   // value unchanged: do not mark dependents stale
cells[i].value = new;
mark_dependents_stale(i);
```

The edit changed a number but not the maximum, so the recomputed `MAX` equals its old value, the cutoff stops there, and the dashboard is never reached - 16 cells instead of 200,017, about 54x faster than full and 37x faster than the cone without the check. The principle in one line: **validation is cheaper than recomputation** - checking "did this actually change?" costs almost nothing, while pushing the change downstream on the assumption that it did costs everything.

## Exercise 6 - The program goes flat

One formula-object per cell at a billion cells is roughly **160 GB** of `Box`-`Expr` before a single value is stored, and cannot be allocated. Represent the same sheet as one template per column instead:

```
250 column templates  =  ~2,000 bytes      (the entire "program" for 1e9 cells)
```

A billion cells are not a billion distinct formulas; a fill-down is one formula repeated, so a real big sheet is a handful of templates stamped across huge ranges. What collapsed: the formula graph went from a tree-per-cell to a template-per-column, and the dependency graph from a stored edge list to an implicit rule ("this column reads that one, row by row"). The arc's flatten-the-data thesis turned up one level higher - at scale you flatten the *program* too.

## Exercise 7 - Peg the memory

```rust,no_run
// re-sum a column that may be larger than RAM, holding only one tile at a time.
fn sum_column_tiled(path: &Path, col: ColumnSpan, tile_bytes: usize) -> f64 {
    let mut acc = 0.0;
    let mut buf = vec![0f32; tile_bytes / 4];      // the only large allocation
    for tile in col.tiles(path, &mut buf) { for &v in tile { acc += v as f64; } }
    acc
}
```

The read holds at most one 16 MB tile no matter how tall the column or how large the sheet, so peak memory is a constant you set rather than a function of the data. Feed it ten times the rows and the footprint does not move. On a 36 GB sheet on a 30 GB machine the contrast is stark:

| operation | bytes moved | time |
|---|---:|---:|
| full pivot | 36.0 GB (whole file) | 15,986 ms |
| patch (3 dirty columns) | 432 MB | 99 ms |
| working set (either) | a fixed 16 MB tile | - |

The patch reads only the dirty columns, each a contiguous stretch of the file, in a tenth of a second against sixteen seconds of genuine disk reading for the full pivot. Size your own sheet by the arithmetic: each gigabyte of RAM is 250 million `f32` cells, so pick a cell count a little above your RAM and below your free disk - RAM < problem < disk - and the disk-bound time appears while the footprint stays pegged. (Write the scratch file to a real disk, not `/tmp`, which is often a RAM-backed tmpfs and would defeat the point of leaving RAM.) Running out of memory stops being something you hope to avoid: the process has no way to ask for more than a tile, so it **cannot happen** - the named idea the finale returns to.
