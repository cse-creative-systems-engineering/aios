# Session Day-Buckets and Project Persistence

**Status:** Planned — docs only, no code in this phase
**Created:** 2026-08-22
**Starting checkpoint:** `32ffc0f` on `main` (workspace co-partner `file:/workspace` + `web:fetch` staged, artifact scaffold)
**Branch rule:** every code change for this milestone MUST land on `feature/session-day-buckets` (or a sub-branch off it). No direct commits to `main`. See ADR-0009.

## Purpose

Make Aios continue from day to day, login/logout, conversationally seamless. The sidebar is already persistent (`alwaysOnTop` + `skipTaskbar`), but the *session* is not. After `0004` you can `create a file hello.py` and see the artifact, but the next morning the canvas and chat that explain it are gone. This milestone adds day-buckets that survive reboots, projects that group sessions, and an always-contextually-aware view of the system.

This doc is the plan. No code is changed in this docs-only phase.

## Starting Point

What works now (post-0004 on `main` at `32ffc0f`):

- Staged file/web: `file:/workspace` + `file:/artifacts` prefix caps, `FileCheckpoint` (copy), `CompositeDriver {wifi, files}`, `web:fetch` risk 1 with `LiveFetcher`, artifact card deterministic + preview snippet.
- `ActionRecord` per-action WAL in `config_dir/actions/*.json`, `Facade` `history` + `last_tool_results` in-memory, `SystemGraph` discovered once at boot, `Vec<SurfaceCard>` in `src-tauri/src/main.rs` worker.

What is intentionally not done yet:

- No `Session`/`Project` in `SystemGraph` (`Project` `NodeType` exists but is not used for day-buckets).
- No `SessionStore` on disk — `history` drops on logout, surfaces drop on `kill`.
- No grouping of actions — file `file:/workspace/hello.py` is not owned by a session/day.
- No live graph refresh — `SystemGraph` is a boot snapshot, not a live view for always-on awareness.

## Target Behavior

### The day-bucket loop

```
Day N, login: Aios restores Session {2026-08-22, history, surfaces, graph_snapshot} from config_dir/sessions/2026-08-22.json
User: "continue the html server from yesterday" — Aios sees prior surfaces + files file:/workspace/projects/demo/2026-08-22/index.html
Planner → multi-step plan (observe graph → read prior session → write file:/workspace/projects/demo/2026-08-23/server.py)
Broker → gates each write (file:/workspace/projects prefix → Staged → Guardian DATA-003 → health → committed, same as single file)
Surface → new artifact card appended to the day's Vec<SurfaceCard> (existing cards stay, unioned InputRect)
Logout → SessionStore::save (write_file_synced + sync_dir)
Day N+1, login → same SessionStore::load → canvas re-hydrates yesterday's cards, chat history present
```

The user sees one conversation that never forgets, plus a canvas that accumulates that day's artifacts. Projects are `projects/<slug>/<date>/` on disk and `Project --owns--> Session --owns--> File` in the graph.

### Concrete scenarios (acceptance vignettes)

| Scenario | Steps | Expected outcome |
|---|---|---|
| Create `hello.py` on day 1, reboot, ask about it on day 2 | Day1: `create a file hello.py` → committed. Kill `aios-tauri`, boot. Day2: `what did we create yesterday?` | Assistant lists `file:/workspace/hello.py` from `SessionStore` history + shows prior artifact card still on canvas |
| Scaffold project `demo` with `index.html` + `server.py` | `scaffold project demo as python html server that says hello world` | Two files under `file:/workspace/projects/demo/2026-08-22/` staged in one `PlanHash`, artifact shows file tree, health = both files exist |
| Always-on awareness: disk fills while idle | Idle, disk `Degraded` → watchdog → on next prompt Aios mentions `storage:domain Degraded` without being asked | `SystemGraph` refreshed in background, not just at boot |
| Attempt to read yesterday's secret file | `fetch https://example.com/secret` on day 2 where content had `SECRET` | `redact_secret` drops line, audit stores redacted, no Secret in model prompt |
| Kill during `Staged` | `files.write_file` → `Staged` → `kill -9` → `boot` | `Staged` → `RolledBack` via recovery, checkpoint retained, file not half-written |

Failures visible: `Failed` retains checkpoint for `recover`, `RolledBack` shows `health_check` reason, no silent history loss.

## Capability & Safety Design

### Resources

- `project:<slug>` and `session:<YYYY-MM-DD>` as `NodeType::Project`/`Session` in `SystemGraph`. `Project --owns--> File` edges for `file:/workspace/projects/<slug>/**`.
- `file:/workspace/projects/**` as a new prefix capability (alongside `file:/workspace`/`file:/artifacts`). `file:/workspace/projects` token implies subtree. Still the only prefix exception to `capability-model.md` §3.3.
- `ActionRecord.session_id: Option<SessionId>` — existing `*.json` become `None` = pre-day era.

