# ADR-0009: Session Day-Buckets and Project Persistence

**Status:** Draft
**Date:** 2026-08-22
**Decides:** docs/milestones/0005-session-day-buckets.md
**Amends:** docs/decisions/0008-workspace-co-partner-branch-and-scope.md (branch rule extended)

## Context

After `0004-workspace-co-partner` (staged `file:/workspace` + `web:fetch` on `feature/workspace-co-partner`, now merged to `main` at `32ffc0f`), Aios can create files and fetch web content through the broker. The next UX is *projects and sessions that survive login/logout, conversationally seamless, always contextually aware of the system and what the user is seeing*.

The current architecture is per-action, not per-day, by construction:

- `ActionRecord` + `FileActionStore` — one JSON per `action_id` in `config_dir/actions/`, WAL `pending-{id}.json`, recovery per-action (`Staged` → `Committed`/`RolledBack`). Correct for atomic staged safety, but the only grouping is `action_id`. There is no `session_id`/`project_id` that owns a set of actions.
- `Facade` `history: VecDeque<String>` (capped `max_history`) + `last_tool_results` + `Coordinator` `tool_results` — all in-memory. On logout/reboot they drop. The user sees one conversation, but the facade forgets it.
- `SystemGraph` discovered once at `boot_with` via `discovery.rs` `sysfs/procfs`, then only `refresh_connectivity`. If Aios is always-on via the persistent sidebar (`alwaysOnTop` + `set_input_region` union), the graph is a boot snapshot, not a live view. Per `capability-model.md` §10.2 broker `resource_states` is authoritative, graph advisory — that split needs closing for always-on awareness.
- Surfaces — `Vec<SurfaceCard>` in `src-tauri/src/main.rs` worker lives in the `surfaces: &mut Vec` thread. No disk persistence. Drag/close works, but next login is empty.

The prev agent was explicitly asked to *not* make it per-action, but the code did. Adding `file:/workspace/projects/2026-08-22/hello.py` as a path convention would give date-organized files without a session — the file would exist, the conversation that created it would be gone the next morning.

## Decision

### 1. Branch rule (amends 0008)

All **code** for session day-buckets lives on `feature/session-day-buckets` or a sub-branch off it (`feature/session-day-buckets/<subtask>`). No direct code commits to `main`.

- Docs-only (this ADR, milestone 0005, roadmap/progress edits) may land on `main` after review, but code for 0009 must be on the feature branch.
- Merges to `main` require the same gates as 0008 (`cargo test --lib`, `cargo build --manifest-path src-tauri/Cargo.toml`, `npm run build`, desktop baseline) plus the new session restore gate (boot after `kill` → prior day's surfaces/history reappear).

If a code change for sessions lands on `main` directly, it is reverted and re-applied on `feature/session-day-buckets`.

### 2. Session as day-bucket, above actions (not instead)

Keep per-action as the safety primitive. Add a session layer *above* it:

- `Session { id: Date (YYYY-MM-DD), project_id: Option<String>, history: Vec<Message>, tool_results: Vec<ToolResult>, surfaces: Vec<SurfaceCard>, graph_snapshot: Option<SystemGraphSnapshot> }` persisted to `config_dir/sessions/{date}.json` with the same `write_file_synced` + `sync_dir` durability as `FileActionStore` (`action-state-machine.md` §5.3). `ActionRecord` gains `session_id: Option<SessionId>` — existing flat `*.json` become `session_id = None` = pre-day era, no migration.
- `Project { slug, created_at, sessions: Vec<Date> }` — a new `NodeType::Project`/`Session` in `SystemGraph` (`owns` its files, `depends_on` its session). `file:/workspace/projects/<slug>/<YYYY-MM-DD>/` becomes a real resource (prefix `file:/workspace/projects`), so `capability_model.md` `resource_covers` and broker `grant_capability` enforce it, and `audit.log` can record per-project.
- The day bucket is a *view*, not a store: `SessionStore::load(date)` on `Coordinator::boot_with` before `configure_read_only_broker`, so the first prompt after login already has yesterday's surfaces/history.

Organizing projects by date is then `file:/workspace/projects/<slug>/<date>/...` and `session_id = date` points to that dir. The file exists, the chat that explains it survives.

### 3. Persistence and seamless login/logout

- `Facade` `history` + `last_tool_results` → `SessionStore` (JSON, same durability). Saved after each `chat_with_tools_outcome`, loaded at boot. The `4`-turn cap and `PROMPT_TIMEOUT` remain — sessions are long-lived, turns are still short.
- Surfaces — `Vec<SurfaceCard>` → `sessions/{date}.json` alongside history. On next login the canvas re-hydrates the prior day's cards (same `SurfaceCard {id, html}` that already supports per-card drag/close and unioned `InputRect`).
- `SessionStore` survives `AppHandle` restart (`AppState.requests` is in-memory, `SessionStore` is on disk). `kill` → `boot` → prior day's surfaces/history reappear is the acceptance gate.

### 4. Always-on awareness (not always-on model)

Sidebar is persistent (`alwaysOnTop` + `skipTaskbar`), agent is *session-persistent*, not busy-looping. The model is hit only on prompt or explicit `health: Degraded` watchdog, not on a heartbeat. That keeps `ModelGateway` `ConnectivityState` routing and `Clearance` budgets advisory (ADR-0005 deferred) from burning the provider. If a heartbeat is added later, it needs per-session token budgets.

`replay_log` `(PrincipalId, nonce)` dedup currently grows unbounded (`expires_at = granted_at + 10_000_000` ~115 days). Day-buckets need expiry/GC — will be added with `SessionStore`.

### 5. What stays per-action

- `FileCheckpoint` (copy/overlay) still per-action, health → `RolledBack` with retained checkpoint, `RollbackFailed` → `Failed` with manual recovery, per `action-state-machine.md`.
- Broker remains sole authority, `Guardian` per-request, `FileDriver` per-resource. `Session` does not grant capabilities — `file:/workspace/projects` prefix does.

## Consequences

- `SystemGraph`, `action-state-machine.md`, `capability-model.md`, `message-protocol.md`, `observability.md` gain `Project`/`Session` entries, but the TCB (broker + Guardian + executor + audit) is unchanged.
- `main` stays shippable for file/web while sessions are built on `feature/session-day-buckets`.
- The `per-action only` design is explicitly superseded. If a future task needs sessions longer than a day, it requires a new ADR — not an extension of this one.
- `facade.rs` `history` becomes disk-backed; `coordinator/mod.rs` `boot_with` gains a `SessionStore::load` before `configure_read_only_broker`.

## Related

- docs/milestones/0005-session-day-buckets.md — full plan
- docs/milestones/0004-workspace-co-partner.md — staged file/web just merged at `32ffc0f`
- docs/action-state-machine.md — per-action WAL
- docs/system-graph.md §2.3 — `file`/`web` node types
- docs/capability-model.md §5.2 — broker decision
- docs/decisions/0004-two-dimensional-authorization.md
- docs/decisions/0008-workspace-co-partner-branch-and-scope.md
