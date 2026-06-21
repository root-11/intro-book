//! §56 discrete-GPU probe. Measures the motion pass - new position = old position
//! + velocity*dt, two multiply-adds and ~24 bytes moved per element, memory-bound -
//! three ways, and prints the two numbers that test the chapter's claim:
//!   - round-trip: upload the four arrays, run the kernel, read two back (offload of
//!     CPU-resident data). The claim is this LOSES to the CPU for a memory-bound pass.
//!   - resident: the kernel only, data already in VRAM. The claim is this WINS big,
//!     because VRAM bandwidth far exceeds system RAM (the "unless resident" caveat).
//!   - cpu: the same pass on the host, as the baseline.
//!
//! Cross-vendor via wgpu (Vulkan / Metal / DX12), so NVIDIA, AMD and Intel all run it;
//! no CUDA toolkit needed. `cargo run --release`.

use std::time::Instant;
use wgpu::util::DeviceExt;

const N: usize = 16_000_000; // 62,500 workgroups of 256, under wgpu's 65535/dim dispatch cap
const DT: f32 = 1.0e-3;
const BYTES_PER_ELEM: f64 = 24.0; // px,py read+written (8 each) + vx,vy read (4 each)
const REPS: u32 = 50;

const SHADER: &str = r#"
@group(0) @binding(0) var<storage, read_write> px: array<f32>;
@group(0) @binding(1) var<storage, read_write> py: array<f32>;
@group(0) @binding(2) var<storage, read>       vx: array<f32>;
@group(0) @binding(3) var<storage, read>       vy: array<f32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&px)) { return; }
    px[i] = px[i] + vx[i] * f32(0.001);
    py[i] = py[i] + vy[i] * f32(0.001);
}
"#;

fn main() {
    pollster::block_on(run());
}

async fn run() {
    let bytes = (N * 4) as u64;

    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .expect("no GPU adapter found");
    let info = adapter.get_info();
    println!(
        "adapter        : {} ({:?}, {:?})",
        info.name, info.device_type, info.backend
    );

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(), // 64 MB buffers fit the defaults
            },
            None,
        )
        .await
        .expect("request_device failed (try lowering N if the buffer exceeds limits)");

    // host data
    let px: Vec<f32> = (0..N).map(|i| (i as f32 * 0.000_001).fract()).collect();
    let py = px.clone();
    let vx = px.clone();
    let vy = px.clone();

    let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
    let mk = |data: &[f32]| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(data),
            usage,
        })
    };
    let b_px = mk(&px);
    let b_py = mk(&py);
    let b_vx = mk(&vx);
    let b_vy = mk(&vy);
    let staging = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: bytes,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: None,
        module: &module,
        entry_point: "main",
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    });
    let bgl = pipeline.get_bind_group_layout(0);
    let bind = |a: &wgpu::Buffer, b: &wgpu::Buffer, c: &wgpu::Buffer, d: &wgpu::Buffer| {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: a.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: b.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: c.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: d.as_entire_binding() },
            ],
        })
    };
    let bind_group = bind(&b_px, &b_py, &b_vx, &b_vy);
    let groups = (N as u32).div_ceil(256);

    let dispatch = |bg: &wgpu::BindGroup| {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            cp.set_pipeline(&pipeline);
            cp.set_bind_group(0, bg, &[]);
            cp.dispatch_workgroups(groups, 1, 1);
        }
        enc.finish()
    };

    // --- 1. CPU all-core baseline (scoped threads, no extra crate) ---
    let ncpu = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let mut cx = px.clone();
    let mut cy = py.clone();
    let cpu_pass = |cx: &mut [f32], cy: &mut [f32]| {
        let chunk = (cx.len() + ncpu - 1) / ncpu;
        std::thread::scope(|s| {
            for (((xc, yc), vxc), vyc) in cx
                .chunks_mut(chunk)
                .zip(cy.chunks_mut(chunk))
                .zip(vx.chunks(chunk))
                .zip(vy.chunks(chunk))
            {
                s.spawn(move || {
                    for i in 0..xc.len() {
                        xc[i] += vxc[i] * DT;
                        yc[i] += vyc[i] * DT;
                    }
                });
            }
        });
    };
    for _ in 0..3 { cpu_pass(&mut cx, &mut cy); }
    let t = Instant::now();
    for _ in 0..REPS { cpu_pass(&mut cx, &mut cy); }
    let cpu_ns = t.elapsed().as_nanos() as f64 / REPS as f64 / N as f64;

    // --- 2. round-trip: upload, kernel, read back ---
    let round_trip = || {
        queue.write_buffer(&b_px, 0, bytemuck::cast_slice(&px));
        queue.write_buffer(&b_py, 0, bytemuck::cast_slice(&py));
        queue.write_buffer(&b_vx, 0, bytemuck::cast_slice(&vx));
        queue.write_buffer(&b_vy, 0, bytemuck::cast_slice(&vy));
        let cmd = dispatch(&bind_group);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_buffer_to_buffer(&b_px, 0, &staging, 0, bytes);
        queue.submit([cmd, enc.finish()]);
        staging.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        device.poll(wgpu::Maintain::Wait);
        staging.unmap();
    };
    round_trip(); // warm (driver/context init)
    let t = Instant::now();
    for _ in 0..REPS { round_trip(); }
    let trip_ns = t.elapsed().as_nanos() as f64 / REPS as f64 / N as f64;

    // --- 3. resident: kernel only, data already in VRAM ---
    let resident = || {
        let cmd = dispatch(&bind_group);
        queue.submit([cmd]);
        device.poll(wgpu::Maintain::Wait);
    };
    for _ in 0..3 { resident(); }
    let t = Instant::now();
    for _ in 0..REPS { resident(); }
    let res_ns = t.elapsed().as_nanos() as f64 / REPS as f64 / N as f64;

    println!("N              : {N} f32  ({BYTES_PER_ELEM} bytes/elem)");
    println!("CPU all-core   : {cpu_ns:7.3} ns/elem  ({:.1} GB/s, {ncpu} threads)", BYTES_PER_ELEM / cpu_ns);
    println!("GPU round-trip : {trip_ns:7.3} ns/elem  (upload + kernel + download)");
    println!("GPU resident   : {res_ns:7.3} ns/elem  (data already in VRAM)");
    println!(
        "verdict        : offloading CPU-resident data {} ({:.1}x the CPU pass); a resident kernel is {:.1}x the CPU pass",
        if trip_ns > cpu_ns { "LOSES" } else { "wins" },
        trip_ns / cpu_ns,
        cpu_ns / res_ns
    );
}