### Persistence

- `SessionStore` `config_dir/sessions/{date}.json` — `Session { id, project_id, history: Vec<Message>, tool_results, surfaces, graph_snapshot, updated_at }`, written via `write_file_synced` + `sync_dir` like `FileActionStore`.
- `Facade` `history` capped `max_history` but now persisted — the cap is per-session, not per-boot.
- `SystemGraph` live refresh: `discovery.rs` re-runs `discover_*` on a timer or on `ServiceStateChanged` events, not just at boot. `broker.resource_states` remains authoritative, but the graph's `last_observed`/`expires_at` are updated.

### Invariants

- `SESS-001`: a `Session` is present and restorable after `kill` → `boot`.
- `SESS-002`: a `File` is present and readable after `boot` if its `Action` was `Committed`.

## Module Spec Changes (docs phase)

This milestone does NOT edit Rust code. It updates these docs:

- `system-graph.md` §2.3 — add `Project`/`Session` node types, `owns`/`depends_on` lifecycle.
- `action-state-machine.md` §3 — add `session_id` to `ActionRecord`, recovery per-session.
- `capability-model.md` §3.1 — add `file:/workspace/projects` prefix, document `Project`/`Session` resources.
- `message-protocol.md` — `ToolResult` already has provenance; no change.
- `human-interaction.md` — sessions are day-long, not per-action; sidebar is session-persistent.

## Implementation Stages

### Stage 0: Lock the baseline (docs only, this PR)

1. Land ADR-0009 + milestone 0005 + roadmap/progress on `feature/session-day-buckets` (no code on `main`).
2. No code changes on this branch yet. All subsequent code lands via PR after tests.

Acceptance: docs build, `git branch` shows `feature/session-day-buckets` at `32ffc0f` + docs, `cargo test --lib` still 410.

### Stage 1: SessionStore + Facade persistence

Persist `Facade` `history` + `last_tool_results` to `sessions/{date}.json` with `write_file_synced`, load at `Coordinator::boot_with` before `configure_read_only_broker`. `kill` → `boot` → prior day's chat reappears.

Acceptance: vignette 1 (yesterday's `hello.py`) passes via harness `Coordinator::boot_with` after `kill`.

### Stage 2: Graph + surfaces persistence

Persist `Vec<SurfaceCard>` + `graph_snapshot` alongside history. Canvas re-hydrates on login (same unioned `InputRect` logic).

Acceptance: kill `aios-tauri` during staged, boot → canvas shows prior day's artifact cards.

### Stage 3: Project scaffolding

Add `project.scaffold` tool (risk 2) with tiny template registry (python/html/rust) expanding to N `files.write_file` under `file:/workspace/projects/<slug>/<date>/` in one `PlanHash`. `health` = all files exist.

Acceptance: `scaffold project demo` creates `index.html` + `server.py` in one commit, artifact shows file tree.

### Stage 4: Always-on awareness

Background `discovery` refresh on timer/event, `replay_log` GC per-session expiry.

Acceptance: idle disk `Degraded` is mentioned on next prompt without being asked.

### Stage 5: Hardening

`replay_log` expiry, `audit.log` per-session redaction, manual live run with `AIOS_WORKSPACE=/tmp/ws` days-long.

## Error & Fallback Rules

- No silent history loss — `SessionStore::save` failure → `audit_broken` → broker denies further actions (same as `action-state-machine.md` §5.3).
- No silent graph staleness — `expires_at` past → `Stale` health, not `Healthy`.
- No `run_any_command` — projects are files, not execution.

## Regression Gates

Every stage must pass on the feature branch:

```bash
cargo test --lib
cargo build --manifest-path src-tauri/Cargo.toml
npm run build --prefix frontend
cargo test --test harness_drive -- --nocapture
```

Manual desktop baseline: create `hello.py` on day 1, reboot, day 2 `what did we create yesterday?` lists it, canvas re-hydrates.

## Branch & Review Rule

Per ADR-0009, all code for this milestone lives on `feature/session-day-buckets` (or `feature/session-day-buckets/<subtask>`). Direct pushes to `main` for code are rejected. Docs-only changes may land on `main` after review. Merges to `main` require `cargo test --lib` + desktop baseline + session restore gate + reviewer.

## Restart Instructions

After an interruption, read in this order:

1. `PROJECT_GROUNDING.md` + latest `docs/grounding/*.md`
2. `docs/decisions/0009-session-day-buckets.md`
3. This file
4. `docs/action-state-machine.md` §3, `docs/system-graph.md` §2.3, `docs/capability-model.md` §5.2
5. `src/action.rs`, `src/coordinator/mod.rs`, `src/facade.rs`, `src-tauri/src/main.rs`
6. `git status --short` + `git branch`
