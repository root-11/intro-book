//! Reproduce the §37 logger throughput numbers.
//!
//!     cargo run --release --bin benchmark
//!
//! Times `SimLog::log()` at 5 and 11 populated fields per row, 2 million rows
//! each - the workload §37 quotes. The Python original reports ~934 ns (5
//! fields) and ~1906 ns (11 fields) on the author's box; this is the compiled
//! analogue. Run it; the number is on your machine, not on trust.

use logger::{SimLog, Value};
use std::time::Instant;

const N_ROWS: u32 = 2_000_000;
const BUFFER: u32 = 200_000;

fn bench(label: &str, row: &[(&str, Value)]) {
    let dir = std::env::temp_dir().join(format!(
        "simlog_bench_{}_{}",
        label.replace(' ', "_"),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);

    let mut lg = SimLog::create(&dir, BUFFER).unwrap();
    let t0 = Instant::now();
    for _ in 0..N_ROWS {
        lg.log(row);
    }
    let elapsed = t0.elapsed(); // the hot loop only; close()/flush is outside
    lg.close();

    let per_ns = elapsed.as_nanos() as f64 / N_ROWS as f64;
    println!(
        "  {label:<11} {per_ns:7.0} ns/call   ({:.2} s for {} rows)",
        elapsed.as_secs_f64(),
        N_ROWS
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn main() {
    println!("SimLog::log(), {N_ROWS} rows, buffer_size={BUFFER}");

    // 5 populated fields (the other six of the 11-field schema stay unset).
    let row5 = [
        ("time", Value::Float(0.042)),
        ("value", Value::Float(51.6)),
        ("activity", Value::Str("picking")),
        ("entity_type", Value::Str("bot")),
        ("entity_id", Value::Int(42)),
    ];
    // 11-field simulation-event shape: 2 strings, 9 numeric.
    let row11 = [
        ("time", Value::Float(0.042)),
        ("value", Value::Float(51.6)),
        ("activity", Value::Str("picking")),
        ("entity_type", Value::Str("bot")),
        ("entity_id", Value::Int(42)),
        ("mission_id", Value::Int(500)),
        ("lp", Value::Int(2)),
        ("task_id", Value::Int(100)),
        ("priority", Value::Int(3)),
        ("priority_group", Value::Int(1)),
        ("derived_priority", Value::Int(6)),
    ];

    bench("5 fields", &row5);
    bench("11 fields", &row11);
}
