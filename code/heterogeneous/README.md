# heterogeneous - "where SoA does not pay", project E

The finale, and the answer to "you need a GPU for a simulator this size." SoA is the
precondition for SIMD, multiple cores, and accelerators - the book's point - and the Intro
stops at one core's bandwidth. This crate measures how far one box actually reaches,
because that is what decides whether you reach off it at all.

The reviewer's framing - a 10M-1B-node simulator *needs* the GPU - is **false by
irrelevance** under the incremental discipline projects B and C built. You never recompute
a billion nodes; you keep the *active cone* current within the frame. So the question is
not "how fast is the GPU" but "how big an active set can one box keep current in a frame,"
and the GPU earns its place only when that active set, by itself, outgrows the box.

```sh
cargo test --release   # the parallel pass must match the serial pass bit for bit
cargo run  --release   # one core's reach, the scaling ceiling, the budget, the GPU model
```

## The honest scope

There is no GPU on this box. Measurements 1-3 are real CPU numbers; measurement 4 is a
cost *model* with **assumed** GPU constants and a real run pending a GPU host - the same
"cross-hardware pending" stance the Pi/i7/i3 columns take elsewhere. The book does not
fabricate a benchmark it cannot run.

## Reference run (single host, cross-machine pending)

Captured on a **Ryzen 9 270** (16 threads, dual-channel, 32 GB), rustc 1.94.0, `--release`,
median of 5.

### 1. One core's reach: SoA motion pass across the cache hierarchy

| elements | footprint | ns/elem | GB/s |
|---:|---:|---:|---:|
| 4,096 | 0.1 MB | 0.127 | 188.6 |
| 65,536 | 1.0 MB | 0.151 | 158.6 |
| 1,048,576 | 16.8 MB | 0.282 | 85.1 |
| 4,194,304 | 67.1 MB | 0.457 | 52.5 |
| 16,777,216 | 268.4 MB | 1.027 | 23.4 |

The loop autovectorizes (the SoA layout is what allows it), so SIMD is already in play. The
GB/s staircase is the cache hierarchy: ~190 GB/s while it lives in L1/L2, down to ~23 GB/s
once it spills to RAM. One core's RAM-resident ceiling is ~23 GB/s.

### 2. Scaling to the box ceiling (16,777,216 elements, RAM-resident)

| threads | GB/s | speedup |
|---:|---:|---:|
| 1 | 23.2 | 1.00x |
| 2 | 28.3 | 1.22x |
| 4 | 46.1 | 1.99x |
| 8 | 50.7 | 2.19x |
| 16 | 50.6 | 2.18x |

The headline: **16 cores do no better than 8, and the whole machine tops out around 2.2x.**
A bandwidth-bound pass saturates the memory channel at ~4 threads; the remaining cores wait
on RAM. The real ceiling of "one big box" is its memory bandwidth, not its core count - so
the way to go faster is to *touch less data*, which is exactly what projects B and C do. You
cannot out-core, or out-GPU, the memory channel on a pass like this.

### 3. The active-set budget (33 ms frame at 30 Hz)

- one core keeps **32.2 M** elements current per frame;
- all 16 cores keep **70.3 M** per frame.

This is the reframe, measured. A billion-node graph does not fit a frame on any single box -
but you never recompute it. The active cone projects B and C maintain is a few million cells,
which fits one core's frame budget with room to spare. The GPU is irrelevant to *staleness*:
it answers a question - recompute everything, fast - that an incremental design stopped asking.

### 4. GPU break-even (cost model; GPU constants ASSUMED, not measured)

Assumed: PCIe ~16 GB/s one way, kernel launch ~5 us. Real run pending a GPU host.

To offload one pass of CPU-resident data you ship 16 B/elem to the device and read 8 B/elem
back - 24 B/elem round trip, the same traffic the compute itself needs:

- round-trip transfer: **1.500 ns/elem**
- the CPU all-core pass: **0.474 ns/elem**

Just shipping the data across the bus and back already costs more than doing the pass on the
box. For a bandwidth-bound kernel, offload wins only when the data already lives on the
device, or the arithmetic intensity is high enough that compute (not transfer) dominates.
Otherwise the bus is the bottleneck and the GPU loses. Plug your own hardware's numbers in;
the structure is what matters.

## The arc's close

Columns are the precondition for SIMD, cores, and accelerators - but they are a default, not
a law, and neither is the GPU. You reach for more hardware when the active set itself outgrows
the box, not to brute-force away a staleness that an incremental design already avoids.
