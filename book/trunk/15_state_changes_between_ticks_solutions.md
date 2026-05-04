# Solutions: 15 — State changes between ticks

## Exercise 1 — The bug

```rust,ignore
// BUG: do not do this.
let mut i = 0;
while i < creatures.len() {
    if energy[i] <= 0.0 {
        creatures.swap_remove(i);
        energy.swap_remove(i);
        // Do NOT advance i — the swap put a fresh creature here.
    } else {
        i += 1;
    }
}
```

This *almost* works — but the fix `else { i += 1 }` is fragile. Forget it once and you end up incrementing past the swapped-in creature, skipping the check. Worse, if `apply_starve` is one of three systems that both read and write `creatures` mid-tick, the indexes other systems hold become stale silently.

## Exercise 2 — The fix

```rust
fn apply_starve(energy: &[f32], to_remove: &mut Vec<u32>, ids: &[u32]) {
    for i in 0..energy.len() {
        if energy[i] <= 0.0 {
            to_remove.push(ids[i]);
        }
    }
}
```

The function only reads. Mutation lives in `cleanup`, which runs once after every system has had its say. All 30 starvers die — no accidental skips, no mid-loop index hazards.

## Exercise 3 — The cleanup pass

```rust,no_run
fn cleanup(world: &mut World, to_remove: &mut Vec<u32>, to_insert: &mut Vec<CreatureRow>) {
    // Removals first
    for &id in to_remove.iter() {
        let slot = world.id_to_slot[id as usize];
        for col in world.columns_mut() {
            col.swap_remove(slot);
        }
        world.id_to_slot[id as usize] = u32::MAX; // mark dead
    }
    to_remove.clear();

    // Insertions second
    for row in to_insert.drain(..) {
        let slot = world.append_creature(row);
        world.id_to_slot[row.id as usize] = slot as u32;
    }
}
```

Removals first because they free slots that an insertion *might* reuse if you implemented slot recycling. If you insert before removing, you grow the table needlessly only to immediately shrink it.

## Exercise 4 — Two ticks

After tick 1, log `creatures.len()`. The 30 dead creatures are still in `creatures` *during* the systems of tick 1 (each system saw them) but `cleanup` removed them at tick 1's boundary. Tick 2's input has 70 creatures. A creature killed in tick 1 is gone for tick 2.

## Exercise 5 — Insertions are tick-delayed

Add an `age_in_ticks: Vec<u32>` column. Initialise to 0. Increment every creature in a system at the end of each tick. An offspring inserted in tick 5 is in `to_insert` during tick 5 → moved to `creatures` by `cleanup` at the end of tick 5 → first visible to systems in tick 6. Its `age_in_ticks` starts at 0 and reaches 1 after tick 6's increment. The newborn never receives tick 5's update.

## Exercise 6 — A bad design

```
Tick start: creatures = [A, B, C, D].
A starves: collected.
B reproduces, producing X.
C is fine.
D starves: collected.

In-tick application, reverse-index order:
  swap_remove D from slot 3 → creatures = [A, B, C]
  swap_remove A from slot 0 → creatures = [C, B]   (C was at slot 2, gets swapped in)
  append X                  → creatures = [C, B, X]

But X's ID was supposed to map to its slot for everyone holding a reference;
some other system collected before our `apply_starve` was holding `id(X) → ?`
because X did not exist yet. Now X exists at slot 2. The reference is stale or invalid.
```

The principled fix is the rule: between systems, no mutations. Defer to cleanup. The reverse-index trick fixes one specific bug while opening the door to many.
