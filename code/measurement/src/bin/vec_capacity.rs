//! Vec layout: size_of::<Vec<u32>>, capacity growth, pre-sizing.
//! Used by §3 exercises 1, 2, 3.

fn main() {
    println!("size_of::<Vec<u32>>: {} bytes", std::mem::size_of::<Vec<u32>>());

    println!("\nGrowth from Vec::new():");
    let mut v: Vec<u32> = Vec::new();
    let mut last_cap = v.capacity();
    println!("  init:    len={:>3}  cap={:>3}", v.len(), v.capacity());
    for i in 0..200u32 {
        v.push(i);
        if v.capacity() != last_cap {
            println!("  push #{:>3}: len={:>3}  cap={:>3}", i + 1, v.len(), v.capacity());
            last_cap = v.capacity();
        }
    }

    println!("\nWith Vec::with_capacity(100):");
    let mut v = Vec::with_capacity(100);
    let cap0 = v.capacity();
    for i in 0..100u32 { v.push(i); }
    println!("  after 100 pushes: len={} cap={} (started at {})",
             v.len(), v.capacity(), cap0);
}
