# Aios Test Ledger

**Status:** Living document.
**Purpose:** Track meaningful changes to the automated test suite — additions,
retirements, and any `#[ignore]` transitions — so that a shrinking test count
can never be conflated with a growing one, and so that reviewers can trace why
coverage moved.

## Convention

Every commit that adds, deletes, renames, or ignores a test on `main` or a
feature branch adds a row to this file in the commit that carries the change.
A retirement without a row is treated as a regression during review.

- **Added** — new test, describe what it locks down.
- **Retired** — deliberate removal, must state the successor coverage.
- **Ignored** — `#[ignore]` attribute added; must state the reason and the
  ticket/ADR that will restore it.
- **Restored** — `#[ignore]` removed; describes what changed.

## Ledger

### Baseline `main @ 32ffc0f` (M9 merge — workspace co-partner)

- **Total:** 411 tests passed, 1 ignored.
- **Ignored test:** one real-model gateway test, gated on `AIOS_LIVE_MODEL=1`,
  documented at the point of `#[ignore]`. Not counted as regression; it is a
  live-only test by design.

### `feature/session-day-buckets @ HEAD` (`c3f0038`)

- **Total:** 413 passed, 1 ignored.
- **Delta from `32ffc0f`:** +3 added, 0 retired, 0 newly ignored.

#### Added (net +3)

| Commit | Test | What it locks down |
|---|---|---|
| `131f817` (session day-bucket SessionStore) | `session::tests::*` (2 tests) | `SessionStore::save`/`load` round-trips history + `last_tool_results` + surfaces through `write_file_synced` + `sync_dir`; corrupt file falls back to empty session (ADR-0003 fail-closed for corrupt payload, not for missing file). |
| `c3f0038` (exec specialist) | `exec::tests::exec_runs_echo` | `exec.run` invokes `sh -c` and captures stdout/stderr/exit through the `ToolParameters::Execute` path. Note: this test does **not** cover the sandbox path — that arrives in Phase 2 with the `Sandbox` trait. |

#### Retired (0)

None on this branch. Prior handoff copy suggested a 438 → 413 drop; that was
based on a stale `doc-progress.md` line reading "438" that was itself
incorrect at the time of writing. Verified 2026-08-22 by counting
`#[test]` / `#[tokio::test]` attributes at `32ffc0f` (411) and `c3f0038`
(414 attributes / 413 executed + 1 ignored).

#### Newly ignored (0)

None.

### `feature/type-level-tcb @ HEAD` (Phase 1.1 — type-level authorization proof)

- **Total:** 415 lib tests passed, 1 ignored, **+ 1 integration test
  containing 3 `trybuild` compile-fail cases** (`tests/type_level_tcb.rs`).
- **Delta from `c3f0038`:** +2 unit tests, +1 integration test, +3
  compile-fail cases, 0 retired.

#### Added

| Test | What it locks down |
|---|---|
| `broker::tests::authorize_denies_when_guardian_blocks` | `PolicyBroker::authorize` returns `Err(GuardianBlocked)` (no proof minted) when the guardian rejects a Critical-risk request, AND the audit entry is still written (fail-closed audit, ADR-0009). |
| `broker::tests::authorize_mints_proof_on_allow` | On Allow, `authorize` mints an `AuthorizationProof` whose `risk()` matches the tool registry and whose `request_id()` matches the request. |
| `tests/type_level_tcb.rs::authorization_proof_is_not_synthesizable_outside_broker` | Wrapper around three `trybuild` compile-fail cases below; if any of them compiles, the type-level TCB invariant (ADR-0010 §1.1) is broken. |
| `tests/compile-fail/authorization_proof_not_synthesizable.rs` | E0624 — `AuthorizationProof::new` is private (only `broker.rs` can mint). |
| `tests/compile-fail/authorization_proof_struct_literal.rs` | E0451 — all fields (`request_id`, `risk`, `_private`) are private, so external struct-literal synthesis is refused. |
| `tests/compile-fail/authorization_proof_for_test_hidden.rs` | E0599 — the `#[cfg(test)]`-gated `for_test` escape hatch is invisible to integration tests, so it cannot be used to bypass the invariant from `tests/`. |

**Dependency added:** `trybuild = "1"` in `[dev-dependencies]`
(no runtime impact).

**Executor signature changes** (breaking within the crate; 8 existing
executor unit tests updated to pass
`&AuthorizationProof::for_test(RiskLevel::_)`; production callers in
`LocalBroker::run_staged` updated to thread the real proof from
`PolicyBroker::authorize`):

