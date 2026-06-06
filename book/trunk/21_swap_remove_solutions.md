# Solutions: 21 - `swap_remove`

## Exercise 1 - Compare timings

```rust,no_run
use std::time::Instant;

let mut v: Vec<u64> = (0..1_000_000).collect();
let t = Instant::now();
for _ in 0..1000 { v.remove(0); }
println!("remove(0): {:?}", t.elapsed());

let mut v: Vec<u64> = (0..1_000_000).collect();
let t = Instant::now();
for _ in 0..1000 { v.swap_remove(0); }
println!("swap_remove(0): {:?}", t.elapsed());
```

Typical: `remove(0)` takes around 500 ms; `swap_remove(0)` takes around 5 µs. The ratio is roughly `N / 1`. swap_remove is essentially free; `remove` is essentially the cost of the table.

## Exercise 2 - Mid-table delete

`remove(500_000)` shifts ~500 000 elements left by one - half the work of `remove(0)`. `swap_remove(500_000)` is unchanged: one read, one write, one decrement. The asymmetry is the whole point.

## Exercise 3 - The iteration hazard

```rust,ignore
// Form A: the range fixes its bound at the original length.
for i in 0..v.len() {
    if v[i] % 2 == 0 { v.swap_remove(i); }
}

// Form B: the length is re-read every pass.
let mut i = 0;
while i < v.len() {
    if v[i] % 2 == 0 { v.swap_remove(i); }
    i += 1;
}
```

Both are wrong, and they fail *differently*. `swap_remove(i)` moves the last element into the hole at `i`, so the element now sitting at `i` is never inspected - and the vector got shorter.

Form A builds the range `0..v.len()` once, capturing the original length. As elements are removed `v` shrinks, but `i` keeps climbing toward the original length, so `v[i]` eventually indexes past the end and **panics**.

Form B re-reads `v.len()` each pass, so it never indexes out of bounds - but it advances `i` past the element that was just swapped into the hole, skipping it. No panic; a silently wrong `Vec` that is *some mix*, not "all odd values."

One mistake, two failure modes: a crash or a quiet lie. Exercise 4 shows the discipline that avoids both.

## Exercise 4 - Iterate backwards

```rust
let mut v: Vec<u64> = (0..100).collect();
for i in (0..v.len()).rev() {
    if v[i] % 2 == 0 { v.swap_remove(i); }
}
```

This works because `swap_remove(i)` moves an element from index `len - 1` (which we have *already* visited) into index `i`. Future iterations only visit smaller indices, which are unaffected. The reverse-iteration trick is correct, but fragile: future maintainers may forget the invariant.

## Exercise 5 - Deferred cleanup

```rust
let mut v: Vec<u64> = (0..100).collect();
let mut to_remove: Vec<usize> = Vec::new();
for i in 0..v.len() {
    if v[i] % 2 == 0 { to_remove.push(i); }
}
for &i in to_remove.iter().rev() {
    v.swap_remove(i);
}
```

The collection and the mutation are separated. Iteration over `v` runs to completion *before* the first swap_remove. The reverse-order drain ensures the indices remain valid as the table shrinks. This is the §22 pattern in miniature.

## Exercise 6 - Aligned swap_remove

```rust,no_run
fn delete_creature(world: &mut World, slot: usize) {
    let c = &mut world.creatures;
    c.px.swap_remove(slot);
    c.py.swap_remove(slot);
    c.vx.swap_remove(slot);
    c.vy.swap_remove(slot);
    c.energy.swap_remove(slot);
    c.id.swap_remove(slot);
    c.generation.swap_remove(slot);
    c.birth_t.swap_remove(slot);
}
```

All eight columns swap_remove the same slot. The row that was at the end is now at `slot`, with all eight fields aligned. The row that was at `slot` is gone. The `id_to_slot` map (§23) gets the same treatment.

## Exercise 7 - The bandwidth cost

`vec.remove(0)` on a 1 GB `Vec` moves ~1 GB through L3+RAM. `vec.swap_remove(0)` moves ~16 bytes (one row's worth). The ratio is `N / 1`. At 30 Hz, naive `remove` on a 1 GB table is impossible (~10 s per call); swap_remove is comfortably under a microsecond.
