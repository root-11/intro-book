# Solutions: 3 — The `Vec` is a table

## Exercise 1 — Layout

```rust
use std::mem::size_of;
fn main() {
    println!("Vec<u32> = {}", size_of::<Vec<u32>>());  // 24
    println!("Vec<u64> = {}", size_of::<Vec<u64>>());  // 24
    println!("Vec<u8>  = {}", size_of::<Vec<u8>>());   // 24
}
```

A `Vec<T>` is always 24 bytes on a 64-bit machine (three 8-byte fields: ptr, len, cap), regardless of `T`. The element data lives elsewhere on the heap.

## Exercise 2 — Capacity growth

```rust
let mut v: Vec<u32> = Vec::new();
for i in 0..100 {
    v.push(i);
    if v.len().is_power_of_two() || v.len() < 5 {
        println!("len={}, cap={}", v.len(), v.capacity());
    }
}
```

Output (Rust's current strategy roughly doubles, but starts at 4):

```
len=1, cap=4
len=2, cap=4
len=4, cap=4
len=8, cap=8
len=16, cap=16
len=32, cap=32
len=64, cap=64
```

Each transition is a reallocation: a new heap region is allocated, all elements are memcpy'd across, the old one is freed.

## Exercise 3 — Pre-size

```rust
let mut v = Vec::with_capacity(100);
for i in 0..100 { v.push(i); }
println!("len={}, cap={}", v.len(), v.capacity()); // len=100, cap=100
```

No reallocations happened. This is the right pattern when you know the upper bound — and most simulations do.

## Exercise 4 — Indexing cost

A sequential `Vec<u32>` sum runs ~1 ns/elem. A `HashMap<usize, u32>` lookup costs ~50-100 ns each (hash, probe, compare). Multiple orders of magnitude.

## Exercise 5 — `swap_remove` vs `remove`

100 calls to `vec.remove(500_000)` on a 1M `Vec<u32>` move ~50 million elements (each `remove` shifts ~half the vector). At ~1 ns per move that is ~50 ms total.

100 calls to `vec.swap_remove(500_000)` on the same vector move 100 elements total — under a microsecond.

The factor is roughly `N / 2`. For 1 million entries, that is half a million times faster.

## Exercise 6 — Slices in function signatures

```rust
fn sum(xs: &[u32]) -> u64 {
    xs.iter().map(|&x| x as u64).sum()
}

let v: Vec<u32> = (0..1000).collect();
let total = sum(&v);  // &Vec<u32> auto-derefs to &[u32]
```

The function takes a slice; the caller passes `&v`. The conversion (`Deref`) is automatic. This is why almost every system in the book has signatures over `&[T]` and `&mut [T]`, not `&Vec<T>`.

## Exercise 7 — A from-scratch `MyVec<u32>`

The full implementation is in [the Rustonomicon](https://doc.rust-lang.org/nomicon/vec/vec.html); about 200 lines including tests. The key shape:

```rust
struct MyVec<T> {
    ptr: NonNull<T>,
    len: usize,
    cap: usize,
}
```

`new` starts with `cap = 0` and a dangling pointer. `push` allocates on first push (`grow`), then doubles capacity when full. `get` returns `Option<&T>` with bounds check. `Drop` frees both elements (running their destructors) and the heap allocation.

Working through this once is the cheapest way to convince yourself a `Vec<T>` is a small piece of careful work — and to internalise [§42 — You can only fix what you wrote](42_you_can_only_fix_what_you_wrote.md).
