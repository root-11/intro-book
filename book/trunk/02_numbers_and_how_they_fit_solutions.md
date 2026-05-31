# Solutions: 2 - Numbers and how they fit

## Exercise 1 - Sizes

```rust
use std::mem::size_of;

fn main() {
    println!("u8:    {}", size_of::<u8>());     // 1
    println!("u16:   {}", size_of::<u16>());    // 2
    println!("u32:   {}", size_of::<u32>());    // 4
    println!("u64:   {}", size_of::<u64>());    // 8
    println!("i32:   {}", size_of::<i32>());    // 4
    println!("f32:   {}", size_of::<f32>());    // 4
    println!("f64:   {}", size_of::<f64>());    // 8
    println!("usize: {}", size_of::<usize>());  // 8 on 64-bit
}
```

## Exercise 2 - Cache-line packing

|  type | bytes | per 64-byte line |
|------:|------:|-----------------:|
|  `u8` |   1   |        64        |
| `u16` |   2   |        32        |
| `u32` |   4   |        16        |
| `u64` |   8   |         8        |
| `f32` |   4   |        16        |
| `f64` |   8   |         8        |

## Exercise 3 - Width and speed

A `Vec<u8>` sum reads roughly 1/8 the bytes that a `Vec<u64>` sum does. Modern CPUs are usually memory-bandwidth bound on simple sums, so expect about 4-8× speed difference (not always 8×, because the small-element loop may not auto-vectorise as well, or because the wider type fits more arithmetic per instruction).

## Exercise 4 - Float weirdness

```
0.0 / 0.0 = NaN
1.0 / 0.0 = inf
(-1.0).sqrt() = NaN
let nan = 0.0_f64 / 0.0_f64;
nan != nan  // true!
```

`NaN != NaN` is by IEEE 754 definition: there is no sensible value to compare with, so equality is false. `assert!(nan == nan)` would *panic*; we want `assert!(nan != nan)`.

## Exercise 5 - Catastrophic cancellation

```rust
let a: f32 = 1e10;
let b: f32 = 1e10 - 1.0;  // f32 may not even represent this distinctly
println!("{}", a - b);    // expected 1.0; you may get 0.0 or 2.0 or 1024.0

let a: f64 = 1e10;
let b: f64 = 1e10 - 1.0;
println!("{}", a - b);    // closer to 1.0
```

`f32` has ~7 decimal digits; `1e10` already exhausts those. `f64` has ~15.

## Exercise 6 - Choose a width

| column | type | reasoning |
|---|---|---|
| age in ticks at 30 Hz × 1 yr | `u32` | 30 × 60 × 60 × 24 × 365 ≈ 9.5×10⁸; fits in u32 |
| card suit | `u8` | 4 values |
| 4K pixel count | `u32` | 8.3 million pixels |
| user id, 100M users | `u32` | 4×10⁹ headroom |
| 16-bit PCM sample | `i16` | the format defines it |

## Exercise 7 - `f32` ranges

`f32::MAX ≈ 3.4×10³⁸`. `f32::EPSILON ≈ 1.2×10⁻⁷`. EPSILON is the smallest `x` for which `1.0 + x ≠ 1.0`. Adding many `EPSILON`-scale numbers to a large value can therefore *not increase it* - they get absorbed. Summing 10⁹ small floats is often less accurate than summing them in pairs (a *Kahan sum* fixes this).
