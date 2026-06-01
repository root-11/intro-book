//! row_vs_column_serialize - snapshot 1M creatures three ways: column-direct
//! (bulk), per-row binary, per-row text. Backs §36 exercise 3.
//!
//!     cargo run --release --bin row_vs_column_serialize
//!
//! The column snapshot writes each SoA column's bytes in one go - the format
//! is the memory. The per-row forms walk the table emitting one record each:
//! the OOP shape (`CreatureRecord` + a serialiser per row). This binary uses
//! from-scratch encoders (no serde) so the crate stays dependency-free; serde
//! would land in the same band - serde_json near the text figure, bincode near
//! the binary one. The per-row tax is the function call and small write per
//! creature, not the library.

use std::io::Write;
use std::time::Instant;

const N: usize = 1_000_000;

fn main() {
    // SoA columns - the live world.
    let px: Vec<f32> = (0..N).map(|i| i as f32 * 0.5).collect();
    let py: Vec<f32> = (0..N).map(|i| i as f32 * 0.25).collect();
    let energy: Vec<f32> = (0..N).map(|i| (i % 100) as f32).collect();
    let id: Vec<u32> = (0..N as u32).collect();

    // Column-direct: write each column's raw bytes. The format is the memory.
    let t0 = Instant::now();
    let mut col = Vec::with_capacity(N * 16);
    for c in [&px, &py, &energy] {
        col.extend_from_slice(bytemuck_cast_f32(c));
    }
    col.extend_from_slice(bytemuck_cast_u32(&id));
    std::hint::black_box(&col);
    let dt_col = t0.elapsed();

    // Per-row binary: one record per creature, fields packed in order.
    let t0 = Instant::now();
    let mut rowbin = Vec::with_capacity(N * 16);
    for i in 0..N {
        rowbin.write_all(&px[i].to_le_bytes()).unwrap();
        rowbin.write_all(&py[i].to_le_bytes()).unwrap();
        rowbin.write_all(&energy[i].to_le_bytes()).unwrap();
        rowbin.write_all(&id[i].to_le_bytes()).unwrap();
    }
    std::hint::black_box(&rowbin);
    let dt_rowbin = t0.elapsed();

    // Per-row text: one JSON-ish line per creature (the serde_json shape).
    let t0 = Instant::now();
    let mut rowtxt = String::with_capacity(N * 48);
    for i in 0..N {
        use std::fmt::Write as _;
        writeln!(rowtxt, "{{\"id\":{},\"x\":{},\"y\":{},\"e\":{}}}",
                 id[i], px[i], py[i], energy[i]).unwrap();
    }
    std::hint::black_box(&rowtxt);
    let dt_rowtxt = t0.elapsed();

    let ms = |d: std::time::Duration| d.as_secs_f64() * 1000.0;
    println!("snapshot of {N} creatures (4 fields):");
    println!("  column-direct (bulk):  {:>8.2} ms   1.0x", ms(dt_col));
    println!("  per-row binary:        {:>8.2} ms   {:>4.0}x", ms(dt_rowbin), ms(dt_rowbin) / ms(dt_col));
    println!("  per-row text (JSON):   {:>8.2} ms   {:>4.0}x", ms(dt_rowtxt), ms(dt_rowtxt) / ms(dt_col));
}

// Tiny local casts so the crate needs no `bytemuck` dependency.
fn bytemuck_cast_f32(v: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}
fn bytemuck_cast_u32(v: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}
