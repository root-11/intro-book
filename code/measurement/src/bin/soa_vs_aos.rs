//! soa_vs_aos - count entries with a given location in SoA vs AoS layouts.
//! Used by §7 exercises 3, 4, 5.
//!
//!     cargo run --release --bin soa_vs_aos
//!
//! The headline claim: at large N, SoA reads only the column it needs;
//! AoS pulls the whole row through cache. The gap widens as the row grows.

use std::time::Instant;

#[derive(Clone, Copy)]
#[repr(C)]
struct Card {
    suit:     u8,
    rank:     u8,
    location: u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct CardWithNickname {
    suit:     u8,
    rank:     u8,
    location: u8,
    _pad:     u8,
    nickname: [u8; 16],
}

#[inline(never)]
fn count_held_soa(locations: &[u8], player: u8) -> usize {
    locations.iter().filter(|&&l| l == player).count()
}

#[inline(never)]
fn count_held_aos(cards: &[Card], player: u8) -> usize {
    cards.iter().filter(|c| c.location == player).count()
}

#[inline(never)]
fn count_held_aos_padded(cards: &[CardWithNickname], player: u8) -> usize {
    cards.iter().filter(|c| c.location == player).count()
}

fn time(label: &str, ns: f64) {
    println!("  {:<32} {:>10.0} ns", label, ns);
}

fn run(n: usize) {
    println!("\n--- N = {} ---", n);
    println!("  size_of::<Card>             = {} bytes", std::mem::size_of::<Card>());
    println!("  size_of::<CardWithNickname> = {} bytes", std::mem::size_of::<CardWithNickname>());

    let locations: Vec<u8> = (0..n).map(|i| (i % 5) as u8).collect();
    let cards: Vec<Card> = (0..n).map(|i| Card {
        suit: 0, rank: 0, location: (i % 5) as u8,
    }).collect();
    let cards_padded: Vec<CardWithNickname> = (0..n).map(|i| CardWithNickname {
        suit: 0, rank: 0, location: (i % 5) as u8, _pad: 0, nickname: [0; 16],
    }).collect();

    let player = 2u8;
    let iters = (50_000_000usize / n.max(1)).max(10) as u32;

    // Warm-up.
    for _ in 0..2 {
        std::hint::black_box(count_held_soa(&locations, player));
        std::hint::black_box(count_held_aos(&cards, player));
        std::hint::black_box(count_held_aos_padded(&cards_padded, player));
    }

    let t0 = Instant::now();
    let mut s = 0;
    for _ in 0..iters { s = count_held_soa(&locations, player); }
    std::hint::black_box(s);
    time("count SoA (Vec<u8>)", t0.elapsed().as_nanos() as f64 / iters as f64);

    let t0 = Instant::now();
    for _ in 0..iters { s = count_held_aos(&cards, player); }
    std::hint::black_box(s);
    time("count AoS (Vec<Card>, 3 B)", t0.elapsed().as_nanos() as f64 / iters as f64);

    let t0 = Instant::now();
    for _ in 0..iters { s = count_held_aos_padded(&cards_padded, player); }
    std::hint::black_box(s);
    time("count AoS (with 16 B nickname)", t0.elapsed().as_nanos() as f64 / iters as f64);
}

fn main() {
    run(10_000);
    run(1_000_000);
    run(10_000_000);
}
