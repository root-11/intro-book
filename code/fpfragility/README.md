# fpfragility - "where SoA does not pay", project D

The correctness capstone. Project C made the spreadsheet incremental and
bigger-than-RAM; this is where the answers turn out to be wrong. A pivot is a pile of
additions, and floating-point addition is neither associative nor exact. **Layout cannot
rescue you**: a perfectly columnar sum is still wrong, and a perfectly SoA geometric
predicate is still wrong on a degenerate input. Correctness is orthogonal to layout - the
arc's honest counterweight, restated where columns simply do not help.

```sh
cargo test --release   # non-associativity, Kahan vs naive, exact orientation - all deterministic
cargo run  --release   # the three measurements below
```

## The three faces

1. **Summation.** The same numbers in a different order give a different total; a naive
   sequential sum can lose everything to a single large running value; compensated
   (Kahan/Neumaier) and pairwise sums recover it.
2. **Incremental drift.** Maintaining a running sum by deltas - the cheap "incremental
   aggregate" C wanted - never equals a fresh recompute, and the relative gap explodes
   when the true total nearly cancels. Aggregates must be re-anchored, not trusted.
3. **Geometric predicates.** A naive `f64` orientation test gives the wrong sign on
   near-collinear points almost every time; the exact integer test does not, at nearly
   the same cost.

## Reference run (single host, cross-machine pending)

Captured on a **Ryzen 9 270** (16 threads, 32 GB), rustc 1.94.0, `--release`, median of 5.
One box; the Pi 4 / i7 / i3 columns are not captured yet. The error figures are
properties of IEEE-754 and portable; the timings are this machine's.

### 1. Summation: same 10,000,000 numbers, different order, different total

An ill-conditioned ledger column: small entries (cents either way) plus one huge
offsetting pair (a big credit and its matching debit). The true sum is the small entries;
the giants cancel - but a naive running total climbs to 1e16, where a 1.0 is below the ulp.

| method | result | abs error | ns |
|---|---:|---:|---:|
| kahan (reference) | -2284.449323 | 0 | 9,540,640 |
| naive | -0.917414 | 2283.53 | 5,928,102 |
| naive reversed | -0.710347 | 2283.74 | - |
| pairwise | -2276.000000 | 8.45 | 2,659,159 |
| tiled / 256 | -2448.0 | 163.55 | - |
| tiled / 4096 | -2308.0 | 23.55 | - |
| tiled / 65536 | -2392.0 | 107.55 | - |

Naive loses the entire true sum (it reports ~0 where the answer is ~-2284), and reversing
the input changes the result - addition is not associative. Pairwise and tiled recover
most of it, and **a different tile size gives a different total** (this is exactly why C's
pivot patch matched the full pivot only because it summed in the same order). Kahan is
right, at ~1.6x the naive cost.

A bonus the timings show: **pairwise is 2.2x faster than naive**, not slower. The naive
sum is a serial dependency chain (each `+=` waits for the last); pairwise breaks it into
independent subsums the compiler can vectorize. The accurate method is also the fast one -
and it is the tree-reduction shape project E reaches for.

### 2. Incremental drift: running sum maintained by deltas vs a fresh recompute

1,000,000 mixed-magnitude entries (0.01 to ~1e10, either sign); the running total starts
exact and is then maintained only by `running += new - old`.

| edits | drift vs recompute | relative |
|---:|---:|---:|
| 100,000 | 2.56 | 7.8e-15 |
| 1,000,000 | 2.53 | 1.9e-14 |
| 10,000,000 | 3.02 | 8.7e-12 |

The maintained total never matches a fresh recompute. The absolute gap stays small (a few
units), but as a fraction it jumps ~1000x by the last row, because cancellation shrank the
true total and the same few-unit error became a far larger share of it. You cannot trust a
long-lived incremental aggregate; re-anchor it with a recompute.

### 3. Geometric predicate: orientation of near-collinear points

1,000,000 near-collinear triples with ~2^30 integer coordinates (exact answer known via
128-bit integers).

- naive `f64` disagrees with the exact answer on **992,697 of 1,000,000 (99.3%)**.
- e.g. a=(0,0) b=(1558697093, 1558697091) c=(1558697094, 1558697092): exact = +1, naive = 0.
- cost: naive **1.34 ns/test**, exact (i128) **1.38 ns/test** - correctness is ~free here.

The products are ~2^60, far past f64's 53-bit mantissa, so the rounded determinant of a
near-collinear triple is dominated by noise. No layout changes this; the fix is exact (or
adaptive) arithmetic.

## The cliffhanger

Now the answers are correct (Kahan, exact predicates) and incremental - but the sum is
still one core's bandwidth, and Kahan made it more compute-bound. That is where reaching
for more hardware comes in. Project E.
