//! Through-line ecosystem simulator - UNIFIED ENTITY model, the Rust edition's reference sim.
//! Faithful port of the Python `sim1b.py`: same systems, same lifecycle, same arena.
//!
//! Everything that exists is an entity in ONE table; a *species* is nothing but a set of
//! subscriptions, held in a generic registry. `apply` maintains EVERY subscription the same way
//! and never learns what a grass or a grazer is - so adding a species (see the `sim2` bin's
//! predator) is registering a name and wiring two systems, not surgery on the join. The only
//! thing distinguishing a grass blade from a grazer is which motion system and which forage edge
//! hold its id; parameters live on the systems, not the entities.
//!
//! The tick (`step`) lives in each bin, not here: `sim` wires grass + grazers, `sim2` adds the
//! predator. The diff between the two bin files is the extendability lesson. The lib provides the
//! generic machinery: the arena, the registry, the systems, the join, the GC, run/replay.
//!
//! Internally deterministic (replay reconstructs bit-for-bit) but does NOT match Python's
//! trajectories: the PRNG and float order differ. The framerate curve is trajectory-independent.

use std::time::Instant;

pub const INVALID: u32 = u32::MAX;

#[derive(Clone)]
pub struct Config {
    pub world: f32,
    pub seed: u64,
    pub ticks: usize,
    pub dt: f32,
    pub cap: usize,
    pub gc_interval: usize,
    pub repro_threshold: f32,
    pub init_energy: f32,
    pub n0_grass: usize,
    pub n0_grazers: usize,
    pub grass_photosynthesis: f32,
    pub grass_drift: f32,
    pub grass_burn: f32,
    pub herd_speed: f32,
    pub herd_burn: f32,
    pub cohesion: f32,
    pub wander: f32,
    pub max_herd: usize,
    pub graze_radius: f32,
    pub graze_gain: f32,
    // predator trophic level (used only by the `sim2` bin; `sim` ignores these)
    pub n0_predators: usize,
    pub hunter_speed: f32,
    pub hunter_burn: f32,
    pub hunt_radius: f32,
    pub hunt_gain: f32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            world: 100.0,
            seed: 1,
            ticks: 600,
            dt: 1.0 / 30.0,
            cap: 200_000,
            gc_interval: 30,
            repro_threshold: 24.0,
            init_energy: 10.0,
            n0_grass: 1500,
            n0_grazers: 300,
            grass_photosynthesis: 6.0,
            grass_drift: 0.8,
            grass_burn: 0.0,
            herd_speed: 6.0,
            herd_burn: 4.0,
            cohesion: 0.15,
            wander: 0.3,
            max_herd: 400,
            graze_radius: 2.0,
            graze_gain: 8.0,
            n0_predators: 20,
            hunter_speed: 3.5,
            hunter_burn: 8.0,
            hunt_radius: 3.0,
            hunt_gain: 20.0,
        }
    }
}

/// Deterministic splitmix64 PRNG (std-only, from-scratch; no `rand` crate).
pub struct Rng {
    state: u64,
}
impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed.wrapping_add(0x9E37_79B9_7F4A_7C15) }
    }
    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    #[inline]
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0 // 2^53
    }
    /// Uniform f32 in [lo, hi).
    #[inline]
    pub fn uniform(&mut self, lo: f32, hi: f32) -> f32 {
        lo + self.unit() as f32 * (hi - lo)
    }
}

/// A subscription: a named id-set (§17/§19). The registry is ordered, for determinism.
pub struct Sub {
    pub name: &'static str,
    pub ids: Vec<u32>,
}

