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

At 1M creatures, scan-all-and-branch vs the slot-keyed subscription gather, on a modern desktop (`ebp_partition`; per-machine spread in `code/README`):

- At 10 % subscribed: filtered ~0.58 ms, EBP ~0.36 ms - about 1.6×. The subscription does a tenth of the *work*, but its slots are scattered through the column, so the gather misses cache and spends most of the bandwidth win on the misses.
- At 1 % subscribed: filtered ~0.52 ms, EBP ~0.04 ms - about 14×. Sparse enough that a scattered gather still beats scanning a million flags.

The headline "10× less work at 10 %" is real as *work and memory traffic*; it shows up in *wall time* only once the subscription is compacted so the gather streams ([§26](26_subscription_tables.md)'s locality, several× there). Scattered, the 10 % win is modest; dense, it is the full order of magnitude. This is why EBP and the §26 compaction belong together.

## Exercise 2 - Sparsity test

| fraction hungry | filtered (ms) | EBP scattered (ms) |
|----------------:|--------------:|-------------------:|
|             1 % |         ~0.52 |              ~0.04 |
|            10 % |         ~0.58 |              ~0.36 |
|            50 % |         ~0.88 |              ~0.86 |
|           100 % |         ~1.03 |              ~1.20 |

(Modern desktop; numbers vary by chip - see `code/README`. The *shape* is what matters.) Filtered rises gently with the active fraction: it reads all N flags every time but only *writes* the active ones. The scattered subscription rises faster and crosses filtered near full participation - at 100 % it is a scan with extra bookkeeping (the [§26](26_subscription_tables.md) anti-pattern). Two things pull the EBP curve down: high sparsity (the 1 % row), and compaction ([§26](26_subscription_tables.md)), which turns the scattered gather sequential. The durable claim is the *work* ratio - touch the subset, not the population - and wall time follows once the gather is dense.

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
