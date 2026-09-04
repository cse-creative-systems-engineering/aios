# Grounding Snapshot: Harden A2UI surface lifecycle with durable visibility, z-order, resize, and stale binding state

## Current State

ADR-0012's A2UI lifecycle runtime now has durable visibility, ordering, and
user-selected size. A minimized surface remains a persisted `SurfaceRecord`
and can be restored by its stable ID from the resident Surfaces inspector.
Visual revision is untouched by lifecycle changes. Pointer interaction raises
the focused card using backend-owned z-order; a resize grip persists an
explicit size only after user interaction, retaining unconstrained intrinsic
model sizing for new surfaces.

Surface bindings now carry explicit stale state. An expired observation retains
its last verified text but causes its exact `data-aios` element to receive a
visible stale marker and tooltip. Fresh data clears it through the ordinary
delta stream. The canvas recalculates/clears its input shape on all lifecycle
changes and restores records after its webview restarts.

## Relevant Paths

- `src/surface/runtime.rs` — layout validation, visibility, z-order, stale
  binding state, atomic binding snapshots, and lifecycle unit coverage.
- `src/state.rs` — exact `BindingSnapshot` separating fresh replacement
  values from explicitly stale keys.
- `src-tauri/src/main.rs` — lifecycle commands/events and durable persistence.
- `frontend/src/main.ts` and `frontend/src/sidebar.ts` — canvas controls and
  resident surface inspector/restore affordance.
- `tests/wdio/live-surfaces.e2e.cjs` — native minimize/restore, resize, and
  canvas-restart recovery coverage.

## Open Work

- Add read-only AMD and Intel GPU adapters with fixtures, keeping process
  traffic attribution out of correlation claims until a truthful collector is
  available.
- Replace the basic native surface Edit prompt with an Aios conversation
  affordance while retaining its explicit ID/revision concurrency contract.
