//! sim - the ecosystem with two species: grass and grazers. The base the predator extends.
//!
//!     cargo run --release --bin sim            # run + summarise
//!     cargo run --release --bin sim -- --check # determinism (§16) + replay (§37)
//!
//! Diff this file against `sim2.rs`: adding a predator is a registration plus two wired systems,
//! and `apply` and every system in the lib are untouched. That diff is the extendability lesson.

use sim::*;

fn setup(cfg: &Config) -> (World, Rng) {
    let mut rng = Rng::new(cfg.seed);
    let mut w = World::new(cfg);
    let founders = w.spawn_founders(cfg, &mut rng, cfg.n0_grass + cfg.n0_grazers);
    w.register("grass", founders[..cfg.n0_grass].to_vec());
    w.register("grazers", founders[cfg.n0_grass..].to_vec());
    (w, rng)
}

fn seed_log(cfg: &Config) -> Log {
    let mut log = Log::default();
    for cid in 0..(cfg.n0_grass + cfg.n0_grazers) {
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
    let foraged = vec![forage(w, cfg, &grazers, &grass, cfg.graze_radius, cfg.graze_gain)];
    w.put_sub("grass", grass);
    w.put_sub("grazers", grazers);

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
