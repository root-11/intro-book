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

## Cross-machine results

Captured 2026-05-04 across four hardware profiles. The chapter prose stays light on specific numbers because they age — but the *order of magnitude* is robust, as the table shows.

| Test | Pi 4 (Cortex-A72, 4 GB) | i7-3610QM (2012 laptop, 8 GB) | i3-5010U (2015 NUC, 8 GB) | Modern Ryzen-class |
|---|---:|---:|---:|---:|
| Vec sum, ns/elem at N=100M | 2.03 | 0.44 | 0.70 | 0.20 |
| Cache cliffs visible (§1.4) | 3 (clean staircase) | 3 (clean staircase) | 2-3 | 1 (L3→RAM only) |
| Pointer chase ratio (§1.5) | 63× | 120× | 103× | 296× |
| Vec vs HashMap, 1M (§3.4) | 65× | 89× | 77× | 175× |
| swap_remove vs remove (§3.5) | 201,546× | 95,091× | 83,603× | 18,624× |
| u8 vs u64 sum, 100M (§2.3) | 4.6× | 2.0× | 2.5× | 5.3× |
| SoA vs padded AoS, 10M (§7.5) | **5.7×** | 2.4× | 1.9× | 2.0× |

Two findings worth keeping in mind:

1. **The Pi shows the textbook cache staircase**: three clean cliffs at L1→L2→L3→RAM. This is the *original* shape the chapter describes, before modern prefetchers learned to hide L1/L2/L3 transitions for streaming sums. A reader on modern hardware who sees only one cliff is not measuring wrong — their CPU is just hiding the others.
2. **The Pi shows the strongest SoA win** (5.7× when the row grows from 3 B to 20 B). With no L3 and a tight LPDDR4 channel, the Pi pays for every wasted byte the AoS row drags through cache. Modern desktops with generous L3 mute the gap to ~2×. The principle is the same; the slope of the cliff scales with the cache budget.

If you reproduce these on your own hardware, treat the chapter notes as ranges (e.g. "60-200×" for Vec-vs-HashMap), not absolutes.

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
