# Grounding Snapshot: Add projection-keyed live surface deltas with in-place canvas updates

## Current State

The live system-state branch now carries versioned, projection-keyed A2UI
updates in addition to the runnable native desktop and OpenRouter onboarding
coverage from `3e69c87`.

`SystemStateStore` publishes only exact, non-stale values for declared
projection keys. `SurfaceRuntime` records those values separately from the
model-authored visual revision and emits monotonic `SurfaceDelta` payloads.
The canvas applies them directly to matching `data-aios` elements, preserving
the existing HTML, placement, dimensions, z-order, and surface identity.

The deterministic native harness now proves the full user-visible provider
onboarding and multi-surface flow, then injects a WebDriver-only collector
sample through normal Tauri IPC. It verifies that a live CPU projection binding
changes in place and the original surface identity remains intact. The test
hook is compiled only with the `webdriver` feature and has no production entry
point.

Projection relevance now ignores generic presentation terms such as
"generate", "surface", and "usage" so a CPU request cannot be dominated by
unrelated filesystem usage metrics.

## Relevant Paths

- `src/state.rs` — exact fresh binding lookup and domain-focused projection
  term handling.
- `src/surface/runtime.rs`, `src/session.rs` — persisted data revisions and
  delta lifecycle, distinct from visual revisions.
- `src-tauri/src/main.rs` — collector refresh emits only declared binding
  deltas; WebDriver test sample route is feature-gated.
- `frontend/src/main.ts` — canvas applies delta text replacements without a
  surface rerender.
- `src/surface/composer.rs` — accepts stable dotted projection keys in
  `data-aios` declarations.
- `tests/wdio/live-surfaces.e2e.cjs` — asserts the in-place update journey.
- `docs/message-protocol.md`, `docs/ui.md` — desktop delta contract and
  user-visible lifecycle semantics.

## Open Work

- Add the GPU runtime capability adapter and bounded multi-domain correlation
  findings.
- Add user-directed surface edit routing and a stronger presentation isolation
  boundary before retiring the conversational specialist path.
- The default native suite passes. The opt-in OpenRouter suite remains a
  separate external-state check using a current free model.
