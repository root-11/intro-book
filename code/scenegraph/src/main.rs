//! Reference for Part II - "Where SoA does not pay", project B: the scenegraph.
//!
//! A scenegraph is a tree of nodes, each with a *local* transform; a node's *world*
//! transform is its parent's world transform composed with its local one. Every frame
//! something moves, and the world transforms below it go stale. The question this
//! crate measures is the one project A's cliffhanger raised: when only part of the
//! tree changed, is it cheaper to recompute *only the dirty subtrees* or to just
//! recompute *everything*? The answer is a crossover in the dirty fraction, not a rule.
//!
//! Two ideas from project A carry forward and get re-measured here:
//!
//!   - Layout decides whether the full sweep is even cheap. We lay the tree out flat in
//!     DFS pre-order, so every parent sits at a lower index than its children and a
//!     subtree is a *contiguous* index range. The full repropagate is then one
//!     sequential pass - each node reads a parent world transform already computed.
//!     The pointer-tree version (recursive, scattered allocations) is the A baseline:
//!     same work, worse layout.
//!   - SoA is not automatic even here. A transform is a 6-field record always touched
//!     as a unit (you read all of the parent's world to write all of a child's), so the
//!     `Affine` struct - an AoS row - is the right grain. Splitting it into six columns
//!     would buy nothing. This is the arc's point: columns are a default, not a law.
//!
//! Run:    cargo run --release
//! Tests:  cargo test --release

use std::hint::black_box;
use std::time::Instant;

// ============================================================================
// Deterministic RNG - Numerical Recipes LCG, the same one code/deck uses.
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
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// ============================================================================
// The transform: a 2D affine matrix [a b c; d e f; 0 0 1]. Compose = matrix
// multiply. Local transforms are kept near identity so that composition down a
// tree stays finite and the three propagation paths agree bit-for-bit.
// ============================================================================

#[derive(Clone, Copy, PartialEq)]
struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

const IDENTITY: Affine = Affine {
    a: 1.0,
    b: 0.0,
    c: 0.0,
    d: 0.0,
    e: 1.0,
    f: 0.0,
};

/// `parent` applied to `local`: the child's world transform. The composition order
/// is fixed and identical in every propagation path, so the results match bit-for-bit.
#[inline]
fn compose(p: Affine, l: Affine) -> Affine {
    Affine {
        a: p.a * l.a + p.b * l.d,
        b: p.a * l.b + p.b * l.e,
        c: p.a * l.c + p.b * l.f + p.c,
        d: p.d * l.a + p.e * l.d,
        e: p.d * l.b + p.e * l.e,
        f: p.d * l.c + p.e * l.f + p.f,
    }
}

/// A near-identity local transform: small rotation, scale within 1%, small
/// translation. Bounded so a deep chain of compositions never blows up to inf/NaN.
fn rand_local(rng: &mut Lcg) -> Affine {
    let angle = (rng.unit() - 0.5) * 0.1;
    let scale = 0.99 + 0.02 * rng.unit();
    let (sin, cos) = angle.sin_cos();
    Affine {
        a: scale * cos,
        b: -scale * sin,
        c: (rng.unit() - 0.5) * 0.2,
        d: scale * sin,
        e: scale * cos,
        f: (rng.unit() - 0.5) * 0.2,
    }
}

// ============================================================================
// The flat scenegraph: pre-order layout. parent[i] < i for every non-root node,
// so index order is a valid parent-before-child (topological) order, and the
// subtree rooted at i is exactly the contiguous range [i, i + subtree[i]).
// ============================================================================

struct Scene {
    parent: Vec<u32>,   // parent[0] = u32::MAX (the root)
    subtree: Vec<u32>,  // subtree[i] = number of nodes in i's subtree, i included
    local: Vec<Affine>, // the editable per-node transform
    world: Vec<Affine>, // the derived per-node transform (what we recompute)
}

const NO_PARENT: u32 = u32::MAX;

impl Scene {
    fn len(&self) -> usize {
        self.parent.len()
    }

