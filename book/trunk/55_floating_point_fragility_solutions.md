# Solutions: 55 - The same numbers, a different total

The error figures below are properties of IEEE-754 and portable; the timings are the Ryzen 9 270 figures from the [`fpfragility`](https://github.com/root-11/intro-book/tree/main/code/fpfragility) crate, and cross-machine capture is pending.

## Exercise 1 - Two orders, two answers

```
  (1e16 + -1e16) + 1   =  0 + 1     =  1     giants cancel first, then the 1 lands
  (1e16 + 1) + -1e16   =  1e16 + -1e16 = 0   the 1 is lost first, then giants cancel
```

Same three numbers, two answers. The losing step is `1e16 + 1`: a `double` near ten quadrillion has a gap between representable values larger than 1, so there is no room to store the difference and the `1` is rounded away. By the time `-1e16` arrives, nothing of it remains. A triple of your own with the same shape - any tiny value added to a giant before its canceling partner arrives - reproduces it; the small addition is the one that loses information, because its magnitude falls below the spacing of representable numbers at the running total's scale. **Floating-point addition is not associative**, and this is how every conforming machine behaves, not a fault in yours.

## Exercise 2 - Lose a column

Build a ledger column of millions of small entries (cents either way) plus one large offsetting pair - a big credit and the matching debit. The true total is the accumulated cents; the giants cancel.

| method | result | abs error |
|---|---:|---:|
| kahan (reference) | -2284.45 | 0 |
| naive left to right | -0.92 | 2283.53 |
| naive reversed | -0.71 | 2283.74 |

Added left to right, the running total climbs to the big number and sits there while every small entry is added and lost beneath it - each one below the gap, exactly as in exercise 1 - and then the big debit cancels the big credit back to near zero. The naive sum reports roughly nothing where the true answer was about -2284. Reversed, it gives a *different* wrong answer, because a different set of small entries gets swallowed. Both orders miss the true total, which is the sum of the small values the giants never touch.

## Exercise 3 - Get it back

```rust,no_run
// pairwise: split and recurse, so small entries meet each other before any giant.
fn pairwise(xs: &[f64]) -> f64 {
    if xs.len() <= 64 { return xs.iter().sum(); }
    let mid = xs.len() / 2;
    pairwise(&xs[..mid]) + pairwise(&xs[mid..])
}
```

Both better methods recover the true total. Pairwise summation drops the absolute error from ~2283 to about 8, and a compensated (Kahan) running term lands exactly on the reference:

| method | abs error | ns |
|---|---:|---:|
| naive | 2283.53 | 5,928,102 |
| pairwise | 8.45 | 2,659,159 |
| kahan | 0 | 9,540,640 |

The timings carry a bonus: pairwise is about **2.2x faster** than naive, not slower. The naive sum is one dependent chain where each `+=` waits for the last, while the paired version splits into independent subsums the compiler vectorizes and runs at once. The accurate method is the fast one - and it is the same tree-shaped reduction [§56](56_bandwidth_is_the_ceiling.md) leans on. Kahan costs about 1.6x the naive sum for the extra compensation arithmetic, and is exact.

## Exercise 4 - Watch it drift

Start a running total from the exact sum of a million mixed-magnitude entries, then maintain it only by `running += new - old` through many random edits, comparing against a fresh recompute:

| edits | drift | relative |
|---:|---:|---:|
| 100,000 | 2.56 | 7.8e-15 |
| 1,000,000 | 2.53 | 1.9e-14 |
| 10,000,000 | 3.02 | 8.7e-12 |

The maintained total never matches the recompute, and the gap never closes. The absolute drift stays small, a few units, but as a *fraction* of the answer it jumps about 1000x by the last row - because cancellation shrank the true total while the same few-unit error stayed put, so its share of the answer exploded exactly when the total is near zero. You cannot tell by looking, which is why a real system periodically re-anchors its aggregates with a fresh recompute rather than trusting the running patch forever: the incremental total buys speed by spending correctness, a little at a time.

## Exercise 5 - The wrong side of the line

```rust,no_run
// orientation: sign of the cross product (b - a) x (c - a).
fn orient_f64(a: P, b: P, c: P) -> f64 { (b.x-a.x)*(c.y-a.y) - (b.y-a.y)*(c.x-a.x) }
fn orient_i128(a: P, b: P, c: P) -> i128 {
    let bx=b.x as i128 - a.x as i128; /* ... */ bx*cy - by*cx   // exact
}
```

On a million near-collinear triples with ~2^30 integer coordinates, the naive `f64` test disagrees with the exact integer answer on **992,697 of 1,000,000 - 99.3%**. The two products are about 2^60, far past f64's 53-bit mantissa, so each is rounded before the subtraction and the rounded difference of two near-equal giants is dominated by noise; the sign comes out wrong. The exact i128 test is right every time, and costs about the same - **1.34 ns** naive against **1.38 ns** exact. Correctness was nearly free here; the naive version was not buying speed, only error.

## Exercise 6 - A layout cannot save you

Store the inputs of any of the above in perfect SoA columns and the wrong answer is exactly as wrong as before: the naive column sum still reports ~0, the naive orientation still flips on 99% of degenerate triples. The arc's usual move - lay the data out flat so the access streams - does nothing here, because the error is in the *arithmetic*, not the *storage*. **Correctness is orthogonal to layout.** What fixes it is real arithmetic: add in a defined order, compensate, accumulate in a wider type, or compute the predicate exactly. None of those is a layout choice, which is the point of putting them in this arc - a flawless column buys you speed and nothing about being right.
