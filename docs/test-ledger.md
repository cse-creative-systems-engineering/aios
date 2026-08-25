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

## Coverage Gaps Known Now (Phase 1 targets)

These are not test *regressions*, they are known-missing tests that Phase 1
of the plan (`docs/decisions/0011-…` and follow-ups, in progress) will add
before the code lands. Recorded here so the plan is traceable:

- **`exec_denies_brickable`** — Guardian denylist smoke test per ADR-0010 §3.
  Will land in Phase 2 along with the sandbox.
- **`exec_mode_default_requires_approval`**, **`exec_mode_auto_auto_approves_safe`**,
  **`exec_mode_yolo_still_denies_brickable`** — approval-mode matrix.
- **`verifier_skip_records_audit`** — audit records `verifier_skipped=true`
  when the toggle is off for risk ≤ 2.
- **`sudo_missing_returns_unsupported`** — `sudo -n` detection returns
  `ToolError::OperationNotSupported` when `/etc/sudoers.d/aios` is absent.
- **`sandbox_present_bwrap`**, **`sandbox_fallback_landlock`**,
  **`sandbox_absent_forces_default_mode`** — Phase 2 sandbox tiers.
- **`guardian_allow_is_non_synthesizable`** — Phase 1 type-level proof:
  broker cannot construct `GuardianAllow` without calling `guardian.review()`.
  Compile-fail test (`trybuild` or equivalent).
- **`owner_of_map_authoritative`** — Phase 1 broker-owned owner map;
  request routed to a non-owner fails with `Deny(NotResourceOwner)` even
  when the graph's `owns` edge says otherwise.
- **`resource_lock_serializes_overlapping_writes`** — Phase 1 lock manager.

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
