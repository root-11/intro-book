# Solutions: 24 - Append-only and recycling

## Exercise 1 - Append-only logs

```rust,no_run
fn record_eat(eaten: &mut Vec<EatEvent>, tick: u64, creature: u32, food: u32) {
    eaten.push(EatEvent { tick, creature, food });
}

fn record_birth(born: &mut Vec<BirthEvent>, tick: u64, parent: u32, offspring: u32) {
    born.push(BirthEvent { tick, parent, offspring });
}
```

After 1 000 ticks of a 100-creature simulation with average 5 events per tick, the logs hold ~5 000 entries each. The `len()` only grows. The order of entries reflects insertion order, which is deterministic ([§16](16_determinism_by_order.md)).

## Exercise 2 - Recycling pool

Allocate 1 000 → slots 0-999 (high-water mark grows). Free 500 → slots 0-499 enter `free_slots`. Allocate 500 more → slots 0-499 are reused (LIFO from `free_slots.pop()`). The high-water mark stays at 1 000; no growth.

If you do not free anything, the pool grows indefinitely. If you free everything, the pool's allocations always reuse.

## Exercise 3 - Stale reference detection

```rust,no_run
let mut pool = SlotPool::new();
let (slot, gen0) = pool.allocate();   // (0, 0)
pool.free(slot);                       // gen[0] is now 1
let (slot2, gen1) = pool.allocate();   // (0, 1) - same slot, new gen
assert_eq!(slot, slot2);
assert_ne!(gen0, gen1);

// Old reference (slot, gen0) - should fail.
let valid = pool.gen[slot as usize] == gen0;
assert!(!valid);
```

The check is two reads and a comparison. References that hold the wrong generation get `None` from the dereference; references that hold the right generation get `Some(&row)`. No data corruption, no aliasing - the system is sound by construction.

## Exercise 4 - Append-only creatures

Run with no recycling. Plot `creatures.len()` over time:

```
tick     0:        100
tick  1000:       1100
tick 10000:      11000
tick 100000:    101000
```

Linear growth, unbounded. Memory leaks. The fix is one of: (a) recycle creatures, (b) truncate-and-snapshot periodically, (c) accept the bound for short runs.

## Exercise 5 - Recycling eaten

After 100 ticks, 90% of the `eaten` log has been overwritten by recent events. Asking "what did creature 42 eat at tick 50" returns either a different event (overwriting an old slot) or `None` if the old slot is on the free list. History is gone.

The lesson is the inverse of exercise 4: each strategy is correct for a *kind* of table, wrong for the other kind.