// ------------------------------------------------------------------------------
// World: one entity table + the subscription registry. Committed state only.
// ------------------------------------------------------------------------------
pub struct World {
    // the 12 per-slot columns (compacted together by the GC)
    pub cid: Vec<u32>,
    pub generation: Vec<u32>,
    pub px: Vec<f32>,
    pub py: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    pub intent_x: Vec<f32>,
    pub intent_y: Vec<f32>,
    pub energy: Vec<f32>,
    pub birth_t: Vec<f64>,
    pub alive: Vec<bool>,
    pub herd: Vec<u32>,
    // arena machinery
    pub id_to_slot: Vec<u32>,
    pub live: Vec<u32>,
    pub d_energy: Vec<f32>, // the tick's energy delta buffer (§15)
    // the species registry: apply maintains every entry, knowing none of their names
    pub subs: Vec<Sub>,
    pub n_live: usize,
    pub n_active: usize,
    pub next_id: u32,
    pub next_herd_id: u32,
    pub tick: usize,
    pub t: f64,
    pub gc_runs: usize,
    pub peak_n_active: usize,
    pub late_ticks: usize,
    pub peak_tick_s: f64,
}

impl World {
    /// An empty arena. Bins seed founders with `spawn_founders` and `register` their subscriptions.
    pub fn new(cfg: &Config) -> Self {
        let cap = cfg.cap;
        World {
            cid: vec![0; cap],
            generation: vec![0; cap],
            px: vec![0.0; cap],
            py: vec![0.0; cap],
            vx: vec![0.0; cap],
            vy: vec![0.0; cap],
            intent_x: vec![0.0; cap],
            intent_y: vec![0.0; cap],
            energy: vec![0.0; cap],
            birth_t: vec![0.0; cap],
            alive: vec![false; cap],
            herd: vec![0; cap],
            id_to_slot: vec![INVALID; cap],
            live: vec![0; cap],
            d_energy: vec![0.0; cap],
            subs: Vec::new(),
            n_live: 0,
            n_active: 0,
            next_id: 0,
            next_herd_id: 1,
            tick: 0,
            t: 0.0,
            gc_runs: 0,
            peak_n_active: 0,
            late_ticks: 0,
            peak_tick_s: 0.0,
        }
    }

    /// Mint `n` identical founder entities; return their ids. Character comes from the
    /// subscription they are registered into, not from how they are seeded.
    pub fn spawn_founders(&mut self, cfg: &Config, rng: &mut Rng, n: usize) -> Vec<u32> {
        let mut ids = Vec::with_capacity(n);
        for _ in 0..n {
            let slot = self.n_active;
            let id = self.next_id;
            self.px[slot] = rng.uniform(0.0, cfg.world);
            self.py[slot] = rng.uniform(0.0, cfg.world);
            let ang = rng.uniform(0.0, std::f32::consts::TAU);
            self.vx[slot] = ang.cos() * cfg.herd_speed;
            self.vy[slot] = ang.sin() * cfg.herd_speed;
            self.intent_x[slot] = self.vx[slot];
            self.intent_y[slot] = self.vy[slot];
            self.energy[slot] = cfg.init_energy;
            self.cid[slot] = id;
            self.alive[slot] = true;
            self.id_to_slot[id as usize] = slot as u32;
            self.live[self.n_live] = slot as u32;
            self.n_active += 1;
            self.n_live += 1;
            self.next_id += 1;
            ids.push(id);
        }
        ids
    }

    pub fn register(&mut self, name: &'static str, ids: Vec<u32>) {
        self.subs.push(Sub { name, ids });
    }
    pub fn sub(&self, name: &str) -> &[u32] {
        &self.subs.iter().find(|s| s.name == name).expect("no such subscription").ids
    }
    /// Move a subscription's ids out (leaving it empty) so a system can read them while the
    /// World is mutated; pair with `put_sub`. The systems never modify the id-list - only `apply`
    /// does - so this is a borrow accommodation, not a semantic change.
    pub fn take_sub(&mut self, name: &str) -> Vec<u32> {
        let s = self.subs.iter_mut().find(|s| s.name == name).expect("no such subscription");
        std::mem::take(&mut s.ids)
    }
    pub fn put_sub(&mut self, name: &str, ids: Vec<u32>) {
        let s = self.subs.iter_mut().find(|s| s.name == name).expect("no such subscription");
        s.ids = ids;
    }
}

