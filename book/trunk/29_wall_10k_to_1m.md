# 29 - The wall at 10K → 1M

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 29](../../concepts/glossary.md#29--the-wall-at-10k--1m).*

<p align="center"><img src="../illustrations/hard_hat_repeat.jpg" alt="Construction mouse - scale up the build, MEASURE / CALCULATE / DESIGN / BUILD / REPEAT" style="max-height: 300px; max-width: 100%;"></p>

A simulator that runs cleanly at 10 000 creatures often grinds to a halt at 1 000 000. Not because the algorithm changed - because constant factors that were invisible at the smaller scale now bind.

This chapter is about *finding the wall*. The fixes are techniques you already have: narrower fields (§7), subscriptions (§26), working-set discipline (§27), sort for locality (§28), pre-sized buffers, batched cleanup. The chapter's job is to teach the reader to *measure* - to find which constant factors blew up.

Constant-factor bugs that bind at 10K → 1M:

- **Reallocation.** A `to_insert: Vec<CreatureRow>` that grew lazily was fine at 100 pushes per tick (10K creatures × 1% reproduction). At 10K pushes per tick (1M × 1%), the reallocations dominate. Fix: `Vec::with_capacity(estimated_max)`.
- **Linear scans.** `hungry.iter().any(|&id| id == target_id)` was 0.1 ms at 10K, but 10 ms at 1M. Fix: the `id_to_slot` map (§23) plus parallel presence flags.
- **Cache spillover.** `creature` working set at 10K is 200 KB (L2-resident). At 1M it is 20 MB (L3-resident). Per-element time triples. Fix: narrower fields (§7) and sort for locality (§28); for a system that touches only a subset, a subscription (§26).
- **`HashMap` iteration order.** A `HashMap<u32, _>` iterated by systems that need deterministic order. At 10K the cost was tolerable; at 1M the bandwidth cost is high. Fix: `BTreeMap` or `Vec<(K, V)>`.
- **Per-tick allocation.** A system that allocates a fresh `Vec` per tick was fine when the `Vec` was 1 KB. At 1M it is 100 KB; allocation latency starts to matter. Fix: reuse buffers across ticks.
- **Logging.** A `println!` per creature was tolerable at 10K. At 1M it is the simulator's bottleneck. Fix: buffered logging, periodic snapshots, or simply turn it off.

The pattern: any cost that was O(1) per creature, multiplied by 1M, is no longer free. Anything that was O(N) per tick at 10K is now O(N²)-equivalent in wall time. The fixes are local - each cost is a single-line change - but finding them requires measurement.

The right tool is a profiler. `cargo flamegraph` (or `perf record` + `perf report`) tells you where the time goes. The same simulator at 10K and 1M produces different flame graphs; the wall is the difference.

A useful exercise: run your simulator at 10K for 1000 ticks; time it. Run at 1M for 100 ticks (same total entity-ticks); time it. The 1M version should take ~10× longer, not 100×. If it takes 100×, something has crossed a constant-factor wall and the profiler will show you what.

The fix is structural. Apply the techniques: narrow fields, subscriptions, working set, sort for locality, pre-sized buffers, batched cleanup, deterministic structures. Each is a chapter you have already read. The wall is the moment they all become non-optional.

## Exercises

1. **Calibration.** Run your simulator at N = 10K for 1000 ticks. Time it. Note the wall-clock total.
2. **Scale up.** Run at N = 1M for 100 ticks (same total entity-ticks). Time it. Compute the ratio.
3. **Profile.** Use `cargo flamegraph` (or `perf`) on the 1M run. Identify the top three hottest functions.
4. **Pre-size `to_insert`.** Apply `Vec::with_capacity` to your cleanup buffers. Re-run; re-profile. Did the hot list change?
5. **Subscribe a subset system.** Take a system that acts on a fraction of creatures (starvation, reproduction) and give it a slot-keyed subscription table (§26) instead of scanning all 1M and branching. Re-run; re-profile. The scan-all frame should drop off the flame graph; the system's work falls in proportion to the subscribed fraction.
6. **Use index maps.** Replace any linear `iter().any()` with the §23 `id_to_slot` lookup. Re-run; re-profile.
7. *(stretch)* **Find one new wall.** Pick any system in your simulator and find one constant factor that scales worse than expected. The fix is usually one of the techniques above; identifying *which* one is the lesson.

Reference notes in [29_wall_10k_to_1m_solutions.md](29_wall_10k_to_1m_solutions.md).

## What's next

[§30 - Moving beyond the wall](30_streaming_wall.md) takes the next step: when even your fastest, tightest, subscription-driven, sorted-for-locality simulator no longer fits in RAM, the architecture itself shifts.
