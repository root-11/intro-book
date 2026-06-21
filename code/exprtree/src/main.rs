//! Reference for Part II - "Where SoA does not pay", project A: the expression evaluator.
//!
//! One arithmetic expression, three representations, measured against each other.
//! The point is a *crossover*, not a verdict: a flat layout wins when you walk the
//! tree far more often than you rewrite it; pointers win when you rewrite it far
//! more often than you walk it. We measure where the line is, on this machine.
//!
//! The three representations of the same expression `(x*0.7 + 1.2) - (x - 0.3)*x ...`:
//!
//! 1. `Boxed` - the idiomatic pointer tree: `enum Expr { Add(Box<Expr>, Box<Expr>), ... }`.
//!    Each node is its own heap allocation; eval chases pointers.
//! 2. `Arena` - a flat `Vec<Node>` with `u32` child indices. Contiguous memory, but eval
//!    still hops by index in tree order.
//! 3. `Flat` - the same tree linearized into post-order and evaluated by a single forward
//!    pass over a `Vec<Op>` with a value stack. Pure sequential access. This *is* a
//!    stack-machine / RPN bytecode VM: flattening a recursive structure for traversal is
//!    compilation to a linear instruction stream.
//!
//! Two workloads expose the crossover:
//!
//!   - Bulk evaluation (traversal-dominated), swept over tree size N. Expect Flat to
//!     pull ahead of Arena and Boxed as N outgrows cache, and the three to converge at
//!     tiny N where the constant factor dominates (the small-N carve-out, measured).
//!   - Structural mutation (topology-dominated). Boxed/Arena swing one child in
//!     O(path); Flat must re-linearize the whole expression, O(N), on every edit. The
//!     derived crossover is an edit-to-traverse ratio.
//!
//! Run:    cargo run --release
//! Tests:  cargo test --release

use std::hint::black_box;
use std::time::Instant;

// ============================================================================
// Deterministic RNG - Numerical Recipes LCG, the same one code/deck uses.
// Seeded, so every run and every machine builds the identical tree.
// ============================================================================

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    /// A float in [0, 1).
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Leaf constants live in [0.5, 1.5) and the variable `x` is fed values in the same
/// range, so sums grow gently and products stay near 1: no inf, no NaN, even at
/// depth 20. That keeps the three representations bit-for-bit identical, which is
/// the correctness contract the tests check.
fn leaf_const(rng: &mut Lcg) -> f64 {
    0.5 + rng.unit()
}

// ============================================================================
// Representation 1: the idiomatic pointer tree.
// ============================================================================

#[derive(Clone)]
enum Expr {
    Const(f64),
    Var,
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}

#[inline(never)]
fn eval_boxed(e: &Expr, x: f64) -> f64 {
    match e {
        Expr::Const(c) => *c,
        Expr::Var => x,
        Expr::Add(l, r) => eval_boxed(l, x) + eval_boxed(r, x),
        Expr::Sub(l, r) => eval_boxed(l, x) - eval_boxed(r, x),
        Expr::Mul(l, r) => eval_boxed(l, x) * eval_boxed(r, x),
    }
}

/// Build a full balanced binary tree of the given depth: `2^depth` leaves,
/// `2^(depth+1) - 1` nodes. Balanced keeps recursion depth at `depth` (~20 for a
/// million nodes), so none of the recursive walks overflow the stack.
fn gen_tree(depth: u32, rng: &mut Lcg) -> Expr {
    if depth == 0 {
        if rng.next() & 1 == 0 {
            Expr::Var
        } else {
            Expr::Const(leaf_const(rng))
        }
    } else {
        let l = Box::new(gen_tree(depth - 1, rng));
        let r = Box::new(gen_tree(depth - 1, rng));
        match rng.below(3) {
            0 => Expr::Add(l, r),
            1 => Expr::Sub(l, r),
            _ => Expr::Mul(l, r),
        }
    }
}

/// Replace the subtree reached by following `steps` (false = left, true = right)
/// with a clone of `new`. If a leaf is hit before the steps run out, the leaf is
/// replaced. O(steps) navigation plus O(|new|) clone, plus the drop of the old
/// subtree. The arena and flat versions implement the identical edit.
fn mutate_boxed(root: &mut Expr, steps: &[bool], new: &Expr) {
    let mut node = root;
    for &go_right in steps {
        match node {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) => {
                node = if go_right { &mut **r } else { &mut **l };
            }
            _ => break, // hit a leaf early - replace it
        }
    }
    *node = new.clone();
}