// patches: producers return these; `apply` is the sole consumer.
pub struct ForagePatch {
    pub forager_slots: Vec<usize>,
    pub target_ids: Vec<u32>,
    pub gains: Vec<f32>,
}
pub struct BornPatch {
    pub parent_ids: Vec<u32>,
    pub px: Vec<f32>,
    pub py: Vec<f32>,
    pub vx: Vec<f32>,
    pub vy: Vec<f32>,
    pub energy: Vec<f32>,
    pub herd: Vec<u32>,
}
pub struct DeadPatch {
    pub ids: Vec<u32>,
    pub times: Vec<f64>,
}

#[derive(Default, PartialEq)]
pub struct Log {
    pub born: Vec<(f64, u32, i64)>,
    pub dead: Vec<(f64, u32)>,
    pub eaten: Vec<(f64, u32, f32)>,
    pub population: Vec<(f64, Vec<i64>)>, // (t, live count per subscription, in registry order)
}

// Resolve a subscription's ids to current slots (§23), dropping any that are gone.
fn slots(ids: &[u32], id_to_slot: &[u32]) -> Vec<usize> {
    let mut out = Vec::with_capacity(ids.len());
    for &id in ids {
        let s = id_to_slot[id as usize];
        if s != INVALID {
            out.push(s as usize);
        }
    }
    out
}

// ==============================================================================
// MOTION systems - each takes the subscription it serves and writes pos/vel for it.
// ==============================================================================
pub fn regenerate(w: &mut World, cfg: &Config, rng: &mut Rng, grass: &[u32]) {
    let s = slots(grass, &w.id_to_slot);
    let gain = (cfg.grass_photosynthesis - cfg.grass_burn) * cfg.dt;
    for &i in &s {
        let ang = rng.uniform(0.0, std::f32::consts::TAU);
        w.vx[i] = ang.cos() * cfg.grass_drift;
        w.vy[i] = ang.sin() * cfg.grass_drift;
        w.px[i] = (w.px[i] + w.vx[i] * cfg.dt).rem_euclid(cfg.world);
        w.py[i] = (w.py[i] + w.vy[i] * cfg.dt).rem_euclid(cfg.world);
        w.d_energy[i] += gain;
    }
}

/// Herd motion for ANY subscription: steer toward the herd's eldest leader, wander, integrate,
/// burn at this subscription's rate. Parameterised by (members, speed, burn), so grazers and
/// predators share it - the predator is just a second call with different numbers.
pub fn herd_move(w: &mut World, cfg: &Config, rng: &mut Rng, members: &[u32], speed: f32, burn: f32) {
    let s = slots(members, &w.id_to_slot);
    if s.is_empty() {
        return;
    }
    let nh = w.next_herd_id as usize;
    let mut best_birth = vec![f64::INFINITY; nh];
    let mut leader = vec![usize::MAX; nh];
    for &i in &s {
        let h = w.herd[i] as usize;
        if w.birth_t[i] < best_birth[h] {
            best_birth[h] = w.birth_t[i];
            leader[h] = i;
        }
    }
    let mut lpx = vec![0.0f32; nh];
    let mut lpy = vec![0.0f32; nh];
    for h in 0..nh {
        if leader[h] != usize::MAX {
            lpx[h] = w.px[leader[h]];
            lpy[h] = w.py[leader[h]];
        }
    }
    let half = cfg.world * 0.5;
    for &i in &s {
        let h = w.herd[i] as usize;
        let dx = (lpx[h] - w.px[i] + half).rem_euclid(cfg.world) - half;
        let dy = (lpy[h] - w.py[i] + half).rem_euclid(cfg.world) - half;
        let d = (dx * dx + dy * dy).sqrt() + 1e-6;
        let mut ix = w.vx[i] + cfg.cohesion * (dx / d) * speed;
        let mut iy = w.vy[i] + cfg.cohesion * (dy / d) * speed;
        let turn = rng.uniform(-cfg.wander, cfg.wander);
        let (c, sn) = (turn.cos(), turn.sin());
        let (nx, ny) = (c * ix - sn * iy, sn * ix + c * iy);
        ix = nx;
        iy = ny;
        let sp = (ix * ix + iy * iy).sqrt() + 1e-6;
        w.vx[i] = ix / sp * speed;
        w.vy[i] = iy / sp * speed;
        w.px[i] = (w.px[i] + w.vx[i] * cfg.dt).rem_euclid(cfg.world);
        w.py[i] = (w.py[i] + w.vy[i] * cfg.dt).rem_euclid(cfg.world);
        w.d_energy[i] -= burn * cfg.dt;
    }
    // split a herd that has outgrown max_herd.
    let mut sizes = vec![0usize; nh];
    for &i in &s {
        sizes[w.herd[i] as usize] += 1;
    }
    for hid in 0..nh {
        if sizes[hid] > cfg.max_herd {
            let members: Vec<usize> = s.iter().copied().filter(|&i| w.herd[i] as usize == hid).collect();
            let mut xs: Vec<f32> = members.iter().map(|&i| w.px[i]).collect();
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let m = xs.len();
            let median = if m % 2 == 0 { (xs[m / 2 - 1] + xs[m / 2]) * 0.5 } else { xs[m / 2] };
            for &i in &members {
                if w.px[i] > median {
                    w.herd[i] = w.next_herd_id;
                }
            }
            w.next_herd_id += 1;
        }
    }
}

