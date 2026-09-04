# Grounding Snapshot: Replace external desktop driver with embedded Wayland-compatible E2E coverage

## Current State

Commit `78ce93f` made the desktop shell native-Wayland-first. This change
replaces the unusable external `tauri-driver` / `WebKitWebDriver` path with
Tauri's in-process WebDriver server, enabled only in the `webdriver` build
feature. The test suite now launches a real Aios binary, a local OpenAI-style
stub provider, the sidebar and canvas windows, then verifies five generated
surfaces accumulate with verified bindings and close cleanly.

The desktop harness is self-contained and does not read credentials or contact
external model providers. Cargo/CMake build concurrency remains capped at two
jobs from the earlier collector change, which keeps native test builds safe on
this workstation.

## Relevant Paths

- `src-tauri/Cargo.toml`, `src-tauri/src/main.rs`, and
  `src-tauri/capabilities/default.json` — optional embedded WebDriver server
  and its narrowly scoped capability manifest.
- `tests/wdio.conf.cjs`, `tests/wdio/live-surfaces.e2e.cjs`, and
  `scripts/ui-e2e.sh` — native desktop test configuration, user-flow test,
  build, stub-provider lifecycle, and cleanup.
- `src/bin/stub_provider.rs` — fixture now chooses domain-relevant tools and
  parses JSON request content before echoing valid `data-aios` evidence.
- `docs/ui.md` — current desktop test transport and the remaining static-data
  limitation.

## Open Work

- Add the versioned projection/delta protocol and update declared A2UI
  bindings in place without regenerating HTML.
- Add GPU collection through a runtime capability adapter and then implement
  bounded multi-domain correlation findings.
- Add surface-edit routing and an isolation boundary for generated
  presentation before retiring the existing conversational specialist path.
