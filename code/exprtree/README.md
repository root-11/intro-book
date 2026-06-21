# exprtree - "where SoA does not pay", project A

The first of the three weekend-project specimens for the Part II chapter group on the
limits of column layout. One arithmetic expression, three representations, measured
against each other. The point is a *crossover*, not a verdict.

The three representations of the same expression tree:

1. **Boxed** - the idiomatic pointer tree, `enum Expr { Add(Box<Expr>, Box<Expr>), ... }`.
   Each node is its own heap allocation; eval chases pointers.
2. **Arena** - a flat `Vec<Node>` with `u32` child indices. Contiguous storage, but eval
   still hops by index *in tree order* (and pays a bounds check per node).
3. **Flat** - the same tree linearized into post-order and evaluated by a single forward
   pass over a `Vec<Op>` with a value stack. Pure sequential access. This *is* a
   stack-machine / RPN bytecode VM: flattening a recursive structure for traversal is
   compilation to a linear instruction stream.

```sh
cargo test --release   # the three representations must agree, bit for bit
cargo run  --release   # the two workloads and the derived crossover
```

## What it measures

- **Workload 1 - bulk evaluation** (traversal-dominated), swept over tree size N.
- **Workload 2 - structural mutation** (topology-dominated) at a fixed N: boxed/arena
  swing one child in O(path); flat must re-linearize the whole expression, O(N), on
  every edit. The crossover is then derived from the measured per-op costs.

## The lesson

The win is not "arrays beat pointers". It is the **access pattern**: sequential beats
scattered. The **arena** is the control that proves it - contiguous storage traversed in
tree order index-chases just like the pointer tree and does **not** beat it. Only the
**flat** form, which traverses sequentially, pulls ahead, and only once N escapes cache.
That is nodes 1 and 27 of the trunk, restated on a recursive structure.

And it cuts the other way under edits: linearization is compilation, and you cannot edit
compiled code in place. The flat form pays O(N) to rebuild on every structural change, so
it wins only when you evaluate far more often than you rewrite - the compile-once,
evaluate-many regime a bytecode VM lives in.

## Reference run (single host, cross-machine pending)

Captured on a **Ryzen 9 270** (16 threads, 32 GB), rustc 1.94.0, `--release`, median of 5.
These are one box; the Pi 4 / i7-3610QM / i3-5010U columns that the rest of `code/` carries
are not captured yet. Treat the shape as the claim, not the digits.

Bulk evaluation, ns per evaluation (lower is better); `flat vs boxed` is boxed / flat:

| depth | nodes | boxed | arena | flat | flat vs boxed |
|---:|---:|---:|---:|---:|---:|
| 3 | 15 | 27.0 | 28.5 | 18.0 | 1.50x |
| 5 | 63 | 144.4 | 154.4 | 81.9 | 1.76x |
| 6 | 127 | 249.4 | 284.2 | 282.1 | 0.88x |
| 8 | 511 | 1339.9 | 1431.1 | 1609.9 | 0.83x |
| 10 | 2047 | 7216.6 | 7919.8 | 7001.8 | 1.03x |
| 12 | 8191 | 34708 | 37487 | 28751 | 1.21x |
| 14 | 32767 | 139841 | 145778 | 115255 | 1.21x |
| 16 | 131071 | 573222 | 600835 | 459304 | 1.25x |
| 18 | 524287 | 2864273 | 2579992 | 1843448 | 1.55x |
| 20 | 2097151 | 14125889 | 15235907 | 7398149 | 1.91x |

Three regimes, all on one curve:

- **Tiny N (under ~100 nodes):** flat wins ~1.5-1.8x. Everything is register/L1 resident, so
  the only thing that matters is per-node overhead, and a tight linear loop beats recursion.
- **Cache-resident band (~127-1023 nodes):** flat **loses** (0.83-0.93x). The whole pointer
  tree fits in cache, the chase is nearly free, and flat's stack push/pop overhead dominates.
  This is the carve-out the chapter has to be honest about - it is a *cache-resident band*,
  not simply "small N".
- **Past cache (depth 12+):** flat pulls ahead and keeps widening (1.21x to 1.91x at 2M
  nodes) as scattered access starts paying cache-miss tax that sequential streaming avoids.

The **arena** sits at or below boxed everywhere: contiguous storage alone buys nothing while
the traversal order stays scattered.

Structural mutation at depth 16 (131071 nodes), 4000 edits:

| rep | ns / edit | ns / eval |
|---|---:|---:|
| boxed | 150 | 577103 |
| arena | 18 | 600560 |
| flat | 495768 | 460555 |

Derived crossover (flat vs boxed): **r\* = 0.19**, about **1 edit per 4 evaluations**. Below
that edit-fraction flat wins; above it, the O(N) re-linearization sinks it. Because both
flat's eval advantage and its recompile cost scale with N, that ratio is roughly
scale-invariant - it is a property of the workload mix, not the tree size.
