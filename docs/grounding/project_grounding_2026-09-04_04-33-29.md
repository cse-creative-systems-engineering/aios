# Grounding Snapshot: Add read-only AMD and Intel GPU collectors

## Current State

The feature branch now has read-only GPU runtime coverage for NVIDIA, AMD, and
Intel hardware. NVIDIA continues to use `nvidia-smi`; AMD and Intel use the
compiled-in Linux DRM/sysfs adapter selected only after exact PCI-vendor
discovery. All adapters are observational and publish only values the host
actually exposes.

GPU metric keys are collision-safe on mixed-driver hosts. NVIDIA preserves
numeric IDs such as `gpu.0.*`; DRM adapters use `gpu.amd-card0.*` and
`gpu.intel-card1.*`. Only NVIDIA's adapter currently publishes a
GPU-to-process-memory relationship, so the existing temporal correlation
finding remains limited to evidence it can prove and never attributes host
network traffic to a process.

This follows the durable A2UI lifecycle commit `addca9b`. The next commit will
contain this collector expansion and this snapshot.

## Relevant Paths

- `src/gpu_runtime.rs` — compiled adapter catalog, DRM vendor discovery,
  read-only sysfs collection, and fixture-style unit coverage.
- `src/state.rs` — ingests multiple adapters with per-adapter provenance and
  stable GPU key segments.
- `docs/modules/gpu.md` — collector support, stable key contract, and
  correlation limits.

## Open Work

- Replace the basic native surface Edit prompt with an Aios conversation
  affordance while retaining its explicit ID/revision concurrency contract.
- Add richer vendor-specific metrics only when a stable read-only source and
  fixtures can prove their unit and freshness semantics.
