---
type: "query"
date: "2026-09-04T08:33:11.262375+00:00"
question: "How do GPU collectors discover hardware collect metrics normalize observations and test fixture data?"
contributor: "graphify"
outcome: "useful"
source_nodes: ["gpu_runtime.rs", "state.rs", "GpuRuntimeAdapter", "SystemStateStore"]
---

# Q: How do GPU collectors discover hardware collect metrics normalize observations and test fixture data?

## Answer

GPU runtime collectors are selected from a compiled-in catalog and feed SystemStateStore ingestion. NVIDIA uses nvidia-smi; AMD and Intel use Linux DRM/sysfs with vendor-qualified keys to prevent mixed-driver collisions. Missing fields remain absent and only adapters with truthful per-process memory may contribute to GPU/process correlation.

## Outcome

- Signal: useful

## Source Nodes

- gpu_runtime.rs
- state.rs
- GpuRuntimeAdapter
- SystemStateStore