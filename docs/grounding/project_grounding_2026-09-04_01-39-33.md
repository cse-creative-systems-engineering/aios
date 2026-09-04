# Grounding Snapshot: Add NVIDIA GPU runtime collector and bounded temporal correlations

## Current State

The live state store now has a compiled-in, read-only NVIDIA runtime adapter.
On Linux it discovers `nvidia-smi` once, samples it no more often than every
five seconds, and only publishes values the driver actually supplied. The
collector exposes stable GPU/device/process projection keys without fabricating
an unavailable GPU or treating `N/A` as zero.

`SystemStateStore` also now emits a bounded `temporal_overlap` finding for a
same-GPU process, an increasing GPU temperature, and contemporaneous
non-loopback host-interface traffic. It carries source keys, its threshold and
window rule, freshness, and an explicit non-causation disclaimer. General
trend findings now exclude stale metrics and expose the same provenance fields.

This snapshot accompanies the GPU collector commit that follows
`1af3b11` (projection-keyed live surface deltas).

## Relevant Paths

- `src/gpu_runtime.rs` — fixed-command NVIDIA capability adapter and parsers.
- `src/state.rs` — adapter lifecycle, stable metric ingestion, freshness-aware
  findings, and bounded same-GPU/network overlap analysis.
- `src/lib.rs` — exposes the adapter module.
- `docs/modules/gpu.md` — documents absence semantics, sampling, projection
  keys, and the non-causal finding contract.
- `docs/decisions/0012-live-system-state-and-a2ui-runtime.md` — governing
  collector and interpretation decision.

## Open Work

- Add other signed, compiled-in adapters (for example AMD/Intel) only with
  equivalent read-only parsing and fixture coverage; do not dynamically compile
  hardware-provided code.
- Add attribution-quality process-network evidence before claiming a process is
  the source of an interface's traffic. The current finding intentionally says
  only that a host interface carried traffic during the overlap.
- Continue hardening surface edit/revision isolation and add broader
  collector-to-A2UI integration coverage.