// ==============================================================================
// FORAGE - one general trophic system. Loop-based rep-per-cell: each cell answers once with a
// representative forager; each target matches the <=9 reps of its 3x3. O(targets) regardless of
// density. Eating grass and (in sim2) eating a grazer are the SAME write. Producer: mutates nothing.
// ==============================================================================
pub fn forage(w: &World, cfg: &Config, foragers: &[u32], targets: &[u32], radius: f32, gain: f32) -> ForagePatch {
    let fo = slots(foragers, &w.id_to_slot);
    let ta = slots(targets, &w.id_to_slot);
    if fo.is_empty() || ta.is_empty() {
        return ForagePatch { forager_slots: vec![], target_ids: vec![], gains: vec![] };
    }
    let cs = radius;
    let ncol = (cfg.world / cs) as i64 + 1;
    let cell = |x: f32, y: f32| -> usize {
        let cx = ((x / cs) as i64).rem_euclid(ncol);
        let cy = ((y / cs) as i64).rem_euclid(ncol);
        (cx * ncol + cy) as usize
    };
    let mut rep = vec![-1i64; (ncol * ncol) as usize];
    for (k, &i) in fo.iter().enumerate() {
        rep[cell(w.px[i], w.py[i])] = k as i64;
    }
    let half = cfg.world * 0.5;
    let r2 = radius * radius;
    let mut forager_slots = Vec::new();
    let mut target_ids = Vec::new();
    for &t in &ta {
        let cx = ((w.px[t] / cs) as i64).rem_euclid(ncol);
        let cy = ((w.py[t] / cs) as i64).rem_euclid(ncol);
        let (mut bd, mut bf) = (f32::INFINITY, usize::MAX);
        for ox in -1..=1i64 {
            for oy in -1..=1i64 {
                let c = (((cx + ox).rem_euclid(ncol)) * ncol + ((cy + oy).rem_euclid(ncol))) as usize;
                let k = rep[c];
                if k >= 0 {
                    let f = fo[k as usize];
                    let dx = (w.px[t] - w.px[f] + half).rem_euclid(cfg.world) - half;
                    let dy = (w.py[t] - w.py[f] + half).rem_euclid(cfg.world) - half;
                    let d2 = dx * dx + dy * dy;
                    if d2 <= r2 && d2 < bd {
                        bd = d2;
                        bf = f;
                    }
                }
            }
        }
        if bf != usize::MAX {
            forager_slots.push(bf);
            target_ids.push(w.cid[t]);
        }
    }
    let gains = vec![gain; forager_slots.len()];
    ForagePatch { forager_slots, target_ids, gains }
}

