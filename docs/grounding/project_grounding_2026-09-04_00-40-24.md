# Grounding Snapshot: Add continuously refreshed CPU, process, network, and thermal live state

## Current State

Commit `5a9acbd` established ADR-0012's bounded observation store, consent-gated
context projection, and durable A2UI surface runtime. This change makes the
Linux host observations continuously live in the desktop process: CPU
utilization, top process CPU use and names, network byte rates, and thermal
zones refresh once per second while the backend is idle. Native builds are
limited to two jobs to avoid saturating the workstation during C++ compilation.

## Relevant Paths

- `src/state.rs` — deterministic procfs/sysfs collector state and relevant
  projections, including paired process names for top-process requests.
- `src-tauri/src/main.rs` — refreshes the observation plane independently of
  prompt traffic.
- `.cargo/config.toml` — caps Cargo and CMake concurrency at two jobs.
- `tests/ui_e2e.rs` — its stub configuration now assigns chat, surface, and
  verification roles.

## Open Work

- Add the versioned projection/delta protocol and bind live surface data to it.
- Add GPU collector support through the selected runtime capability adapter.
- The current external WebKit driver reaches only `about:blank` in this host's
  mixed Wayland/XWayland session; deterministic unit and integration harnesses
  pass, but manual desktop validation remains required until the test backend
  is replaced or isolated.