    /// Grow a random tree of about `target` nodes in pre-order. Fan-out 0..=4 (many
    /// leaves, like a real scene full of sprites), depth capped so recursion is safe.
    fn generate(target: usize, rng: &mut Lcg) -> Self {
        let mut parent = Vec::with_capacity(target);
        let mut subtree = Vec::with_capacity(target);
        grow(NO_PARENT, 0, &mut parent, &mut subtree, target, rng);
        let local: Vec<Affine> = (0..parent.len()).map(|_| rand_local(rng)).collect();
        let world = vec![IDENTITY; parent.len()];
        Scene {
            parent,
            subtree,
            local,
            world,
        }
    }
}

/// Push one node (pre-order: self first, then each child's whole subtree) and return
/// this node's subtree size. The slot for our subtree count is reserved before the
/// children recurse and filled in afterwards.
fn grow(
    parent_idx: u32,
    depth: u32,
    parent: &mut Vec<u32>,
    subtree: &mut Vec<u32>,
    target: usize,
    rng: &mut Lcg,
) -> u32 {
    const MAX_DEPTH: u32 = 24;
    let me = parent.len() as u32;
    parent.push(parent_idx);
    subtree.push(0); // placeholder, filled after the children are grown
    let mut size = 1u32;
    if depth < MAX_DEPTH {
        let fan = rng.below(5); // 0..=4 children
        for _ in 0..fan {
            if parent.len() >= target {
                break;
            }
            size += grow(me, depth + 1, parent, subtree, target, rng);
        }
    }
    subtree[me as usize] = size;
    size
}

/// Full repropagate: recompute every world transform in one sequential pass. The root
/// is index 0; every other node's parent is at a strictly lower index and is already
/// done, so the inner loop is branchless and streams forward through memory.
#[inline(never)]
fn propagate_full(world: &mut [Affine], local: &[Affine], parent: &[u32]) {
    world[0] = local[0]; // root: compose(IDENTITY, local[0]) == local[0]
    for i in 1..world.len() {
        world[i] = compose(world[parent[i] as usize], local[i]);
    }
}

/// Incremental repropagate: recompute only the dirty nodes. `dirty` is the sorted list
/// of every index in the changed subtrees (a real engine maintains this as objects
/// move; here it is precomputed, so this times the recompute, not the marking).
/// Ascending order is what makes it correct: a dirty node's parent is either clean -
/// its world is still valid from the last full frame - or dirty and earlier in the
/// list, hence already recomputed this frame.
#[inline(never)]
fn propagate_incremental(world: &mut [Affine], local: &[Affine], parent: &[u32], dirty: &[u32]) {
    for &i in dirty {
        let p = parent[i as usize];
        world[i as usize] = if p == NO_PARENT {
            local[i as usize]
        } else {
            compose(world[p as usize], local[i as usize])
        };
    }
}

// ============================================================================
// The pointer-tree baseline (project A's "scattered" layout, on a scenegraph).
// Same composition, same order; only the memory layout differs.
// ============================================================================

struct SNode {
    // `idx` and `collect_pointer` are read only by the equality test, which maps each
    // pointer node back to its flat slot; the benchmark just propagates in place.
    #[allow(dead_code)]
    idx: u32,
    local: Affine,
    world: Affine,
    // The Box is deliberate, not redundant: it puts every node in its own heap
    // allocation so the tree is genuinely scattered, the same scattered baseline
    // project A's `Box<Expr>` set up. `Vec<SNode>` would pack children contiguously
    // and quietly hand the pointer tree the locality we are trying to deny it.
    #[allow(clippy::vec_box)]
    children: Vec<Box<SNode>>,
}

/// Rebuild the pre-order flat tree as a pointer tree. The first child of `i` is `i+1`;
/// its next sibling is `i+1 + subtree[i+1]`, and so on until the subtree range ends.
fn to_pointer_tree(i: u32, local: &[Affine], subtree: &[u32]) -> Box<SNode> {
    let mut node = Box::new(SNode {
        idx: i,
        local: local[i as usize],
        world: IDENTITY,
        children: Vec::new(),
    });
    let end = i + subtree[i as usize];
    let mut c = i + 1;
    while c < end {
        node.children.push(to_pointer_tree(c, local, subtree));
        c += subtree[c as usize];
    }
    node
}

