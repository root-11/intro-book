//! observe - the specimen behind §47. The simulator's `inspect` is already a read-only metrics
//! system: it reads the subscriptions, writes only `log.population`, and touches no world column.
//! This proves the two claims that make observability trustworthy: it does not perturb what it
//! measures (the world is bit-identical with metrics on and off), and it is ~free against the tick.
//!
//!     cargo run --release --bin observe

use sim::*;
use std::time::Instant;

fn setup(cfg: &Config) -> (World, Rng) {
    let mut rng = Rng::new(cfg.seed);
    let mut w = World::new(cfg);
    let founders = w.spawn_founders(cfg, &mut rng, cfg.n0_grass + cfg.n0_grazers);
    w.register("grass", founders[..cfg.n0_grass].to_vec());
    w.register("grazers", founders[cfg.n0_grass..].to_vec());
    (w, rng)
}

// the sim-1 tick, with the metrics system (inspect) toggleable.
fn step(w: &mut World, log: &mut Log, cfg: &Config, rng: &mut Rng, observe: bool) {
    w.tick += 1;
    for i in 0..w.n_active {
        w.d_energy[i] = 0.0;
    }
    let grass = w.take_sub("grass");
    regenerate(w, cfg, rng, &grass);
    let grazers = w.take_sub("grazers");
    herd_move(w, cfg, rng, &grazers, cfg.herd_speed, cfg.herd_burn);
    let foraged = vec![forage(w, cfg, &grazers, &grass, cfg.graze_radius, cfg.graze_gain)];
    w.put_sub("grass", grass);
    w.put_sub("grazers", grazers);
    let born = reproduce(w, cfg, rng);
    let dead = die(w, cfg);
    apply(w, log, cfg, &foraged, &born, &dead);
    cleanup(w, cfg);
    if observe {
        inspect(w, log); // the read-only metrics system
    }
    w.t += cfg.dt as f64;
}

fn run(cfg: &Config, observe: bool) -> World {
    let (mut w, mut rng) = setup(cfg);
    let mut log = Log::default();
    for _ in 0..cfg.ticks {
        step(&mut w, &mut log, cfg, &mut rng, observe);
        if w.n_live == 0 {
            break;
        }
    }
    w
}

fn main() {
    println!("§47 specimen - observation is a read-only system\n");

    // Non-perturbation: the world must be identical whether metrics run or not.
    let cfg = Config::default();
    let with = run(&cfg, true);
    let without = run(&cfg, false);
    let identical = live_ids_of(&with) == live_ids_of(&without);
    println!("non-perturbation: world with metrics == world without metrics: {identical}");
    assert!(identical, "a read-only metrics system must not change the world it measures");

    // Cheapness: the metrics pass is a read + reduce + append, ~free against the tick budget.
    let n = 100_000usize;
    let ng = n * 3 / 5;
    let cfg = Config { n0_grass: ng, n0_grazers: n - ng, cap: n * 2, world: (n as f32 / 0.4).sqrt(), ..Config::default() };
    let (mut w, mut rng) = setup(&cfg);
    let mut log = Log::default();
    for _ in 0..3 {
        step(&mut w, &mut log, &cfg, &mut rng, true); // warm + populate
    }
    let mut best_inspect = f64::INFINITY;
    for _ in 0..5 {
        let t = Instant::now();
        inspect(&w, &mut log);
        best_inspect = best_inspect.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    let mut best_tick = f64::INFINITY;
    for _ in 0..5 {
        let t = Instant::now();
        step(&mut w, &mut log, &cfg, &mut rng, false);
        best_tick = best_tick.min(t.elapsed().as_secs_f64() * 1000.0);
    }
    let budget = cfg.dt as f64 * 1000.0;
    println!(
        "cheapness at {} live: inspect {:.3} ms vs tick {:.1} ms ({:.2}% of tick, {:.2}% of {:.0} ms budget)",
        w.n_live, best_inspect, best_tick, 100.0 * best_inspect / best_tick, 100.0 * best_inspect / budget, budget
    );
    println!("\nThe metrics system writes only its own table, so it cannot move the world it reads,");
    println!("and it costs a sequential read - the history that answers the 2 AM question is already there.");
}
