# Grounding Snapshot: Move A2UI revisions into the Aios chat composer

## Current State

Surface edits now stay within Aios's visible conversation. The canvas Edit
control hands its stable surface ID and observed revision to the sidebar; the
sidebar opens a targeted composer state and submits the explicit revision
request through the existing optimistic-concurrency boundary. A successful
revision is broadcast as `surface_lifecycle`, so the canvas and sidebar both
converge on the complete persisted record.

The user sees the instruction and the resulting revision confirmation in the
chat history. Cancellation leaves the surface untouched. Failed or stale
revision requests leave the existing surface intact and produce an explicit
chat failure state. This follows GPU collector support in commit `048b1df` and
will be committed with the current UX implementation.

## Relevant Paths

- `frontend/src/main.ts` — revision handoff event, targeted composer state,
  and revision submission/feedback.
- `frontend/src/sidebar.ts` — revision target display, cancellation, and
  contextual composer placeholder.
- `src-tauri/src/main.rs` — broadcasts a successful revision to both desktop
  webviews.
- `tests/wdio/live-surfaces.e2e.cjs` — drives the native canvas-to-chat edit
  flow without a browser prompt.
- `docs/ui.md` and `docs/message-protocol.md` — user-visible and event
  contracts for the handoff.

## Open Work

- Consider richer vendor-specific GPU metrics only when a stable read-only
  source and fixtures can prove their unit and freshness semantics.
- ADR-0012's acceptance gates are complete on the feature branch. Before
  merging, run the full native desktop journey on the intended target hardware
  and configure real provider credentials in the UI for a production model
  smoke test.