// ==============================================================================
// LIFECYCLE - uniform for every entity.
// ==============================================================================
pub fn reproduce(w: &World, cfg: &Config, rng: &mut Rng) -> BornPatch {
    let mut p = BornPatch {
        parent_ids: vec![],
        px: vec![],
        py: vec![],
        vx: vec![],
        vy: vec![],
        energy: vec![],
        herd: vec![],
    };
    for &id in &w.live[..w.n_live] {
        let i = id as usize;
        if w.energy[i] >= cfg.repro_threshold {
            let half = w.energy[i] * 0.5;
            p.parent_ids.push(w.cid[i]);
            for _ in 0..2 {
                let ang = rng.uniform(0.0, std::f32::consts::TAU);
                p.px.push(w.px[i] + rng.uniform(-1.0, 1.0));
                p.py.push(w.py[i] + rng.uniform(-1.0, 1.0));
                p.vx.push(ang.cos() * cfg.herd_speed);
                p.vy.push(ang.sin() * cfg.herd_speed);
                p.energy.push(half);
                p.herd.push(w.herd[i]);
            }
        }
    }
    p
}

pub fn die(w: &World, cfg: &Config) -> DeadPatch {
    let mut d = DeadPatch { ids: vec![], times: vec![] };
    for &id in &w.live[..w.n_live] {
        let i = id as usize;
        let rate = -w.d_energy[i] / cfg.dt;
        if rate > 0.0 {
            let t_die = w.energy[i] / rate;
            if t_die <= cfg.dt {
                d.ids.push(w.cid[i]);
                d.times.push(t_die.clamp(0.0, cfg.dt) as f64);
            }
        }
    }
    d
}

// ==============================================================================
// THE JOIN - the single writer of committed state. Maintains EVERY subscription generically.
// ==============================================================================
pub fn apply(w: &mut World, log: &mut Log, cfg: &Config, foraged: &[ForagePatch], born: &BornPatch, dead: &DeadPatch) {
    let n = w.n_active;

    let mut is_repro = vec![false; n];
    for s in slots(&born.parent_ids, &w.id_to_slot) {
        is_repro[s] = true;
    }
    let mut eaten_ids: Vec<u32> = Vec::new();
    for p in foraged {
        eaten_ids.extend_from_slice(&p.target_ids);
    }
    let mut is_eaten = vec![false; n];
    for s in slots(&eaten_ids, &w.id_to_slot) {
        is_eaten[s] = true;
    }
    let mut is_fed = vec![false; n];
    for p in foraged {
        for &s in &p.forager_slots {
            is_fed[s] = true;
        }
    }

    // eat-saves-starve: a forager that fed (or reproduced, or was eaten) is not a starvation death.
    let mut dead_ids: Vec<u32> = Vec::new();
    let mut dead_times: Vec<f64> = Vec::new();
    for (k, &id) in dead.ids.iter().enumerate() {
        let raw = w.id_to_slot[id as usize];
        if raw == INVALID {
            continue;
        }
        let s = raw as usize;
        if !(is_repro[s] || is_eaten[s] || is_fed[s]) {
            dead_ids.push(id);
            dead_times.push(dead.times[k]);
        }
    }

    // 1) energy: commit this tick's net motion delta, then the forage gains. Sole writer.
    for i in 0..n {
        w.energy[i] += w.d_energy[i];
    }
    for p in foraged {
        for (k, &s) in p.forager_slots.iter().enumerate() {
            w.energy[s] += p.gains[k];
        }
    }

    // 2) remove: dead + fissioned parents + everything eaten (entity death is uniform).
    let mut gone: Vec<u32> = Vec::new();
    gone.extend_from_slice(&dead_ids);
    gone.extend_from_slice(&born.parent_ids);
    gone.extend_from_slice(&eaten_ids);
    gone.sort_unstable();
    gone.dedup();
    for &id in &gone {
        let s = w.id_to_slot[id as usize];
        if s != INVALID {
            let s = s as usize;
            w.alive[s] = false;
            w.generation[s] += 1;
            w.id_to_slot[id as usize] = INVALID;
        }
    }

    // 3) insert offspring at the tail, minting ids.
    let nb = born.energy.len();
    let mut ids: Vec<u32> = Vec::new();
    let mut parent_of_child: Vec<u32> = Vec::new();
    if nb > 0 {
        if w.n_active + nb > cfg.cap {
            compact(w);
        }
        assert!(w.n_active + nb <= cfg.cap, "capacity exceeded even after GC");
        for k in 0..nb {
            let slot = w.n_active + k;
            let id = w.next_id + k as u32;
            ids.push(id);
            w.cid[slot] = id;
            w.px[slot] = born.px[k];
            w.py[slot] = born.py[k];
            w.vx[slot] = born.vx[k];
            w.vy[slot] = born.vy[k];
            w.intent_x[slot] = born.vx[k];
            w.intent_y[slot] = born.vy[k];
            w.energy[slot] = born.energy[k];
            w.herd[slot] = born.herd[k];
            w.birth_t[slot] = w.t;
            w.alive[slot] = true;
            w.id_to_slot[id as usize] = slot as u32;
        }
        w.next_id += nb as u32;
        w.n_active += nb;
        for &pid in &born.parent_ids {
            parent_of_child.push(pid);
            parent_of_child.push(pid);
        }
        for k in 0..nb {
            log.born.push((w.t, ids[k], parent_of_child[k] as i64));
        }
    }

    // 4) maintain EVERY subscription the same way (§26), knowing none of their names. Adding a
    //    species is registering a name (the `sim2` bin), not editing this loop.
    let gone_happened = !gone.is_empty();
    let mut subs = std::mem::take(&mut w.subs);
    for s in &mut subs {
        maintain_sub(&mut s.ids, &born.parent_ids, &parent_of_child, &ids, &w.id_to_slot, gone_happened);
    }
    w.subs = subs;

    // 5) history: apply commits, so apply logs (§37).
    for (k, &id) in dead_ids.iter().enumerate() {
        log.dead.push((w.t + dead_times[k], id));
    }
    for &pid in &born.parent_ids {
        log.dead.push((w.t, pid));
    }
    for p in foraged {
        for &id in &p.target_ids {
            log.dead.push((w.t, id));
        }
        for (k, &s) in p.forager_slots.iter().enumerate() {
            log.eaten.push((w.t, w.cid[s], p.gains[k]));
        }
    }

    rebuild_live(w);
}

