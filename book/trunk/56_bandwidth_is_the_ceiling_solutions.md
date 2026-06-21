# Solutions: 56 - The ceiling is bandwidth, not cores

The CPU figures below are the Ryzen 9 270 (16 threads, dual-channel) figures from the [`heterogeneous`](https://github.com/root-11/intro-book/tree/main/code/heterogeneous) crate; cross-machine capture is pending. The GPU figures are a labelled cost model with assumed constants, not a measurement - there is no GPU on the reference machine.

## Exercise 1 - Watch the hierarchy

```rust,no_run
// advance the particles: two multiply-adds per element, all the cost is moving the numbers.
for i in 0..n { px[i] += vx[i] * dt; py[i] += vy[i] * dt; }
```

Run it on one core across sizes and the cache hierarchy shows up directly in the bandwidth:

| elements | footprint | ns/elem | GB/s |
|---:|---:|---:|---:|
| 4,096 | 0.1 MB | 0.127 | 188.6 |
| 65,536 | 1.0 MB | 0.151 | 158.6 |
| 1,048,576 | 16.8 MB | 0.282 | 85.1 |
| 16,777,216 | 268.4 MB | 1.027 | 23.4 |

The step from ~190 GB/s to ~23 GB/s is the data spilling out of cache into main memory. The drop sets in once the footprint passes the last-level cache - here between 1 MB and 17 MB, so the L2/L3 boundary on this box. Because the pass does almost no arithmetic, its speed *is* memory speed, and the RAM-resident floor of ~23 GB/s is the number that matters: real working sets do not fit in cache.

## Exercise 2 - More cores, less help

Split the same RAM-resident pass across threads:

| threads | GB/s | speedup |
|---:|---:|---:|
| 1 | 23.2 | 1.00x |
| 2 | 28.3 | 1.22x |
| 4 | 46.1 | 1.99x |
| 8 | 50.7 | 2.19x |
| 16 | 50.6 | 2.18x |

Sixteen cores do no better than eight, and the machine tops out around 2.2x. A memory-bound pass stops scaling well before you run out of cores because a single memory channel feeds all of them: past about four threads the cores are not computing in parallel, they are queueing for memory. The ceiling is the channel, not the core count.

## Exercise 3 - The frame budget

From the all-core bandwidth and a 33 ms frame (30 Hz), work out how much one box keeps current per frame:

- one core: ~**32.2 million** elements per frame;
- all cores: ~**70.3 million** elements per frame.

That is the active-set budget. Compare it not to the whole world but to the *active* part - the cells that changed this frame. A simulation can hold a billion nodes and still touch only a few million in any given frame, and a few million sits comfortably inside one box's budget. The number that decides whether you need more hardware is the size of the active set, not the size of the world.

## Exercise 4 - The argument against the GPU

A million-cell active cone is well under the ~32 M one core keeps current per frame, so it needs no accelerator at all - it fits a single core with room to spare. The GPU answers "how do I recompute *everything* fast?", but the incremental discipline of [§53](53_dirty_propagation.md) and [§54](54_recompute_the_cone.md) already stopped recomputing everything; it keeps only the active cone current. The precise case in which you would reach for more hardware is when the *active set itself* - not the whole world - exceeds what one box can feed in a frame. That is a measurement about the size of what actually changes, not a reflex triggered by the size of the data.

## Exercise 5 - The bus is the bottleneck

```
ship to device:  16 B/elem      read back:  8 B/elem      round trip: 24 B/elem
transfer cost @ ~16 GB/s PCIe  =  1.500 ns/elem
CPU all-core pass              =  0.474 ns/elem
```

To offload one pass of CPU-resident data you move about the same number of bytes the computation itself needs, and just shipping it across the bus and back costs **1.5 ns/elem** against the CPU's **0.474 ns/elem** for the whole pass. The round trip alone is already slower than doing the work on the box. The break-even is an arithmetic-intensity argument: offload wins only when there is enough compute per byte that the transfer is hidden, or when the data already lives on the device. For two multiply-adds per element - a memory-bound pass - neither holds, so offload loses. (Plug your own bus and launch numbers into the same model; the structure is the point, not these assumed constants.)

## Exercise 6 - Touch less, not more

Take a pass from an earlier chapter and speed it up two ways. Throwing cores at it buys about 2.2x before the memory channel saturates and the extra cores wait on RAM. Shrinking the working set - computing only the active cone, as [§53](53_dirty_propagation.md) and [§54](54_recompute_the_cone.md) did - buys orders of magnitude when little changed (the scenegraph's ~900x at 0.1% dirty, the spreadsheet's 16x for a single edit), because it moves less data through the one channel that gates everything. The hardware lever is capped by the channel; the touch-less lever attacks the thing the channel is busy with. That is the lever the whole arc has been pulling, and for these workloads it beats buying more hardware - you go faster by moving fewer bytes, not by adding compute that then queues for memory.
