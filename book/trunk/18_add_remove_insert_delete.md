# 18 — Add/remove = insert/delete

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 18](../../concepts/glossary.md#18--addremove--insertdelete).*

<p align="center"><img src="../illustrations/ebp_banner.jpg" alt="Three mice: EXISTENCE, BASED, PROCESSING" style="max-height: 300px; max-width: 100%;"></p>

In the flag world, a state transition is a write. To make a creature hungry, set `is_hungry = true`. To stop it being hungry, set `is_hungry = false`. The flag was always there; only its value changed.

In the presence world, a state transition is *a move between tables*. To make a creature hungry, *insert* a row into `hungry`. To stop it being hungry, *remove* the row. The state has no field to flip; it has only the question of which table the creature is currently a row of.

Code-wise, the difference is small:

```rust,ignore
// flag
fn become_hungry_flag(is_hungry: &mut [bool], slot: usize) {
    is_hungry[slot] = true;
}

// presence
fn become_hungry_presence(hungry: &mut Vec<u32>, id: u32) {
    hungry.push(id);
}

fn stop_being_hungry_presence(hungry: &mut Vec<u32>, id: u32) {
    if let Some(pos) = hungry.iter().position(|&x| x == id) {
        hungry.swap_remove(pos);
    }
}
```

Two consequences worth naming.

**The transition is structural.** When a creature crosses the hunger threshold, a row in `hungry` actually appears or disappears. There is no in-place mutation; the table grows by one or shrinks by one. This is why [§22](22_mutations_buffer.md) (mutations buffer; cleanup is batched) exists — adds and removes during a tick must be queued, then applied at the boundary, so that the iteration in progress does not see half the change. The deferred-cleanup pattern is born in this section.

**The vocabulary disappears.** There is no `set_hungry(true)`, no `set_hungry(false)`, no `is_hungry()` accessor pair. There is `become_hungry` (insert) and `stop_being_hungry` (remove), and even those are usually inlined into the system that detects the transition. The data-oriented program does not have getters and setters; it has *systems that move rows between tables*.

A useful test: can you describe the transition without naming a `bool`? *"This creature became hungry"* — well, did anything change? Yes: the `hungry` table grew by one entry. *"This creature stopped being hungry"* — the table shrank by one entry. Every state change in the system has a structural counterpart, and the structural counterpart is the canonical description.

> [!NOTE]
> *"Hungry" generalises further than this chapter uses it.* In an MMORPG, the presence table for "creatures the player needs to know about" is the ones inside the player's render radius — and the radius itself can shrink dynamically when CPU is tight, trading visible-creature count against the tick-budget headroom from [§4](04_cost_and_budget.md). **The presence table is a query, not a metaphysical state**; its entries change when the system asks a different question. *"Alive," "hungry," "in-scope," "subscribed," "active-this-frame"* — same shape, different question, same discipline of inserts and removes between tables.

The same pattern handles richer transitions. Imagine a creature that can be hungry, sleepy, or dead. Three tables: `hungry`, `sleepy`, `dead`. A creature transitions by moving between them. Becoming sleepy while hungry adds a row to `sleepy` (it can be in both). Dying removes the creature from `hungry` and `sleepy` (cleanup affects all relevant presence tables) and adds to `dead`. The transition is a multi-table operation, but each table is still just a list of ids.

This shape — state changes as inserts and removes — is the precondition for everything else EBP gives you. The dispatch in [§19](19_ebp_dispatch.md) iterates *over the table directly*, so the table's contents *being* the canonical state of the world is structurally necessary. There is no flag to consult; there is only what is in the table right now.

## Exercises

1. **Hunger transitions.** Use your `hungry` table from [§17](17_presence_replaces_flags.md). Each tick: read `energy`; for any creature that crossed below the threshold, push to `hungry`; for any that crossed back above, swap-remove. Run for 100 ticks with energy varying randomly; verify `hungry` always contains exactly the creatures whose current energy is below threshold.
2. **No flag, no setter.** Search your code for any boolean field on a creature. Replace it with a presence table. The setter and getter both disappear.
3. **A second presence state.** Add a `sleepy` table. A creature is sleepy if its energy is *high enough that it does not need to eat right now*. A creature can be in both `sleepy` and `hungry`? No — by definition the conditions are mutually exclusive. (Or: design them so they are.) Verify the invariant by checking after each tick that no creature appears in both tables.
4. **Death.** Add a `dead` table. When a creature's energy drops below zero, push to `dead` *and* remove from `hungry` (and from `sleepy` if present). The cleanup logic is now multi-table; introduce a small `transition_to_dead(id)` helper that handles all the affected presence tables.
5. **The transition log.** Add `events: Vec<(u64, u32, &'static str)>` (tick number, creature id, event name). Every insert/remove emits a row. After 100 ticks, the events log is the *canonical history* — every state change recorded. This is a preview of [§37 — The log is the world](37_log_is_world.md).
6. *(stretch)* **Reconstruct from the log.** Given only the events log and the initial `creatures` table, reconstruct the final `hungry`, `sleepy`, and `dead` tables. The reconstruction is a one-shot replay; if it produces the same tables as the live simulation, your transitions are correctly captured.

Reference notes in [18_add_remove_insert_delete_solutions.md](18_add_remove_insert_delete_solutions.md).

## What's next

[§19 — EBP dispatch](19_ebp_dispatch.md) names the dispatch shape that the table-membership representation makes free.
