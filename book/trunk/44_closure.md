# 44 — What you have built

The previous forty-three sections were a long climb. This one is a look down.

You have built a small ecosystem simulator that runs deterministically, scales from one hundred creatures to streaming workloads, and exposes its state to inspection at every tick. You did this with `Vec`s and functions — no inheritance, no traits unless you wanted them, no framework. The discipline that made it work is the entire content of the book.

## The shape that carried the whole thing

Three patterns showed up everywhere:

**Tables, not objects.** A creature is not a struct of fields with methods. It is a row across columns kept aligned by index — `pos[i]`, `vel[i]`, `energy[i]`. Each column is a `Vec`. The columns have one writer each; they grow and shrink in lockstep. There is no container holding them together — only the discipline.

**Systems, not state.** Behaviour is a function over tables. `motion` reads `vel`, writes `pos`. `apply_starve` reads `energy`, pushes ids to `to_remove`. Each system has a name, a read-set, a write-set. The simulator is the DAG of systems composed in order. State changes happen between ticks, not inside them.

**Mechanism separated from policy.** The kernel exposes verbs (insert, remove, swap, push to buffer, batched cleanup). The rules live at the edges (when does a creature die, when does food spawn, what counts as a collision). The same kernel runs every variation; the policies change without it.

Those three are not Rust-specific. They are not even ECS-specific. They are what data-oriented design names. The rest of the book — locality, parallelism, persistence, anytime algorithms — falls out of taking those three seriously.

<p align="center"><img src="../illustrations/mathematics_describes.jpg" alt="Mathematics describes, models, implements, and improves the world." style="max-height: 300px; max-width: 100%;"></p>

## What this approach buys

- **Speed by default**, because the layout matches the machine.
- **Determinism without locks**, because ordering is the contract.
- **Testability**, because each system is a pure function over its inputs.
- **Onboardability**, because the data is visible. A reader can `print!` every column and see the world.
- **Refactor cheap**, because there are no objects with hidden state to migrate.

## What this approach costs

- **Less abstraction.** You feel the machine. Some find this freeing; some find it exhausting.
- **More discipline.** Single-writer rules, mutation buffering, lockstep sorts — the language does not enforce these. You do.
- **Less idiomatic Rust.** The book uses very little of Rust's type system: traits, lifetimes, and generics appear when they pay rent and not before. Idiomatic Rust looks different.
- **A different mental model.** Engineers trained in OOP will not naturally reach for tables. The translation cost is real.

## Open questions the book did not settle

The book made choices. Other books make different ones. Worth knowing where you sit:

- **Why not Bevy, specs, or another existing ECS framework?** Faster to start, harder to see through. We did the slow thing on purpose. After §43 you can read Bevy's ECS source and tell whether its choices match yours.
- **Is a row really better than a struct?** For a single creature, no. For a million, yes. The crossover depends on your workload; §3 names the tradeoff but does not prescribe.
- **Could this have been C, or Zig?** Yes. The ideas are language-independent. Rust contributes the borrow checker and zero-cost abstractions; the rest is layout discipline.
- **What about networking and rollback?** §31–§34 covers single-machine concurrency. Distributing the world across machines is a different book — see Glenn Fiedler's GDC talks for the rollback-netcode pattern.
- **What about types and traits?** Two of Rust's three big features barely appear in the trunk. Future work might explore where generics and traits *do* pay rent in an ECS — usually at the boundary (serialisation, debug rendering) rather than the kernel.

## Where to go next

- **Read Mike Acton's "Data-Oriented Design and C++"** (CppCon 2014). Forty-five minutes; the most concentrated case for this approach you will find.
- **Read Casey Muratori's *Handmade Hero*** episodes on grid storage and cache locality. Another route to the same conclusions.
- **Open Bevy's `bevy_ecs` crate.** You will recognise every pattern. The names will differ; the shapes are identical.
- **Extend the simulator.** The genetics and predator-prey extensions flagged in the [simulator spec](../../code/sim/SPEC.md) break new ground without leaving the framework you have already built.

<p align="center"><img src="../illustrations/model_real_world.jpg" alt="Model the real world." style="max-height: 300px; max-width: 100%;"></p>

The book ends here. The simulator does not — it runs as long as you keep the discipline.
