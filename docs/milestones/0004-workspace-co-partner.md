# Workspace Co-Partner — From Surfaces to Work

**Status:** Planned work — docs only, no code in this phase  
**Created:** 2026-08-22  
**Starting checkpoint:** `003f70a` + `4ff3d63` on `main` (multi-surface canvas + adaptive budgets)  
**Branch rule:** every code change for this milestone MUST land on `feature/workspace-co-partner` (or a sub-branch off it). No direct commits to `main`. See ADR-0008.

## Purpose

Aios today can generate a styled surface from live system evidence. The user can ask "show CPU" and get a faithful widget. This milestone makes Aios capable of *doing work on the system* as the user's co-partner: setting up development environments, creating and editing files, generating content for display, and pulling content from the web and the system to assist with arbitrary tasks.

The change is deliberately framed as an extension of the existing safety architecture, not a bypass of it. The same `capability × clearance → Guardian → staged + health → approval` pipeline that gates `stage_driver` will gate `write_file` and `stage_env`.

This doc is the plan. No code is changed in this docs-only phase.

## Starting Point

What works now (post-M8):

- Groundless generation: `EvidenceIndex` → prompt → unconstrained HTML → `verify_value_fidelity` → `SurfaceCard { id, revision, html, evidence }` in [src/surface/composer.rs](../src/surface/composer.rs) and [src-tauri/src/main.rs](../../src-tauri/src/main.rs). Multi-surface canvas with per-card drag and unioned `InputRect`.
- Read-only specialists: `observe / diagnose / query / deps / impact / health` in [src/tools.rs](../src/tools.rs) against the live `SystemGraph` (489 nodes on boot). Mutations only for `packages`/`drivers` via broker → executor.
- Policy enforcement: 7-step `PolicyBroker.evaluate()` in [src/capability.rs](../src/capability.rs) / [src/broker.rs](../src/broker.rs) — tool registry is authoritative for `risk_level`, deadline/nonce replay protection, token expiry, Guardian for ≥2, scoped approval for ≥3, audit always.
- Model gateway: [src/model.rs](../src/model.rs) + [src/http.rs](../src/http.rs) + [src/coordinator/routing.rs](../src/coordinator/routing.rs), task pinning, `DataPolicy` (Public/Personal/Protected/Secret), adaptive budget retry for `empty_content`.

What is deliberately not done yet:

- Files/data is read-only in all 19 module specs (e.g. [modules/files-data.md](../modules/files-data.md) lists only `observe_file:0`). Mutating file ops are deferred.
- Packages can `stage_update:2` but there is no dev-environment lifecycle (venv, toolchain, `cargo test` health).
- No web-fetch specialist. External content cannot enter the evidence path in a provenance-tracked way.
- Planner loop is single-shot with bounded read-only tool calls. Multi-step work (observe → fetch → write → verify) needs a looped plan.
- Surfaces are display-only. No artifact card that can be saved or applied as a file.

## Target Behavior

### The co-partner loop

```
User intent: "set up a Rust project that visualizes memory pressure from this machine,
             pull tokio docs from the web, write src/main.rs, show me the result"

Planner → multi-step plan (observe memory, fetch docs, write files, verify)
Verifier → reviews plan + risk
Broker → gates each step (capability + clearance + Guardian + approval per risk)
Executor → checkpoints workspace → stages files in sandbox → runs health (cargo test / parse)
Surface → artifact card with diff + health + Save/Apply/Rollback actions
```

The user sees one conversational answer plus one or more artifact cards. Approvals are batched per plan hash, not per file.

### Concrete scenarios (acceptance vignettes)

| Scenario | Steps | Risk | Expected outcome |
|---|---|---|---|
| Create `hello.py` that prints CPU temps from live evidence | `observe` memory/cpu sensors → `write_file` hello.py | 2 | File staged, health = `python -m py_compile` passes, artifact card shown, `Commit` writes to `~/workspace/hello.py` |
| Set up Python venv + install `numpy` | `stage_env` venv → `stage_update` pip install | 2 | Env staged in sandbox, `import numpy` health, rollback on failure |
| Pull tokio docs from web to help write async code | `fetch_url https://docs.rs/tokio` | 1 | Content fetched, `DataPolicy` enforced, NOT sent as capability, shown as evidence |
| Edit existing surface's generated html into a file | `Save as file` on SurfaceCard | 2 | Fidelity-verified html written through `write_file`, same health path |
| Attempt to overwrite `/etc/hosts` | `write_file /etc/hosts` | 3 | Guardian + approval required; without approval → `DENY(NoUserApproval)` visible |

