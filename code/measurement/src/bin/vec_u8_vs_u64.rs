//! Sum a Vec<u8> vs a Vec<u64> of equal length. Bandwidth differs by 8x.
//! Used by §2 exercise 3.

use std::time::Instant;

const N: usize = 100_000_000;

#[inline(never)]
fn sum_u8(v: &[u8]) -> u64 {
    v.iter().map(|&x| x as u64).sum()
}

#[inline(never)]
fn sum_u64(v: &[u64]) -> u64 {
    v.iter().sum()
}

fn main() {
    let vu8:  Vec<u8>  = vec![1; N];
    let vu64: Vec<u64> = vec![1; N];

    let t0 = Instant::now();
    let s1 = std::hint::black_box(sum_u8(&vu8));
    let dt_u8 = t0.elapsed();

    let t0 = Instant::now();
    let s2 = std::hint::black_box(sum_u64(&vu64));
    let dt_u64 = t0.elapsed();

    println!("Vec<u8>  ({} MB):  {:>10.3} ms  ({:>5.2} ns/elem)",
             N / (1 << 20),
             dt_u8.as_secs_f64() * 1000.0,
             dt_u8.as_nanos() as f64 / N as f64);
    println!("Vec<u64> ({} MB): {:>10.3} ms  ({:>5.2} ns/elem)",
             N * 8 / (1 << 20),
             dt_u64.as_secs_f64() * 1000.0,
             dt_u64.as_nanos() as f64 / N as f64);
    println!("Ratio:                {:>10.2}x  (u64 slower)",
             dt_u64.as_nanos() as f64 / dt_u8.as_nanos() as f64);
    assert_eq!(s1, N as u64);
    assert_eq!(s2, N as u64);
}
