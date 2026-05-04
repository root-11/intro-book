# 3 — The `Vec` is a table

> *Concept node: see the [DAG](../../concepts/dag.md) and [glossary entry 3](../../concepts/glossary.md#3--the-vec-is-a-table).*

<p align="center"><img src="../illustrations/linear_algebra.jpg" alt="Linear algebra: Ax = b — a table is a matrix of columns indexed in lockstep" style="max-height: 300px; max-width: 100%;"></p>

A `Vec<T>` is three things stored on the stack: a pointer to a contiguous run of `T` values on the heap, the current length, and the current capacity. The values themselves live on the heap, side by side, with no padding between them. `vec[i]` computes `ptr + i * size_of::<T>()` and reads.

This is the only container the trunk of this book uses. There are no hash maps, no linked lists, no trees — not because they do not exist, but because almost every problem the book teaches is a problem of "process all the rows of a table", and a `Vec<T>` *is* the table. Adding any other container costs cache, costs allocations, and breaks the sequential-access pattern that nodes 1 and 2 just told you to want.

`vec.push(x)` adds an element. If there is capacity, it writes into the next slot — O(1). If not, it allocates a larger heap region (typically twice the current capacity), copies everything across, and frees the old one. Amortised over many pushes that is O(1), but each individual push *might* be expensive. If you know how many elements you are going to insert, `Vec::with_capacity(n)` allocates once and avoids the copies.

`vec.swap_remove(i)` removes the element at `i` in O(1) by moving the last element into the freed slot. Order is sacrificed for speed. This will earn its keep at [§21](21_swap_remove.md).

`vec.iter()` walks the slots in order. The compiler can usually turn this into a tight memory-bandwidth-bound loop with auto-vectorisation. `vec.iter_mut()` does the same, with mutation.

A `&[T]` is a *slice* — a pointer plus a length, without the capacity. It is what functions usually take when they want to read a `Vec` without owning it. `&mut [T]` is the same with mutation. Most systems in this book have signatures like `fn motion(pos: &mut [Pos], vel: &[Vel])` — read this, write that, no ownership taken.

That is the full vocabulary you need from `Vec` for the next several phases. Everything else (`HashMap`, `BTreeMap`, `Box<Node>`, `Rc<RefCell<T>>`, `LinkedList`) is something you will reach for only when an exercise demands it and the from-scratch test (node 40) shows it earns its weight.

## Exercises

1. **Layout.** Print `std::mem::size_of::<Vec<u32>>()`. It should be 24 on a 64-bit machine — three pointer-sized fields. Notice that the size of the *Vec value* does not depend on how many elements it holds.
2. **Capacity vs length.** Build `let mut v: Vec<u32> = Vec::new();`. In a loop from 0 to 100, print `v.len()` and `v.capacity()` after each `v.push(i)`. Observe the capacity doubling pattern: 0, 4, 8, 16, 32, 64, 128.
3. **Pre-size.** Build `let mut v = Vec::with_capacity(100);` and push 100 elements. Print `len` and `capacity` once at the end. There were no reallocations.
4. **Indexing cost.** Time `vec[i]` on a 1M `Vec<u32>` accessed sequentially. Compare with the same access on a `HashMap<usize, u32>` of the same size. Sequential `Vec` reads should be ~10-100× faster.

> [!NOTE]
> Measured ratios: ~65× on a Raspberry Pi 4, ~75-90× on mid-2010s Intel laptops, ~175× on a modern Ryzen-class chip. All use Rust's default `HashMap` (SipHash). Modern hardware widens the gap because the `Vec` sum is auto-vectorized and well-prefetched; `HashMap::get` cannot be either. Order-of-magnitude (60-200×) is the durable claim.

5. **`swap_remove` vs `remove`.** Build a `Vec<u32>` of 1,000,000 elements. Time removing 100 elements from the middle with `vec.remove(500_000)` (in a loop, because each `remove` shifts roughly half the vector). Time the same with `vec.swap_remove(500_000)`. Note the orders-of-magnitude difference.
6. **Slices in function signatures.** Write `fn sum(xs: &[u32]) -> u64`. Call it with `sum(&v)` where `v: Vec<u32>`. Note that you did not have to write `&v[..]` — the conversion is automatic.
7. *(stretch)* **A from-scratch `MyVec<u32>`.** Implement `MyVec` with a raw pointer, length, and capacity. Implement `new`, `push`, `get`, and `Drop`. (You will use `unsafe`. Read [the Rustonomicon's `Vec` chapter](https://doc.rust-lang.org/nomicon/vec/vec.html) when stuck.) Convince yourself a `Vec<T>` is a few hundred lines of careful work, no magic.

Reference notes in [03_the_vec_is_a_table_solutions.md](03_the_vec_is_a_table_solutions.md).

## What's next

[§4 — Cost is layout, and you have a budget](04_cost_and_budget.md) is where the layout reasoning from §1 and §2 meets the per-tick clock the rest of the book runs on. After that, [§5 — Identity is an integer](05_identity_is_an_integer.md) is the card game.
