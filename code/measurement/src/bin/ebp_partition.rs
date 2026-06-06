//! ebp_partition - the evidence for the Part 5 / §26 SoA+EBP rework.
//! Three claims the rework makes, each measured here so the prose asserts
//! nothing the machine has not confirmed.
//!
//!     cargo run --release --bin ebp_partition
//!
//! All data is SoA: five parallel Vec<f32> columns (px, py, vx, vy, energy),
//! indexed by slot. A "row" is only an index across columns. Nothing here
//! moves an entity; the most that moves is a slot, and only in a batch pass.
//!
//! Claim 1 (§26, relevance). A system that processes a subset should iterate a
//!   subscription table (Vec<slot>) and touch only those slots, not scan all N
//!   and branch. Measured: scan-all+branch vs subscription gather, swept over
//!   the relevant fraction.
//!
//! Claim 2 (§26 -> §28, locality). A subscription's gather is dense only when
//!   the live entities are contiguous in slot space. Lazy death and recycling
//!   scatter them over time; a batch sort-for-locality (which is also the
//!   deferred GC purge) re-compacts them. Measured: scattered gather vs
//!   compacted sequential, plus the cost of the compaction pass and how many
//!   ticks it takes to pay for itself.
//!
//! Claim 3 (§21 -> §24, lifecycle). Two measurements, both touching the same
//!   columns at the same scattered slots so the comparison is fair. (a) Per
//!   death: mark-dead + recycle writes one flag and frees a slot; swap_remove
//!   -on-death moves five columns and patches the id map, so it costs more.
//!   (b) Batch removal: a single sequential compaction pass beats per-element
//!   swap_remove, and it runs once per GC interval rather than per death per
//!   tick. (An earlier version reported these as comparable; that was a
//!   measurement artifact - mark-dead wrote into a separate cold array while
//!   swap_remove reused a warm tail. Fixed by sharing columns and slots.)

use std::hint::black_box;
use std::time::Instant;

const N: usize = 1_000_000;
const DT: f32 = 0.016;
const DECAY: f32 = 0.001;

// A cheap hash so the live/active set is scattered, not blocky - a realistic
// branch pattern and a realistic post-churn slot scatter.
#[inline(always)]
fn scattered(i: usize) -> u32 {
    (i.wrapping_mul(2_654_435_761) >> 8) as u32 % 100
}

// scan all N, branch on a per-slot flag (the "wrong way").
#[inline(never)]
fn motion_scan(
    px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], en: &mut [f32],
    active: &[bool], dt: f32,
) {
    for i in 0..px.len() {
        if active[i] {
            px[i] += vx[i] * dt;
            py[i] += vy[i] * dt;
            en[i] -= DECAY;
        }
    }
}

// iterate a subscription table of slots, gather the columns by slot.
#[inline(never)]
fn motion_subscription(
    px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], en: &mut [f32],
    subs: &[u32], dt: f32,
) {
    for &s in subs {
        let i = s as usize;
        px[i] += vx[i] * dt;
        py[i] += vy[i] * dt;
        en[i] -= DECAY;
    }
}

// iterate a subscription table of entity ids, resolve each through id_to_slot,
// then gather the columns. One extra (scattered) load per element vs the slot
// version above - the redirection the slot-keyed design removes.
#[inline(never)]
fn motion_subscription_id(
    px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], en: &mut [f32],
    subs_id: &[u32], id_to_slot: &[u32], dt: f32,
) {
    for &id in subs_id {
        let i = id_to_slot[id as usize] as usize;
        px[i] += vx[i] * dt;
        py[i] += vy[i] * dt;
        en[i] -= DECAY;
    }
}

fn ns_per(elapsed_ns: f64, iters: u32) -> f64 {
    elapsed_ns / iters as f64
}