Failures must be visible: `DENY`, `RolledBack`, stale evidence, or health failure. No silent fallback panel, no invented values, no swallowed broker error.

## Capability & Safety Design

### Resources

Add two scoped resource classes. All file resources remain `ResourceId(String)` with prefix matching:

- `file:/workspace/**` — user workspace (default `~/workspace`, configurable). Capability on `file:/workspace` implies subtree.
- `file:/artifacts/**` — generated artifact staging area (`~/.aios/artifacts` or `~/workspace/.aios-artifacts`).
- `web:fetch` — pseudo-resource for network fetches (single resource, not per-URL, to keep token cost bounded).

Per-resource exact matching is relaxed for `file:` prefixes only. This is an explicit, documented exception to `capability-model.md §3.3` — precision is kept for `device:`, `driver:`, etc. `file:/workspace` + `web:fetch` are the only prefix capabilities.

### Operations + Risk

| Operation | Resource | Risk | Gates |
|---|---|---|---|
| `observe` | `file:*`, `web:fetch` | 0 | capability only |
| `fetch` | `web:fetch` | 1 | capability + broker |
| `write` | `file:/workspace`, `file:/artifacts` | 2 | + Guardian + staging + health |
| `create` | `file:/workspace` | 2 | same |
| `patch` | `file:/workspace` | 2 | same (diff-aware) |
| `stage_env` | `file:/workspace`, `package:*` | 2 | same |
| `delete` | `file:/workspace` | 3 | + scoped approval |
| `reset` / `rollback` | `file:*` | 4 | + approval, checkpoint retained on Failed |

System paths outside the two prefixes (`file:/etc/**`, `file:/usr/**`) remain risk 3-4 and require explicit approval per plan scope. The broker resolves `risk_level` from the registry — the request never carries it.

### Invariants (Guardian)

Extend `Guardian` with:

- `DATA-003`: no write outside declared workspace/artifacts without approval.
- `DATA-004`: no `Secret` data in a `fetch` or model prompt (enforced by `DataPolicy` + redaction before `HttpBackend`).
- `PKG-003`: `stage_env` does not silently broaden an agent's capabilities or install unsigned packages without provenance.

Unknown or stale evidence → `Deny` (fail-closed per ADR-0003). Health check failure → `RolledBack`, checkpoint retained.

### Executor

Generalize `Checkpoint` from driver module snapshot to `FileCheckpoint`:

- Baseline: copy-on-write or overlayfs or `git` snapshot of `file:/workspace` subtree (implementation choice deferred to code phase, but must be testable hermetically with a temp dir mock).
- Health: `parse` + `cargo test`/`pytest` if manifest present, else `py_compile`/`tsc --noEmit` / `file exists`. Each specialist declares its health probe.
- On `CommitFailed` or `HealthCheckFailed` → automatic rollback. On `RollbackFailed` → `Failed` state, checkpoint retained, user-facing `manual recovery` hint.

## Module Spec Changes (docs phase)

This milestone does NOT edit Rust code. It updates these docs:

- `modules/files-data.md` — promote from read-only to staged mutating ops table above, add `Graph relationships: owns file:*` + `depends_on filesystem`, recovery section referencing `FileCheckpoint`.
- `modules/packages.md` — add `stage_env:2`, `commit_env:3` alongside existing `stage_update:2`.
- New `modules/web-fetch.md` — scope: `web:fetch` resource, `fetch_url:1` and `search_web:1` tools, `DataPolicy` wiring, provenance label for fetched content.
- `capability-model.md §3.1` — add `write/create/patch/delete/fetch/stage_env` operations and document prefix capability exception.
- `system-graph.md §2.3` — add `file` and `web` node types, lifecycle for file nodes.
- `message-protocol.md` — add `ToolResult` provenance field for `fetched_from` URL + timestamp.

## Planner & Surface Changes (docs phase)

