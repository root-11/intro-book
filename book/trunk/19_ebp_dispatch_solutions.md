# Solutions: 19 - EBP dispatch

## Exercise 1 - Compare the two

```rust,no_run
const HUNGER_BURN: f32 = 0.1;

fn drive_hunger_filtered(
    is_hungry: &[bool], energy: &mut [f32], dt: f32,
) {
    for i in 0..is_hungry.len() {
        if is_hungry[i] {
            energy[i] -= HUNGER_BURN * dt;
        }
    }
}

fn drive_hunger_ebp(
    hungry: &[u32], energy: &mut [f32], dt: f32,
) {
    for &i in hungry {
        energy[i as usize] -= HUNGER_BURN * dt;
    }
}
```

At 1M creatures with 10 % hungry, on a typical desktop:

- Filtered: ~1 ms (1M slots × ~1 ns each, all sequential, prefetcher is happy).
- EBP: ~0.35 ms (100 K slot accesses into `energy`, scattered, but only 10 % the work, and no id-to-slot hop).

The ratio is roughly 3× at 10 %, and it widens fast as the state gets sparser - about 14× at 1 %.

## Exercise 2 - Sparsity test

|  fraction hungry | filtered (ms) | EBP (ms) |
|-----------------:|--------------:|---------:|
|             1 % |          ~1.0 |    ~0.04 |
|            10 % |          ~1.0 |     ~0.36 |
|            50 % |          ~1.0 |     ~0.86 |
|            90 % |          ~1.0 |     ~1.1 |

(Numbers vary by chip; the *shape* is what matters.) The filtered cost is roughly constant - it walks the full table regardless. The EBP cost is roughly linear in the active fraction. Their cross-over is up near 85-90 %: only when nearly every creature is hungry does walking the whole table beat walking the subscription, and even then EBP loses only because it also reads the subscription array. The crossover used to sit lower because the old hot loop paid a random `id_to_slot` lookup per entry; keying `hungry` by slot removes that, so EBP stays ahead across almost the entire range. [§26](26_subscription_tables.md) measures this directly.

## Exercise 3 - Multi-state systems

```rust,no_run
drive_hunger(&hungry, &mut energy, dt);
drive_sleep(&sleepy, &mut energy, dt);
drive_death(&dead, &mut energy, dt);
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

The EBP version's inner loop has no `je` for membership; the dispatch *is* the iteration. Freed from the branch, the loop is a straight gather over the slots listed in `hungry` - no per-row test, and no id-to-slot indirection to serialise it.

## Exercise 5 - `&[T]` slices

```rust,no_run
fn drive_hunger(
    hungry: &[u32],            // <- slice of slots, not Vec
    energy: &mut [f32],
    dt: f32,
) { /* ... */ }
```

The function takes the minimal data it needs. The caller passes `&world.hungry` and the autoderef does the rest. This is the usual shape for systems and integrates cleanly with the parallel scheduling described in [§31](31_disjoint_writes_parallelize.md).

## Exercise 6 - The naive bug

```rust,no_run
// BUG: do not do this.
for &i in hungry.iter() {
    if /* some condition */ {
        hungry.push(/* a new slot */); // mutating while iterating
    }
}
```

Rust's borrow checker actually catches this one - `hungry.iter()` holds a `&` reference; `hungry.push` needs `&mut`. The code does not compile. The lesson is that the data-oriented discipline (deferred cleanup, [§22](22_mutations_buffer.md)) is what Rust's borrow checker enforces structurally. Push to a side table; apply at tick boundary.
