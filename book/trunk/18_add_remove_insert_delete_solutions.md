# Solutions: 18 — Add/remove = insert/delete

## Exercise 1 — Hunger transitions

```rust,no_run
fn classify_hunger(energy: &[f32], ids: &[u32], hungry: &mut Vec<u32>) {
    let in_hungry: Vec<bool> = (0..energy.len())
        .map(|i| hungry.contains(&ids[i]))
        .collect();
    for i in 0..energy.len() {
        let starving = energy[i] < HUNGER_THRESHOLD;
        match (starving, in_hungry[i]) {
            (true, false) => hungry.push(ids[i]),
            (false, true) => {
                if let Some(p) = hungry.iter().position(|&x| x == ids[i]) {
                    hungry.swap_remove(p);
                }
            }
            _ => {} // no transition
        }
    }
}
```

After each tick, a sanity check: `hungry` contains exactly the creatures whose `energy < HUNGER_THRESHOLD`. Verifying this every tick is the kind of test [§43](43_tests_are_systems.md) names as "tests are systems".

## Exercise 2 — No flag, no setter

The conversion is mechanical. Find every `is_*: bool` field on a creature struct, delete it, add a presence table for the corresponding state. Replace `creature.is_hungry = true` with `hungry.push(creature.id)` and `creature.is_hungry = false` with `swap_remove`. The setter and getter pair disappear.

The diff usually shrinks the codebase. Most flag-based systems have boilerplate — assertion that flag is in correct state, log on flag change, setter that fires events — that becomes redundant once the transition is itself a structural move.

## Exercise 3 — A second presence state

```rust,no_run
const SLEEPY_HIGH: f32 = 50.0;

fn classify_sleepy(energy: &[f32], ids: &[u32], sleepy: &mut Vec<u32>, hungry: &[u32]) {
    for i in 0..energy.len() {
        let is_now = energy[i] >= SLEEPY_HIGH;
        let in_hungry = hungry.contains(&ids[i]);
        let in_sleepy = sleepy.contains(&ids[i]);
        match (is_now, in_sleepy, in_hungry) {
            (true, false, false) => sleepy.push(ids[i]),
            (false, true, _) => {
                if let Some(p) = sleepy.iter().position(|&x| x == ids[i]) {
                    sleepy.swap_remove(p);
                }
            }
            _ => {}
        }
    }
}

// Invariant check
fn invariant(hungry: &[u32], sleepy: &[u32]) {
    for &h in hungry {
        debug_assert!(!sleepy.contains(&h), "creature {} in both hungry and sleepy", h);
    }
}
```

Mutually exclusive states are enforced by *whoever can transition into them*. If only the classification system can add to either, and the classification system reads energy and decides which table to add to, the invariant is maintained by construction.

## Exercise 4 — Death

```rust,no_run
fn transition_to_dead(
    id: u32,
    hungry: &mut Vec<u32>,
    sleepy: &mut Vec<u32>,
    dead: &mut Vec<u32>,
) {
    if let Some(p) = hungry.iter().position(|&x| x == id) { hungry.swap_remove(p); }
    if let Some(p) = sleepy.iter().position(|&x| x == id) { sleepy.swap_remove(p); }
    dead.push(id);
}
```

The helper makes the multi-table cleanup explicit and centralised. Future systems that add new presence states only need to update this one helper to handle deaths correctly.

## Exercise 5 — The transition log

```rust,no_run
events.push((tick, ids[i], "became_hungry"));
// ... or
events.push((tick, ids[i], "stopped_being_hungry"));
```

After 100 ticks the log is a complete history. To verify, take an initial empty world plus the log and replay. The reconstructed `hungry`, `sleepy`, `dead` should match the live tables exactly. If they don't, an event was missed (the simulator mutated state without logging) or the state was loaded from somewhere outside the log. Both are bugs, both are caught by the equality check.

## Exercise 6 — Reconstruct from the log

```rust,no_run
fn replay(initial_creatures: &[CreatureRow], events: &[(u64, u32, &str)])
    -> (Vec<u32>, Vec<u32>, Vec<u32>)
{
    let mut hungry: Vec<u32> = Vec::new();
    let mut sleepy: Vec<u32> = Vec::new();
    let mut dead:   Vec<u32> = Vec::new();
    for (_t, id, kind) in events {
        match *kind {
            "became_hungry" => hungry.push(*id),
            "stopped_being_hungry" => {
                if let Some(p) = hungry.iter().position(|&x| x == *id) {
                    hungry.swap_remove(p);
                }
            }
            "became_sleepy" => sleepy.push(*id),
            // etc.
            "died" => {
                hungry.retain(|&x| x != *id);
                sleepy.retain(|&x| x != *id);
                dead.push(*id);
            }
            _ => {}
        }
    }
    (hungry, sleepy, dead)
}
```

This is replay in miniature. The same shape generalises to the full simulator at [§37](37_log_is_world.md).
