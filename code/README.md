# Reference code

Working implementations and measurement binaries that back the §1-§10 chapters of the book. Built so that every numerical claim in the prose can be reproduced on the reader's own hardware.

## Layout

- `deck/` - the through-line program for §5-§10. SoA card deck with shuffle, sort, deal, query, single-writer reorder, and stable-id lookup. Tests cover the contracts.
- `measurement/` - eleven binaries, one per measurement-bearing exercise group.
- `sim/` - specification only (`SPEC.md`); the simulator chapters (§11+) live here when written.

## Running

Each Cargo project is independent.

```sh
# §5-§10 deck
cd deck && cargo run --release && cargo test --release

# §1-§4 and §7 measurements
cd measurement
cargo run --release --bin cache_cliffs       # §1.4, §4.4
cargo run --release --bin pointer_chase      # §1.5
cargo run --release --bin type_sizes         # §2.1, §2.2
cargo run --release --bin vec_u8_vs_u64      # §2.3
cargo run --release --bin float_weird        # §2.4, §2.5
cargo run --release --bin vec_capacity       # §3.1, §3.2, §3.3
cargo run --release --bin vec_vs_hashmap     # §3.4, §4.3
cargo run --release --bin swap_remove_perf   # §3.5
cargo run --release --bin soa_vs_aos         # §7.3-7.5
cargo run --release --bin motion_working_set # §26.4, §27 (motion loop ns/creature)
cargo run --release --bin false_sharing      # §33 (false sharing, negative scaling)
cargo run --release --bin scope_speedup      # §31.2 (thread::scope 2-system speedup)
cargo run --release --bin batched_write      # §38.3 (batched vs unbatched write)
cargo run --release --bin row_vs_column_serialize # §36.3 (per-row vs column snapshot)
cargo run --release --bin l1_sweet_spot      # §27.6 (L1 vs L2 streaming motion)
cargo run --release --bin power_loop -- sequential   # §4.9 (run perf in another terminal)
```

## Cross-machine results

The Pi 4 / i7 / i3 columns were captured 2026-05-04; the Ryzen column was captured 2026-06-01 on a Ryzen 9 270 (3-run medians). A full re-run of every binary on all four hosts reproduced the Pi/i7/i3 cells to within run-to-run noise. The one cell that did not survive the hardware change is u8-vs-u64 on the Ryzen (5.3× on the original Ryzen-class box, 1.8× on the Ryzen 9 270): how far `u8` summation outruns `u64` is a property of that chip's vector units and memory channel, not a portable constant. The chapter prose stays light on specific numbers because they age - the *order of magnitude* is the durable claim.

| Test | Pi 4 (Cortex-A72, 4 GB) | i7-3610QM (2012 laptop, 8 GB) | i3-5010U (2015 NUC, 8 GB) | Ryzen 9 270 |
|---|---:|---:|---:|---:|
| Vec sum, ns/elem at N=100M | 2.03 | 0.44 | 0.70 | 0.14 |
| Cache cliffs visible (§1.4) | 3 (clean staircase) | 3 (clean staircase) | 2-3 | 1 (L3→RAM only) |
| Pointer chase ratio (§1.5) | 63× | 120× | 103× | 270× |
| Vec vs HashMap, 1M (§3.4) | 65× | 89× | 77× | 160× |
| swap_remove vs remove (§3.5) | 201,546× | 95,091× | 83,603× | ~20,000× |
| u8 vs u64 sum, 100M (§2.3) | 4.6× | 2.0× | 2.5× | 1.8× |
| SoA vs padded AoS, 10M (§7.5) | **5.7×** | 2.4× | 1.9× | 1.6× |

Two findings worth keeping in mind:

1. **The Pi shows the textbook cache staircase**: three clean cliffs at L1→L2→L3→RAM. This is the *original* shape the chapter describes, before modern prefetchers learned to hide L1/L2/L3 transitions for streaming sums. A reader on modern hardware who sees only one cliff is not measuring wrong - their CPU is just hiding the others.
2. **The Pi shows the strongest SoA win** (5.7× when the row grows from 3 B to 20 B). With no L3 and a tight LPDDR4 channel, the Pi pays for every wasted byte the AoS row drags through cache. Modern desktops with generous L3 mute the gap to ~2×. The principle is the same; the slope of the cliff scales with the cache budget.

If you reproduce these on your own hardware, treat the chapter notes as ranges (e.g. "60-200×" for Vec-vs-HashMap), not absolutes.

### Motion loop and false sharing (captured 2026-06-01)

The `motion_working_set` and `false_sharing` binaries back §26, §27, and §33. Same four hosts.

