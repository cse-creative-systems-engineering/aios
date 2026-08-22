# ADR-0008: Workspace Co-Partner Scope and Branch Rule

**Status:** Accepted
**Date:** 2026-08-22
**Decides:** docs/milestones/0004-workspace-co-partner.md

## Context

After M8 (multi-surface canvas), Aios can display faithful system state. The next capability is *doing work*: creating/editing files, scaffolding dev environments, generating content, and fetching web content to assist arbitrary tasks.

Two risks must be settled before code begins:

1. **Scope creep into unrestricted execution.** Adding `run_any_command` or per-file arbitrary shell would bypass `capability-model.md` and `security-model.md`. The capability system is the TCB — it must not be bypassed for convenience.
2. **Uncontrolled branching.** Feature work that lands directly on `main` interleaves with the shipping canvas and makes the grounding snapshots meaningless. Contributors and agents need a single rule for where code for this feature lives.

## Decision

### 1. Branch rule (normative)

All **code** changes for the workspace co-partner feature MUST be made on `feature/workspace-co-partner` or a sub-branch off it (`feature/workspace-co-partner/<subtask>`). No direct code commits to `main`.

- Docs-only changes (this ADR, milestone 0004, roadmap/progress edits) may land on `main` after review.
- Code on the feature branch is merged to `main` only via PR after the regression gates in `0004` pass (`cargo test --lib`, `cargo build --manifest-path src-tauri/Cargo.toml`, `npm run build --prefix frontend`, desktop baseline).
- The feature branch is created from the `main` checkpoint that includes milestones 0002/0003 and commits `003f70a`/`4ff3d63`.

If a code change is made on `main` in violation of this rule, it is reverted and re-applied on the feature branch.

### 2. Capability scope for files and web

- File mutations are scoped to `file:/workspace/**` and `file:/artifacts/**` with **prefix** capabilities. `file:/workspace` grants the subtree. This is the sole prefix exception to the per-resource granularity in `capability-model.md §3.3`; all other resources remain per-resource.
- Web fetch is a single resource `web:fetch` (risk 1) with `DataPolicy` enforcement before `HttpBackend`. Fetched content carries provenance (`fetched_from` + timestamp) and never grants capabilities.
- New operations `write/create/patch/delete/fetch/stage_env` are added to `capability-model.md §3.1` with the risk table in milestone 0004. The broker resolves risk from the registry; the request never carries it.
- Writes outside the two prefixes remain risk 3–4 and require a scoped, plan-hash-bound approval. No shell tool is added.

### 3. Execution

Staged file/env changes use the existing `StagedExecutor` via a `FileCheckpoint` (overlay/copy/git — choice deferred to code phase but must be hermetically testable). Health failure → automatic `RolledBack` with retained checkpoint; `RollbackFailed` → `Failed` with manual recovery, matching `action-state-machine.md`.

## Consequences

- `main` remains shippable for surface work while the co-partner is built.
- `capability-model.md`, `system-graph.md`, `message-protocol.md`, and `modules/files-data.md` gain file/web entries but their authority model is unchanged.
- No new TCB component is added. Broker, Guardian, and executor retain sole authority.
- The `run_any_command` design is explicitly rejected. If a future task needs broader execution, it requires a new ADR and a sandbox design — not a scope extension of this milestone.

## Related

- docs/milestones/0004-workspace-co-partner.md — full plan
- docs/capability-model.md §5.2 — broker decision algorithm
- docs/action-state-machine.md — staged execution
- docs/decisions/0004-two-dimensional-authorization.md — capability × clearance
- docs/decisions/0007-groundless-generation-model.md — groundless generation retained; file outputs are tool results, not free html
