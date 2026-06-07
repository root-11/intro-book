# 44 - What you have built

The previous forty-three sections were a long climb. This one is a look down.

You have built a small ecosystem simulator that runs deterministically, scales from one hundred creatures to streaming workloads, and exposes its state to inspection at every tick. You did this with `Vec`s and functions - no inheritance, no traits unless you wanted them, no framework. The discipline that made it work is the entire content of the book.

## The shape that carried the whole thing

Three patterns showed up everywhere:

**Tables, not objects.** A creature is not a struct of fields with methods. It is a row across columns kept aligned by index - `pos[i]`, `vel[i]`, `energy[i]`. Each column is a `Vec`. The columns have one writer each; they grow and shrink in lockstep. There is no container holding them together - only the discipline.

**Systems, not state.** Behaviour is a function over tables. `motion` reads `vel`, writes `pos`. `apply_starve` reads `energy`, pushes ids to `to_remove`. Each system has a name, a read-set, a write-set. The simulator is the DAG of systems composed in order. State changes happen between ticks, not inside them.

**Mechanism separated from policy.** The kernel exposes verbs (insert, remove, swap, push to buffer, batched cleanup). The rules live at the edges (when does a creature die, when does food spawn, what counts as a collision). The same kernel runs every variation; the policies change without it.

Those three are not Rust-specific. They are not even ECS-specific. They are what data-oriented design names. The rest of the book - locality, parallelism, persistence, anytime algorithms - falls out of taking those three seriously.

<p align="center"><img src="../illustrations/mathematics_describes.jpg" alt="Mathematics describes, models, implements, and improves the world." style="max-height: 300px; max-width: 100%;"></p>

## What this approach buys

- **Speed by default**, because the layout matches the machine.
- **Determinism without locks**, because ordering is the contract.
- **Testability**, because each system is a pure function over its inputs.
- **Onboardability**, because the data is visible. A reader can `print!` every column and see the world.
- **Refactor cheap**, because there are no objects with hidden state to migrate.

## What this approach costs

- **Less abstraction.** You feel the machine. Some find this freeing; some find it exhausting.
- **More discipline.** Single-writer rules, mutation buffering, lockstep sorts - the language does not enforce these. You do.
- **Less idiomatic Rust.** The book uses very little of Rust's type system: traits, lifetimes, and generics appear when they pay rent and not before. Idiomatic Rust looks different.
- **A different mental model.** Engineers trained in OOP will not naturally reach for tables. The translation cost is real.

## Two acts: building it, and living with it

Read back, the book has two acts. The first is *building something that works, and lasts*. Sections 1-39 made it run - deterministic, scaled from a hundred creatures past the million-entity wall, parallel on disjoint writes, persisted and replayable. Sections 40-43 made it durable to *change*: mechanism vs policy, deferred abstraction, dependency pricing, tests-are-systems - the discipline that holds four of the five costs of ownership: extendibility, maintainability, performance, and memory.

The second act is *living with it* once it is in service - a different question entirely. The fifth cost of ownership, **operations** - recovering it, observing it, trusting it across machines and deadlines - only bites when the system is deployed and the human who used to watch it is gone. That act begins in [§45](45_living_with_it.md).

## Open questions the book did not settle

The book made choices. Other books make different ones. Worth knowing where you sit:

- **Why not Bevy, specs, or another existing ECS framework?** Faster to start, harder to see through. We did the slow thing on purpose. After §43 you can read Bevy's ECS source and tell whether its choices match yours.
- **Is a row really better than a struct?** For a single creature, no. For a million, yes. The crossover depends on your workload; §3 names the tradeoff but does not prescribe.
- **Could this have been C, or Zig?** Yes. The ideas are language-independent. Rust contributes the borrow checker and zero-cost abstractions; the rest is layout discipline.
- **What about networking and rollback?** §31-§34 covers single-machine concurrency. Distributing the world across machines is a different book - see Glenn Fiedler's GDC talks for the rollback-netcode pattern.
- **What about types and traits?** Two of Rust's three big features barely appear in the trunk. Future work might explore where generics and traits *do* pay rent in an ECS - usually at the boundary (serialisation, debug rendering) rather than the kernel.

