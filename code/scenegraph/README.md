# scenegraph - "where SoA does not pay", project B

The second weekend-project specimen. A scenegraph is a tree of nodes, each with a
*local* transform; a node's *world* transform is its parent's world transform composed
with its local one. Every frame something moves and the world transforms below it go
stale. Project A's cliffhanger was: when only part of the structure changed, do you
recompute the dirty part or just recompute everything? This measures the answer.

```sh
cargo test --release   # flat, pointer, and incremental paths must agree bit for bit
cargo run  --release   # layout baseline, the dirty-fraction crossover, and locality
```

## The design

The tree is laid out flat in **DFS pre-order**, so every parent sits at a lower index
than its children and a subtree is a **contiguous index range** `[i, i + subtree[i])`.
That single fact does the heavy lifting:

- **Full repropagate** is one sequential pass: `world[i] = world[parent[i]] o local[i]`
  for `i` in order, each parent already done. Branchless, streaming.
- **Incremental** recompute walks only the dirty indices, in ascending order, so each
  dirty node's parent is either clean (still valid from last frame) or dirty and already
  recomputed this frame.
- The **pointer tree** (`Box`ed nodes, recursive walk) is project A's scattered layout,
  carried onto a scenegraph: identical work, worse memory.

A transform is a 6-field `Affine` row touched as a unit, so it stays AoS - splitting it
into six columns would buy nothing. Columns are a default, not a law; this is the arc's
whole point, restated where it does not apply.

## The lesson

Two crossovers, both measured:

1. **Dirty fraction.** Incremental recompute wins overwhelmingly when little moved and
   loses once most of the tree is dirty - past roughly half, the branchless full sweep
   beats tracking-and-skipping. "Recompute only what changed" is a default with a ceiling.
2. **Locality of the dirty set.** At the *same* dirty count, a compact subtree streams
   and a scattered handful of leaves misses cache on every node - an order of magnitude
   apart. Incremental only pays when the dirty set is also *local*.

And the cliffhanger to project C: this all works because a tree gives every node exactly
one parent and a contiguous subtree. Real dependencies form a DAG - a node feeds many
others, there is no contiguous dirty range, and you must topologically sort the dirty
cone. That is the spreadsheet.

## Reference run (single host, cross-machine pending)

Captured on a **Ryzen 9 270** (16 threads, 32 GB), rustc 1.94.0, `--release`, median of 5.
One box; the Pi 4 / i7 / i3 columns the rest of `code/` carries are not captured yet.
Treat the shape as the claim.

Full repropagate, ns per frame, flat (sequential) vs pointer tree (scattered):

| nodes | flat | pointer | flat speedup |
|---:|---:|---:|---:|
| 100,000 | 226,053 | 512,711 | 2.27x |
| 1,000,000 | 2,907,273 | 8,063,752 | 2.77x |

(At N=1 the numbers are timer noise; the layout effect needs scale, exactly as in A.)

Dirty-fraction crossover, 1,000,000 nodes; full repropagate = 2.97 ms/frame:

| dirty | incremental | vs full |
|---:|---:|---:|
| 0.1% | 3,264 ns | 910x |
| 1% | 46,780 ns | 63x |
| 5% | 185,680 ns | 16x |
| 10% | 525,942 ns | 5.7x |
| 20% | 1,720,963 ns | 1.7x |
| 40% | 2,686,883 ns | 1.1x |
| 60% | 4,121,459 ns | 0.72x |
| 100% | 3,846,938 ns | 0.77x |

Incremental wins below ~40-50% dirty and loses above it. The 100% row is the honest
tell: recomputing every node *through the dirty list* is slower than the branchless full
sweep that touches the same nodes, because the list indirection and the per-node branch
cost more than they save once nothing is actually skipped.

Locality, same dirty count (~9% of nodes), contiguous subtree vs scattered leaves:

| dirty set | time | |
|---|---:|---|
| contiguous subtree (92,176 nodes) | 232,938 ns | 13x faster than full |
| scattered leaves (92,176 nodes) | 2,514,819 ns | 10.8x slower than contiguous |

Same node count, same work - a >10x spread from locality alone. A scattered 9% dirty set
is about as expensive as recomputing the whole million-node scene.
