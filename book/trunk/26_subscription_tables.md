# 26 - Subscription tables, keyed by slot

<p align="center"><img src="../covers/phase_scale.jpg" alt="Scale phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 26](../../concepts/glossary.md#26--subscription-tables-keyed-by-slot).*

A system rarely touches every entity. Motion moves all of them, but starvation only reads the hungry, reproduction only the well-fed, a sleep timer only the sleeping. [§17](17_presence_replaces_flags.md) gave the tool for "which entities are in this set": a membership table. [§19](19_ebp_dispatch.md) measured the payoff: iterate the 100 000 hungry instead of scanning 1 000 000 and branching, and the work is proportional to the subset, not the population.

Call that membership table a *subscription table*, and the loop that walks it a system's *hot loop*. A creature *subscribes* to `hungry` when its energy drops; it *unsubscribes* when it eats. The subscription is the system's input; the hot loop is the system. This section settles a question [§17](17_presence_replaces_flags.md) left open: what does a subscription table store, and how fast is the hot loop that reads it?

**A wrong turn first: splitting fields.**

The instinct many readers arrive with is to split the *fields* of a creature into hot and cold: put the fields an inner loop touches in one struct, the rarely-read fields in another, so a load does not drag in bytes the loop ignores. In a row-oriented (array-of-structs) world this is a real technique. In the structure-of-arrays world this book has used since [§7](07_structure_of_arrays.md), it is already done for you: every field is its own column. Reading `px` never touches `birth_t`; they are different arrays. There is no row to drag a cold field along with, so there is nothing to split. The bandwidth win that motivates a field split in AoS is the same win SoA already banked back in [§7](07_structure_of_arrays.md); it is not a separate technique to apply here.

So the attribute table is never split. It stays whole: every column, every slot, reachable by index `i`. What a system changes is not the table but how it *reaches into* it. Rather than scan the whole table and branch, a system keeps a subscription table, the slots `i` it cares about, and indexes straight in: no scan, no field split, direct access. The rest of this section is about making that direct access fast.

**What a subscription stores: ids or slots.**

A creature has a stable [id](10_stable_ids_and_generations.md) and a current [slot](23_index_maps.md), its position in the attribute columns. `id_to_slot` maps one to the other. A subscription table can hold either, and the choice is not cosmetic.

- Hold *ids*. The hot loop reads an id, looks up `id_to_slot[id]` to find the slot, then gathers the columns. One extra load per element, every tick. The table survives relocation untouched: when [cleanup](24_append_only_and_recycling.md) moves entities, only `id_to_slot` changes.
- Hold *slots*. The hot loop reads a slot and gathers the columns directly, no redirection. But when cleanup moves entities, every slot in every subscription is now stale and must be rewritten.

The redirection is paid every tick. The rewrite is paid once per cleanup interval. Which loses?

**Measured.** At 1 000 000 creatures with a tenth subscribed, the id-keyed hot loop runs about twice as slow as the slot-keyed one. The cost is not the extra instruction. `id_to_slot` is a four-megabyte array and the subscribed ids are scattered through it, so each lookup is a cache miss before the column gather has even started. The slot key skips that miss entirely.

The rewrite the slot key pays in return is small and bounded. It scales with how many subscriptions an entity sits in (rewrite each table the entity appears in), but it happens once per cleanup interval, not once per tick. Across the realistic range, a handful of subscriptions and a cleanup every few dozen ticks, the per-tick saving buries the per-interval rewrite by roughly two to one. The numbers are in `code/README.md`; the benchmark is `ebp_partition`.

So **subscription tables hold slots.** This is also *why* the lifecycle keeps [stable slots](24_append_only_and_recycling.md) and lets cleanup own the reindex: slot keys are only safe when one system is responsible for rewriting them when entities move. The cleanup can do that for any reference it owns - a subscription, or a cross-entity link stored in a column - remapping them all in one pass.

So what is the stable [id](10_stable_ids_and_generations.md) for, once the hot loop runs on slots? For every reference the cleanup *cannot* reach to rewrite. A save file ([§36](36_persistence_is_serialization.md)), a replay log ([§37](37_log_is_world.md)) whose events are `(entity, key, value)`, a packet on the wire, an entity the UI has selected, a snapshot a slow background system is still reading ([§39](39_system_of_systems.md)): a slot is meaningless to all of them, because the next compaction moves it. They hold the id and resolve it through `id_to_slot` once, at the boundary ([§35](35_boundary_is_the_queue.md)), never per element. Slots are an internal, momentary fact; the id (with its generation, [§10](10_stable_ids_and_generations.md)) is the identity that survives a relocation, a save, and a network hop.

