//! batched_write - 1,000,000 rows of 32 bytes, written as 1M separate writes
//! vs one bulk write. Backs §38 exercise 3.
//!
//!     cargo run --release --bin batched_write
//!
//! The unbatched version pays one `write` syscall per row; the batched version
//! pays one for the whole buffer. The gap is IOPS vs bandwidth, made literal.
//! It depends heavily on the filesystem (tmpfs hides the syscall cost less than
//! a journalled disk), so treat the ratio as order-of-magnitude.

use std::fs::File;
use std::io::Write;
use std::time::Instant;

const ROWS: usize = 1_000_000;
const ROW_BYTES: usize = 32;

fn main() {
    let row = [0xABu8; ROW_BYTES];
    let dir = std::env::temp_dir();

    // Unbatched: one write() call per row.
    let path_u = dir.join("batched_write_unbatched.bin");
    let t0 = Instant::now();
    {
        let mut f = File::create(&path_u).unwrap();
        for _ in 0..ROWS {
            f.write_all(&row).unwrap();
        }
        f.flush().unwrap();
    }
    let dt_unbatched = t0.elapsed();

    // Batched: assemble the whole payload, one write() call.
    let path_b = dir.join("batched_write_batched.bin");
    let t0 = Instant::now();
    {
        let mut buf = Vec::with_capacity(ROWS * ROW_BYTES);
        for _ in 0..ROWS {
            buf.extend_from_slice(&row);
        }
        let mut f = File::create(&path_b).unwrap();
        f.write_all(&buf).unwrap();
        f.flush().unwrap();
    }
    let dt_batched = t0.elapsed();

    let _ = std::fs::remove_file(&path_u);
    let _ = std::fs::remove_file(&path_b);

    println!("{ROWS} rows x {ROW_BYTES} bytes = {} MB", ROWS * ROW_BYTES / 1_000_000);
    println!("  unbatched (1 write/row):  {:>9.1} ms", dt_unbatched.as_secs_f64() * 1000.0);
    println!("  batched   (1 write total):{:>9.1} ms", dt_batched.as_secs_f64() * 1000.0);
    println!("  ratio:                    {:>9.0}x  (batched faster)",
             dt_unbatched.as_secs_f64() / dt_batched.as_secs_f64());
}