// ============================================================================
// Representation 2: the flat arena. One Vec, u32 child indices.
// ============================================================================

#[derive(Clone, Copy, PartialEq)]
enum Tag {
    Const,
    Var,
    Add,
    Sub,
    Mul,
}

impl Tag {
    fn is_internal(self) -> bool {
        matches!(self, Tag::Add | Tag::Sub | Tag::Mul)
    }
}

#[derive(Clone, Copy)]
struct Node {
    tag: Tag,
    lhs: u32,
    rhs: u32,
    val: f64,
}

#[derive(Clone)]
struct Arena {
    nodes: Vec<Node>,
    root: u32,
}

impl Arena {
    fn from_expr(e: &Expr) -> Self {
        let mut nodes = Vec::new();
        let root = build_arena(e, &mut nodes);
        Arena { nodes, root }
    }

    fn eval(&self, x: f64) -> f64 {
        eval_arena(&self.nodes, self.root, x)
    }

    /// Same edit as `mutate_boxed`: the new subtree's nodes are appended (the old
    /// nodes are simply orphaned - deferred GC, the book's pattern), and one parent
    /// child-index is repointed.
    fn mutate(&mut self, steps: &[bool], new: &Expr) {
        let new_root = build_arena_into(new, &mut self.nodes);
        if steps.is_empty() {
            self.root = new_root;
            return;
        }
        let mut parent = u32::MAX;
        let mut side_right = false;
        let mut idx = self.root;
        for &go_right in steps {
            let n = self.nodes[idx as usize];
            if !n.tag.is_internal() {
                break; // leaf reached early - replace it via its parent
            }
            parent = idx;
            side_right = go_right;
            idx = if go_right { n.rhs } else { n.lhs };
        }
        if parent == u32::MAX {
            self.root = new_root;
        } else if side_right {
            self.nodes[parent as usize].rhs = new_root;
        } else {
            self.nodes[parent as usize].lhs = new_root;
        }
    }
}

fn build_arena(e: &Expr, nodes: &mut Vec<Node>) -> u32 {
    build_arena_into(e, nodes)
}

/// Append the nodes of `e` to `nodes` in post-order and return the root index.
/// Children are pushed before their parent, so a parent's child indices always
/// point at already-placed nodes.
fn build_arena_into(e: &Expr, nodes: &mut Vec<Node>) -> u32 {
    let node = match e {
        Expr::Const(c) => Node {
            tag: Tag::Const,
            lhs: 0,
            rhs: 0,
            val: *c,
        },
        Expr::Var => Node {
            tag: Tag::Var,
            lhs: 0,
            rhs: 0,
            val: 0.0,
        },
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) => {
            let lhs = build_arena_into(l, nodes);
            let rhs = build_arena_into(r, nodes);
            let tag = match e {
                Expr::Add(..) => Tag::Add,
                Expr::Sub(..) => Tag::Sub,
                _ => Tag::Mul,
            };
            Node {
                tag,
                lhs,
                rhs,
                val: 0.0,
            }
        }
    };
    nodes.push(node);
    (nodes.len() - 1) as u32
}

#[inline(never)]
fn eval_arena(nodes: &[Node], i: u32, x: f64) -> f64 {
    let n = &nodes[i as usize];
    match n.tag {
        Tag::Const => n.val,
        Tag::Var => x,
        Tag::Add => eval_arena(nodes, n.lhs, x) + eval_arena(nodes, n.rhs, x),
        Tag::Sub => eval_arena(nodes, n.lhs, x) - eval_arena(nodes, n.rhs, x),
        Tag::Mul => eval_arena(nodes, n.lhs, x) * eval_arena(nodes, n.rhs, x),
    }
}

// ============================================================================
// Representation 3: the linearized post-order stack machine (RPN bytecode).
// ============================================================================

#[derive(Clone, Copy)]
enum Op {
    Const(f64),
    Var,
    Add,
    Sub,
    Mul,
}

#[derive(Clone)]
struct Flat {
    /// The editable source of truth. Edits go here; `code` is recompiled from it.
    arena: Arena,
    /// The read-optimized projection: the expression in post-order.
    code: Vec<Op>,
}

impl Flat {
    fn from_arena(arena: Arena) -> Self {
        let mut code = Vec::with_capacity(arena.nodes.len());
        compile(&arena.nodes, arena.root, &mut code);
        Flat { arena, code }
    }

