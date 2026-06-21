# Solutions: 53 - Staleness flows downhill

Numbers below are the Ryzen 9 270 figures from the [`scenegraph`](https://github.com/root-11/intro-book/tree/main/code/scenegraph) crate; cross-machine capture is pending, so treat the shape as the claim, not the digits.

## Exercise 1 - Move a joint by hand

Take the three-node arm with local offsets shoulder 0, elbow +2, hand +1:

```
shoulder   local 0    world 0
  elbow    local +2    world 2     (0 + 2)
    hand   local +1    world 3     (2 + 1)
```

Swing the elbow to +5 and recompute. The shoulder is unchanged at world 0; the elbow becomes world 5; the hand, which reads the elbow, is dragged to world 6. The shoulder above the change never moved. The rule in one sentence: **a change to a node's local transform restates the world transform of that node and everything beneath it, and nothing else.** Staleness flows downhill and stops where the subtree stops.

## Exercise 2 - Flat, top-down

```rust,no_run
// nodes in DFS pre-order: every parent sits at a lower index than its children.
let mut world = vec![Affine::IDENTITY; n];
for i in 0..n {
    world[i] = if i == 0 { local[0] } else { world[parent[i]].compose(&local[i]) };
}
```

Because the layout is pre-order, `parent[i] < i` for every node, so by the time the loop reaches `i` its parent's world transform is already final from earlier in this same pass. The full recompute is one forward loop with no recursion, no stack, and no revisiting - each node reads a slot that was written a moment ago and is still warm in cache.

## Exercise 3 - The straight sweep vs the pointer walk

Build the same hierarchy a second way as boxed nodes with a recursive walk, and recompute every world transform both ways:

| nodes | flat sweep | pointer walk | flat speedup |
|---:|---:|---:|---:|
| 100,000 | 226,053 ns | 512,711 ns | 2.27x |
| 1,000,000 | 2,907,273 ns | 8,063,752 ns | 2.77x |

The flat sweep is 2.3x to 2.8x faster across the range. Same work, same answers, in the words of §52: the flat sweep reads memory in order while the pointer walk hops around it, and once the tree outgrows cache every hop pays a miss the sweep avoids. Even the dumb option - recompute everything - is cheap when the layout streams.

## Exercise 4 - The subtree is a slice

```rust,no_run
// subtree[i] = number of nodes in i's subtree, including i (filled once at build time).
fn descendants_of(i: usize, subtree: &[u32]) -> std::ops::Range<usize> {
    i .. i + subtree[i] as usize
}
```

In pre-order a node is immediately followed by all of its descendants, so "everything beneath node `i`" is the contiguous range `[i, i + subtree[i])`. Finding the stale set after a joint moves is therefore a subtraction, not a tree walk, and recomputing it touches one packed run of array slots rather than chasing pointers across the heap.

## Exercise 5 - The dirty crossover

Mark a joint and everything below it dirty, recompute only that range, and compare against the full sweep as the dirty fraction grows:

| dirty fraction | recompute-dirty vs recompute-all |
|---:|---:|
| 0.1% | ~900x |
| 10% | 5.7x |
| 20% | 1.7x |
| 40-50% | break-even |
| 100% | 0.77x (slower) |

Incremental wins enormously when little moved and the win shrinks as more goes stale, until somewhere past forty to fifty percent the branchless full sweep takes the lead. At a hundred percent dirty the incremental version is actually *slower* than the sweep: it does all the same arithmetic, plus the bookkeeping of carrying a dirty list and skipping nothing. Recompute-only-what-changed is a default with a ceiling - once you are touching most of the tree, stop tracking and sweep.

## Exercise 6 - Packed versus scattered

Hold the dirty *count* fixed and arrange it two ways. One contiguous subtree recomputes about **13x faster than a full sweep**; the same number of scattered single leaves runs about **as slow as a full sweep**, leaving the two arrangements more than **10x apart** at identical work. The count was the same; only the packing differed. So the condition for incremental recompute to be worth doing at all is that the stale set is *local* - packed closely enough that the recompute streams instead of hopping. This sharpens §28's "recompute beats maintain" into "recompute beats maintain when the thing you recompute is local."

## Exercise 7 - Break the tree

Let one node be read by two parents. Now it has no single position "beneath" one parent: it belongs to two subtrees at once, so there is no contiguous index range that is exactly "everything downstream of an edit." Marking the dirty set means walking the feeds-into edges and collecting whatever they reach, and that set is scattered across the array rather than packed into a slice. The pre-order shortcut from exercise 4 no longer applies, and with it the packed-recompute win from exercise 6 evaporates. You have just rediscovered the spreadsheet: dependencies that form a graph, a stale set you must compute rather than point at, and the subject of [§54](54_recompute_the_cone.md).
