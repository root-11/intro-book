//! Reference for Part II - "Where SoA does not pay", project E: heterogeneous compute.
//!
//! The finale, and the answer to "you need a GPU for a simulator this size". SoA is the
//! precondition for SIMD, multiple cores, and accelerators - the book's point - and the
//! Intro stops at one core's bandwidth. This crate measures how far one box actually
//! reaches, because that is what decides whether you reach off it at all.
//!
//! The reviewer's framing - 10M-1B-node simulators *need* the GPU - is false by
//! irrelevance under the incremental discipline projects B and C just built. You never
//! needed to recompute a billion nodes; you needed the *active cone* current within the
//! frame. The question is therefore not "how fast is the GPU" but "how big an active set
//! can one box keep current in a frame", and the GPU earns its place only when that
//! active set, by itself, exceeds the box.
//!
//! Four measurements:
//!
//! 1. One core's reach - the SoA motion pass across cache levels (the compiler
//!    autovectorizes it, so SIMD is already in play), bandwidth-bound at scale.
//! 2. Scaling to the box ceiling - the same pass across cores; speedup plateaus once the
//!    memory channel saturates, so the real ceiling is bandwidth, not core count.
//! 3. The active-set budget - how many elements one core and all cores keep current in a
//!    33 ms frame. This is the number the reframe turns on.
//! 4. The GPU break-even - a cost model (transfer + launch vs compute), NOT a measurement:
//!    there is no GPU on this box, so the GPU constants are labelled assumptions and a
//!    real run is pending a GPU host, exactly like the Pi/i7/i3 columns elsewhere.
//!
//! Run:    cargo run --release
//! Tests:  cargo test --release

use std::hint::black_box;
use std::thread;
use std::time::Instant;

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

const DT: f32 = 1.0e-3;

/// Bytes of memory traffic per element per pass: px and py are read and written (8 each),
/// vx and vy are read (4 each). The arithmetic is two multiply-adds - almost nothing - so
/// the pass is bound by this 24 bytes, not by flops. That is the whole point of the budget.
const BYTES_PER_ELEM: f64 = 24.0;

/// A 30 Hz frame: the time budget to keep the active set current.
const FRAME_NS: f64 = 33_333_333.0;

/// The SoA motion pass: advance position by velocity. Independent per element, so the
/// compiler vectorizes it; the SoA layout is what makes that possible.
#[inline(never)]
fn motion(px: &mut [f32], py: &mut [f32], vx: &[f32], vy: &[f32], dt: f32) {
    for (((p, q), u), v) in px
        .iter_mut()
        .zip(py.iter_mut())
        .zip(vx.iter())
        .zip(vy.iter())
    {
        *p += *u * dt;
        *q += *v * dt;
    }
}

/// The same pass spread across `threads` cores via scoped threads over disjoint chunks.
fn motion_parallel(
    px: &mut [f32],
    py: &mut [f32],
    vx: &[f32],
    vy: &[f32],
    dt: f32,
    threads: usize,
) {
    let n = px.len();
    let chunk = n.div_ceil(threads).max(1);
    thread::scope(|s| {
        for (((p, q), u), v) in px
            .chunks_mut(chunk)
            .zip(py.chunks_mut(chunk))
            .zip(vx.chunks(chunk))
            .zip(vy.chunks(chunk))
        {
            s.spawn(move || motion(p, q, u, v, dt));
        }
    });
}

