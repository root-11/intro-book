# Solutions: 28 - Proximity is a property of position

Numbers below are from the `proximity` benchmark; see `code/README.md`.

## Exercise 1 - The all-pairs wall

```rust,no_run
fn count_all_pairs(px: &[f32], py: &[f32], r: f32) -> u64 {
    let n = px.len();
    let mut total = 0u64;
    for i in 0..n {
        for j in 0..n {
            if i != j {
                let dx = px[i] - px[j];
                let dy = py[i] - py[j];
                if dx * dx + dy * dy <= r * r { total += 1; }
            }
        }
    }
    total
}
```

Measured: ~270 ms at N = 20 000. The cost is N², so 10K is ~4x cheaper and 1M is ~2500x more expensive (a thousand seconds - never). Twenty thousand creatures already blow a 33 ms frame budget eight times over, on a query the world has to answer every tick.

## Exercise 2 - Cell as a derived column

```rust,no_run
fn cell_of(px: f32, py: f32, cell_size: f32) -> u32 {
    let x = (px / cell_size).floor() as i32 as u32 & 0xFFFF;
    let y = (py / cell_size).floor() as i32 as u32 & 0xFFFF;
    (x << 16) | y
}

// computed in the same pass that integrates motion:
for i in 0..n {
    px[i] += vx[i] * dt;
    py[i] += vy[i] * dt;
    cell[i] = cell_of(px[i], py[i], cell_size); // one extra arithmetic op
}
```

The `cell` column costs a divide, two floors, and some bit ops per creature, in a loop that is already streaming `px`/`py`. It rides on bandwidth motion is already paying. There is no separate structure and no separate pass.

## Exercise 3 - Dense binning

```rust,no_run
// count -> prefix-sum -> scatter (a counting sort by cell)
let mut offsets = vec![0u32; ncells + 1];
for &c in &cell { offsets[c as usize + 1] += 1; }
for c in 0..ncells { offsets[c + 1] += offsets[c]; }
let mut items = vec![0u32; n];
let mut cursor = offsets.clone();
for i in 0..n {
    let c = cell[i] as usize;
    items[cursor[c] as usize] = i as u32;
    cursor[c] += 1;
}

// query: the 3x3 block of cells around creature i, as contiguous ranges
for (nx, ny) in neighbours_3x3(i) {
    let c = ny * GX + nx;
    for &j in &items[offsets[c] as usize .. offsets[c + 1] as usize] {
        // distance test against j
    }
}
```

The counts match exercise 1 exactly (it is the same query, restricted to candidates that can possibly be within `r`). Three linear passes build it; the query reads contiguous ranges.

## Exercise 4 - Bolt-on hash vs dense bin

The `HashMap<u32, Vec<u32>>` version inserts each creature into `grid.entry(cell).or_default().push(i)`, and the query does `grid.get(&c)` for each of nine cells.

Measured at 1M:

| | build | query | total |
|---|---|---|---|
| bolt-on hash | 31 ms | 1437 ms | 1468 ms |
| dense bin | 3.7 ms | 513 ms | 517 ms |

End to end the dense bin is ~2.8x faster, its build ~8x faster. The hash pays for a heap allocation per occupied cell, hashes a key for every insert and every one of the nine lookups per query, and chases a pointer into a separately-allocated `Vec` whose contents are scattered across the heap. The dense bin has one `offsets` array and one `items` array; the query indexes `offsets` and walks a contiguous slice. Same algorithm, opposite memory behaviour.

## Exercise 5 - Recompute beats maintain

The dense bin's build is 3.7 ms; its query is 513 ms. The build is **0.7%** of the query. A maintained index exists to avoid paying that rebuild - so it is buying a 0.7% saving, and paying for it with insert/remove bookkeeping on every move, a structure that must stay correct across ticks, and (for a hash) pointer-chasing on every access. Recomputing the whole binning from scratch each tick, in the motion pass, is both simpler and faster. The maintenance you were told you need is maintenance of the wrong thing.

## Exercise 6 - The pack-leader

```rust,no_run
// all-pairs cohesion: each agent steers toward the average of all others - O(N^2)
for i in 0..n {
    let (mut sx, mut sy) = (0.0, 0.0);
    for j in 0..n { if i != j { sx += px[j]; sy += py[j]; } }
    steer[i] = (sx / (n - 1) as f32 - px[i], sy / (n - 1) as f32 - py[i]);
}

// one leader/centroid: one pass for the centre, every agent reads it - O(N)
let (mut cx, mut cy) = (0.0, 0.0);
for i in 0..n { cx += px[i]; cy += py[i]; }
cx /= n as f32; cy /= n as f32;
for i in 0..n { steer[i] = (cx - px[i], cy - py[i]); }
```

Measured at N = 20 000: all-pairs cohesion ~240 ms, one centroid ~0.03 ms - about 9000x, and the gap grows linearly with N because one side is O(N²) and the other O(N). The leader gives swarm-like motion without any agent knowing about any other: each beast only ever reads the single shared centre (or, for richer behaviour, an invisible leader NPC that does the pack's navigation). Coordination cost collapses from "every pair" to "one pass."

## Exercise 7 - Z-order and the compaction

A stripe pack (`x << 16 | y`) puts cells with the same `x` adjacent; cells one step apart in `y` are adjacent, but a step in `x` jumps 65 536 cells away. A Z-order (Morton) curve interleaves the bits so 2D neighbours stay close in the 1D order in both axes:

```rust
fn morton_2d(x: u16, y: u16) -> u32 {
    let mut x = x as u32; let mut y = y as u32;
    x = (x | (x << 8)) & 0x00FF00FF; x = (x | (x << 4)) & 0x0F0F0F0F;
    x = (x | (x << 2)) & 0x33333333; x = (x | (x << 1)) & 0x55555555;
    y = (y | (y << 8)) & 0x00FF00FF; y = (y | (y << 4)) & 0x0F0F0F0F;
    y = (y | (y << 2)) & 0x33333333; y = (y | (y << 1)) & 0x55555555;
    x | (y << 1)
}
```

Binning finds the candidates, but the neighbour query still gathers their positions from scattered slots - that is the bulk of the ~513 ms. Order the [§24](24_append_only_and_recycling.md) compaction by Morton cell so a cell's creatures sit on adjacent cache lines, and the gather streams ([§26](26_subscription_tables.md)'s measured ~4x for scattered-vs-dense). The compaction runs on the GC's slow cadence, not as a separate per-tick sort: §28 decides *which cell*; the compaction makes *reading a cell* sequential.
