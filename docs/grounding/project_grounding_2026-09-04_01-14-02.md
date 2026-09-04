# Grounding Snapshot: Add isolated native and live OpenRouter provider onboarding E2E coverage

## Current State

The native desktop test harness now starts every run with an empty, isolated
configuration and drives the actual settings UI: provider catalog selection,
credential entry, model discovery, per-role assignment, chat, surface
generation, and surface close. Its deterministic OpenAI-compatible fixture
advertises separate chat, verification, and surface models and makes the
selected routing observable in the UI output.

There is also an explicit, opt-in live OpenRouter suite. It reads the
operator's local OpenRouter key only at execution time, never logs it, adds
OpenRouter through the visible form, discovers the current `/models` catalog,
selects an ID ending in `:free`, assigns it to all three core roles, and
completes a real chat request. The test passed on this workstation on
2026-09-04. It is deliberately excluded from the normal deterministic suite.

Before this change, the embedded-WebDriver test only began from a
preconfigured local stub provider and therefore did not validate the
user-facing configuration journey or any real provider call.

## Relevant Paths

- `tests/wdio/live-surfaces.e2e.cjs` — deterministic full onboarding and
  multi-surface regression journey.
- `tests/wdio/live-provider.e2e.cjs` and
  `scripts/ui-e2e-live-openrouter.sh` — opt-in real OpenRouter free-model
  discovery, role assignment, and chat check.
- `tests/wdio.conf.cjs` — defaults safely to the deterministic suite; accepts
  a single opt-in spec through `AIOS_UI_SPEC`.
- `scripts/ui-e2e.sh` — local fixture lifecycle and a unique temporary
  configuration/session directory per test run.
- `src-tauri/src/main.rs` — WebDriver-only catalog-endpoint override used by
  the isolated local fixture, never by normal builds.
- `src/bin/stub_provider.rs` — `/models` response and selected-model markers
  used to assert the route selected by the settings UI.

## Open Work

- Add the versioned projection/delta protocol and update declared A2UI
  bindings in place without regenerating HTML.
- Add GPU collection through a runtime capability adapter and then implement
  bounded multi-domain correlation findings.
- Add surface-edit routing and an isolation boundary for generated
  presentation before retiring the existing conversational specialist path.
- The test dependency probe reports missing Debian package names even though
  this desktop test environment is functional; treat that advisory separately
  from an actual native dependency failure.
