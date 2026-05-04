# 2 — Numbers and how they fit

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 2](../../concepts/glossary.md#2--numbers-and-how-they-fit).*

<p align="center"><img src="../illustrations/multimeter.jpg" alt="A mouse with a multimeter — numbers measured to the precision the budget allows" style="max-height: 300px; max-width: 100%;"></p>

A cache line is 64 bytes. That is the unit of memory the CPU loads at a time. Everything you do with data is, in part, a question of how many things fit in 64 bytes.

Rust gives you several integer widths: `u8` (one byte, range 0..256), `u16` (two bytes, 0..65 536), `u32` (four bytes, around four billion), `u64` (eight bytes, around 1.8×10¹⁹). The signed versions — `i8`, `i16`, `i32`, `i64` — use one bit for the sign and the rest for magnitude. For floating-point: `f32` (four bytes, ~7 decimal digits of precision), `f64` (eight bytes, ~15 decimal digits).

A `Vec<u8>` of length N is N bytes. A `Vec<u64>` is 8N bytes. So a `Vec<u8>` fits 64 elements per cache line; a `Vec<u64>` fits 8. If your loop touches one element per cache line, the `u64` version makes 8× as many memory loads as the `u8` version.

This is the *width budget*. Picking a wider type than you need is not free; it costs cache lines, and at the scales this book targets, cache lines are the budget you spend.

The rule is simple: pick the narrowest type that holds your range, and write down why. A 52-card deck's `suits` need 4 values, `ranks` need 13, `locations` need maybe 8 — all fit in `u8`. A creature's `pos` needs about ten kilometres of grid resolved to centimetre precision; that fits in `f32`. A timestamp in microseconds for a year-long simulation needs something like 3×10¹³, which does not fit in `u32` (4×10⁹) but fits comfortably in `u64`. Choose, and write the choice down.

Floats are the trickier case. They look like real numbers but are not. There are only about 4 billion `f32` values; there are only about 18 quintillion `f64` values; that is finite. Operations have edges: `1.0 / 0.0 = inf`, `0.0 / 0.0 = NaN`, and `NaN != NaN` — yes, equality is broken on purpose, because there is no reasonable answer. Subtracting two nearly equal floats loses most of their precision (this is *catastrophic cancellation*). Adding a tiny float to a large one quietly drops the tiny one (this is *absorption*). None of this is a problem if you know it is there; all of it is a problem if you assume floats are mathematics.

Most of this book uses `u8`, `u16`, `u32`, `f32`, and `u64` for time. `i*` and `f64` appear when the range or precision genuinely demands it. The choice is documented at every column declaration.

## Exercises

1. **Sizes.** Print `std::mem::size_of::<u8>()`, `<u16>`, `<u32>`, `<u64>`, `<i32>`, `<f32>`, `<f64>`, `<usize>`. Confirm `usize` is 8 on a 64-bit machine.
2. **Cache-line packing.** For each type above, compute how many fit in a 64-byte cache line. A `Vec<u32>` of 16 elements is exactly one line; a `Vec<u64>` of 8 elements is exactly one line.
3. **Width and speed.** Sum a `Vec<u8>` of 100,000,000 ones, then a `Vec<u64>` of the same length. Compare times. Some of the difference is memory bandwidth (8× more bytes); some is cache pressure.
4. **Float weirdness.** Compute `0.0_f64 / 0.0_f64`, `1.0_f64 / 0.0_f64`, and `(0.0_f64).sqrt()`. Print them. Then check `let nan = 0.0_f64 / 0.0_f64; assert!(nan != nan);` — confirm it does not panic.
5. **Catastrophic cancellation.** Compute `1e10_f32 - (1e10_f32 - 1.0_f32)`. The result should be `1.0`; on `f32` it usually is not. Repeat with `f64` and observe it gets closer.
6. **Choose a width.** For each of these columns, write down the type you would pick and why: a creature's age in ticks at 30 Hz over a year-long simulation; a card's suit; the pixel count of a 4K screen; the user id in a system with up to 100 million users; an audio sample value in 16-bit PCM.
7. *(stretch)* **The actual range of `f32`.** Read [the `f32` documentation](https://doc.rust-lang.org/std/primitive.f32.html). What is `f32::MAX`? `f32::EPSILON`? What does the latter mean for a sum of small numbers?

Reference notes in [02_numbers_and_how_they_fit_solutions.md](02_numbers_and_how_they_fit_solutions.md).

## What's next

[§3 — The `Vec` is a table](03_the_vec_is_a_table.md) takes the next step: now that you know how big the elements are, what does a `Vec<T>` *do* with them?