`motion`'s SoA loop, **sequential** ns per creature per tick (reads pos+vel+energy, writes pos+energy):

| creatures | Pi 4 | i7-3610QM | i3-5010U | Ryzen 9 270 |
|---|---:|---:|---:|---:|
| 10,000 | 4.18 | 0.75 | 1.04 | 0.27 |
| 1,000,000 | 10.05 | 1.70 | 3.30 | 0.44 |
| 10,000,000 | 17.38 | 1.80 | 3.15 | 0.71 |

Sequential motion stays bandwidth-bound and cheap even at 10M; the cost explosion is in *random* order:

| Test | Pi 4 | i7-3610QM | i3-5010U | Ryzen 9 270 |
|---|---:|---:|---:|---:|
| Motion random ns/creature, 10M (§27.4) | 392 | 80 | 84 | 31 |
| Random / sequential, 10M (§27.4) | 23× | 45× | 27× | 43× |
| Hot/cold, AoS-40B / SoA-20B, 1M (§26.4) | 2.3× | 2.3× | 2.3× | 2.9× |

So §27's "ns/elem ladder" is a *random-access* ladder; sequential motion is an order of magnitude cheaper. The random/sequential gap is ~25-45×, not the 50-100× a single-pointer chase would show (motion amortises five columns per creature).

`false_sharing`: eight (or `nproc`) threads each incrementing one counter, vs the same counters padded to their own 64-byte line, vs one thread doing all the work:

| Test | Pi 4 (4t) | i7-3610QM (8t) | i3-5010U (4t) | Ryzen 9 270 (8t) |
|---|---:|---:|---:|---:|
| Padded speedup vs shared, [u64;N] (§33) | 13.6× | 8.3× | 6.3× | 21.1× |
| Shared [u64;N] parallel vs 1 thread (§33) | 0.27× | 0.43× | 0.30× | 0.37× |
| Partitioned reduction, packed vs 1 thread (§33) | 0.26× | 0.42× | 0.30× | 0.38× |

The bottom rows are below 1.0 on every machine: the false-shared "parallel" run is 2.3-3.8× *slower* than a single thread. The third row is the realistic case - a naive parallel reduction over disjoint input slices, folding into a packed per-thread accumulator array - and it scales just as negatively as the `[u64;N]` microbenchmark. Padding each accumulator to its own cache line recovers near-linear scaling (1.9-7.1×). This is the negative-scaling claim in §33, measured; the "partitioned everything correctly and it still got slower" opener is the 0.26-0.42× number.

The exercise-prediction binaries (§31, §36, §38, §27.6), same four hosts:

| Test | Pi 4 | i7-3610QM | i3-5010U | Ryzen 9 270 |
|---|---:|---:|---:|---:|
| Batched vs unbatched write (§38.3) | 14× | 256× | 30× | 38× |
| thread::scope 2-system speedup (§31.2) | 1.99× | 1.92× | 1.81× | 1.96× |
| Per-row JSON vs column snapshot (§36.3) | 33× | 55× | 64× | 31× |
| Per-row binary vs column snapshot (§36.3) | ~2× | ~2× | ~2× | ~1× |
| L1 vs L2, streaming motion (§27.6) | 1.08× | 1.19× | 1.20× | 1.02× |

Two of these corrected the prose: §38's "50-1000×" became the measured 14-256× (buffered writes), and §27.6's "L1-resident ~5-10× faster" became ~1.0-1.2× - sequential motion is bandwidth-bound at both sizes, so the L1/L2 boundary is invisible to it (the L1 win is a *random*-access effect). §36's "5-50×" splits by format: the text encoder lands at ~30-65×, the binary encoder at ~1-2×.

## A note on benchmark anti-patterns

Several binaries had to use `std::hint::black_box` to defeat the optimizer. The compiler will happily hoist a pure `sum_seq(&v)` out of an inner loop if the result feeds a deterministic accumulator - making the loop O(1) regardless of `iters` and the timing meaningless. The pattern is:

```rust
for _ in 0..iters {
    s = s.wrapping_add(sum_seq(std::hint::black_box(&v[..])));
}
std::hint::black_box(s);
```

Both ends matter: black-boxing the input prevents hoisting the call; black-boxing the output prevents dead-code elimination of the whole expression.

The same logic applies to `swap_remove_perf` (where the index is black-boxed so the compiler cannot constant-fold the loop) and `pointer_chase` (where the linked list is *shuffled* before traversal - without the shuffle, the boxes sit at sequential heap addresses and the cache cost is invisible).
