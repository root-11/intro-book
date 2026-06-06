# 19 - EBP dispatch

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 19](../../concepts/glossary.md#19--ebp-dispatch).*

<p align="center"><img src="../illustrations/tip_simplify_full.jpg" alt="Break complex problems into smaller parts - dispatch by decomposition" style="max-height: 300px; max-width: 100%;"></p>

A system that needs to act on hungry creatures has two ways to find them.

**Filtered iteration.** Walk all creatures; for each, ask "is it hungry?"; do work if yes:

```rust,ignore
for slot in 0..creatures.len() {
    if is_hungry[slot] {
        drive_hunger_behaviour(slot);
    }
}
```

**Existence-based dispatch.** Walk the `hungry` table directly; do work for every entry:

```rust,ignore
for &i in hungry.iter() {
    drive_hunger_behaviour(i);
}
```

The two produce the same result. The two have very different costs.

The filtered version walks 1 000 000 rows when 100 000 are hungry - 900 000 of those iterations are wasted. Each wasted iteration loads a cache line, runs a branch, and does nothing. The branch is predictable on the way *into* a `false` flag (the predictor learns "mostly false") and unpredictable at the boundaries (where flags change), so the cost is dominated by memory bandwidth: 1 MB of `is_hungry` flags loaded to do 100 000 units of work.

The EBP version walks 100 000 rows. Every iteration does work. There is no per-row branch; the dispatcher *is* the table. Memory traffic is proportional to active rows, not to population.

The cost difference scales with the *sparsity* of the state. If 90 % of creatures are hungry, the two approaches are similar (both touch most of the data). If 10 % are hungry, EBP does 10× less work; at 0.1 %, 1000× less. Most simulator states are sparse - a small fraction of creatures are eating at any given tick, a small fraction are reproducing, a small fraction are dying - so EBP's advantage compounds everywhere.

One honest qualifier, since this book measures: that ratio is in *work and memory traffic*. In *wall time* a scattered subscription shows less of it - a tenth of the rows, but each entry a cache miss into the columns - so at 10 % active the measured speedup is nearer 1.5× than 10× until the subscription is compacted ([§26](26_subscription_tables.md)) and the gather streams. The work win is immediate; the time win follows locality. At high sparsity (1 %) the gather is rare enough that even scattered it wins outright.

A useful intuition: it is the difference between a wandering shopper trying to remember what they need and a shopper with a list. The list version is shorter, faster, and correct by construction. You do not consult the list to ask "is this aisle on my list?" - you walk down the list and visit each aisle once.

The shape EBP produces in code is also a clue. A system that uses EBP looks like:

```rust,no_run
fn drive_hunger(hungry: &[u32], energy: &mut [f32], dt: f32) {
    for &i in hungry {
        energy[i as usize] -= HUNGER_BURN_RATE * dt;
    }
}
```

Read-set: `hungry`. Write-set: `energy` (only the slots listed in `hungry`). The signature is the contract - exactly the contract from [§13](13_system_as_function.md). EBP is not a separate idea; it is the natural shape that a system takes when its inputs are presence tables.

Because `hungry` holds slots, each entry indexes the columns directly - there is no id-to-slot lookup inside the loop. That directness is the whole point of keying the table by slot; [§26](26_subscription_tables.md) measures what it is worth (and why the table holds slots, not ids), once the lifecycle in [§24](24_append_only_and_recycling.md) makes slots stable enough to store.

EBP also composes cleanly with parallelism. A million creatures with 100 000 hungry can be split across eight threads - each thread takes a 12 500-row slice of `hungry` and does its work. The threads never need to consult creatures that are not hungry; their loads do not interfere. [§31](31_disjoint_writes_parallelize.md) develops this.

The takeaway: EBP is the dispatch that falls out of [§17](17_presence_replaces_flags.md)'s presence-replaces-flags substitution. You do not need to choose to use EBP - once your state is in presence tables, every system naturally iterates them. The flag version does not even arise.

## Exercises

1. **Compare the two.** Implement both `drive_hunger_filtered` (walks creatures, checks flag) and `drive_hunger_ebp` (walks `hungry`). Run both on a 1M-creature world with 10 % hungry. Time both. Note the ratio.
2. **Sparsity test.** Repeat exercise 1 at three sparsity levels: 1 %, 10 %, 50 %, 90 % of creatures hungry. Plot the cost per tick. The filtered version should stay roughly constant in cost; the EBP version's cost should be roughly proportional to the fraction.
3. **A multi-state system.** A creature can be in any combination of `hungry`, `sleepy`, `dead`. Write three EBP systems: `drive_hunger`, `drive_sleep`, `drive_death`. Each iterates *only its own* presence table. Compare with a single filtered loop that handles all three with `if-else`.
4. **The branch you do not write.** Compile both versions in release mode. Look at the assembly (`cargo rustc --release -- --emit asm`). Confirm the EBP version has no `cmp` / `je` for the per-row check, while the filtered version has one. Note that the filtered version's branch *is correctly predicted*, but the cache line is still read.
5. **EBP with `&[T]` slices.** Most exercises so far use `Vec<u32>` for the presence table; in production, systems take `&[u32]` slices. Refactor your `drive_hunger_ebp` to take `hungry: &[u32]`. Confirm it still compiles cleanly with the rest of the system DAG.
6. *(stretch)* **A naive EBP bug.** A system that iterates `hungry` while also calling `hungry.push` on the table corrupts iteration. (You knew this from [§9](09_sort_breaks_indices.md) and [§22](22_mutations_buffer.md).) Construct a small case that demonstrates the bug. Then fix it via deferred cleanup.

Reference notes in [19_ebp_dispatch_solutions.md](19_ebp_dispatch_solutions.md).

## What's next

[§20 - Empty tables are free](20_empty_tables_are_free.md) names the consequence at scale: cost is proportional to active rows, not to population.
