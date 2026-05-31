//! pointer_chase - sum a Vec<u64> vs a *shuffled* linked list of Box<Node>.
//! Used by §1 exercise 5.
//!
//!     cargo run --release --bin pointer_chase
//!
//! The shuffle matters: a linked list built by `let mut head = None; for i in
//! (0..N).rev() { head = Some(Box::new(Node { value: i, next: head })) }`
//! allocates N Boxes at *sequential* heap addresses. The "linked list" is a
//! Vec in disguise; the pointer-chase tax is invisible. After shuffling the
//! order in which we thread them, traversal scatters across the heap and the
//! cache cost is real.

use std::time::Instant;

const N: usize = 1_000_000;

struct Node {
    value: u64,
    next:  Option<Box<Node>>,
}

#[inline(never)]
fn sum_vec(v: &[u64]) -> u64 {
    v.iter().sum()
}

#[inline(never)]
fn sum_list(head: &Node) -> u64 {
    let mut total = head.value;
    let mut cur = &head.next;
    while let Some(n) = cur {
        total += n.value;
        cur = &n.next;
    }
    total
}

/// Build a list of N nodes such that traversal scatters across the heap.
/// All N Boxes are allocated first (sequential heap addresses), then we
/// shuffle the *order in which they are threaded*. Each box still lives
/// at its original address; the chain visits them in scrambled order.
fn build_shuffled_list(n: usize) -> Box<Node> {
    let mut nodes: Vec<Box<Node>> = (0..n as u64)
        .map(|i| Box::new(Node { value: i, next: None }))
        .collect();

    // Fisher-Yates over the Vec of Boxes.
    let mut s = 0x1234_5678_u64;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s
    };
    for i in (1..nodes.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        nodes.swap(i, j);
    }

    // Thread them in shuffled order: pop from the end, prepend to head.
    // Result: traversing head → next → ... visits nodes[0], nodes[1], ...
    let mut head: Option<Box<Node>> = None;
    while let Some(mut node) = nodes.pop() {
        node.next = head;
        head = Some(node);
    }
    head.unwrap()
}

/// Iterative drop - recursive `Box<Node>` Drop blows the stack at N=1M.
fn drop_list(head: Box<Node>) {
    let mut cur = Some(head);
    while let Some(mut node) = cur {
        cur = node.next.take();
    }
}

fn main() {
    let v: Vec<u64> = (0..N as u64).collect();
    let head = build_shuffled_list(N);

    let t0 = Instant::now();
    let s_vec = std::hint::black_box(sum_vec(&v));
    let dt_vec = t0.elapsed();

    let t0 = Instant::now();
    let s_list = std::hint::black_box(sum_list(&head));
    let dt_list = t0.elapsed();

    println!("Vec  sum:  {:>10.3} ms  ({:>5.2} ns/elem)",
             dt_vec.as_secs_f64() * 1000.0,
             dt_vec.as_nanos() as f64 / N as f64);
    println!("List sum:  {:>10.3} ms  ({:>5.2} ns/elem)",
             dt_list.as_secs_f64() * 1000.0,
             dt_list.as_nanos() as f64 / N as f64);
    println!("Ratio:     {:>10.1}x  (list slower)",
             dt_list.as_nanos() as f64 / dt_vec.as_nanos() as f64);

    // The Vec sum is 0+1+…+(N-1); the list sum visits the same set of values
    // in shuffled order. Both reduce to N(N-1)/2.
    let expected: u64 = (N as u64 * (N as u64 - 1)) / 2;
    assert_eq!(s_vec, expected);
    assert_eq!(s_list, expected);

    drop_list(head);
}