- `StagedExecutor::stage_and_commit(..., proof: &AuthorizationProof)`
- `StagedExecutor::reset_and_commit(..., proof: &AuthorizationProof)`

Runtime cross-check added: both entry points reject with
`StagingError::CheckpointFailed` if `proof.risk() != record.risk_level`
(defense-in-depth on top of the compile-time property).

**Deviation from Phase 1 spec, noted honestly:** the spec listed
`StagedExecutor::commit` (line 187) as also gaining a `&AuthorizationProof`
param. That method is called from four sites: `stage_and_commit`,
`reset_and_commit` (both already gated), and two internal recovery paths
(`recover_staged`, `recover_health_verified`) that replay a
previously-authorized on-disk `ActionRecord`. Recovery has no live
authorization to attach; the `pub(crate) AuthorizationProof::for_recovery`
escape hatch was defined for this, but adding the parameter to `commit`
provides zero additional safety since the recovery paths are
`StagedExecutor`-internal and cannot be reached from outside the crate.
The compile-time boundary is therefore drawn at the two public entry
points, which is where untrusted callers cross it. `commit` remains
`pub` to avoid churn but has no external production callers today
(verified by `grep`).

#### Retired (0)

None.

#### Newly ignored (0)

None.

## Coverage Gaps Known Now (Phase 1–5 targets)

These are not test *regressions*, they are known-missing tests that
subsequent phases will add before the code lands. Recorded here so the
plan is traceable:

### Phase 1 (in progress)

- ✅ **Phase 1.1** — `authorization_proof_is_not_synthesizable_outside_broker`
  (trybuild) + `authorize_denies_when_guardian_blocks` +
  `authorize_mints_proof_on_allow`. *Landed.*
- **`owner_of_map_authoritative`** (Phase 1.2) — broker-owned owner map;
  request routed to a non-owner fails with `Deny(NotResourceOwner)` even
  when the graph's `owns` edge says otherwise.
- **`verifier_skipped_records_audit`** (Phase 1.3) — `VerifierVerdict::Skipped`
  is recorded on the `ApprovalRequest` and surfaced in audit but does not
  gate the broker decision for risk ≤ 2.
- **`resource_lock_serializes_overlapping_writes`**, **`resource_lock_shared_reads_do_not_serialize`**,
  **`resource_lock_timeout_denies_with_resource_busy`** (Phase 1.4) —
  2PL lock manager with shared/exclusive semantics.

### Phase 2 (Sandbox)

- **`sandbox_present_bwrap`**, **`sandbox_fallback_landlock`**,
  **`sandbox_absent_forces_default_mode`** — sandbox tier detection
  cached in `BackendStatus`.
- **`exec_denies_brickable`** — Guardian denylist smoke test per
  ADR-0010 §3 (defense-in-depth, not the safety boundary).

### Phase 5 (feature-work resumption)

- **`exec_mode_default_requires_approval`**, **`exec_mode_auto_auto_approves_safe`**,
  **`exec_mode_yolo_still_denies_brickable`** — approval-mode matrix
  wired into `broker.rs::request_approval`.
- **`sudo_missing_returns_unsupported`** — `sudo -n` detection returns
  `ToolError::OperationNotSupported` when `/etc/sudoers.d/aios` is absent.

### Pre-existing broken (not this branch, tracked)

- **`tests/ui_file_artifact.rs`** — does not compile on `f4c0376` due to
  a `client` double-move (E0382) at lines 94–95. Predates the
  `feature/type-level-tcb` branch. Not part of Phase 1 scope; will be
  fixed alongside the WebDriver harness work in a later branch.

## Rules of engagement

1. **Never `#[ignore]` a failing test without a ledger entry and a linked ADR
   or ticket.** Ignoring silences the alarm; the ledger is how we remember
   to come back.
2. **A shrinking test count is a review-blocker until each retirement has a
   ledger entry naming the successor coverage.**
3. **Renamed tests are not retirements** — but if a rename crosses a module
   boundary, note it here so `git log --follow` isn't the only trail.
4. **Live-only tests (`AIOS_LIVE_*` gated)** count as ignored in the normal
   `cargo test --lib` baseline; that is expected.

## References

- `docs/decisions/0003-fail-fast-no-silent-fallbacks.md` — no silent
  degradations; a disappeared test is a silent degradation.
- `docs/testing-strategy.md` — the 6 test layers and the safety-critical
  layer's non-negotiable list.
- `docs/doc-progress.md` — top-line baseline count, must match this file.