## The horizon: living with it at production scale

The list above is choices of taste - other books choose differently. This list is not. It is where what the first act built leaves a real gap the moment the system is in service. Turning a deterministic in-memory simulator into a system you ship, evolve, observe, and recover is the next mile - and the second act walks it. Each gap is named here against the criterion it threatens; together they are the map of the chapters ahead.

- **Schema evolution** (extendibility). [§36](36_persistence_is_serialization.md) versions a save with a header byte. Renaming a column, splitting one, changing a unit, back-filling a derived column - each is a project, not a paragraph. The fast column-direct format makes every file in the wild a hostage to today's layout. The triple-store of [§37](37_log_is_world.md) is the start of a fix; schema-as-data - a column registry and a forward/back migration runner - is the rest.
- **Crash consistency** (operations). "The log is the world" holds only while the log survives power loss. Torn writes, fsync barriers, atomic rename, idempotent replay after a half-written batch - [§38](38_storage_systems.md) names fsync once and stops. For a save-game that is fine; for a system of record it is the whole problem.
- **Numerical determinism under parallelism** (operations). The parallel-reduction gotcha named in [§16](16_determinism_by_order.md): same seed, different thread count, different bits. Replay across heterogeneous hardware needs a fixed reduction order or integer accumulation, not just "no threads inside a system".
- **Observability** (operations). "The data is visible; `print!` every column" is a debugger's story, not an on-call engineer's at 2 AM. Metrics, tracing across queue boundaries, structured logs, and alerting want to be read-only systems whose write-set is a metrics table the storage system ships out beside the log.
- **Hard real-time** (operations). [§39](39_system_of_systems.md)'s anytime algorithms are soft real-time: a missed deadline costs quality. Hard real-time - where a missed deadline is a fault - needs WCET analysis, bounded jitter, and no allocation in the inner loop. A different discipline layered on top.
- **Heterogeneous compute** (performance). SoA is the precondition for SIMD, GPU offload, and accelerators; the book makes the precondition and stops at one core's bandwidth. For all-pairs shortest paths on a million-node graph, the next bus is the difference between thirty minutes and thirty seconds. Its cost model - transfer bandwidth and kernel-launch latency - deserves the same dollars-and-cents treatment [§4](04_cost_and_budget.md) gives the cache hierarchy.
- **Where SoA does not pay** (memory, maintainability). The simulator's domain - things with positions and a few scalars - is unusually friendly to columns. Recursive structures dominated by topology rather than slot order, very small N where pointer-chasing's constant factor wins, and APIs that must hand structured rows to non-ECS consumers are where columns can cost more than they save. SoA is a default, not a law.
- **Floating-point geometry** (correctness). Data layout is orthogonal to the hard part of geometric computation: degeneracies, robust predicates, exact-versus-interval arithmetic. A perfectly SoA Delaunay triangulation can still be wrong on collinear points. The book does not need to teach robust predicates; it needs to admit they exist for the readers building CAD, GIS, or path planning.
- **The social layer** (maintainability). Code review, ownership transfer, deprecation policy, runbooks. "Onboardable because the data is visible" is one bullet; the rest of the team-scale layer - the lone maintainer, the silent deprecation, the unwritten convention - is where every criterion above degrades fastest under turnover.

The first act is the harder problem, and the book finishes it. The second act - ship, evolve, observe, recover - begins now, in [§45](45_living_with_it.md).

## Where to go next

- **Read Mike Acton's "Data-Oriented Design and C++"** (CppCon 2014). Forty-five minutes; the most concentrated case for this approach you will find.
- **Read Casey Muratori's *Handmade Hero*** episodes on grid storage and cache locality. Another route to the same conclusions.
- **Open Bevy's `bevy_ecs` crate.** You will recognise every pattern. The names will differ; the shapes are identical.
- **Extend the simulator.** The genetics and predator-prey extensions flagged in the [simulator spec](../../code/sim/SPEC.md) break new ground without leaving the framework you have already built.

<p align="center"><img src="../illustrations/model_real_world.jpg" alt="Model the real world." style="max-height: 300px; max-width: 100%;"></p>

The book ends here. The simulator does not - it runs as long as you keep the discipline.
