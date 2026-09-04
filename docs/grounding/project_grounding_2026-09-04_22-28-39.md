# Grounding Snapshot: Redesign Aios sidebar UI for an ultra-premium instrument look and remove fixed sizing from generated surfaces

## Current State

- The Aios frontend has a refined dark instrument aesthetic: obsidian foundation, teal accent, Inter + IBM Plex Mono typography, improved contrast and spacing, premium interactions and polished loading/empty/error states.
- `frontend/index.css` now drives the premium sidebar UI while preserving the surface renderer, canvas geometry and input-region behavior.
- Generated `SurfaceComposition` surfaces are no longer constrained to a predetermined size: the model prompt and stub provider HTML no longer set fixed `width`/`height`, and `frontend/index.css` surfaces use `max-content` sizing with CSS clamped bounds and `overflow: auto` to prevent clipping.
- Feature branch `feature/live-system-state` was used as the base; `devin/ui-redesign-live` carries the merged changes.

## Relevant Paths

- `frontend/index.css` — complete UI redesign and surface-host sizing/clipping rules.
- `src/surface/composer.rs` — `unconstrained_generation_instructions()` now instructs the model to avoid fixed root sizing and prefer content-fitting bounds.
- `src/bin/stub_provider.rs` — stub surface HTML no longer emits fixed `width`/`height` and stays draggable via `data-tauri-drag-region`.
- `frontend/dist/index.html` — rebuilt to reference the new Vite assets.

## Open Work

- Open a PR from `devin/ui-redesign-live` into the appropriate feature branch (`feature/live-system-state` unless the user specifies otherwise) and monitor CI.
- Confirm the three pre-existing `cargo test` failures (`coordinator::tests::power_diagnose_reports_domain_invariants`, `coordinator::tests::power_observe_runs_through_broker`, `exec::tests::exec_runs_echo`) are environmental and unrelated to this UI/surface work.
