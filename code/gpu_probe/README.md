# gpu_probe - the §56 discrete-GPU measurement

§56 argues that a memory-bound pass (advance positions: two multiply-adds, ~24 bytes
moved per element) is bound by the memory channel, and that offloading it to a GPU is a
bus-versus-memory trade. The reference machines (Pi 4, i7, i3, the dev box's 780M) all
have only *integrated* GPUs, which share the memory channel - so the *discrete*-GPU case
the cost model assumes is the one cell `code/heterogeneous` cannot fill. This probe fills it.

It times the motion pass three ways and prints the verdict:

- **CPU all-core** - the baseline (scoped threads, no extra crate).
- **GPU round-trip** - upload the four arrays, run the kernel, read two back. This is
  offloading CPU-resident data; for a memory-bound pass the claim is it *loses*.
- **GPU resident** - the kernel only, data already in VRAM. The claim is it *wins* big,
  because VRAM bandwidth far exceeds system RAM (the "unless the data is resident" caveat).

Cross-vendor via [`wgpu`](https://crates.io/crates/wgpu) (Vulkan / Metal / DX12), so NVIDIA,
AMD, and Intel all run it with no CUDA toolkit.

```
cargo run --release
```

If you have a discrete GPU, please run it and post the output plus your GPU model and PCIe
generation - it turns the last hypothetical in the book into a measurement.