#[inline(never)]
fn propagate_pointer(node: &mut SNode, parent_world: Affine) {
    node.world = compose(parent_world, node.local);
    for child in &mut node.children {
        propagate_pointer(child, node.world);
    }
}

#[allow(dead_code)]
fn collect_pointer(node: &SNode, out: &mut [Affine]) {
    out[node.idx as usize] = node.world;
    for child in &node.children {
        collect_pointer(child, out);
    }
}

// ============================================================================
// Building dirty sets of a target fraction.
// ============================================================================

/// Mark random movers' subtrees until at least `target` of the nodes are covered, and
/// return the affected indices in ascending order. Each mover dirties a contiguous
/// subtree range, so the set is a realistic mix of contiguous runs.
fn dirty_fraction(scene: &Scene, target: f64, rng: &mut Lcg) -> Vec<u32> {
    let n = scene.len();
    if target >= 1.0 {
        return (0..n as u32).collect();
    }
    let want = (target * n as f64) as usize;
    let mut mask = vec![false; n];
    let mut covered = 0usize;
    let mut attempts = 0usize;
    while covered < want && attempts < 16 * n {
        attempts += 1;
        let m = rng.below(n as u64) as usize;
        let s = scene.subtree[m] as usize;
        // Skip movers whose subtree would overshoot the target. Subtree sizes span
        // from 1 (a leaf) to N (the root), so without this gate one unlucky pick near
        // the root blows a 1% target up to 16%. Capping at the remaining budget keeps
        // the achieved fraction tracking the target, and naturally shifts to leaves as
        // the budget runs out.
        if s > want - covered {
            continue;
        }
        for slot in &mut mask[m..m + s] {
            if !*slot {
                *slot = true;
                covered += 1;
            }
        }
    }
    (0..n as u32).filter(|&i| mask[i as usize]).collect()
}

/// A single subtree of about `target` of the nodes: one contiguous dirty range.
fn dirty_contiguous(scene: &Scene, target: f64) -> Vec<u32> {
    let n = scene.len();
    let want = (target * n as f64) as u32;
    // Pick the node whose subtree is closest to the target size.
    let mut best = 0u32;
    let mut best_err = u32::MAX;
    for i in 0..n as u32 {
        let s = scene.subtree[i as usize];
        let err = s.abs_diff(want);
        if err < best_err {
            best_err = err;
            best = i;
        }
    }
    (best..best + scene.subtree[best as usize]).collect()
}

/// `count` scattered single leaves: maximally bad locality at a given dirty count.
fn dirty_scattered(scene: &Scene, count: usize, rng: &mut Lcg) -> Vec<u32> {
    let n = scene.len();
    let mut mask = vec![false; n];
    let mut have = 0usize;
    let mut attempts = 0usize;
    while have < count && attempts < 8 * n {
        attempts += 1;
        let i = rng.below(n as u64) as usize;
        if scene.subtree[i] == 1 && !mask[i] {
            mask[i] = true;
            have += 1;
        }
    }
    (0..n as u32).filter(|&i| mask[i as usize]).collect()
}

// ============================================================================
// Benchmark plumbing.
// ============================================================================

fn median5(mut sample: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..5).map(|_| sample()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[2]
}

