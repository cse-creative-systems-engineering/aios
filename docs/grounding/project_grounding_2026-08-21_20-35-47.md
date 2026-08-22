# Grounding Snapshot: Groundless Surfaces Validated Live, Provider Teardown Fix

## Current State

The groundless surface path from earlier today is no longer just compiling —
it ran all evening against real OpenRouter models on the owner's desktop and
renders. The owner confirmed the result matches what he had in mind: the
canvas sizes itself to whatever the model drew (the old fixed frame that
clipped widgets is gone) and generated surfaces can be dragged around the
screen. One surface slot still exists, so a second request still replaces the
first; the multi-surface lifecycle remains open work.

What it took to get there, in order:

- The typed widget IR removal (`99f8d32`) left the relay as the only path.
  First live runs failed for reasons outside the refactor: a stealth reasoning
  model timed out at 60s, and `dots-studio/dots-3-note-preview:free` returned
  empty content because its thinking consumed the whole token budget. Both
  failure modes appear verbatim in the Aug 18 audit log under the old typed
  path, so they were never caused by the flip.
- Removing a provider in Settings tore down config but left the runtime
  registry entry behind, so the panel kept listing a ghost that could be
  neither re-added ("model already registered") nor re-keyed ("is not
  configured"), and every later status snapshot failed on it. Fixed:
  `ModelRegistry::deregister_provider` and `ModelGateway::unregister_backend`
  exist now, `Coordinator::remove_provider` calls both, and the sidebar
  snapshot skips orphan registry entries with a warning instead of freezing
  (`17945fe`).
- The fidelity gate rejected a correct generation: dots-3 rendered
  `rss_kb=167080` as "167 MB", which the prompt explicitly allows ("you may
  re-shape values (formatting, units)") but the gate did not. The gate now
  compares numbers across decimal and binary k/M/G ratios with 1% slack;
  invented numbers still fail. Composition max tokens floor went 4096 → 8192
  so reasoning models have room to finish, and the empty-content error now
  says what actually happened instead of "message has no content or tool
  calls" (`8ccc9fa`). With that, dots-3 renders end to end.

Test baseline: `cargo test --lib` is 394 passing tests (391 before tonight),
one ignored real-model test.

## Relevant Paths

- `src/surface/composer.rs` — relay call, coverage gate, fidelity gate with
  unit-scale matching (`UNIT_SCALES`, `numbers_match`)
- `src/model.rs` — `deregister_provider`, `unregister_backend`
- `src/coordinator/providers.rs` — remove_provider full teardown
- `src/coordinator/mod.rs:228` — compose_max_tokens floor
- `src/http.rs` — empty-content error wording
- `src-tauri/src/main.rs` — status snapshot skips orphans
- `~/.aios/config.toml` — owner's live config: one openrouter provider, all
  roles on `dots-studio/dots-3-note-preview:free`, 60s timeout

## Open Work

- M8 lifecycle stages 2–4 (surface manager, several surfaces at once,
  persistent editing) per `docs/milestones/0002-multi-surface-lifecycle-plan.md`.
  The single-slot renderer lives at `previous_experimental_html` in
  `src-tauri/src/main.rs`.
- Sidebar polish and full chat experience per milestone 0003.
- Safety/adversarial test families from testing-strategy.md.
- Reasoning models are still a poor fit for the surface role; the error
  message now says so, but a role-level "recommended models" hint could help.