**Locality: a slot-keyed loop is fast only when its slots are dense.**

A slot-keyed hot loop gathers columns at the slots the subscription lists. If those slots are scattered through the column, which is what churn produces as deaths and births leave holes, the gather misses cache on nearly every element. If they are contiguous, the gather streams. Compacting the live, subscribed entities to the front of the columns turns a scattered gather into a sequential one; measured, that is several times faster. The compaction is not free, but it pays for itself within a few ticks, and it is the same batch pass that reclaims dead slots. [§28](28_sort_for_locality.md) is that pass.

**The one case a split would help, in full view.**

There is a single scenario where grouping fields would still pay. A hot loop that reads several columns at *scattered* slots touches one cache line per column per element; interleaving those columns into one record would touch one. That case is real, and worth stating plainly rather than hiding behind the principle. We keep the columns separate anyway. The book's answer to scatter is to remove it: compaction ([§28](28_sort_for_locality.md)) makes the subscribed slots dense, a dense gather streams each column at full bandwidth, and the per-column cost the interleaving would have saved is gone, paid back within a few ticks. Interleaving would also forfeit what SoA bought in [§7](07_structure_of_arrays.md), whole-column streaming and SIMD, on every loop that is not scattered, to win the one that is. So the rule stands with its exception in the open: keep the columns separate, and compact when the gather scatters.

**Name the subscription before you build it.**

A subscription is earned by a system that genuinely processes a subset. "Most creatures are not hungry on most ticks, so `hungry` is far smaller than the population" is a sound reason to build one. "Every creature is always in `alive`, but other engines keep an alive-set" is not. A subscription that holds the whole population is a scan-all with extra bookkeeping, and the measurement says so: at full participation the subscription loop is marginally *slower* than a plain scan. The subscription wins in proportion to how much it excludes, and not otherwise.

## Exercises

These extend the simulator's `creature` columns and the `id_to_slot` map from [§23](23_index_maps.md).

1. **Build a slot-keyed subscription.** Add `hungry: Vec<u32>` holding the *slots* of hungry creatures. A creature subscribes (push its slot) when `energy[slot]` drops below a threshold. Write the hot loop: iterate `hungry`, gather the columns by slot directly. Verify it touches only the subscribed creatures.
2. **Key it by id instead, and time both.** Build a second version where `hungry` holds entity ids and the hot loop resolves each through `id_to_slot[entity]` before the gather. At 1M creatures with 10% subscribed, time both hot loops. Reproduce the ~2x gap. Where does the id version's time go? Compare the size of `id_to_slot` with one cache line.
3. **Unsubscribe in O(1).** When a creature stops being hungry, remove its slot from `hungry` with `swap_remove`. What do you need alongside `hungry` to find the slot's *position in the table* without scanning it? (It is the [§23](23_index_maps.md) trick again, one level up.)
4. **Reindex on compaction.** Relocate the live creatures to the front of the columns (a stand-in for the [§24](24_append_only_and_recycling.md)/[§28](28_sort_for_locality.md) cleanup), producing an old-slot to new-slot map. Rewrite the slot-keyed `hungry` through that map; confirm the hot loop still processes the same creatures. Now do the same for the id-keyed version: what has to change? Time both reindex passes.
5. **Dense vs scattered.** Time the slot-keyed hot loop with the subscription's slots scattered through the column, then again after compacting them to the front. Reproduce the several-fold speedup. How many ticks of hot-loop saving pay back one compaction pass?
6. **The subscription that holds everyone.** Subscribe every creature and time the hot loop against a plain `for i in 0..n` scan. The subscription should be no faster, and slightly slower. Explain why, and state the rule for when a subscription is worth building.
7. *(stretch)* **Two subscriptions, one entity.** Put creatures in both `hungry` and `sleepy`. On compaction, both tables need rewriting. Measure how the reindex cost grows with the number of subscriptions an entity sits in, and argue why it stays cheaper than the id key's per-tick redirection for any realistic cleanup interval.

Reference notes in [26_subscription_tables_solutions.md](26_subscription_tables_solutions.md).

## What's next

[§27 - Working set vs cache](27_working_set_vs_cache.md) puts numbers on the question this section kept leaning on: how big *is* the hot loop's footprint, and what cache level does it fit in?