fn maintain_sub(
    sub: &mut Vec<u32>,
    parent_ids: &[u32],
    parent_of_child: &[u32],
    ids: &[u32],
    id_to_slot: &[u32],
    gone_happened: bool,
) {
    let members: std::collections::HashSet<u32> = sub.iter().copied().collect();
    let parents: std::collections::HashSet<u32> =
        parent_ids.iter().copied().filter(|id| members.contains(id)).collect();
    if gone_happened {
        sub.retain(|&id| id_to_slot[id as usize] != INVALID);
    }
    for (k, &id) in ids.iter().enumerate() {
        if parents.contains(&parent_of_child[k]) {
            sub.push(id);
        }
    }
}

pub fn cleanup(w: &mut World, cfg: &Config) {
    w.peak_n_active = w.peak_n_active.max(w.n_active);
    if w.tick % cfg.gc_interval == 0 {
        compact(w);
        rebuild_live(w);
    }
}

/// Observe the subscriptions (§ inspect): live count per registered subscription, in order.
pub fn inspect(w: &World, log: &mut Log) {
    let counts: Vec<i64> = w
        .subs
        .iter()
        .map(|s| s.ids.iter().filter(|&&id| w.id_to_slot[id as usize] != INVALID).count() as i64)
        .collect();
    log.population.push((w.t, counts));
}

fn rebuild_live(w: &mut World) {
    let mut k = 0;
    for i in 0..w.n_active {
        if w.alive[i] {
            w.live[k] = i as u32;
            k += 1;
        }
    }
    w.n_live = k;
}

