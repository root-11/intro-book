# Reference code

Working implementations and measurement binaries that back the §1-§10 chapters of the book. Built so that every numerical claim in the prose can be reproduced on the reader's own hardware.

## Layout

- `deck/` - the through-line program for §5-§10. SoA card deck with shuffle, sort, deal, query, single-writer reorder, and stable-id lookup. Tests cover the contracts.
- `measurement/` - eleven binaries, one per measurement-bearing exercise group.
- `logger/` - the dependency-free Rust logger specimen for §37. Triple-store COO + evolving string codebook + `f64` type inference + a double-buffered background-writer revolver; raw little-endian column-byte chunks (no `.npz`, no `serde`). Tests cover the contracts; a `benchmark` bin reproduces the throughput numbers.
- `exprtree/` - Part II project A ("where SoA does not pay"). One arithmetic expression in three representations - pointer tree, flat arena, linearized stack machine - measured across a traversal-dominated and an edit-dominated workload. Tests assert all three agree bit-for-bit; `cargo run` prints the size sweep, the edit costs, and the derived crossover. Its own `README.md` carries the reference run.
- `scenegraph/` - Part II project B ("where SoA does not pay"). A transform hierarchy laid out flat in pre-order (subtrees are contiguous ranges). Measures full-repropagate flat-vs-pointer layout, the incremental-vs-full crossover over dirty fraction, and how locality of the dirty set changes the answer. Tests assert flat/pointer/incremental agree bit-for-bit. Reference run in its `README.md`.
- `spreadsheet/` - Part II project C ("where SoA does not pay"). A recalc engine: per-cell formula trees (project A) over a dependency DAG (generalizing B's dirty propagation), recomputed in topological order. Measures the dirty-cone crossover under realistic fill-down edits, early-cutoff pruning at a high-fan-out hub, and a pivot patch that re-sums only the dirty columns. Tests assert cone and cutoff match a full recompute. A second binary, `scale`, takes it to a billion cells: the per-cell `Box`-`Expr` cannot fit (~160 GB), so the program goes SoA (a template per column, an implicit dependency rule), and a disk-resident pivot bigger than RAM shows the patch reading only the dirty columns. Reference run in its `README.md`.
- `fpfragility/` - Part II project D ("where SoA does not pay"). Floating-point fragility: summation is order-dependent and a naive sum loses everything an ill-conditioned column holds; a delta-maintained aggregate drifts from a recompute; a naive geometric predicate is wrong on ~99% of near-collinear points while exact integer arithmetic is right at the same cost. The point: layout cannot rescue correctness. Tests cover non-associativity, Kahan accuracy, and exact orientation. Reference run in its `README.md`.
- `heterogeneous/` - Part II project E ("where SoA does not pay"), the arc finale. How far one box reaches: a SoA motion pass across the cache hierarchy, multi-core scaling (which plateaus at the memory-bandwidth ceiling, not the core count), the active-set-per-frame budget that makes "you need a GPU" false by irrelevance, and a labelled GPU break-even cost model (no GPU on the box; real run pending). Tests assert the parallel pass matches the serial one bit-for-bit. Reference run in its `README.md`.
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
cargo run --release --bin ebp_partition      # §24, §26 (subscription vs scan, slot vs id keying, locality, lifecycle)
cargo run --release --bin proximity          # §28 (all-pairs vs bolt-on hash vs dense binning; pack-leader cohesion)
cargo run --release --bin power_loop -- sequential   # §4.9 (run perf in another terminal)

# §37 logger
cd logger && cargo test && cargo run --release --bin benchmark   # log() ns/call at 5 and 11 fields

# Part II project A - where SoA does not pay
cd exprtree && cargo test --release && cargo run --release   # 3 representations; traversal/edit crossover

# Part II project B - where SoA does not pay
cd scenegraph && cargo test --release && cargo run --release # flat vs pointer; dirty-fraction crossover

# Part II project C - where SoA does not pay
cd spreadsheet && cargo test --release && cargo run --release # dependency DAG; cone crossover, cutoff, pivot patch
cd spreadsheet && cargo run --release --bin scale            # the same, at a billion cells (4 GB)
cd spreadsheet && cargo run --release --bin scale -- 9000000000 # a 36 GB disk pivot, past 32 GB of RAM

# Part II project D - where SoA does not pay
cd fpfragility && cargo test --release && cargo run --release # summation order, drift, geometric predicates

# Part II project E - where SoA does not pay
cd heterogeneous && cargo test --release && cargo run --release # one-box reach, scaling ceiling, GPU model

# §56 discrete-GPU probe (separate crate; pulls wgpu via its Cargo.lock)
cd gpu_probe && cargo run --release            # CPU vs GPU round-trip vs GPU resident, on a real GPU
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

### Subscription keying and proximity (captured 2026-06-06)

`ebp_partition` backs §26 (and §19/§24); `proximity` backs §28. Same four hosts.

`ebp_partition`, 1M creatures, 10% subscribed unless noted:

| Test | Pi 4 | i7-3610QM | i3-5010U | Ryzen 9 270 |
|---|---:|---:|---:|---:|
| Keying: id-keyed ÷ slot-keyed hot loop (§26) | 1.4× | 3.2× | 1.3× | 2.2× |
| Relevance: scan+branch ÷ subscription, **1%** active (§19) | 2.0× | 10× | 4.3× | 14× |
| Relevance: same, **10%** active | 1.0× | 1.1× | 1.0× | 1.6× |
| Locality: scattered ÷ compacted gather (§26.5/§28) | 9.0× | 6.8× | 9.0× | 4.4× |
| Compaction payback (ticks) | 0.7 | 1.5 | 1.1 | 3.0 |
| Lifecycle: batch compaction vs per-element swap_remove (§24) | 8.0× | 8.5× | 8.0× | 5.1× |

The amortized keying verdict (slot vs id, over the GC interval) favours **slot keys on every host, at every subscription count and interval tested** - that is the durable result; the hot-loop ratio above is only its per-tick component. The relevance rows carry the important nuance: a *scattered* subscription at 10% is barely faster than scan-all in wall time on any machine (the 10× is a reduction in *work and bandwidth*, not yet in time); the wall-time win arrives at high sparsity (the 1% row) or once the subscription is compacted (the locality row). On the small-cache Pi the scattered gather is most punishing and the compaction win is largest.

`proximity` (§28), N as noted:

| Test | Pi 4 | i7-3610QM | i3-5010U | Ryzen 9 270 |
|---|---:|---:|---:|---:|
| All-pairs neighbour test, N=20k (ms) | 1720 | 864 | 960 | 270 |
| Dense bin vs bolt-on hash, 1M (end to end) | 1.5× | 1.7× | 1.6× | 2.8× |
| Dense bin rebuild ÷ its query, 1M | 0.8% | 1.0% | 1.4% | 0.7% |
| Pack-leader vs all-pairs cohesion, N=20k | 9439× | 7296× | 6712× | 9087× |

Every direction holds on every machine: all-pairs is hopeless, the dense bin beats the bolt-on hash, the rebuild is ~1% of the query (so recompute beats maintain), and one leader read by all crushes all-pairs cohesion by thousands of times.

### Knowing the limits arc (§52-§56, captured 2026-06-21)

Static `musl` binaries (`x86_64` and `aarch64`, cross-linked with the bundled `rust-lld`), 5-run medians, 3-run on the Pi. These are a fresh same-round capture and differ in places from the chapter figures (e.g. §53's flat-sweep speedup), which is the "treat the shape as the claim, not the digits" note made concrete: every claim's *shape* survives all four machines, the magnitudes drift with build, cache, and run.

The Pi 4 has no heatsink. Under sustained load it reaches its soft thermal limit (~84 C) and frequency-caps, so its §56 cells are a throttled floor: one Cortex-A72 core already saturates the ~3 GB/s LPDDR4 channel, so "more cores do not help" holds regardless - the throttle only lowers the absolute GB/s, not the conclusion.

Two cells depend on memory bandwidth rather than being portable constants, and both are now phrased that way in the prose (§53, §56): the recompute-dirty **crossover** (the dev box is the fast-memory, conservative end; slower memory makes recompute-dirty win further) and the **GPU cost model** (it is bus-versus-memory: the assumed 16 GB/s bus beats the i3 and Pi's slower RAM, so the simple model tips there - the durable rule is that offload pays only when data is device-resident or arithmetic-heavy).

The discrete-GPU measurement is the one cell these machines cannot fill: all four reference machines have only integrated GPUs that share the memory channel. `code/gpu_probe` runs the motion pass on any Vulkan/Metal/DX12 card (no CUDA toolkit) and prints CPU vs GPU round-trip vs GPU resident; contributed numbers from a discrete GPU are welcome.

| Test | Pi 4 (Cortex-A72, 4 GB) | i7-3610QM (2012, 8 GB) | i3-5010U (2015, 8 GB) | Ryzen 9 270 |
|---|---:|---:|---:|---:|
| §52 flat vs pointer tree, 2M nodes | 1.76x | 2.13x | 1.84x | 2.64x |
| §52 edit/eval break-even | ~1:2 | ~1:3 | ~1:2 | ~1:4 |
| §53 flat sweep vs pointer walk, 1M | 2.41x | 2.61x | 2.14x | 6.20x |
| §53 recompute-dirty crossover (dirty fraction) | ~90% | ~90% | ~90% | ~50% |
| §53 packed vs scattered dirty set | 5.0x | 5.9x | 4.6x | 10.3x |
| §54 early cutoff: cells recomputed | 16 | 16 | 16 | 16 |
| §54 early cutoff: wall-clock speedup | 18x | 31x | 24x | 39x |
| §54 pivot patch, 1 dirty column | 106x | 93x | 102x | 97x |
| §55 orientation wrong in `f64` | 99.3% | 99.3% | 99.3% | 99.3% |
| §55 exact `i128` predicate cost vs `f64` | ~equal | ~equal | ~equal | ~equal |
| §56 one core, RAM-resident | ~3.1 GB/s\* | 17.5 GB/s | 10.1 GB/s | 27.7 GB/s |
| §56 multi-core plateau (max speedup) | ~1.0x\* | 1.15x | 1.07x | 1.83x |
| §56 GPU cost model (offload vs CPU pass) | tips (bus > RAM) | loses | tips (bus > RAM) | loses |

\* Pi 4 §56 is thermally bounded (no cooling; throttled at ~84 C under the sustained motion pass). The shape ("more cores do not help") holds; the GB/s is a throttled floor.

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
