# Solutions: 17 - Presence replaces flags

## Exercise 2 - `classify_hunger`

```rust
const HUNGER_THRESHOLD: f32 = 5.0;

fn classify_hunger(energy: &[f32], hungry: &mut Vec<u32>) {
    for i in 0..energy.len() {
        let slot = i as u32;
        let is_now = energy[i] < HUNGER_THRESHOLD;
        let was_in = hungry.iter().any(|&s| s == slot);
        if is_now && !was_in {
            hungry.push(slot);
        } else if !is_now && was_in {
            if let Some(pos) = hungry.iter().position(|&s| s == slot) {
                hungry.swap_remove(pos);
            }
        }
    }
}
```

The loop index `i` is the slot, so `hungry` holds slots. The linear `iter().any` is O(N). At 100 000 hungry creatures × 1 000 000 creatures per tick, this is 10¹¹ comparisons - far too slow. [§23 - Index maps](23_index_maps.md) replaces it with an O(1) lookup, a sparse set rather than a per-creature boolean. For now the linear version makes the conceptual point.

## Exercise 4 - Bytes touched

The presence write touches at most 100 000 × 4 bytes ≈ 400 KB (each new slot is a `u32`). The flag write touches all 1 000 000 × 1 byte = 1 MB.

The presence *read* (the membership check) is more expensive without an index - every classification check costs O(hungry.len()), and the worst case is total O(N²). The flag read is O(1). [§23](23_index_maps.md) is the fix; until then, the chapter argues for presence on storage and persistence grounds, not on read-cost grounds.

## Exercise 5 - Membership queries

```rust
fn is_hungry_p(hungry: &[u32], slot: u32) -> bool {
    hungry.iter().any(|&s| s == slot)
}

fn is_hungry_f(is_hungry: &[bool], slot: usize) -> bool {
    is_hungry[slot]
}
```

`is_hungry_p` is O(N); `is_hungry_f` is O(1). The chapter ends here intentionally - until the index map arrives in §23, the presence cost is *real* for queries. The shape of the data, however, is what we are committing to. The cost gets fixed by adding more structure - a sparse set (§23) that restores O(1) without a per-creature flag - not by reverting to flags.

## Exercise 6 - Counting

```rust
let n_hungry_p = hungry.len();         // O(1)
let n_hungry_f = is_hungry.iter()      // O(N)
    .filter(|&&b| b).count();
```

Counting is the easiest case for presence: the table's length is the answer. The flag version walks the full vector even though the answer is the same.

## Exercise 7 - Persistence

A naive bincode-style serialisation:

- Presence: `hungry.len() * 4 + 8` bytes (the `Vec<u32>` plus a length prefix). At 10 % hungry: ~400 KB.
- Flag: `is_hungry.len()` bytes. At 1M creatures: ~1 MB.

Both can be compressed; both can be written incrementally. The key observation is that the presence representation *reflects what is true*: the file is small when the world is mostly at rest. The flag representation is the same size whether or not anything is hungry.
