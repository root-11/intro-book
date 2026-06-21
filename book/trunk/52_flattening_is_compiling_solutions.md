# Solutions: 52 - Flattening a tree is compiling it

Numbers below are the Ryzen 9 270 figures from the [`exprtree`](https://github.com/root-11/intro-book/tree/main/code/exprtree) crate; cross-machine capture is pending, so treat the shape as the claim, not the digits.

## Exercise 1 - Three forms, one number

```rust,no_run
// Boxes and arrows: each node its own allocation, children behind pointers.
enum Boxed { Const(f64), Var, Add(Box<Boxed>, Box<Boxed>), Mul(Box<Boxed>, Box<Boxed>) }

// The same shape in one Vec: children named by index.
struct Node { tag: Tag, lhs: u32, rhs: u32, val: f64 }   // arena: Vec<Node>

// The steps in compute order, run over a value stack.
enum Op { Const(f64), Var, Add, Mul }                     // flat: Vec<Op>

fn eval_flat(ops: &[Op], x: f64) -> f64 {
    let mut stack = Vec::with_capacity(16);
    for op in ops {
        match op {
            Op::Const(c) => stack.push(*c),
            Op::Var      => stack.push(x),
            Op::Add => { let b = stack.pop().unwrap(); let a = stack.pop().unwrap(); stack.push(a + b); }
            Op::Mul => { let b = stack.pop().unwrap(); let a = stack.pop().unwrap(); stack.push(a * b); }
        }
    }
    stack.pop().unwrap()
}
```

The contract test builds all three from the same expression and asserts `eval_boxed(x) == eval_arena(x) == eval_flat(x)` bit for bit across many values of `x`. It passes. That agreement is the floor: every timing below compares three implementations of the *same* arithmetic, so a divergence would mean you are timing three different sums, not three layouts of one.

## Exercise 2 - Trace the stack by hand

For `(x + 2) * 3` the compute-order list is `x 2 + 3 *`, traced in the chapter to 18 at `x = 4`. Take a subtraction, `(x - 5) * (x + 1)`. Post-order writes every child before its parent: `x 5 - x 1 + *`. At `x = 4`:

```
x  -> push           pad: [4]
5  -> push           pad: [4, 5]
-  -> pop two, sub   pad: [-1]        (4 - 5)
x  -> push           pad: [-1, 4]
1  -> push           pad: [-1, 4, 1]
+  -> pop two, add   pad: [-1, 5]     (4 + 1)
*  -> pop two, mul   pad: [-5]        (-1 * 5)
```

The list never looks back. Each operator consumes values already on the pad and leaves one behind, so the read head only ever moves forward. Subtraction is the one to be careful with: it is not commutative, so the order of the two pops matters - the first pop is the right operand, the second is the left (`a - b` with `a` popped second), which is exactly how the evaluator above is written.

## Exercise 3 - The size sweep

Evaluate each form in bulk across tree sizes and read nanoseconds per evaluation. Three regimes appear on one curve:

| nodes | boxed | arena | flat | flat vs boxed |
|---:|---:|---:|---:|---:|
| 15 | 27.0 | 28.5 | 18.0 | 1.50x |
| 127 | 249.4 | 284.2 | 282.1 | 0.88x |
| 511 | 1340 | 1431 | 1610 | 0.83x |
| 8191 | 34708 | 37487 | 28751 | 1.21x |
| 131071 | 573222 | 600835 | 459304 | 1.25x |
| 2097151 | 14125889 | 15235907 | 7398149 | 1.91x |

Under about a hundred nodes the flat form wins ~1.5x: everything is register- and L1-resident, so the only thing that matters is per-node overhead, and a tight linear loop beats recursion. Through a cache-resident band of roughly 127 to 1000 nodes the flat form *loses* (0.83x to 0.88x): the whole pointer tree fits in cache, the chase is nearly free, and the stack's push and pop is overhead with nothing to hide behind. Past cache it pulls ahead and keeps widening, 1.21x at 8K to 1.91x at 2M, as scattered access starts paying a cache-miss tax that sequential streaming avoids.

## Exercise 4 - The array is the control

The arena sits at or below the boxed tree at every size; it never beats it. Its evaluator still hops from each node to its children *in tree order* - the arrows became `u32` indices, but the walk jumps around the `Vec` exactly as the pointer walk jumps around the heap, and now pays a bounds check per access on top. Putting the nodes side by side bought nothing because the access order stayed scattered. The actual win was the straight-through walk of the flat form, which reads memory front to back; the layout was never the point, the access pattern was.

## Exercise 5 - The cost of editing compiled code

Swap one subtree on each form, 4000 edits at 131071 nodes:

| rep | ns / edit |
|---|---:|
| boxed | 150 |
| arena | 18 |
| flat | 495768 |

The boxed tree swings a single pointer, in time set by how deep the changed node sits. The arena repoints one `u32`, cheaper still. The flat form has no cheap edit: its order *is* the program, so any change to the shape invalidates the linear sequence, and you re-linearize the whole expression from scratch - O(N), about half a millisecond here, thousands of times the cost of swinging one pointer. This is the weakness of compiled code stated mechanically: you cannot edit it in place, because the thing that made it fast was committing to one traversal order ahead of time.

## Exercise 6 - The break-even

A workload is some fraction `r` of edits and the rest evaluations. The boxed tree has the cheap edit and the slow walk; the flat form has the slow edit and the fast walk. They break even where the flat form's faster evaluations stop repaying its expensive rebuilds. From the per-op costs at 131071 nodes the crossover is `r* = 0.19`, about **one edit per four evaluations**. Edit the shape more often than that and the O(N) re-linearization sinks the flat form, so keep pointers; evaluate more often than that and compile.

That ratio barely moves with tree size, because both the flat form's per-evaluation advantage and its per-edit rebuild cost scale with N; their ratio cancels. The break-even is therefore a property of the workload mix, not the tree. A spreadsheet formula typed once and recomputed on every neighbouring edit, a query planned once and run over millions of rows, a shader built once and run per pixel - all sit far out at the compute-many end, which is why those systems compile.

## Exercise 7 - Find the regime in the wild

Three real expression-tree systems, placed on the change-it-versus-compute-it line:

| system | edits | evaluations | regime |
|---|---|---|---|
| spreadsheet cell | retyped rarely | recomputed on every dependent edit | compute-many |
| database query | planned once | run over every row | compute-many |
| shader | compiled at load | run per pixel, per frame | compute-many, extreme |

All three live far past `r* = 0.19`. Build one expression once and evaluate it a million times and the single O(N) cost of writing the compute-order list out is amortised to nothing - the per-evaluation saving has repaid the one-time compile thousands of times over. That is the regime a bytecode VM is built for, and it is the same reason the next chapter's full recompute is a single straight sweep rather than a tree walk.
