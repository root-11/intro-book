# Reference code

Working implementations and measurement binaries that back the §1–§10 chapters of the book. Built so that every numerical claim in the prose can be reproduced on the reader's own hardware.

## Layout

- `deck/` — the through-line program for §5–§10. SoA card deck with shuffle, sort, deal, query, single-writer reorder, and stable-id lookup. Tests cover the contracts.
- `measurement/` — eleven binaries, one per measurement-bearing exercise group.
- `sim/` — specification only (`SPEC.md`); the simulator chapters (§11+) live here when written.

## Running

Each Cargo project is independent.

```sh
# §5–§10 deck
cd deck && cargo run --release && cargo test --release

# §1–§4 and §7 measurements
cd measurement
cargo run --release --bin cache_cliffs       # §1.4, §4.4
cargo run --release --bin pointer_chase      # §1.5
cargo run --release --bin type_sizes         # §2.1, §2.2
cargo run --release --bin vec_u8_vs_u64      # §2.3
cargo run --release --bin float_weird        # §2.4, §2.5
cargo run --release --bin vec_capacity       # §3.1, §3.2, §3.3
cargo run --release --bin vec_vs_hashmap     # §3.4, §4.3
cargo run --release --bin swap_remove_perf   # §3.5
cargo run --release --bin soa_vs_aos         # §7.3–7.5
cargo run --release --bin power_loop -- sequential   # §4.9 (run perf in another terminal)
```

## A note on benchmark anti-patterns

Several binaries had to use `std::hint::black_box` to defeat the optimizer. The compiler will happily hoist a pure `sum_seq(&v)` out of an inner loop if the result feeds a deterministic accumulator — making the loop O(1) regardless of `iters` and the timing meaningless. The pattern is:

```rust
for _ in 0..iters {
    s = s.wrapping_add(sum_seq(std::hint::black_box(&v[..])));
}
std::hint::black_box(s);
```

Both ends matter: black-boxing the input prevents hoisting the call; black-boxing the output prevents dead-code elimination of the whole expression.

The same logic applies to `swap_remove_perf` (where the index is black-boxed so the compiler cannot constant-fold the loop) and `pointer_chase` (where the linked list is *shuffled* before traversal — without the shuffle, the boxes sit at sequential heap addresses and the cache cost is invisible).
