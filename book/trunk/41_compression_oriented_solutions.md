# Solutions: 41 - Deferred abstraction

## Exercise 1 - Too-early abstraction

Look for traits with one impl, generic functions with one caller, or `enum`-shaped types with single-variant patterns. Each is a candidate for inlining. The test: what would the code look like if you removed the abstraction? Often it is clearer.

## Exercise 2 - Three concrete versions

```rust,no_run
fn filter_by_hunger(creatures: &[Creature], threshold: f32) -> Vec<u32> {
    let mut result = Vec::new();
    for c in creatures {
        if c.energy < threshold { result.push(c.id); }
    }
    result
}

fn filter_by_age(creatures: &[Creature], max: u32) -> Vec<u32> {
    let mut result = Vec::new();
    for c in creatures {
        if c.age > max { result.push(c.id); }
    }
    result
}

fn filter_by_location(creatures: &[Creature], region: Region) -> Vec<u32> {
    let mut result = Vec::new();
    for c in creatures {
        if region.contains(c.pos) { result.push(c.id); }
    }
    result
}
```

Three independent functions. The obvious shared abstraction is `filter_by(predicate: impl Fn(&Creature) -> bool)`. But - read them again. Each is four lines. The abstraction would be one line at each call site. Is the saving worth the indirection?

## Exercise 3 - Resist extraction

For four-line functions, the concrete versions are often more legible. The extracted `filter_by(creatures, |c| c.energy < HUNGER)` is the same length but adds a closure. The reader has to parse the closure to know what is being filtered.

The abstraction earns its place when:

- The caller would write the same closure many times.
- The closure is non-trivial.
- There are five or more concrete cases of the same shape.

For three small cases, leave them concrete.

## Exercise 4 - A fourth case

`filter_creatures_by_proximity_to_food` takes both `creatures` AND `food`. The signature `fn filter_by(creatures: &[Creature], pred: F)` cannot express it without smuggling `food` through a closure capture. Two options:

1. Pass `food` through a closure: `filter_by(creatures, |c| food.iter().any(|f| close(c.pos, f.pos)))`. Works, but ugly.
2. Recognise this as a different shape (a *join*, not a filter) and write it as its own concrete function.

The fourth case shows the abstraction's limits. A real shared structure would handle it without a special branch.

## Exercise 5 - Library audit

Open-ended. Look at any crate's exported API. For each function or trait, ask: how many concrete cases preceded this in the ecosystem? Often you will find:

- The 5+ cases that justify it (`std::iter::Iterator`, `serde::Serialize`).
- The single speculative case (a one-off DSL with no second user).
- The middle: 2-3 cases, possibly real, possibly the author's domain.

The clarity of the answer says how robust the abstraction is.
