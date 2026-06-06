# Solutions: 18 - Add/remove = insert/delete

Throughout, `i` is a slot and the membership tables hold slots; the event log records the stable entity id (`id[i]`), because the log is read back later ([§37](37_log_is_world.md)) when slots no longer apply.

## Exercise 1 - Hunger transitions

```rust,no_run
fn classify_hunger(energy: &[f32], hungry: &mut Vec<u32>) {
    let in_hungry: Vec<bool> = (0..energy.len())
        .map(|i| hungry.contains(&(i as u32)))
        .collect();
    for i in 0..energy.len() {
        let slot = i as u32;
        let starving = energy[i] < HUNGER_THRESHOLD;
        match (starving, in_hungry[i]) {
            (true, false) => hungry.push(slot),
            (false, true) => {
                if let Some(p) = hungry.iter().position(|&s| s == slot) {
                    hungry.swap_remove(p);
                }
            }
            _ => {} // no transition
        }
    }
}
```

The `contains` precompute is O(N) per slot here, O(N²) overall; [§23](23_index_maps.md)'s sparse set makes each membership test O(1). After each tick, a sanity check: `hungry` contains exactly the slots whose `energy < HUNGER_THRESHOLD`. Verifying this every tick is the kind of test [§43](43_tests_are_systems.md) names as "tests are systems".

## Exercise 2 - No flag, no setter

The conversion is mechanical. Find every `is_*: bool` field on a creature struct, delete it, add a presence table for the corresponding state. Replace `creature.is_hungry = true` with `hungry.push(slot)` and `creature.is_hungry = false` with a `swap_remove`. The setter and getter pair disappear.

The diff usually shrinks the codebase. Most flag-based systems have boilerplate - assertion that the flag is in the correct state, log on flag change, setter that fires events - that becomes redundant once the transition is itself a structural move.

## Exercise 3 - A second presence state

```rust,no_run
const SLEEPY_HIGH: f32 = 50.0;

fn classify_sleepy(energy: &[f32], sleepy: &mut Vec<u32>, hungry: &[u32]) {
    for i in 0..energy.len() {
        let slot = i as u32;
        let is_now = energy[i] >= SLEEPY_HIGH;
        let in_hungry = hungry.contains(&slot);
        let in_sleepy = sleepy.contains(&slot);
        match (is_now, in_sleepy, in_hungry) {
            (true, false, false) => sleepy.push(slot),
            (false, true, _) => {
                if let Some(p) = sleepy.iter().position(|&s| s == slot) {
                    sleepy.swap_remove(p);
                }
            }
            _ => {}
        }
    }
}

// Invariant check
fn invariant(hungry: &[u32], sleepy: &[u32]) {
    for &s in hungry {
        debug_assert!(!sleepy.contains(&s), "slot {} in both hungry and sleepy", s);
    }
}
```

Mutually exclusive states are enforced by *whoever can transition into them*. If only the classification system can add to either, and it reads energy and decides which table to add to, the invariant is maintained by construction.

## Exercise 4 - Death

```rust,no_run
fn transition_to_dead(
    i: u32,
    hungry: &mut Vec<u32>,
    sleepy: &mut Vec<u32>,
    dead: &mut Vec<u32>,
) {
    if let Some(p) = hungry.iter().position(|&s| s == i) { hungry.swap_remove(p); }
    if let Some(p) = sleepy.iter().position(|&s| s == i) { sleepy.swap_remove(p); }
    dead.push(i);
}
```

The helper makes the multi-table cleanup explicit and centralised. Future systems that add new presence states only need to update this one helper to handle deaths correctly.

## Exercise 5 - The transition log

The membership tables move by slot, but the event records the *entity* - read it from the id column at the slot:

```rust,no_run
events.push((tick, id[i], "became_hungry"));
// ... or
events.push((tick, id[i], "stopped_being_hungry"));
```

After 100 ticks the log is a complete history. Logging the entity rather than the slot is what makes the log survive: by the time it is read back, swap_remove and sort ([§21](21_swap_remove.md), [§28](28_proximity.md)) have moved slots around, but the entity id still names the same creature.

## Exercise 6 - Reconstruct from the log

The log records entities, so replay rebuilds *entity* sets:

```rust,no_run
fn replay(events: &[(u64, u32, &str)]) -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut hungry: Vec<u32> = Vec::new();  // entities, not slots
    let mut sleepy: Vec<u32> = Vec::new();
    let mut dead:   Vec<u32> = Vec::new();
    for (_t, entity, kind) in events {
        match *kind {
            "became_hungry" => hungry.push(*entity),
            "stopped_being_hungry" => {
                if let Some(p) = hungry.iter().position(|&e| e == *entity) {
                    hungry.swap_remove(p);
                }
            }
            "became_sleepy" => sleepy.push(*entity),
            // etc.
            "died" => {
                hungry.retain(|&e| e != *entity);
                sleepy.retain(|&e| e != *entity);
                dead.push(*entity);
            }
            _ => {}
        }
    }
    (hungry, sleepy, dead)
}
```

To compare with the live simulation, map each live slot through the id column to its entity (`id[slot]`) and compare the entity sets. They match if and only if every transition was logged. This is replay in miniature; the same shape generalises to the full simulator at [§37](37_log_is_world.md).
