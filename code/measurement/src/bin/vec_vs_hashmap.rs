//! Vec[i] vs HashMap::get — sequential access on N=1M.
//! Used by §3 exercise 4 and §4 exercise 3.

use std::collections::HashMap;
use std::time::Instant;

const N: usize = 1_000_000;

#[inline(never)]
fn sum_vec(v: &[u64]) -> u64 {
    v.iter().sum()
}

#[inline(never)]
fn sum_hashmap(m: &HashMap<u32, u64>) -> u64 {
    let mut s = 0u64;
    for k in 0..N as u32 {
        s += m.get(&k).copied().unwrap_or(0);
    }
    s
}

fn main() {
    let v: Vec<u64> = (0..N as u64).collect();
    let mut m: HashMap<u32, u64> = HashMap::with_capacity(N);
    for i in 0..N as u32 { m.insert(i, i as u64); }

    let t0 = Instant::now();
    let sv = std::hint::black_box(sum_vec(&v));
    let dt_v = t0.elapsed();

    let t0 = Instant::now();
    let sm = std::hint::black_box(sum_hashmap(&m));
    let dt_m = t0.elapsed();

    println!("Vec     sum: {:>10.3} ms  ({:>6.1} ns/elem)",
             dt_v.as_secs_f64() * 1000.0,
             dt_v.as_nanos() as f64 / N as f64);
    println!("HashMap sum: {:>10.3} ms  ({:>6.1} ns/elem)",
             dt_m.as_secs_f64() * 1000.0,
             dt_m.as_nanos() as f64 / N as f64);
    println!("Ratio:       {:>10.1}x  (HashMap slower)",
             dt_m.as_nanos() as f64 / dt_v.as_nanos() as f64);
    assert_eq!(sv, sm);
}
