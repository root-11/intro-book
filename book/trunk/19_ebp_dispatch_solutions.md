# Solutions: 19 - EBP dispatch

## Exercise 1 - Compare the two

```rust,no_run
const HUNGER_BURN: f32 = 0.1;

fn drive_hunger_filtered(
    is_hungry: &[bool], energy: &mut [f32], dt: f32,
) {
    for slot in 0..is_hungry.len() {
        if is_hungry[slot] {
            energy[slot] -= HUNGER_BURN * dt;
        }
    }
}

fn drive_hunger_ebp(
    hungry: &[u32], id_to_slot: &[u32], energy: &mut [f32], dt: f32,
) {
    for &id in hungry {
        let slot = id_to_slot[id as usize] as usize;
        energy[slot] -= HUNGER_BURN * dt;
    }
}
```

At 1M creatures with 10 % hungry, on a typical desktop:

- Filtered: ~1-2 ms (1M slots × ~1 ns each, all sequential, prefetcher is happy).
- EBP: ~0.2-0.5 ms (100 K slot accesses, some random via `id_to_slot`, but only 10 % the work).

The ratio is roughly 4-10×. EBP wins more cleanly at sparser states.

## Exercise 2 - Sparsity test

|  fraction hungry | filtered (ms) | EBP (ms) |
|-----------------:|--------------:|---------:|
|             1 % |          ~1.5 |    ~0.05 |
|            10 % |          ~1.5 |     ~0.3 |
|            50 % |          ~1.5 |     ~1.2 |
|            90 % |          ~1.5 |     ~2.0 |

(Numbers vary by chip; the *shape* is what matters.) The filtered cost is roughly constant - it walks the full table regardless. The EBP cost is roughly linear in the active fraction. Their cross-over is around 50-70 %, after which filtered wins (because random `id_to_slot` lookups become the bottleneck for EBP).

## Exercise 3 - Multi-state systems

```rust,no_run
drive_hunger(&hungry, &id_to_slot, &mut energy, dt);
drive_sleep(&sleepy, &id_to_slot, &mut energy, dt);
drive_death(&dead, &id_to_slot, &mut energy, dt);
```

Three EBP systems, each iterating its own table. Each is bandwidth-bound by the *active* count, not by the population. The single-filtered-loop alternative looks like:

```rust,no_run
for slot in 0..1_000_000 {
    if is_hungry[slot] { /* hunger work */ }
    else if is_sleepy[slot] { /* sleep work */ }
    else if is_dead[slot] { /* dead work */ }
}
```

- and walks 1M rows × 3 flag checks per row × cache-bandwidth cost. Three EBP systems combined are typically 5-20× cheaper than the single filtered version, depending on sparsity.

## Exercise 4 - The branch you do not write

The filtered version's inner loop generates roughly:

```asm
mov   al, [is_hungry + slot]
test  al, al
je    .skip
; ... work ...
.skip:
```

The EBP version's inner loop has no `je` for membership; the dispatch *is* the iteration. Freed from the branch, the compiler can usually emit SIMD over `hungry`'s `u32` slots and over the `id_to_slot` mapping in parallel.

## Exercise 5 - `&[T]` slices

```rust,no_run
fn drive_hunger(
    hungry: &[u32],            // <- slice, not Vec
    id_to_slot: &[u32],
    energy: &mut [f32],
    dt: f32,
) { /* ... */ }
```

The function takes the minimal data it needs. The caller passes `&world.hungry` and the autoderef does the rest. This is the usual shape for systems and integrates cleanly with the parallel scheduling described in [§31](31_disjoint_writes_parallelize.md).

## Exercise 6 - The naive bug

```rust,no_run
// BUG: do not do this.
for &id in hungry.iter() {
    if /* some condition */ {
        hungry.push(/* a new id */); // mutating while iterating
    }
}
```

Rust's borrow checker actually catches this one - `hungry.iter()` holds a `&` reference; `hungry.push` needs `&mut`. The code does not compile. The lesson is that the data-oriented discipline (deferred cleanup, [§22](22_mutations_buffer.md)) is what Rust's borrow checker enforces structurally. Push to a side table; apply at tick boundary.
