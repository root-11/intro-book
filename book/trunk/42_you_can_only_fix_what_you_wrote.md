# 42 - You can only fix what you wrote

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 42](../../concepts/glossary.md#42--you-can-only-fix-what-you-wrote).*

<p align="center"><img src="../illustrations/cad_bearing.jpg" alt="The bearing you drew is the bearing you fix" style="max-height: 300px; max-width: 100%;"></p>

Foreign libraries are allowed in this book. They are not banned. They are *priced*.

Every dependency is a bet. The bet is that someone else will keep the library working - fix bugs, ship versions, respond to security issues, support future Rust releases, not abandon the project. The bet has a cost: if the library breaks, you cannot fix it. You can only replace it, fork it, or live with the breakage.

The discipline is to take the bet *consciously*, knowing how much code the dependency saves you and how much risk it carries.

Walk through what risk looks like.

**The leftpad incident.** An eleven-line npm package was unpublished by its author over a naming dispute, and broke thousands of build pipelines worldwide. The package did padding by repeated string concatenation. Every project that depended on it was, structurally, depending on someone else's emotional state.

**Major-version cascade.** A transitive dependency makes a breaking change. Your code does not change. The dependency's dependency does. The build is now broken, sometimes for days, while you wait for an upstream fix or pin a workaround. You have lost agency over your own build.

**The slow fade.** A crate works in production for two years, then its author switches careers, the crate stops getting updates, and a future Rust release deprecates a feature it relies on. The crate still compiles for now, but its days are numbered. Migration is on you.

These are not edge cases. They are the *typical* lifecycle of a dependency relationship. Some libraries beat the curve - `serde`, `tokio`, `rayon` - because they are maintained by ecosystems too large to fail. Most do not.

The discipline that follows from this is not "use no dependencies". It is:

1. **Write the from-scratch version first.** If it is fifty lines and two hours, often you do not need the dependency at all. The from-scratch version is also the calibration: how much code does the crate actually save?
2. **Read the dependency's source.** Not the docs - the source. How much code is it? Who maintains it? What's its history? Is it actively maintained or coasting?
3. **Decide consciously.** Adopt for the right reasons (genuine code savings, ecosystem alignment, escape from your own bug-prone reimplementation). Reject for the wrong reasons (it is there, it is popular, no one questioned it).

A useful classification by size:

- **Trivial** (a few hundred lines or less). Easy to fork, easy to inline. Often easier to write yourself than to take the dependency.
- **Small** (around a thousand lines). Forkable in a day or two. Reasonable to depend on; reasonable to vendor.
- **Mid-size** (a few thousand lines, e.g. `slotmap`). Forkable but a real commitment. Adopt cautiously; have a migration plan.
- **Ecosystem-scale** (many thousands of lines, large team - `tokio`, `serde`). Not realistically forkable. Adoption is a commitment to the ecosystem; pretending otherwise is the bug.

The book's through-line example: `slotmap`. It implements the generational arena pattern from [§10](10_stable_ids_and_generations.md) plus [§23](23_index_maps.md). Most simulators benefit from it because the from-scratch version is non-trivial. But the from-scratch version is *also* small enough - a few hundred lines for the core operations - that you could fork and own it if needed. That balance - small enough to fix, complex enough to want - is the sweet spot.

The opposite end is `tokio`. Adoption is a commitment to the maintainer team. For most projects this is fine - the team is competent and the ecosystem is durable. But the commitment is real.

The middle ground is uncomfortable. A 2 000-line single-author crate that is exactly what you need: too big to fork comfortably, too small for ecosystem support. Adopt cautiously; consider vendoring (copying into your repo); be ready to maintain.

The book's discipline lives at this evaluation. Not "no deps" - "consciously chosen deps, sized to the maintenance you can do".

## Exercises

1. **Audit your `Cargo.toml`.** For each direct dependency, classify by the size categories above. The small ones are easiest to fork; the ecosystem-scale ones are too big to fork.
2. **The from-scratch test.** Pick one mid-size or small dependency. Estimate: how long would it take to write the relevant 80 % of it from scratch? If less than two days, you have an alternative - keep it in mind for the day the dependency breaks.
3. **A breakage drill.** Pick one dependency. Pretend it is unmaintained. What is your migration path? (Fork? Replace? Live with the bug?) Write the answer in your project's README or `CONTRIBUTING.md`. The drill is cheap; the breakage is not.
4. **Small over big.** When two crates do the same job, prefer the smaller. A small crate is forkable; a large one usually is not. The bigger crate's extra features are someone else's needs, not yours.
5. *(stretch)* **Vendoring.** Copy one small crate's source into `vendor/foo` in your repo. Update `Cargo.toml` to use `path = "vendor/foo"`. The crate is now under your control. Future breakages are yours to fix; future improvements are yours to apply. The trade is more work for more agency. Document the decision so future maintainers know why.

Reference notes in [42_you_can_only_fix_what_you_wrote_solutions.md](42_you_can_only_fix_what_you_wrote_solutions.md).

## What's next

[§43 - Tests are systems; TDD from day one](43_tests_are_systems.md) is the closing discipline: tests are not a separate framework, they are systems. The same shape that runs the simulator runs its tests.