fn bench_full(scene: &mut Scene, iters: u64) -> f64 {
    let t = Instant::now();
    for _ in 0..iters {
        propagate_full(&mut scene.world, &scene.local, &scene.parent);
        black_box(scene.world[scene.len() - 1]);
    }
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn bench_incremental(scene: &mut Scene, dirty: &[u32], iters: u64) -> f64 {
    // Start from a fully consistent world so the dirty recompute is meaningful.
    propagate_full(&mut scene.world, &scene.local, &scene.parent);
    let t = Instant::now();
    for _ in 0..iters {
        propagate_incremental(
            &mut scene.world,
            &scene.local,
            &scene.parent,
            black_box(dirty),
        );
        black_box(scene.world[*dirty.last().unwrap() as usize]);
    }
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn bench_pointer(root: &mut SNode, iters: u64) -> f64 {
    let t = Instant::now();
    for _ in 0..iters {
        propagate_pointer(black_box(root), IDENTITY);
        black_box(root.world);
    }
    t.elapsed().as_nanos() as f64 / iters as f64
}

fn iters_for(n: usize) -> u64 {
    (50_000_000 / n as u64).clamp(20, 20_000)
}

// ============================================================================
// main: layout baseline, then the dirty-fraction crossover, then locality.
// ============================================================================

fn main() {
    // ---- Layout baseline: full repropagate, flat-sequential vs pointer-recursive ----
    println!("== Full repropagate: flat (sequential) vs pointer tree (scattered) ==");
    println!("ns per frame, median of 5; the work is identical, only the layout differs.\n");
    println!(
        "{:>12} {:>14} {:>14}   flat speedup",
        "nodes", "flat", "pointer"
    );

    for &target in &[1_000usize, 100_000, 1_000_000] {
        let mut rng = Lcg::new(0x5CE7E_u64 ^ target as u64);
        let mut scene = Scene::generate(target, &mut rng);
        let n = scene.len();
        let iters = iters_for(n);

        let flat = median5(|| bench_full(&mut scene, iters));

        let mut root = to_pointer_tree(0, &scene.local, &scene.subtree);
        let ptr = median5(|| bench_pointer(&mut root, iters));

        println!(
            "{:>12} {:>14.0} {:>14.0}   {:>5.2}x",
            n,
            flat,
            ptr,
            ptr / flat
        );
    }

    // ---- The crossover: incremental (dirty subtrees) vs full, swept over dirty fraction ----
    let mut rng = Lcg::new(0xD1287_u64);
    let mut scene = Scene::generate(1_000_000, &mut rng);
    let n = scene.len();
    let iters = iters_for(n);
    let full = median5(|| bench_full(&mut scene, iters));

    println!("\n== Dirty-fraction crossover (incremental vs full) ==");
    println!("scene: {n} nodes; full repropagate = {full:.0} ns/frame (flat).\n");
    println!(
        "{:>8} {:>12} {:>14} {:>10}",
        "target", "dirty", "incr ns", "vs full"
    );

    let fractions = [0.001, 0.005, 0.01, 0.05, 0.1, 0.2, 0.4, 0.6, 0.8, 1.0];
    let mut crossed = None;
    let mut prev_ratio = f64::INFINITY;
    for &target in &fractions {
        let dirty = dirty_fraction(&scene, target, &mut rng);
        let actual = dirty.len() as f64 / n as f64;
        let incr = median5(|| bench_incremental(&mut scene, &dirty, iters));
        let ratio = full / incr; // > 1 means incremental still wins
        println!(
            "{:>8.3} {:>11.1}% {:>14.0} {:>9.2}x",
            target,
            actual * 100.0,
            incr,
            ratio
        );
        if crossed.is_none() && prev_ratio > 1.0 && ratio <= 1.0 {
            crossed = Some(actual);
        }
        prev_ratio = ratio;
    }
    match crossed {
        Some(f) => println!(
            "\nCrossover near dirty fraction {:.0}%: above it, recompute-everything wins.",
            f * 100.0
        ),
        None => println!(
            "\nNo crossover in range: incremental still wins even at ~100% dirty on this box."
        ),
    }

    // ---- Locality: same dirty count, one contiguous subtree vs scattered leaves ----
    let target_f = 0.1;
    let contiguous = dirty_contiguous(&scene, target_f);
    let scattered = dirty_scattered(&scene, contiguous.len(), &mut rng);
    let c_ns = median5(|| bench_incremental(&mut scene, &contiguous, iters));
    let s_ns = median5(|| bench_incremental(&mut scene, &scattered, iters));
    println!(
        "\n== Locality of the dirty set (same count, ~{:.0}% of nodes) ==",
        target_f * 100.0
    );
    println!(
        "contiguous subtree ({} nodes): {:.0} ns",
        contiguous.len(),
        c_ns
    );
    println!(
        "scattered leaves  ({} nodes): {:.0} ns   ({:.2}x slower)",
        scattered.len(),
        s_ns,
        s_ns / c_ns
    );
    println!(
        "\nSame work, different locality: a compact dirty subtree streams; scattered leaves miss cache."
    );
}

// ============================================================================
// Contract tests. The three propagation paths must agree, bit for bit, and the
// contiguous-subtree assumption the whole flat design rests on must hold.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn worlds_equal(a: &[Affine], b: &[Affine]) {
        assert_eq!(a.len(), b.len());
        for i in 0..a.len() {
            assert_eq!(a[i].a.to_bits(), b[i].a.to_bits(), "node {i} field a");
            assert_eq!(a[i].c.to_bits(), b[i].c.to_bits(), "node {i} field c");
            assert_eq!(a[i].f.to_bits(), b[i].f.to_bits(), "node {i} field f");
        }
    }

    #[test]
    fn flat_and_pointer_agree() {
        for seed in 0..8u64 {
            let mut rng = Lcg::new(0xABC ^ seed);
            let mut scene = Scene::generate(5_000, &mut rng);
            propagate_full(&mut scene.world, &scene.local, &scene.parent);

            let mut root = to_pointer_tree(0, &scene.local, &scene.subtree);
            propagate_pointer(&mut root, IDENTITY);
            let mut from_pointer = vec![IDENTITY; scene.len()];
            collect_pointer(&root, &mut from_pointer);

            worlds_equal(&scene.world, &from_pointer);
        }
    }

    #[test]
    fn incremental_over_all_equals_full() {
        let mut rng = Lcg::new(0xF00D);
        let mut scene = Scene::generate(20_000, &mut rng);
        propagate_full(&mut scene.world, &scene.local, &scene.parent);
        let full = scene.world.clone();

        // Wipe world, then recompute every node incrementally in index order.
        scene.world.iter_mut().for_each(|w| *w = IDENTITY);
        let all: Vec<u32> = (0..scene.len() as u32).collect();
        propagate_incremental(&mut scene.world, &scene.local, &scene.parent, &all);
        worlds_equal(&scene.world, &full);
    }

    #[test]
    fn incremental_subset_matches_full_recompute() {
        let mut rng = Lcg::new(0x1357);
        let mut scene = Scene::generate(20_000, &mut rng);
        propagate_full(&mut scene.world, &scene.local, &scene.parent);

        // Move a subset of nodes (change their local), mark the affected subtrees.
        let movers = [3u32, 100, 5000, 12345];
        let mut mask = vec![false; scene.len()];
        for &m in &movers {
            scene.local[m as usize] = rand_local(&mut rng);
            let end = m + scene.subtree[m as usize];
            for j in m..end {
                mask[j as usize] = true;
            }
        }
        let dirty: Vec<u32> = (0..scene.len() as u32)
            .filter(|&i| mask[i as usize])
            .collect();
        propagate_incremental(&mut scene.world, &scene.local, &scene.parent, &dirty);
        let incremental = scene.world.clone();

        // A from-scratch full recompute must agree everywhere.
        propagate_full(&mut scene.world, &scene.local, &scene.parent);
        worlds_equal(&scene.world, &incremental);
    }

    #[test]
    fn subtree_ranges_are_contiguous() {
        // The flat design's load-bearing claim: a subtree is a contiguous index range.
        // Every index in [m, m+subtree[m]) must have m on its ancestor chain, and the
        // node just past the range must not.
        let mut rng = Lcg::new(0x2468);
        let scene = Scene::generate(10_000, &mut rng);
        let has_ancestor = |mut node: u32, anc: u32| -> bool {
            while node != NO_PARENT {
                if node == anc {
                    return true;
                }
                node = scene.parent[node as usize];
            }
            false
        };
        for m in [0u32, 1, 50, 500, 4000] {
            let end = m + scene.subtree[m as usize];
            for j in m..end {
                assert!(has_ancestor(j, m), "{j} should be in subtree of {m}");
            }
            if (end as usize) < scene.len() {
                assert!(
                    !has_ancestor(end, m),
                    "{end} should not be in subtree of {m}"
                );
            }
        }
    }
}
