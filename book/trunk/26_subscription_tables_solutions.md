# Solutions: 26 - Subscription tables, keyed by slot

Throughout, `i` is a slot (a creature's position in the columns) and `entity` is a stable id. The numbers below are modern-desktop figures from `ebp_partition` at N = 1 000 000; the chapter's Measurements table and `code/README.md` carry the per-machine spread.

## Exercise 1 - Build a slot-keyed subscription

`hungry` holds the slots of hungry creatures. A creature subscribes when its energy crosses below the threshold; a parallel `in_hungry: Vec<bool>` (one per slot) stops it being pushed twice.

```rust,no_run
let mut hungry: Vec<u32> = Vec::new();        // slots, not ids
let mut in_hungry: Vec<bool> = vec![false; n];

fn classify(energy: &[f32], in_hungry: &mut [bool], hungry: &mut Vec<u32>, thr: f32) {
    for i in 0..energy.len() {
        if energy[i] < thr && !in_hungry[i] {
            hungry.push(i as u32);
            in_hungry[i] = true;
        }
    }
}

// the hot loop: walk the subscription, index the columns directly by slot
fn feed(energy: &mut [f32], hungry: &[u32], feed: f32) {
    for &i in hungry {
        energy[i as usize] += feed;
    }
}
```

The hot loop never looks at a creature that is not hungry. There is no scan of the attribute table and no branch.

## Exercise 2 - Key it by id instead, and time both

```rust,no_run
fn feed_id(energy: &mut [f32], hungry_id: &[u32], id_to_slot: &[u32], feed: f32) {
    for &entity in hungry_id {
        let i = id_to_slot[entity as usize] as usize; // the extra hop
        energy[i] += feed;
    }
}
```

Measured, 10 % subscribed: the slot-keyed loop runs about 0.38 ms/pass, the id-keyed loop about 0.85 ms/pass - roughly 2x. The id version's extra time is the `id_to_slot[entity]` load. `id_to_slot` is a 4 MB array (one `u32` per id at 1M ids) and the subscribed entities are scattered through it, so consecutive entries almost never share a 64-byte cache line: the lookup is a miss *before* the column gather has even started. The slot key has no such array to consult.

## Exercise 3 - Unsubscribe in O(1)

To `swap_remove` slot `i` out of `hungry` you need its *position in the table*, not the slot value. Keep a parallel `pos: Vec<u32>` mapping a slot to its index in `hungry` - the [§23](23_index_maps.md) trick, one level up (there it mapped id to slot; here it maps slot to table-position).

```rust,no_run
fn unsubscribe(hungry: &mut Vec<u32>, pos: &mut [u32], in_hungry: &mut [bool], i: u32) {
    let p = pos[i as usize] as usize;
    let moved = *hungry.last().unwrap();   // the slot that will backfill p
    hungry.swap_remove(p);
    pos[moved as usize] = p as u32;        // its new table-position
    in_hungry[i as usize] = false;
}
```

`pos` is itself indexed by slot, so it rides along with the columns and must be rebuilt at compaction (Exercise 4), the same as any other slot-keyed structure.

## Exercise 4 - Reindex on compaction

Compaction produces `old_to_new[old_slot] = new_slot` for the survivors. The slot-keyed subscription is rewritten through it in one pass:

```rust,no_run
fn reindex(hungry: &mut [u32], old_to_new: &[u32]) {
    for i in hungry.iter_mut() {
        *i = old_to_new[*i as usize];
    }
}
```

The id-keyed version needs *no* change to `hungry_id`: it holds entities, and entities do not move. Only `id_to_slot` is rebuilt over the live set - which the slot-keyed design also rebuilds, because identity at the boundary still resolves through it.

Measured, ~100 000 live: the id-keyed reindex is about 0.13 ms (rebuild `id_to_slot`); the slot-keyed reindex is about 0.17 ms at one subscription per entity (the same rebuild plus the table rewrite). Both run once per cleanup interval, not once per tick - which is why the slot key wins overall (Exercise 7).

## Exercise 5 - Dense vs scattered

Measured: the slot-keyed hot loop costs about 0.37 ms/pass when the subscribed slots are scattered through the column, and about 0.084 ms/pass after compacting them to the front - roughly 4.4x. The compaction pass itself costs about 0.88 ms (one batch reindex of the ~100 000 live). It saves ~0.29 ms/pass, so it pays for itself after about 3 passes (ticks) and is pure profit until churn re-scatters the slots. This is the same pass as [§28](28_proximity.md) and the same pass that reclaims dead slots ([§24](24_append_only_and_recycling.md)).

## Exercise 6 - The subscription that holds everyone

Measured at full participation: the subscription loop costs about 1.20 ms/pass, a plain `for i in 0..n` scan about 1.03 ms/pass. The subscription is *slower*. It does the same N column touches, plus an extra read of the subscription array per element, and it gives up the clean `0..n` bound the compiler optimises best.

The rule: build a subscription when the subset is durably a fraction of the population (hungry, sleeping, in-combat). For a set that holds nearly everyone on most ticks (`alive`), scan. A subscription earns its keep by what it *excludes*; one that excludes nothing is bookkeeping with no payoff.

## Exercise 7 - Two subscriptions, one entity

On compaction every subscription an entity sits in must be remapped, so the slot-keyed reindex grows with the count `S`. Measured, ~100 000 live: about 0.17 ms at `S = 1`, 0.23 ms at `S = 2`, 0.36 ms at `S = 4`. The id-keyed reindex stays flat (~0.13 ms) because the id tables are never touched.

The slot key still wins, decisively, because the two costs live on different clocks. The reindex is paid once per cleanup interval; the id key's redirection (~0.47 ms/pass) is paid *every tick*. Over a 30-tick interval the slot key saves about 14 ms in the hot loop and spends at most ~0.4 ms more in reindex. For any realistic interval and any small `S`, slots win - which is the measured basis for keying subscriptions by slot.