    /// Single forward pass over the code with a value stack. `stack` is passed in
    /// and reused across calls so the hot path does no allocation.
    #[inline(never)]
    fn eval(&self, x: f64, stack: &mut Vec<f64>) -> f64 {
        stack.clear();
        for op in &self.code {
            match *op {
                Op::Const(c) => stack.push(c),
                Op::Var => stack.push(x),
                Op::Add => {
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    stack.push(a + b);
                }
                Op::Sub => {
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    stack.push(a - b);
                }
                Op::Mul => {
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    stack.push(a * b);
                }
            }
        }
        stack.pop().unwrap()
    }

    /// The structural edit Flat cannot do incrementally: mutate the source arena
    /// (cheap), then re-linearize the entire expression (O(N)). This O(N) per edit
    /// is the whole reason Flat loses the edit-dominated workload.
    fn mutate(&mut self, steps: &[bool], new: &Expr) {
        self.arena.mutate(steps, new);
        self.code.clear();
        compile(&self.arena.nodes, self.arena.root, &mut self.code);
    }
}

/// Emit `i`'s subtree in post-order: both children before the operator. The value
/// stack then sees the right operand on top, the left beneath it, so each binary op
/// pops `b` (right) then `a` (left) and pushes `a op b` - preserving association so
/// the result is bit-identical to the recursive evaluators.
fn compile(nodes: &[Node], i: u32, out: &mut Vec<Op>) {
    let n = &nodes[i as usize];
    match n.tag {
        Tag::Const => out.push(Op::Const(n.val)),
        Tag::Var => out.push(Op::Var),
        Tag::Add => {
            compile(nodes, n.lhs, out);
            compile(nodes, n.rhs, out);
            out.push(Op::Add);
        }
        Tag::Sub => {
            compile(nodes, n.lhs, out);
            compile(nodes, n.rhs, out);
            out.push(Op::Sub);
        }
        Tag::Mul => {
            compile(nodes, n.lhs, out);
            compile(nodes, n.rhs, out);
            out.push(Op::Mul);
        }
    }
}

// ============================================================================
// Benchmark plumbing.
// ============================================================================

const XS_LEN: usize = 256; // power of two, masked into the eval loop index

fn build_xs(rng: &mut Lcg) -> Vec<f64> {
    (0..XS_LEN).map(|_| 0.5 + rng.unit()).collect()
}

fn nodes_in(depth: u32) -> u64 {
    (1u64 << (depth + 1)) - 1
}

/// Median of five timed samples, in nanoseconds. Median, not mean, to shrug off the
/// occasional scheduler hiccup; not min, so we report a typical run, not a best case.
fn median5(mut sample: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..5).map(|_| sample()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[2]
}

