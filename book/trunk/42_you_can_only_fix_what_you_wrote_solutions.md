# Solutions: 42 - You can only fix what you wrote

## Exercise 1 - Cargo.toml audit

For a typical Rust project's direct dependencies, the classification might look like:

| dependency       | size category    | notes                              |
|------------------|------------------|------------------------------------|
| `serde`          | ecosystem-scale  | not realistically forkable         |
| `tokio`          | ecosystem-scale  | not realistically forkable         |
| `rayon`          | mid-size         | could fork; sizable but coherent   |
| `slotmap`        | mid-size         | could fork; core is a few hundred lines |
| `crossbeam-utils`| small            | forkable in a day                  |
| `small_helper`   | trivial          | could be inlined                   |

The classification reveals which dependencies are bets you have already made (and cannot easily walk back) versus which are bets you can still walk away from.

## Exercise 2 - From-scratch test

For `slotmap`: the core (slot allocator, generation counter, get/insert/remove) would be a few hundred lines of Rust. Plausibly a day or two of focused work. If `slotmap` ever became unmaintained, this is your migration path.

For `tokio`: not realistically replaceable. Adoption is a commitment to the ecosystem.

The exercise's value is calibration: how much code does each dependency actually save?

## Exercise 3 - Breakage drill

Possible answers for various scenarios:

- **Trivial dependency unmaintained**: inline its source, remove from `Cargo.toml`. ~1 hour.
- **Small dependency unmaintained**: fork it; it's small enough to vendor and own. ~1 day to integrate.
- **Mid-size dependency unmaintained**: evaluate alternatives; fork as fallback; budget ~1 week to migrate.
- **Ecosystem dependency breaks**: wait for ecosystem fix; pin the working version; budget weeks to a month for community to respond.

Documenting these answers up front is cheap. Discovering them under pressure is expensive.

## Exercise 4 - Small over big

Two crates that do the same job: `easy_thing` (5 000 lines, 50 features) and `simple_thing` (500 lines, 5 features). If you only use the 5 features, prefer `simple_thing`. The smaller crate is easier to read, easier to fork, easier to vendor, easier to debug.

The bigger crate's extra features are someone else's needs, not yours. They are dependency mass without value.

## Exercise 5 - Vendoring

```toml
[dependencies]
foo = { path = "vendor/foo" }
```

After copying the crate's source into `vendor/foo/` and updating `Cargo.toml`, the crate is now part of your repo. You can edit it, fix bugs, simplify it, drop unused features. The trade: you have taken on maintenance. Your crate builds always work; it is also your problem when something breaks.

For small crates with stable APIs, vendoring is often the right move. For ecosystem crates, it is not. Document the decision in the project's README so future maintainers know which crates are vendored and why.
