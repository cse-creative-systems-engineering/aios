# Grounding Snapshot: Read-Only File Tool Handlers Registered at Boot

## Current State

Commit `a4074a` on `feature/workspace-co-partner/planner-loop` fixes a
runtime gap in the Stage 1 files specialist: `files.observe_file` and
`files.diagnose_file` had their tool *definitions* registered with the
broker, but no runtime handler, so any request failed with
`no specialist for files.observe_file`. Mutating file tools were unaffected
(they are served by the staged executor via `CompositeDriver`).

The fix follows the same pattern as every other specialist's boot block:
`spawn_specialist` handlers are registered for the two read-only tools,
delegating to `FilesSpecialist::observe` against the live graph. A
regression test (`harness_files_observe_has_handler` in
`tests/harness_drive.rs`) boots a coordinator and proves the observe path
returns a real result instead of the "no specialist" error.

Also this session: the feature branch was rebased onto current `main` so it
carries the grounding-freshness tooling (`scripts/check-docs.sh` etc.) and
the e2e compile fix from commits `937aad8`–`367abfd`.

Test baseline: `cargo test --lib` = 410 passed, 1 ignored;
`cargo test --test harness_drive` = 3 passed.

## Relevant Paths

- `src/coordinator/mod.rs` — Files boot block now spawns read-only handlers
  (mutating tools intentionally not spawned; executor serves them)
- `tests/harness_drive.rs` — new `harness_files_observe_has_handler` test

## Open Work

- Stage 4 completion: multi-turn planner loop (bounded observe→fetch→write→
  verify loop with batched plan-hash approval) and artifact card actions
  (Save/Apply/Rollback through the broker).
- Stage 2: `stage_env` dev-environment lifecycle in `packages.rs`; Guardian
  PKG-003 invariant.
- Stage 5 hardening: prefix-capability edge tests, live-run gate
  (`AIOS_LIVE_FILE_CONTROL=1`), extended desktop baseline.
- Milestone 0002 leftovers (surface editing v0.2) and sidebar polish (0003).
