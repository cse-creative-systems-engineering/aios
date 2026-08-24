# Grounding Snapshot: Workspace Co-Partner Stage 1+4 Landed (files, web, artifact scaffold)

## Current State

Commit `32ffc0f` on `main` lands the first code for milestone 0004
(workspace co-partner), ahead of its own docs-phase expectations: Stage 1
(Files/Data specialist) is implemented, Stage 3 (web fetch) has a working
specialist, and the Stage 4 artifact-preview surface exists as a scaffold.
Stage 2 (`stage_env`) and the multi-turn planner loop are still open.

What `32ffc0f` covers:

- **Files specialist** (`src/files.rs`, 585 lines): `FilesSpecialist` with
  observe + staged write/create tools; `FileDriver` implementing
  `ResourceDriver`. Workspace root defaults to `~/workspace`
  (`AIOS_WORKSPACE` overrides), artifacts to `~/.aios/artifacts`.
  `resource_to_path` maps `file:/workspace/**` and `file:/artifacts/**`
  resources onto real paths — anything else returns None, so out-of-scope
  writes fail before reaching the filesystem.
- **Prefix capabilities** (`src/capability.rs`): the ADR-0008 exception is in
  — a capability on `file:/workspace` or `file:/artifacts` covers its subtree;
  every other resource stays exact-match. This is the only prefix exception.
- **Web specialist** (`src/web.rs`, 300 lines): `WebSpecialist` +
  `handle_fetch` at risk 1 against the `web:fetch` pseudo-resource,
  `MockFetcher`/`LiveFetcher` behind a `Fetcher` trait, and
  `redact_secret()` applied to fetched content. Fetched content enters as
  evidence only and grants no capabilities.
- **Executor** (`src/executor.rs`): file resources stage arbitrary content
  (10 MiB guard) through the existing checkpoint → stage → health → commit
  path; file checkpoints are hermetically testable via temp dirs.
- **Guardian** (`src/guardian.rs`): DATA-003 (writes outside
  workspace/artifacts blocked unless inside scope) and DATA-004 wired as
  scoped block rules; DATA-003 evaluates the resource path, not just the
  operation.
- **Coordinator** (`coordinator/chat.rs`, `coordinator/mod.rs`): chat tool
  mapping recognizes `files.write_artifact` / `files.create_artifact`,
  resolves targets from `file:/workspace|artifacts` prefixes, and registers
  both file capabilities at boot.
- **Artifact surface scaffold** (`src-tauri/src/main.rs` ~L947): when a plan's
  file write/create succeeds, a deterministic artifact preview card renders
  alongside the groundless surface — it shows the actual ToolResult and file
  content, never an invented value.
- **Tests**: `tests/harness_drive.rs` (harness-driven file-write repro,
  direct-broker path) and `tests/ui_file_artifact.rs` (live-desktop e2e,
  `ignored` by default like the other UI tests).

Test baseline after this session: `cargo test --lib` = 410 passed, 1 ignored.
`cargo test --test harness_drive` = 2 passed. The e2e compile error in
`tests/ui_file_artifact.rs` (borrow-of-moved webdriver `client`) was fixed by
dropping the redundant `close()` call.

Docs automation added this session (separate from `32ffc0f`):
`scripts/check-docs.sh` (fails CI when code is newer than the newest grounding
snapshot; wired into `.github/workflows/ci.yml` as a `docs` job),
`scripts/new-grounding.sh` (creates + auto-links a dated snapshot),
`scripts/install-docs-hook.sh` (pre-commit backstop), and a mandatory
snapshot-discipline section in `AGENTS.md`.

## Relevant Paths

- `src/files.rs` — FilesSpecialist, FileDriver, workspace/artifacts roots
- `src/web.rs` — WebSpecialist, fetchers, redact_secret
- `src/capability.rs` — prefix-capability exception (ADR-0008)
- `src/guardian.rs` — DATA-003/DATA-004 scoped blocks
- `src/executor.rs` — file staging with size guard
- `src/coordinator/chat.rs`, `src/coordinator/mod.rs` — artifact tool mapping, boot registration
- `src-tauri/src/main.rs` — artifact preview card scaffold
- `tests/harness_drive.rs`, `tests/ui_file_artifact.rs` — new test coverage

## Open Work

- **Stage 2**: `stage_env` / dev-environment lifecycle in `packages.rs`
  (venv/toolchain staging, health = import or toolchain probe). Guardian
  PKG-003 not yet written.
- **Stage 4 completion**: multi-turn planner loop (planner emits tool calls,
  broker executes one by one, observations re-enter the loop, bounded turns)
  does not exist yet — current flow is single-shot. Artifact cards render but
  have no Save/Apply/Rollback actions yet.
- **Stage 5 hardening**: prefix-capability edge-case tests, live-run gate
  (`AIOS_LIVE_FILE_CONTROL=1` pattern), manual desktop baseline additions
  (create file, save artifact, failed write shows DENY).
- **Milestone 0002 leftovers**: surface editing/iteration v0.2 (design agreed,
  not started) and sidebar polish per milestone 0003.
- **Windows port planning**: Tauri shell depends on X11 layer-shell/EWMH dock
  behavior and WebKitGTK; WSL2 development will need a different desktop
  story. Not yet designed.

## Test Conditions

```bash
cargo test --lib                        # 410 passed, 1 ignored
cargo test --test harness_drive         # 2 passed
cargo build --manifest-path src-tauri/Cargo.toml   # desktop backend
npm run build --prefix frontend          # frontend bundle
```

UI e2e tests require a running webdriver (`scripts/ui-e2e.sh`) and are
`ignored` by default.
