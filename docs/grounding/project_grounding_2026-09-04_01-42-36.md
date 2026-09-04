# Grounding Snapshot: Add isolated user-directed A2UI surface revisions

## Current State

Generated A2UI surfaces now support a user-directed visual revision path. The
canvas Edit control targets a stable surface ID and sends the visual revision
the client observed. The backend regenerates against that exact record's prior
HTML and fresh scoped state projection, applies the normal fidelity gate, then
atomically replaces only that record. A stale revision is rejected rather than
overwriting a newer design.

Revision keeps the target surface's ID and layout, advances visual `revision`,
and reinitializes only its declared live bindings. It remains separate from
`dataRevision`, which is used for in-place collector updates. The native
Wayland/WebDriver journey now proves a provider can be onboarded, role models
assigned, four surfaces generated, a CPU binding updated in place, and that
CPU surface revised without replacing the other surfaces.

This snapshot accompanies the surface-revision commit that follows `c12d98c`
(NVIDIA live-state collector and bounded temporal correlations).

## Relevant Paths

- `src/surface/runtime.rs` — target lookup plus optimistic-concurrency,
  atomic one-surface revision operation.
- `src-tauri/src/main.rs` — typed revision command, worker request, fresh
  scoped evidence, fidelity gate, and persistence.
- `frontend/src/main.ts` — per-surface Edit action and authoritative record
  replacement after a successful revision.
- `tests/wdio/live-surfaces.e2e.cjs` — visible native desktop revision and
  cross-surface isolation coverage.
- `docs/ui.md` and `docs/message-protocol.md` — lifecycle and transport
  contract for visual revisions.

## Open Work

- Replace the basic native Edit prompt with a richer Aios conversation affordance
  if it can preserve the same explicit ID/revision contract.
- Add minimize/restore and explicit z-order lifecycle state, then exercise
  resize, drag, restart, and concurrent-revision recovery in the desktop suite.
- Add AMD/Intel read-only GPU adapters with equivalent fixture coverage; retain
  the current non-causal, host-interface-only correlation wording until
  per-process traffic attribution is available.
