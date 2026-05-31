# Solutions: 13 - A system is a function over tables

## Exercise 1 - Identify the shape

| operation | shape |
|---|---|
| Squaring every entry in `Vec<f32>` | operation (1→1) |
| Filtering even integers from `Vec<u32>` | filter (1→{0,1}) |
| Splitting each string into words | emission (1→N) |
| Computing the sum of `Vec<u32>` | reduction - strictly speaking *neither* of the three; one row of output for the whole table. Also called *aggregate* in SQL. |

The aggregate case is real but rare in this book - most systems run row-by-row. When you do need an aggregate, treat it as a system whose output is a single-row table.

## Exercise 2 - Motion as a system

```rust
/// motion: advance each creature's position by its velocity over `dt` seconds.
///
/// Read-set:  vel, dt
/// Write-set: pos
fn motion(pos: &mut [(f32, f32)], vel: &[(f32, f32)], dt: f32) {
    assert_eq!(pos.len(), vel.len());
    for i in 0..pos.len() {
        pos[i].0 += vel[i].0 * dt;
        pos[i].1 += vel[i].1 * dt;
    }
}
```

The `assert_eq!` enforces alignment ([§6](06_a_row_is_a_tuple.md)). Without it, a mismatched-length call silently iterates over the shorter array.

## Exercise 4 - Filter

```rust
fn starving(energy: &[f32]) -> Vec<usize> {
    let mut out = Vec::new();
    for i in 0..energy.len() {
        if energy[i] <= 0.0 {
            out.push(i);
        }
    }
    out
}
```

This is the read-only first half of `apply_starve`. The actual `apply_starve` would push these indices into `to_remove`. Splitting the query from the mutation lets you test the query in isolation.

## Exercise 5 - Emission

```rust
fn reproduce(parent_energy: &[f32], threshold: f32) -> Vec<(usize, f32)> {
    let mut out = Vec::new();
    for i in 0..parent_energy.len() {
        if parent_energy[i] >= threshold {
            let half = parent_energy[i] / 2.0;
            out.push((i, half));
            out.push((i, half));
        }
    }
    out
}
```

For each parent above threshold, two output rows. A 1→2 emission. The pattern is clear: the output `Vec` has a variable length depending on how many parents qualified.

## Exercises 3, 6, 7 - Sketches

**Exercise 3.** Doc comments listing read-set and write-set are the system's contract in machine-readable form. A reader of the function knows exactly what can change.

**Exercise 6.** Anti-system patterns: `fn update(world: &mut World)` (no declared write-set), `fn step()` that touches a `static mut` (hidden state), `fn motion(pos: &mut [(f32, f32)])` with an `eprintln!` inside (side effect - reduces parallelism, harms determinism, makes testing harder).

**Exercise 7.** A "test" is a system whose write-set is empty (or a small report table). Read pos/vel, output a list of suspicious creatures. Same code path as a debug inspector.