fn bench_eval_boxed(root: &Expr, xs: &[f64], iters: u64) -> f64 {
    let mut acc = 0.0f64;
    let t = Instant::now();
    for i in 0..iters {
        let x = xs[(i as usize) & (XS_LEN - 1)];
        acc += eval_boxed(black_box(root), black_box(x));
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    black_box(acc);
    ns
}

fn bench_eval_arena(a: &Arena, xs: &[f64], iters: u64) -> f64 {
    let mut acc = 0.0f64;
    let t = Instant::now();
    for i in 0..iters {
        let x = xs[(i as usize) & (XS_LEN - 1)];
        acc += eval_arena(black_box(&a.nodes), black_box(a.root), black_box(x));
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    black_box(acc);
    ns
}

fn bench_eval_flat(f: &Flat, xs: &[f64], iters: u64) -> f64 {
    let mut stack: Vec<f64> = Vec::with_capacity(64);
    let mut acc = 0.0f64;
    let t = Instant::now();
    for i in 0..iters {
        let x = xs[(i as usize) & (XS_LEN - 1)];
        acc += f.eval(black_box(x), &mut stack);
    }
    let ns = t.elapsed().as_nanos() as f64 / iters as f64;
    black_box(acc);
    ns
}

/// A precomputed list of edits: a navigation path plus the small subtree to graft.
/// The same list is replayed against each representation so the edits are identical.
fn make_edits(n: usize, max_depth: u32, rng: &mut Lcg) -> Vec<(Vec<bool>, Expr)> {
    (0..n)
        .map(|_| {
            let len = rng.below(max_depth as u64) as usize + 1;
            let steps: Vec<bool> = (0..len).map(|_| rng.next() & 1 == 0).collect();
            let new = gen_tree(2, rng); // a small graft: 4 leaves, 7 nodes
            (steps, new)
        })
        .collect()
}

fn bench_edits_boxed(initial: &Expr, edits: &[(Vec<bool>, Expr)]) -> f64 {
    let mut tree = initial.clone(); // clone is setup, not timed
    let t = Instant::now();
    for (steps, new) in edits {
        mutate_boxed(black_box(&mut tree), steps, new);
    }
    let ns = t.elapsed().as_nanos() as f64 / edits.len() as f64;
    black_box(eval_boxed(&tree, 1.0));
    ns
}

fn bench_edits_arena(initial: &Arena, edits: &[(Vec<bool>, Expr)]) -> f64 {
    let mut a = initial.clone();
    let t = Instant::now();
    for (steps, new) in edits {
        a.mutate(black_box(steps), new);
    }
    let ns = t.elapsed().as_nanos() as f64 / edits.len() as f64;
    black_box(a.eval(1.0));
    ns
}

fn bench_edits_flat(initial: &Flat, edits: &[(Vec<bool>, Expr)]) -> f64 {
    let mut f = initial.clone();
    let mut stack = Vec::with_capacity(64);
    let t = Instant::now();
    for (steps, new) in edits {
        f.mutate(black_box(steps), new);
    }
    let ns = t.elapsed().as_nanos() as f64 / edits.len() as f64;
    black_box(f.eval(1.0, &mut stack));
    ns
}

// ============================================================================
// main: run the two workloads and print the tables.
// ============================================================================

fn main() {
    let mut rng = Lcg::new(0x1234_5678_9abc_def0);
    let xs = build_xs(&mut rng);

    // ---- Workload 1: bulk evaluation, swept over tree size ----
    println!("== Workload 1: bulk evaluation (traversal-dominated) ==");
    println!("ns per evaluation, median of 5; lower is better.\n");
    println!(
        "{:>6} {:>12} {:>10} {:>10} {:>10}   flat vs boxed",
        "depth", "nodes", "boxed", "arena", "flat"
    );

    let depths: &[u32] = &[3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 16, 18, 20];
    for &depth in depths {
        let nodes = nodes_in(depth);
        // ~2e7 node-visits per timed run, floored so big trees still run enough
        // iterations to time stably without taking minutes.
        let iters = (20_000_000 / nodes).max(50);

        let mut g = Lcg::new(0x51A1_1ED5 ^ depth as u64);
        let boxed = gen_tree(depth, &mut g);
        let arena = Arena::from_expr(&boxed);
        let flat = Flat::from_arena(arena.clone());

        let b = median5(|| bench_eval_boxed(&boxed, &xs, iters));
        let a = median5(|| bench_eval_arena(&arena, &xs, iters));
        let f = median5(|| bench_eval_flat(&flat, &xs, iters));

        println!(
            "{:>6} {:>12} {:>10.1} {:>10.1} {:>10.1}   {:>5.2}x",
            depth,
            nodes,
            b,
            a,
            f,
            b / f
        );
    }

    // ---- Workload 2: structural mutation, fixed N ----
    let edit_depth: u32 = 16;
    let edit_nodes = nodes_in(edit_depth);
    let n_edits = 4000;

    let mut g = Lcg::new(0xED17_5EED);
    let boxed = gen_tree(edit_depth, &mut g);
    let arena = Arena::from_expr(&boxed);
    let flat = Flat::from_arena(arena.clone());
    let edits = make_edits(n_edits, edit_depth, &mut g);

    let eb = median5(|| bench_edits_boxed(&boxed, &edits));
    let ea = median5(|| bench_edits_arena(&arena, &edits));
    let ef = median5(|| bench_edits_flat(&flat, &edits));

    // Eval cost at the same N, to derive the crossover.
    let iters = (20_000_000 / edit_nodes).max(50);
    let vb = median5(|| bench_eval_boxed(&boxed, &xs, iters));
    let va = median5(|| bench_eval_arena(&arena, &xs, iters));
    let vf = median5(|| bench_eval_flat(&flat, &xs, iters));

    println!("\n== Workload 2: structural mutation (topology-dominated) ==");
    println!("fixed tree: depth {edit_depth}, {edit_nodes} nodes; {n_edits} edits; median of 5.\n");
    println!("{:>8} {:>14} {:>14}", "rep", "ns / edit", "ns / eval");
    println!("{:>8} {:>14.1} {:>14.1}", "boxed", eb, vb);
    println!("{:>8} {:>14.1} {:>14.1}", "arena", ea, va);
    println!("{:>8} {:>14.1} {:>14.1}", "flat", ef, vf);

    // ---- The crossover ----
    // Total cost of a workload with edit-fraction r is r*edit + (1-r)*eval per op.
    // Flat has the cheaper eval but the dearer edit, so it loses once edits are
    // frequent enough. Solve flat == boxed: r* = (eval_b - eval_f) / (edit_f - edit_b + eval_b - eval_f).
    println!("\n== Crossover (derived from the measured per-op costs) ==");
    crossover_line("flat vs boxed", ef, vf, eb, vb);
    crossover_line("flat vs arena", ef, vf, ea, va);
    println!(
        "\nReading: flat (compile-once, eval-many) wins only below the crossover edit-fraction."
    );
    println!(
        "A bytecode VM lives at r -> 0 (parse once, evaluate per row/frame); that is its home."
    );
}

fn crossover_line(label: &str, edit_f: f64, eval_f: f64, edit_o: f64, eval_o: f64) {
    let num = eval_o - eval_f; // flat's eval advantage (expected > 0)
    let den = (edit_f - edit_o) + num; // flat's edit penalty plus that advantage
    if num <= 0.0 || den <= 0.0 {
        println!(
            "{label}: no crossover in [0,1] - flat is not uniformly better on traversal here \
             (eval advantage {num:.1} ns, edit penalty {:.1} ns)",
            edit_f - edit_o
        );
        return;
    }
    let r = num / den;
    println!(
        "{label}: crossover at edit-fraction r* = {r:.6}  (about 1 edit per {:.0} evaluations)",
        if r > 0.0 {
            (1.0 - r) / r
        } else {
            f64::INFINITY
        }
    );
}

// ============================================================================
// Contract tests: the three representations must agree, bit for bit, before and
// after an edit. If they ever diverge, the measurement is comparing different
// computations and is meaningless - so this is the load-bearing test.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_all_agree(boxed: &Expr, arena: &Arena, flat: &Flat) {
        let mut stack = Vec::new();
        for k in 0..32 {
            let x = 0.5 + k as f64 / 32.0;
            let b = eval_boxed(boxed, x);
            let a = arena.eval(x);
            let f = flat.eval(x, &mut stack);
            assert_eq!(b.to_bits(), a.to_bits(), "boxed vs arena diverged at x={x}");
            assert_eq!(b.to_bits(), f.to_bits(), "boxed vs flat diverged at x={x}");
        }
    }

    #[test]
    fn representations_agree_across_depths() {
        for depth in [0, 1, 2, 5, 8, 12] {
            let mut rng = Lcg::new(0xC0FFEE ^ depth);
            let boxed = gen_tree(depth as u32, &mut rng);
            let arena = Arena::from_expr(&boxed);
            let flat = Flat::from_arena(arena.clone());
            assert_eq!(arena.nodes.len() as u64, nodes_in(depth as u32));
            assert_all_agree(&boxed, &arena, &flat);
        }
    }

    #[test]
    fn representations_agree_after_edits() {
        let mut rng = Lcg::new(0xBEEF_F00D);
        let mut boxed = gen_tree(8, &mut rng);
        let mut arena = Arena::from_expr(&boxed);
        let mut flat = Flat::from_arena(arena.clone());

        // Replay identical edits against all three and re-check agreement each time.
        let edits = make_edits(50, 8, &mut rng);
        for (steps, new) in &edits {
            mutate_boxed(&mut boxed, steps, new);
            arena.mutate(steps, new);
            flat.mutate(steps, new);
            assert_all_agree(&boxed, &arena, &flat);
        }
    }

    #[test]
    fn edit_at_root_replaces_whole_tree() {
        let mut rng = Lcg::new(7);
        let mut arena = Arena::from_expr(&gen_tree(4, &mut rng));
        let replacement = Expr::Const(0.875);
        arena.mutate(&[], &replacement); // empty path = replace root
        assert_eq!(arena.eval(123.0), 0.875);
    }

    #[test]
    fn flat_stack_is_reusable() {
        // Evaluating twice with the same reused stack must give the same answer:
        // catches a missing clear() or a leftover value on the stack.
        let mut rng = Lcg::new(99);
        let flat = Flat::from_arena(Arena::from_expr(&gen_tree(6, &mut rng)));
        let mut stack = Vec::new();
        let first = flat.eval(0.9, &mut stack);
        let again = flat.eval(0.9, &mut stack);
        assert_eq!(first.to_bits(), again.to_bits());
    }
}
