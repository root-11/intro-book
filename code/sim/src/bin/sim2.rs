//! sim2 - the ecosystem with THREE species: grass, grazers, and a predator that hunts grazers.
//!
//!     cargo run --release --bin sim2            # run + summarise
//!     cargo run --release --bin sim2 -- --check # determinism (§16) + replay (§37)
//!
//! Diff this file against `sim.rs`. The predator is: one extra `register`, one extra `herd_move`,
//! one extra `forage` edge (predators eat grazers, the SAME write as grazers eating grass), and
//! the founder count. `apply` and every system in the lib are untouched - a new trophic level is
//! a subscription and a forage edge, not surgery. That is the extendability of ECS+EBP.

use sim::*;

fn setup(cfg: &Config) -> (World, Rng) {
    let mut rng = Rng::new(cfg.seed);
    let mut w = World::new(cfg);
    let founders = w.spawn_founders(cfg, &mut rng, cfg.n0_grass + cfg.n0_grazers + cfg.n0_predators);
    let (a, b) = (cfg.n0_grass, cfg.n0_grass + cfg.n0_grazers);
    w.register("grass", founders[..a].to_vec());
    w.register("grazers", founders[a..b].to_vec());
    w.register("predators", founders[b..].to_vec()); // <-- the new subscription
    (w, rng)
}

fn seed_log(cfg: &Config) -> Log {
    let mut log = Log::default();
    for cid in 0..(cfg.n0_grass + cfg.n0_grazers + cfg.n0_predators) {
        log.born.push((0.0, cid as u32, -1));
    }
    log
}

fn step(w: &mut World, log: &mut Log, cfg: &Config, rng: &mut Rng) {
    w.tick += 1;
    for i in 0..w.n_active {
        w.d_energy[i] = 0.0;
    }
    let grass = w.take_sub("grass");
    regenerate(w, cfg, rng, &grass);
    let grazers = w.take_sub("grazers");
    herd_move(w, cfg, rng, &grazers, cfg.herd_speed, cfg.herd_burn);
    let predators = w.take_sub("predators"); // <-- the predator moves as a herd too
    herd_move(w, cfg, rng, &predators, cfg.hunter_speed, cfg.hunter_burn);
    let foraged = vec![
        forage(w, cfg, &grazers, &grass, cfg.graze_radius, cfg.graze_gain),
        forage(w, cfg, &predators, &grazers, cfg.hunt_radius, cfg.hunt_gain), // <-- predators eat grazers
    ];
    w.put_sub("grass", grass);
    w.put_sub("grazers", grazers);
    w.put_sub("predators", predators);

    let born = reproduce(w, cfg, rng);
    let dead = die(w, cfg);
    apply(w, log, cfg, &foraged, &born, &dead);
    cleanup(w, cfg);
    inspect(w, log);
    w.t += cfg.dt as f64;
}

fn run(cfg: &Config) -> (World, Log) {
    let (mut w, mut rng) = setup(cfg);
    let mut log = seed_log(cfg);
    run_loop(&mut w, &mut log, cfg, &mut rng, step);
    (w, log)
}

fn main() {
    let cfg = Config::default();
    if std::env::args().any(|a| a == "--check") {
        let (wa, la) = run(&cfg);
        let (_wb, lb) = run(&cfg);
        assert!(la.population == lb.population, "non-deterministic");
        assert!(live_ids_of(&wa) == replay_live_ids(&la), "replay mismatch");
        println!("determinism OK : {} ticks identical (§16)", la.population.len());
        println!("replay OK      : the log reconstructs the live population bit-for-bit (§37)");
        return;
    }
    let (w, log) = run(&cfg);
    summarise(&w, &log, &cfg);
}
