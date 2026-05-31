# 17 - Presence replaces flags

<p align="center"><img src="../covers/phase_ebp.jpg" alt="Existence-based processing phase" style="max-height: 380px; max-width: 100%;"></p>

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 17](../../concepts/glossary.md#17--presence-replaces-flags).*

A creature can be hungry. Two ways to model it.

The instinct most programmers arrive with is a *boolean*: `is_hungry: bool` on every creature, set to `true` when energy drops below a threshold, set to `false` when energy is restored. Every system that cares about hunger checks the flag: `if creature.is_hungry { ... }`. This is everywhere; it is the natural choice; it is what most programmers reach for.

The data-oriented alternative is *membership*. There is a `hungry` table - a `Vec<u32>` of creature ids, or a parallel `Vec<bool>` mask, or a `BTreeSet<u32>`. A creature is hungry if and only if its id is in `hungry`. The flag does not exist as a field; it exists as a *fact about which table the creature appears in*.

The substitution looks small: a `bool` field becomes a row in another table. The implications are not.

**Dispatch** changes shape. The flag version is a per-creature filter inside every consuming system - walk all creatures, check the flag, do work if true. The membership version skips the filter - walk `hungry`, do work for every entry. At 1 000 000 creatures with 100 000 hungry, the flag version processes 1 000 000 rows; the membership version processes 100 000 - a 10× difference in work, and a 10× difference in memory bandwidth. [§19](19_ebp_dispatch.md) names this.

**Storage** changes shape. A flag column stores one byte per creature whether the flag is set or not. A creature with eight possible states needs eight `bool` fields = 8 bytes per creature; a million creatures store 8 MB of flags, most of which are `false`. A presence table stores only the entries that *are* set - if 10 % of creatures are hungry, the `hungry` table is 10 % the size of the flag column.

**Persistence** changes shape. Serialising a flag column writes the flag for every creature, including the ones where it is `false`. Serialising a presence table writes only the entries that exist. The latter is also closer to the natural shape of an event log ([§37](37_log_is_world.md)): a `hungry_added` event per entry, and that is the whole story.

**Concurrency** changes shape. Two flag fields on the same creature struct may share a cache line; concurrent writers to either field fight over it ([§33](33_false_sharing.md) - false sharing). Two presence tables are physically separate `Vec`s; concurrent writers to disjoint tables never collide ([§31](31_disjoint_writes_parallelize.md)).

The clean way to phrase the move comes from Richard Fabian's [*Data-Oriented Design*](https://www.dataorienteddesign.com/dodbook/), in the chapter on Existence Based Processing. *Instead of asking each room about its doors, ask the doors-table which doors are in this room.* The query is reversed; the lookup is reversed; the work shrinks. Most programs spend their lives doing the wrong direction; the data-oriented mindset is to reverse it.

A production example: in a real ECS daemon, an admission decision is `is_admitted(peer) = established_contacts.contains_key(peer)`. There is no `is_admitted: bool` on a peer; there is only the question "is this peer's id in the table?". O(1), no I/O, no enum.

Presence is not the only valid representation. A `bool` flag is sometimes right - when nearly every entity has the state set; when the predicate is computed cheaply on the fly; when the data is short-lived and persistence does not matter. But in this book, presence is the default; flags are a tradeoff to earn.

## Exercises

These extend the §0 simulator skeleton.

1. **Add a `hungry` table.** Add `let mut hungry: Vec<u32> = Vec::new();` to your world. It is empty at start.
2. **Populate it.** Write a system `fn classify_hunger(energy: &[f32], ids: &[u32], hungry: &mut Vec<u32>)`. Walk creatures; if `energy[i] < HUNGER_THRESHOLD` and `ids[i]` is not already in `hungry`, push it. (For now use linear scan to check membership; we will fix this in §23.)
3. **Build the flag version.** Add a parallel `is_hungry: Vec<bool>` indexed by creature slot. Write the equivalent classification system that sets/clears the bool.
4. **Time both at 1M creatures, 10% hungry.** Build a 1 000 000-creature world with 10% energy starvation. Time `classify_hunger` (presence) and the flag-setting version. Note the ratio of *bytes touched*: the flag version writes 1 MB, the presence version writes ~100 KB plus the cost of the membership check.
5. **The membership query.** Write `fn is_hungry_p(hungry: &[u32], id: u32) -> bool` (presence) and `fn is_hungry_f(is_hungry: &[bool], slot: usize) -> bool` (flag). Time both at 1M creatures. Note: presence is O(N) without an index map; the flag is O(1). [§23 - Index maps](23_index_maps.md) is the fix that makes presence O(1) too.
6. **The "how many are hungry" query.** Write it both ways. Presence: `hungry.len()`. Flag: `is_hungry.iter().filter(|&&b| b).count()`. Compare. The presence version is constant-time; the flag version walks all 1M.
7. *(stretch)* **Persist both.** Serialise both representations to a file. Note the disk size for 1M creatures with 10% hungry. The presence version stores ~100 KB; the flag version stores ~1 MB even though most flags are `false`.

Reference notes in [17_presence_replaces_flags_solutions.md](17_presence_replaces_flags_solutions.md).

## What's next

[§18 - Add/remove = insert/delete](18_add_remove_insert_delete.md) names what *changes* between the two representations: in the presence world, state transitions are structural moves between tables, not flag flips.