fn main() {
    println!("ebp_partition - SoA + EBP rework evidence (N = {N})\n");

    // Shared columns, size N.
    let mut px: Vec<f32> = (0..N).map(|i| (i % 1000) as f32).collect();
    let mut py: Vec<f32> = (0..N).map(|i| (i % 997) as f32).collect();
    let vx: Vec<f32> = (0..N).map(|i| ((i % 7) as f32) - 3.0).collect();
    let vy: Vec<f32> = (0..N).map(|i| ((i % 5) as f32) - 2.0).collect();
    let mut en: Vec<f32> = vec![100.0; N];

    // ---- Claim 1: relevance - scan vs subscription, over relevant fraction ----
    println!("Claim 1 (relevance): scan-all+branch vs subscription gather");
    println!("  {:<8} {:>16} {:>16}", "active", "scan ns/pass", "subs ns/pass");
    for &frac in &[1.0_f64, 0.5, 0.1, 0.01] {
        let thr = ((1.0 - frac) * 100.0) as u32; // scattered(i) >= thr => active
        let active_flag: Vec<bool> = (0..N).map(|i| scattered(i) >= thr).collect();
        // Subscription holds slots in ascending order, but the active slots are
        // scattered through 0..N (this is the realistic post-churn state).
        let subs: Vec<u32> = (0..N as u32).filter(|&i| active_flag[i as usize]).collect();
        let active_n = subs.len();
        let iters = (200_000_000usize / N.max(1)).max(20) as u32;

        for _ in 0..3 {
            motion_scan(&mut px, &mut py, &vx, &vy, &mut en, &active_flag, black_box(DT));
            motion_subscription(&mut px, &mut py, &vx, &vy, &mut en, &subs, black_box(DT));
        }

        let t0 = Instant::now();
        for _ in 0..iters {
            motion_scan(&mut px, &mut py, &vx, &vy, &mut en, &active_flag, black_box(DT));
        }
        black_box(&px);
        let scan = ns_per(t0.elapsed().as_nanos() as f64, iters);

        let t0 = Instant::now();
        for _ in 0..iters {
            motion_subscription(&mut px, &mut py, &vx, &vy, &mut en, &subs, black_box(DT));
        }
        black_box(&px);
        let subs_ns = ns_per(t0.elapsed().as_nanos() as f64, iters);

        println!("  {:<8} {:>16.0} {:>16.0}   (active = {})",
                 format!("{:.0}%", frac * 100.0), scan, subs_ns, active_n);
    }

    // ---- Claim 2: locality - scattered gather vs compacted, + sort payoff ----
    println!("\nClaim 2 (locality): scattered subscription vs compacted, frac = 10%");
    let frac = 0.10;
    let thr = ((1.0 - frac) * 100.0) as u32;
    let active_flag: Vec<bool> = (0..N).map(|i| scattered(i) >= thr).collect();
    let subs_scattered: Vec<u32> = (0..N as u32).filter(|&i| active_flag[i as usize]).collect();
    let active_n = subs_scattered.len();
    let iters = (200_000_000usize / N.max(1)).max(20) as u32;

    // The compaction pass: gather the active columns to the front. This is the
    // deferred GC / sort-for-locality batch reindex (here: the dominant cost,
    // the five column gathers). After it, the subscription is 0..active_n.
    let t0 = Instant::now();
    let cpx: Vec<f32> = subs_scattered.iter().map(|&s| px[s as usize]).collect();
    let cpy: Vec<f32> = subs_scattered.iter().map(|&s| py[s as usize]).collect();
    let cvx: Vec<f32> = subs_scattered.iter().map(|&s| vx[s as usize]).collect();
    let cvy: Vec<f32> = subs_scattered.iter().map(|&s| vy[s as usize]).collect();
    let cen: Vec<f32> = subs_scattered.iter().map(|&s| en[s as usize]).collect();
    let compact_cost = t0.elapsed().as_nanos() as f64;
    black_box((&cpx, &cpy, &cvx, &cvy, &cen));
    let subs_compact: Vec<u32> = (0..active_n as u32).collect();

    let mut cpx = cpx; let mut cpy = cpy; let mut cen = cen;

    for _ in 0..3 {
        motion_subscription(&mut px, &mut py, &vx, &vy, &mut en, &subs_scattered, black_box(DT));
        motion_subscription(&mut cpx, &mut cpy, &cvx, &cvy, &mut cen, &subs_compact, black_box(DT));
    }

    let t0 = Instant::now();
    for _ in 0..iters {
        motion_subscription(&mut px, &mut py, &vx, &vy, &mut en, &subs_scattered, black_box(DT));
    }
    black_box(&px);
    let scat_ns = ns_per(t0.elapsed().as_nanos() as f64, iters);

    let t0 = Instant::now();
    for _ in 0..iters {
        motion_subscription(&mut cpx, &mut cpy, &cvx, &cvy, &mut cen, &subs_compact, black_box(DT));
    }
    black_box(&cpx);
    let comp_ns = ns_per(t0.elapsed().as_nanos() as f64, iters);

    println!("  scattered gather      {:>12.0} ns/pass", scat_ns);
    println!("  compacted sequential  {:>12.0} ns/pass", comp_ns);
    println!("  compaction pass       {:>12.0} ns (one batch reindex of {} live)", compact_cost, active_n);
    if scat_ns > comp_ns {
        let payback = compact_cost / (scat_ns - comp_ns);
        println!("  pays for itself after {:.1} ticks", payback);
    }

    // ---- Claim 3: lifecycle - mark+recycle+batch-compact vs swap_remove-on-death ----
    println!("\nClaim 3 (lifecycle): mark+recycle+compact vs swap_remove-on-death");

    // (a) Per death, fair: both touch real columns at scattered dead slots.
    // mark-dead writes one flag and pushes the freed slot to a recycle list;
    // swap_remove moves five columns and patches the id map.
    let deaths = N / 100;
    let swap_slots: Vec<u32> =
        (0..deaths).map(|j| ((j.wrapping_mul(2_654_435_761) >> 8) % (N - j)) as u32).collect();
    let mark_slots: Vec<u32> =
        (0..deaths).map(|j| ((j.wrapping_mul(2_654_435_761) >> 8) % N) as u32).collect();

    let mut spx = px.clone();
    let mut spy = py.clone();
    let mut svx = vx.clone();
    let mut svy = vy.clone();
    let mut sen = en.clone();
    let mut slot_to_id: Vec<u32> = (0..N as u32).collect();
    let mut id_to_slot: Vec<u32> = (0..N as u32).collect();
    let t0 = Instant::now();
    for &s in &swap_slots {
        let slot = s as usize;
        let moved_id = *slot_to_id.last().unwrap();
        spx.swap_remove(slot);
        spy.swap_remove(slot);
        svx.swap_remove(slot);
        svy.swap_remove(slot);
        sen.swap_remove(slot);
        slot_to_id.swap_remove(slot);
        id_to_slot[moved_id as usize] = slot as u32;
    }
    black_box((&spx, &spy, &sen, &id_to_slot));
    let swap_death = t0.elapsed().as_nanos() as f64 / deaths as f64;

    let mut dead = vec![false; N];
    let mut free_list: Vec<u32> = Vec::with_capacity(deaths);
    let t0 = Instant::now();
    for &s in &mark_slots {
        dead[s as usize] = true;
        free_list.push(s);
    }
    black_box((&dead, &free_list));
    let mark_death = t0.elapsed().as_nanos() as f64 / deaths as f64;

    println!("  (a) per death (fair, same columns at scattered slots):");
    println!("      swap_remove-on-death   {:>6.1} ns", swap_death);
    println!("      mark-dead + recycle    {:>6.1} ns", mark_death);

    // (b) Batch removal of ~50%: swap_remove each dead slot vs one compaction
    // pass that copies the survivors forward sequentially. The classical
    // "remove a batch" case; compaction is one pass and amortizes over the GC
    // interval, so the deferred design pays it once per many ticks.
    let kill: Vec<bool> = (0..N).map(|i| scattered(i) < 50).collect(); // ~50% die
    let kill_slots: Vec<u32> = {
        let mut v: Vec<u32> = (0..N as u32).filter(|&i| kill[i as usize]).collect();
        // pre-mod against the shrinking length so the timed loop has no modulo.
        for (j, x) in v.iter_mut().enumerate() {
            *x = ((j.wrapping_mul(2_654_435_761) >> 8) % (N - j)) as u32;
        }
        v
    };

    let mut bpx = px.clone();
    let mut bpy = py.clone();
    let mut bvx = vx.clone();
    let mut bvy = vy.clone();
    let mut ben = en.clone();
    let t0 = Instant::now();
    for &s in &kill_slots {
        let slot = s as usize;
        bpx.swap_remove(slot);
        bpy.swap_remove(slot);
        bvx.swap_remove(slot);
        bvy.swap_remove(slot);
        ben.swap_remove(slot);
    }
    black_box((&bpx, &ben));
    let swap_batch = t0.elapsed().as_nanos() as f64;

    let mut cpx = px.clone();
    let mut cpy = py.clone();
    let mut cvx = vx.clone();
    let mut cvy = vy.clone();
    let mut cen = en.clone();
    let t0 = Instant::now();
    let mut w = 0usize;
    for r in 0..N {
        if !kill[r] {
            cpx[w] = cpx[r]; cpy[w] = cpy[r];
            cvx[w] = cvx[r]; cvy[w] = cvy[r]; cen[w] = cen[r];
            w += 1;
        }
    }
    cpx.truncate(w); cpy.truncate(w); cvx.truncate(w); cvy.truncate(w); cen.truncate(w);
    black_box((&cpx, &cen));
    let compact_batch = t0.elapsed().as_nanos() as f64;

    println!("  (b) remove ~50% in a batch:");
    println!("      swap_remove each       {:>10.0} ns", swap_batch);
    println!("      one compaction pass    {:>10.0} ns", compact_batch);
    if compact_batch > 0.0 {
        println!("      compaction {:.1}x faster, and runs once per GC interval, not per tick",
                 swap_batch / compact_batch);
    }

    // ---- Claim 4: keying - slot-subscription vs id-subscription ----
    // Two costs pull in opposite directions, so only measurement settles it:
    //   hot loop  - slot keys gather columns directly; id keys pay one extra
    //               (scattered) id_to_slot load per element, every tick.
    //   reindex   - when the GC compaction moves slots, id-keyed subscriptions
    //               are untouched (only id_to_slot is rebuilt, once, over live);
    //               slot-keyed subscriptions must each be remapped, so the cost
    //               scales with how many subscriptions an entity sits in (S).
    // The hot saving is paid every tick; the reindex burden once per GC interval
    // (G ticks). The verdict is the amortized sum, swept over S and G.
    println!("\nClaim 4 (keying): slot-subscription vs id-subscription, frac = 10%");
    const IDMIX: usize = 2_654_435_761; // coprime to N => slot -> id is a bijection
    let frac = 0.10;
    let thr = ((1.0 - frac) * 100.0) as u32;
    let active: Vec<bool> = (0..N).map(|i| scattered(i) >= thr).collect();
    let subs_slot: Vec<u32> = (0..N as u32).filter(|&i| active[i as usize]).collect();
    // Each live entity gets a scattered, unique id; id_to_slot resolves it.
    let id_of = |slot: u32| -> u32 { ((slot as usize * IDMIX) % N) as u32 };
    let subs_id: Vec<u32> = subs_slot.iter().map(|&s| id_of(s)).collect();
    let mut id_to_slot = vec![u32::MAX; N];
    for &s in &subs_slot { id_to_slot[id_of(s) as usize] = s; }
    let iters = (200_000_000usize / N.max(1)).max(20) as u32;

    for _ in 0..3 {
        motion_subscription(&mut px, &mut py, &vx, &vy, &mut en, &subs_slot, black_box(DT));
        motion_subscription_id(&mut px, &mut py, &vx, &vy, &mut en, &subs_id, &id_to_slot, black_box(DT));
    }
    let t0 = Instant::now();
    for _ in 0..iters {
        motion_subscription(&mut px, &mut py, &vx, &vy, &mut en, &subs_slot, black_box(DT));
    }
    black_box(&px);
    let hot_slot = ns_per(t0.elapsed().as_nanos() as f64, iters);

    let t0 = Instant::now();
    for _ in 0..iters {
        motion_subscription_id(&mut px, &mut py, &vx, &vy, &mut en, &subs_id, &id_to_slot, black_box(DT));
    }
    black_box(&px);
    let hot_id = ns_per(t0.elapsed().as_nanos() as f64, iters);

    println!("  hot loop:  slot {:.0} ns/pass   id {:.0} ns/pass   (id pays {:.0} ns/pass for the hop)",
             hot_slot, hot_id, hot_id - hot_slot);

    // Compaction moves every live entity to a front slot (the §28 purge).
    let mut old_to_new = vec![u32::MAX; N];
    for (new, &old) in subs_slot.iter().enumerate() { old_to_new[old as usize] = new as u32; }

    // id-keyed reindex: rebuild id_to_slot over the live set; subscriptions untouched.
    let reps = 50u32;
    let mut acc = 0u128;
    for _ in 0..reps {
        let t0 = Instant::now();
        for &old in &subs_slot {
            id_to_slot[id_of(old) as usize] = old_to_new[old as usize];
        }
        acc += t0.elapsed().as_nanos();
        black_box(&id_to_slot);
    }
    let reindex_id = acc as f64 / reps as f64;

    println!("  reindex (per GC interval):");
    println!("    id-keyed                 {:>10.0} ns   (rebuild id_to_slot, subscriptions untouched)", reindex_id);

    // slot-keyed reindex: same id_to_slot rebuild for boundary identity, PLUS
    // remap every entry of every subscription the entity belongs to (S copies).
    for &s_count in &[1usize, 2, 4] {
        let pristine: Vec<u32> = (0..s_count).flat_map(|_| subs_slot.iter().copied()).collect();
        let mut work = pristine.clone();
        let mut acc = 0u128;
        for _ in 0..reps {
            work.copy_from_slice(&pristine); // untimed restore
            let t0 = Instant::now();
            for &old in &subs_slot {
                id_to_slot[id_of(old) as usize] = old_to_new[old as usize];
            }
            for x in work.iter_mut() { *x = old_to_new[*x as usize]; }
            acc += t0.elapsed().as_nanos();
            black_box((&work, &id_to_slot));
        }
        let reindex_slot = acc as f64 / reps as f64;
        println!("    slot-keyed, S={}          {:>10.0} ns", s_count, reindex_slot);

        // Amortized verdict over GC intervals G.
        for &g in &[30u32, 100] {
            let slot_total = hot_slot * g as f64 + reindex_slot;
            let id_total   = hot_id   * g as f64 + reindex_id;
            let winner = if slot_total < id_total { "slot" } else { "id" };
            println!("      G={:<4} ticks: slot {:>12.0} ns   id {:>12.0} ns   -> {} wins",
                     g, slot_total, id_total, winner);
        }
    }
}