- `coordinator/planning.rs` + `coordinator/chat.rs` — describe multi-turn loop: planner may emit `ToolCallRequest[]`, broker executes one by one, observations re-enter planner loop until plan complete or `coverage_gaps` triggers clarification. Bounded loop (e.g. 6 turns) to avoid unbounded agent chatter.
- `surface/composer.rs` — artifact mode: when plan produces file outputs, composer may render a diff/preview card alongside the conversational answer. File content is NOT generated as freeform html — it is a `ToolResult` that passed `verify_value_fidelity` against evidence or `fetch` provenance.
- `src-tauri/src/main.rs` + `frontend/src/main.ts` — `SurfaceCard` gains `kind: DataWidget | ArtifactPreview` and actions `Save / Apply Patch / Rollback`. Actions emit `ToolRequest` through the broker, not direct FS access.

## Implementation Stages

### Stage 0: Lock the baseline (docs only, this PR)

1. Land this milestone, ADR-0008, and roadmap/progress updates on `main`.
2. Create branch `feature/workspace-co-partner` from `main` (see ADR-0008).
3. No code changes on `main`. All subsequent code lands on the feature branch and is merged via PR after tests pass.

Acceptance: docs build, `graphify update .` shows new milestone node, `git branch` shows feature branch exists.

### Stage 1: Files/Data specialist (first code on feature branch)

Implement `FilesDataSpecialist` with prefix capabilities, `FileCheckpoint` over a temp-dir mock (hermetic tests) + real FS when `AIOS_WORKSPACE` is set. Dry-run mode by default, live writes only with explicit opt-in.

Tests: `observe_file` on seeded graph, `write_file` staged → health pass → commit, `write_file` health fail → `RolledBack` with retained checkpoint, `write_file` outside workspace → `DENY`.

Acceptance: vignette 1 passes end-to-end through coordinator → broker → executor → artifact surface.

### Stage 2: Packages / Dev Env specialist

Extend `PackagesSpecialist` with `stage_env` / `install_packages`. Health = toolchain probe + project tests. Reuse `FileCheckpoint`.

Acceptance: `stage_env python3.11 venv + pip install numpy` commits on health pass, rolls back on `import` failure.

### Stage 3: Web/Fetch specialist

New `WebSpecialist` with `fetch_url`/`search_web` at risk 1, `DataPolicy` redaction before `HttpBackend`, audit includes `fetched_from`. Planner can use fetched docs as evidence but cannot grant capabilities from fetched content.

Acceptance: `fetch_url https://docs.rs/tokio` returns text, no `Secret` leaves, audit log contains provenance.

### Stage 4: Loop + Artifact surfaces

Wire multi-turn planner loop and artifact card actions. One `PlanHash` covers multiple file ops for batched approval. Frontend shows diff + health + `Commit All / Rollback`.

Acceptance: "scaffold Rust project, pull docs, write files, show preview" completes as one plan with one approval.

### Stage 5: Hardening & policy

Add the Guardian invariants above, prefix-capability tests, and a manual live run on a real workspace with `AIOS_LIVE_FILE_CONTROL=1` (mirrors `AIOS_LIVE_DRIVER_CONTROL` pattern). All prior regression gates still pass.

## Error & Fallback Rules

- No silent write outside workspace.
- No invented file content that fails fidelity/health.
- No swallowed broker `DENY` — surface shows the typed reason.
- No direct FS access from frontend or model — every mutation is a brokered `ToolRequest`.
- No `run_any_command` or `shell_exec` tool. Typed ops only.

## Regression Gates

Every stage must pass on the feature branch:

```bash
cargo test --lib
cargo build --manifest-path src-tauri/Cargo.toml
npm run build --prefix frontend
```

Manual desktop baseline (from 0002) must still pass: CPU surface, RAM surface, combined surface, drag, click-through, second prompt while surfaces visible. New baseline adds: create file, save artifact, failed write shows DENY.

## Branch & Review Rule

Per ADR-0008, all code for this milestone lives on `feature/workspace-co-partner` (or `feature/workspace-co-partner/<subtask>`). Direct pushes to `main` for code are rejected. Docs-only changes may land on `main` after review. Merges to `main` require `cargo test --lib` + desktop baseline + reviewer.

## Restart Instructions

After an interruption, read in this order:

1. `PROJECT_GROUNDING.md` + latest `docs/grounding/*.md`
2. `docs/decisions/0008-workspace-co-partner-branch-and-scope.md`
3. This file
4. `docs/capability-model.md` §3–5, `docs/action-state-machine.md` §3, `docs/system-graph.md` §2.3
5. `src/capability.rs`, `src/broker.rs`, `src/tools.rs`, `src/surface/composer.rs`
6. `git status --short` + `git branch`