fn compact(w: &mut World) {
    let keep: Vec<bool> = w.alive[..w.n_active].to_vec();
    let m: usize = keep.iter().filter(|&&b| b).count();
    if m == w.n_active {
        return;
    }
    w.gc_runs += 1;
    let n = w.n_active;
    macro_rules! pack {
        ($col:expr) => {{
            let mut wr = 0;
            for r in 0..n {
                if keep[r] {
                    $col[wr] = $col[r];
                    wr += 1;
                }
            }
        }};
    }
    pack!(w.cid);
    pack!(w.generation);
    pack!(w.px);
    pack!(w.py);
    pack!(w.vx);
    pack!(w.vy);
    pack!(w.intent_x);
    pack!(w.intent_y);
    pack!(w.energy);
    pack!(w.birth_t);
    pack!(w.alive);
    pack!(w.herd);
    for i in m..n {
        w.alive[i] = false;
    }
    for i in 0..m {
        w.id_to_slot[w.cid[i] as usize] = i as u32;
    }
    w.n_active = m;
}

/// Print the run summary: per-subscription min/max/final, peak tick vs budget, replay check, and
/// an ASCII population sparkline. Generic over whatever subscriptions the bin registered.
pub fn summarise(w: &World, log: &Log, cfg: &Config) {
    println!("ticks run        : {}", log.population.len());
    for (k, s) in w.subs.iter().enumerate() {
        let series: Vec<i64> = log.population.iter().map(|p| p.1[k]).collect();
        println!(
            "{:8} min/max/fin: {} / {} / {}",
            s.name,
            series.iter().min().unwrap(),
            series.iter().max().unwrap(),
            series.last().unwrap()
        );
    }
    println!("peak tick work   : {:.2} ms  (budget {:.1} ms)", w.peak_tick_s * 1000.0, cfg.dt as f64 * 1000.0);
    println!("GC compactions   : {}", w.gc_runs);
    let ok = live_ids_of(w) == replay_live_ids(log);
    println!(
        "replay (§37)     : {} - {} births / {} deaths / {} meals",
        if ok { "OK" } else { "MISMATCH" },
        log.born.len(),
        log.dead.len(),
        log.eaten.len()
    );
    let total: Vec<i64> = log.population.iter().map(|p| p.1.iter().sum()).collect();
    let (lo, hi) = (*total.iter().min().unwrap(), *total.iter().max().unwrap());
    if hi > lo {
        let bars = [' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];
        let stride = (total.len() / 70).max(1);
        let line: String = total
            .iter()
            .step_by(stride)
            .map(|&v| bars[((v - lo) as f64 / (hi - lo) as f64 * (bars.len() - 1) as f64) as usize])
            .collect();
        println!("total population over time:\n{line}");
    }
}

// ==============================================================================
// Run loop + replay (§37). The bin supplies its own `step`; the loop is generic.
// ==============================================================================
pub fn run_loop(
    w: &mut World,
    log: &mut Log,
    cfg: &Config,
    rng: &mut Rng,
    step: fn(&mut World, &mut Log, &Config, &mut Rng),
) {
    for _ in 0..cfg.ticks {
        let start = Instant::now();
        step(w, log, cfg, rng);
        let elapsed = start.elapsed().as_secs_f64();
        w.peak_tick_s = w.peak_tick_s.max(elapsed);
        if elapsed > cfg.dt as f64 {
            w.late_ticks += 1;
        }
        if w.n_live == 0 {
            break;
        }
    }
}

pub fn live_ids_of(w: &World) -> Vec<u32> {
    let mut v: Vec<u32> = (0..w.n_active).filter(|&i| w.alive[i]).map(|i| w.cid[i]).collect();
    v.sort_unstable();
    v
}

pub fn replay_live_ids(log: &Log) -> Vec<u32> {
    let born: std::collections::HashSet<u32> = log.born.iter().map(|&(_, cid, _)| cid).collect();
    let dead: std::collections::HashSet<u32> = log.dead.iter().map(|&(_, cid)| cid).collect();
    let mut v: Vec<u32> = born.difference(&dead).copied().collect();
    v.sort_unstable();
    v
}
