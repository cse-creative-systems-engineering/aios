# Grounding Snapshot: Implement live state projections and durable A2UI surface runtime

## Current State

`feature/live-system-state` now contains the first executable slice of
accepted ADR-0012. The Coordinator owns a `SystemStateStore`, seeds it from
deterministic discovery, and refreshes permitted procfs observations (CPU
load, memory, network counters) immediately before a consent-gated,
query-relevant model projection. It retains bounded history and reports only
non-causal numeric trends.

The active Tauri canvas now has a backend-owned `SurfaceRuntime`. Surface
identity, intent, revision, declared `data-aios` bindings, layout, z-order,
visibility, close, session persistence, and restore are no longer split
between an in-memory frontend list and a bare backend HTML list. The frontend
only renders, measures, and submits user drag placement.

This snapshot accompanies the initial live-state runtime commit; `main`
remains untouched in its original worktree.

## Relevant Paths

- `src/state.rs` — typed observations, bounded history, projections, trends,
  graph ingestion, and the read-only procfs sampler.
- `src/coordinator/chat.rs`, `src/coordinator/planning.rs`, and
  `src/coordinator/mod.rs` — store ownership, discovery seeding, and
  consent-gated query projections.
- `src/surface/runtime.rs` and `src/session.rs` — durable unconstrained A2UI
  surface lifecycle and backward-compatible session serialization.
- `src-tauri/src/main.rs` and `frontend/src/main.ts` — backend runtime
  commands, placement persistence, and restored-canvas hydration.
- `docs/decisions/0012-live-system-state-and-a2ui-runtime.md` — accepted
  design; milestone 0006 records what this slice implements and defers.

## Open Work

1. Add collector contracts and continuous/event-driven sampling, beginning
   with process, GPU, thermal, and network correlation.
2. Deliver projection deltas to declared surface bindings without regenerating
   HTML, then add the explicit surface-edit revision protocol.
3. Add a proper isolation boundary for generated presentation and desktop E2E
   coverage for restore, drag persistence, and live binding updates.
4. Retire conversational specialist collection only after collector coverage
   and parity tests meet the ADR acceptance gates.
