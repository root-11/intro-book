//! scale_sweep - the Rust framerate curve: full sim tick cost vs scale, at constant density
//! (world grown with N so it holds the count). The staircase chapter reads its heights from here,
//! NOT from the Python numbers. Uses the sim-1 tick (grass + grazers).
//!
//!     cargo run --release --bin scale_sweep

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

fn main() {
    let budget = Config::default().dt as f64 * 1000.0;
    println!("Rust framerate curve, constant density (world = sqrt(N/0.4)); budget {budget:.1} ms (30 Hz)\n");
    println!("{:>10} {:>7} {:>10} {:>9} {:>7} {:>9}", "target N", "world", "live", "tick ms", "Hz", "x budget");
    for n in [100_000usize, 300_000, 1_000_000, 3_000_000, 10_000_000] {
        let ng = n * 3 / 5;
        let cfg = Config {
            n0_grass: ng,
            n0_grazers: n - ng,
            cap: n * 3 / 2,
            world: (n as f32 / 0.4).sqrt(),
            ..Config::default()
        };
        let (mut w, mut rng) = setup(&cfg);
        let mut log = Log::default();
        let mut times = Vec::new();
        for _ in 0..6 {
            let t = Instant::now();
            step(&mut w, &mut log, &cfg, &mut rng);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mut tail = times[1..].to_vec();
        tail.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = tail[tail.len() / 2];
        println!(
            "{:>10} {:>7.0} {:>10} {:>9.1} {:>7.1} {:>8.1}x",
            n, cfg.world, w.n_live, med, 1000.0 / med, med / budget
        );
    }
    println!("\nThe tick is ~O(N), so Hz ~ 1/N; read where 30 Hz and the 15 Hz tolerance fall.");
}
