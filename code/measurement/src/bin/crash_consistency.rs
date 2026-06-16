//! crash_consistency - the specimen behind §46. A write-ahead log with a per-batch commit marker
//! (a checksum) recovers, after a torn write, to the last COMMITTED world - never a half-written
//! one. And the premature-acknowledgement bug: acknowledge before the marker is durable and a
//! crash turns the acknowledgement into a lie.
//!
//!     cargo run --release --bin crash_consistency
//!
//! The "crash" is a truncated tail (§46 exercise 1's method): a batch whose marker never landed.
//! Recovery scans batches, verifies each checksum, and discards the first torn/incomplete one.

use std::fs::File;
use std::io::{Read, Write};

// std-only CRC32 (from-scratch; the book prices dependencies, it does not import them blindly).
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

// The world is a fold over events - an FNV-style rolling hash standing in for real state, so two
// worlds are equal iff the same committed events were applied in the same order.
fn apply(world: u64, ev: u64) -> u64 {
    (world ^ ev).wrapping_mul(0x0100_0000_01b3)
}

// One batch on disk: [n: u32][events: u64 * n][crc32 over those bytes: u32]. The crc is the commit
// marker - present and correct means the batch fully landed.
fn batch_bytes(evs: &[u64]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(evs.len() as u32).to_le_bytes());
    for &e in evs {
        buf.extend_from_slice(&e.to_le_bytes());
    }
    let crc = crc32(&buf);
    buf.extend_from_slice(&crc.to_le_bytes());
    buf
}

// Recover: apply every batch whose marker verifies; stop at the first torn or incomplete one.
fn recover(path: &std::path::Path) -> (u64, usize) {
    let mut bytes = Vec::new();
    File::open(path).unwrap().read_to_end(&mut bytes).unwrap();
    let (mut pos, mut world, mut n) = (0usize, 0u64, 0usize);
    loop {
        if pos + 4 > bytes.len() {
            break; // no room for a header: clean end or torn
        }
        let nev = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let body_len = 4 + nev * 8;
        if pos + body_len + 4 > bytes.len() {
            break; // batch body or marker truncated: torn tail
        }
        let body = &bytes[pos..pos + body_len];
        let crc = u32::from_le_bytes(bytes[pos + body_len..pos + body_len + 4].try_into().unwrap());
        if crc32(body) != crc {
            break; // marker fails: torn tail
        }
        for k in 0..nev {
            let o = pos + 4 + k * 8;
            world = apply(world, u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap()));
        }
        pos += body_len + 4;
        n += 1;
    }
    (world, n)
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0
    }
}

fn main() {
    let path = std::env::temp_dir().join("crash_consistency.log");

    // --- Scenario 1: torn tail recovers to the last committed world ---
    let mut f = File::create(&path).unwrap();
    let mut rng = Lcg(1);
    let mut committed_world = 0u64;
    for _ in 0..100 {
        let evs: Vec<u64> = (0..1 + rng.next() % 8).map(|_| rng.next()).collect();
        let bytes = batch_bytes(&evs);
        f.write_all(&bytes).unwrap();
        f.sync_all().unwrap(); // fsync barrier: this batch is now durable
        for &e in &evs {
            committed_world = apply(committed_world, e);
        }
    }
    // The crash: a 101st batch begins but the marker never lands - write a partial body, no crc.
    let evs: Vec<u64> = vec![11, 22, 33, 44, 55];
    let mut torn = (evs.len() as u32).to_le_bytes().to_vec();
    for &e in &evs[..3] {
        torn.extend_from_slice(&e.to_le_bytes()); // only 3 of 5 events; no commit marker
    }
    f.write_all(&torn).unwrap();
    f.sync_all().unwrap();
    drop(f);

    let (world, n) = recover(&path);
    println!("§46 specimen - crash consistency\n");
    println!("Scenario 1: torn tail");
    println!("  wrote 100 committed batches + 1 torn (partial, no marker)");
    println!("  recovered {n} batches; world == last committed world: {}", world == committed_world);
    assert_eq!(n, 100, "must discard the torn batch");
    assert_eq!(world, committed_world, "must recover the last committed world, not a torn one");

    // --- Scenario 2: the premature-acknowledgement lie ---
    // Both runs write 50 committed batches, then crash mid-51st (marker never lands). The only
    // difference is WHEN the sender is told "ok": before the marker, or after.
    let ack_demo = |ack_before_marker: bool| -> (usize, usize) {
        let mut f = File::create(&path).unwrap();
        let mut acked = 0usize;
        for i in 0..50u64 {
            let bytes = batch_bytes(&[i]);
            if ack_before_marker {
                acked += 1; // told the sender "ok" before fsync
                f.write_all(&bytes).unwrap();
                f.sync_all().unwrap();
            } else {
                f.write_all(&bytes).unwrap();
                f.sync_all().unwrap();
                acked += 1; // told the sender "ok" only after the marker is durable
            }
        }
        // batch 51 crashes between append and marker.
        if ack_before_marker {
            acked += 1; // acknowledged a batch whose marker will never land
        }
        let mut torn = (1u32).to_le_bytes().to_vec();
        torn.extend_from_slice(&50u64.to_le_bytes()); // body, no crc marker
        f.write_all(&torn).unwrap();
        f.sync_all().unwrap();
        drop(f);
        let (_w, recovered) = recover(&path);
        (acked, recovered)
    };

    let (acked_b, rec_b) = ack_demo(true);
    let (acked_a, rec_a) = ack_demo(false);
    println!("\nScenario 2: premature acknowledgement (crash mid-51st batch)");
    println!("  ack BEFORE marker: sender holds {acked_b} acks, log recovered {rec_b}  -> {} ack(s) are a lie",
             acked_b - rec_b);
    println!("  ack AFTER  marker: sender holds {acked_a} acks, log recovered {rec_a}  -> sender and log agree: {}",
             acked_a == rec_a);
    assert!(acked_b > rec_b, "ack-before-marker must be able to over-acknowledge");
    assert_eq!(acked_a, rec_a, "ack-after-marker never acknowledges a record the log lost");

    let _ = std::fs::remove_file(&path);
    println!("\nLogged means one thing: I can read it back after a crash. Everything before the marker is hope.");
}