fn median5(mut sample: impl FnMut() -> f64) -> f64 {
    let mut v: Vec<f64> = (0..5).map(|_| sample()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[2]
}

struct Cols {
    px: Vec<f32>,
    py: Vec<f32>,
    vx: Vec<f32>,
    vy: Vec<f32>,
}

fn make_cols(n: usize, rng: &mut Lcg) -> Cols {
    Cols {
        px: vec![0.0; n],
        py: vec![0.0; n],
        vx: (0..n).map(|_| rng.unit() - 0.5).collect(),
        vy: (0..n).map(|_| rng.unit() - 0.5).collect(),
    }
}

/// Per-call nanoseconds for the single-threaded pass.
fn time_single(c: &mut Cols, iters: u64) -> f64 {
    median5(|| {
        let t = Instant::now();
        for _ in 0..iters {
            motion(&mut c.px, &mut c.py, &c.vx, &c.vy, DT);
        }
        black_box(c.px[c.px.len() - 1]);
        t.elapsed().as_nanos() as f64 / iters as f64
    })
}

/// Per-call nanoseconds for the `threads`-way parallel pass.
fn time_parallel(c: &mut Cols, threads: usize, iters: u64) -> f64 {
    median5(|| {
        let t = Instant::now();
        for _ in 0..iters {
            motion_parallel(&mut c.px, &mut c.py, &c.vx, &c.vy, DT, threads);
        }
        black_box(c.px[c.px.len() - 1]);
        t.elapsed().as_nanos() as f64 / iters as f64
    })
}

fn gbps(n: usize, per_call_ns: f64) -> f64 {
    n as f64 * BYTES_PER_ELEM / per_call_ns // bytes/ns == GB/s
}

fn iters_for(n: usize) -> u64 {
    (2_000_000_000 / n as u64).clamp(20, 200_000)
}

fn main() {
    let cores = thread::available_parallelism()
        .map(|c| c.get())
        .unwrap_or(1);

    // ---- 1. One core's reach across the cache hierarchy ----
    println!("== 1. One core's reach: SoA motion pass, single thread ==");
    println!("the loop autovectorizes; at scale it is bound by memory bandwidth.\n");
    println!(
        "{:>12} {:>12} {:>12} {:>12}",
        "elements", "footprint", "ns/elem", "GB/s"
    );
    let mut rng = Lcg::new(0x0000_B19F);
    let sizes = [
        1 << 12,
        1 << 14,
        1 << 16,
        1 << 18,
        1 << 20,
        1 << 22,
        1 << 24,
    ];
    for &n in &sizes {
        let mut c = make_cols(n, &mut rng);
        let iters = iters_for(n);
        let ns = time_single(&mut c, iters);
        println!(
            "{:>12} {:>10.1} MB {:>12.3} {:>12.1}",
            n,
            n as f64 * 16.0 / 1.0e6, // 4 arrays x 4 bytes resident
            ns / n as f64,
            gbps(n, ns)
        );
    }

    // ---- 2. Scaling to the box ceiling ----
    let n = 1 << 24; // 256 MB working set, well past L3 - bandwidth-bound
    let mut c = make_cols(n, &mut rng);
    let iters = iters_for(n);
    let single = time_single(&mut c, iters);

    println!("\n== 2. Scaling to the box ceiling ({n} elements, RAM-resident) ==");
    println!("more cores stop helping once the memory channel saturates.\n");
    println!("{:>10} {:>12} {:>10}", "threads", "GB/s", "speedup");
    let mut thread_counts = vec![1usize, 2, 4, 8];
    if !thread_counts.contains(&cores) {
        thread_counts.push(cores);
    }
    let mut all_core_ns = single;
    for &t in &thread_counts {
        let ns = if t == 1 {
            single
        } else {
            time_parallel(&mut c, t, iters)
        };
        if t == *thread_counts.last().unwrap() {
            all_core_ns = ns;
        }
        println!("{:>10} {:>12.1} {:>9.2}x", t, gbps(n, ns), single / ns);
    }

    // ---- 3. The active-set budget: what one box keeps current in a frame ----
    let one_core_per_elem = single / n as f64;
    let all_core_per_elem = all_core_ns / n as f64;
    let one_core_budget = FRAME_NS / one_core_per_elem;
    let all_core_budget = FRAME_NS / all_core_per_elem;

    println!("\n== 3. The active-set budget (33 ms frame at 30 Hz) ==");
    println!(
        "one core keeps {:.1} M elements current per frame; {cores} cores keep {:.1} M.",
        one_core_budget / 1.0e6,
        all_core_budget / 1.0e6
    );
    println!(
        "\nThis is the reframe. A 1B-node graph does not fit a frame on any single box - but you"
    );
    println!(
        "never recompute it. Projects B and C update only the active cone, and a cone of a few"
    );
    println!(
        "million cells fits one core's frame budget with room to spare. The GPU is irrelevant"
    );
    println!(
        "to staleness: it answers a question (recompute everything, fast) you stopped asking."
    );

    // ---- 4. The GPU break-even: a cost model, not a measurement ----
    // There is no GPU on this box. These constants are ASSUMED order-of-magnitude figures
    // (PCIe 4.0-ish one-way bandwidth, a typical launch latency); a real run is pending a
    // GPU host. The point is the structure, into which the reader plugs their own numbers.
    let pcie_gbps = 16.0; // assumed: GB/s one way
    let launch_us = 5.0; // assumed: kernel-launch latency, microseconds
    // To offload one pass of CPU-resident data we must ship 16 B/elem to the device and
    // read 8 B/elem back: 24 B/elem round trip, same as the compute's own traffic.
    let transfer_ns_per_elem = BYTES_PER_ELEM / pcie_gbps; // ns/elem (bytes/(GB/s) == ns)

    println!("\n== 4. GPU break-even (cost model; GPU constants are ASSUMED, not measured) ==");
    println!(
        "assumed: PCIe {pcie_gbps:.0} GB/s one way, launch {launch_us:.0} us. Real run pending a GPU host."
    );
    println!(
        "\nround-trip transfer for this pass: {transfer_ns_per_elem:.3} ns/elem; the CPU all-core pass: {all_core_per_elem:.3} ns/elem."
    );
    if transfer_ns_per_elem >= all_core_per_elem {
        println!(
            "Just shipping CPU-resident data to the device and back already costs more than doing"
        );
        println!(
            "the pass on the box. For a bandwidth-bound kernel, offload only wins when the data"
        );
        println!(
            "already lives on the device, or the arithmetic intensity is high enough that compute"
        );
        println!(
            "- not transfer - dominates. Otherwise the bus is the bottleneck, and the GPU loses."
        );
    } else {
        println!(
            "Transfer is cheaper than the CPU pass here; offload can pay above the launch-latency"
        );
        println!(
            "break-even of ~{:.0} K elements (launch / transfer-saved per element).",
            (launch_us * 1000.0 / (all_core_per_elem - transfer_ns_per_elem)) / 1000.0
        );
    }
    println!(
        "\nThe arc's close: columns are the precondition for all of this, but they are a default,"
    );
    println!("not a law - and neither is the GPU. You reach for more hardware when the active set");
    println!(
        "itself outgrows the box, not to brute-force away staleness an incremental design avoids."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_advances_position() {
        let mut px = vec![0.0f32, 1.0, 2.0];
        let mut py = vec![0.0f32; 3];
        let vx = vec![1.0f32, 1.0, 1.0];
        let vy = vec![2.0f32; 3];
        motion(&mut px, &mut py, &vx, &vy, 0.5);
        assert_eq!(px, vec![0.5, 1.5, 2.5]);
        assert_eq!(py, vec![1.0, 1.0, 1.0]);
    }

    #[test]
    fn parallel_matches_serial_bit_for_bit() {
        // Independent per element, identical arithmetic, so chunking across threads must
        // give bit-identical results - the precondition for trusting the parallel pass.
        let n = 100_003; // not a multiple of the chunk count, to exercise the ragged tail
        let mut rng = Lcg::new(7);
        let base = make_cols(n, &mut rng);

        let (mut sx, mut sy) = (base.px.clone(), base.py.clone());
        motion(&mut sx, &mut sy, &base.vx, &base.vy, DT);

        for threads in [2usize, 3, 8, 16] {
            let (mut px, mut py) = (base.px.clone(), base.py.clone());
            motion_parallel(&mut px, &mut py, &base.vx, &base.vy, DT, threads);
            for i in 0..n {
                assert_eq!(px[i].to_bits(), sx[i].to_bits(), "threads={threads} i={i}");
                assert_eq!(py[i].to_bits(), sy[i].to_bits(), "threads={threads} i={i}");
            }
        }
    }
}
