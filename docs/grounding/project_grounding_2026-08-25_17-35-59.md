# Grounding Snapshot: exec.run sandboxed per ADR-0010 Phase 2: bwrap boundary wired, fd bug fixed, suite green

## Current State

ADR-0010 §3.1 is implemented for the bubblewrap tier. `exec.run` no longer
runs `sh -c` on the bare host: `handle_exec` now runs
Guardian pattern checks (EXEC-P-001..004) → root preparation →
`run_confined()` through a detected sandbox tier. Fail-closed everywhere:
`NullSandbox` and a landlock tier that cannot confine refuse to spawn rather
than execute unconfined (ADR-0003). The result JSON carries `sandbox_tier`.

Fixes this session (uncommitted until now):

- `src/sandbox.rs`: removed the broken `--file 11 /etc/passwd` /
  `--file 12 /etc/group` argv entries. They failed twice over — bwrap tried
  to create `/etc/passwd` inside the already-mounted read-only `/` bind
  ("Can't create file /etc/passwd: Read-only file system"), and no parent
  fds were piped anyway. The ro `/` bind already provides both files.
- `src/sandbox.rs` test: fixed off-by-two slice assertion
  (`argv[n-3..n-1] == ["sh", "-c"]`, plus explicit `--` check).

Test baseline: **422 passed + 1 ignored** (`cargo test --lib`; was 419/3
failing at session start). Integration tests also green:
`harness_drive` 3 passed; `ui_e2e` / `ui_file_artifact` 1 ignored each
(live-only). `src-tauri` builds clean. App rebuilt and restarted detached;
backend ready.

Live-behavior proof observed during the session: a command run against the
old stub binary wrote `proof.txt` to the repo root (cwd-relative,
unconfined); the same command through the new handler lands in
`~/workspace/proof.txt` inside the rw bind. Writes outside workspace fail
with EROFS from inside the sandbox (`sandbox_confines_write_outside_workspace`
locks this in).

## Relevant Paths

- `src/sandbox.rs` — `Sandbox` trait, tiers Full/FilesystemOnly/None,
  bwrap argv builder, `detect_sandbox()` boot cache, `run_confined()`
  fail-closed. Landlock `wrap()` still returns None (detection only).
- `src/exec.rs` — EXEC-P-001..004 pattern checks pre-sandbox, confined
  execution with tier reporting, new tests for confinement both ways.
- `docs/decisions/0010-execution.md` — normative spec this implements.

## Open Work

1. ADR-0010 §7 remainder: approval-mode matrix tests
   (`exec_mode_default_requires_approval`, `exec_mode_auto_auto_approves_safe`,
   `exec_mode_yolo_still_runs_sandboxed`, `sandbox_absent_locks_default_mode`).
   Approval modes themselves (Default/Auto/YOLO) are NOT implemented yet —
   frontend toggle + config persistence + broker logic per §4.
2. EXEC-P-005 secret-leak check (DataPolicy::Secret values in expanded
   command) — the only pattern with standalone safety semantics.
3. Verifier toggle + `verifier_skip_records_audit`.
4. `sudo -n true` per-session detection and typed
   `OperationNotSupported` error when `/etc/sudoers.d/aios` is absent
   (§6); `install_sudoers` helper binary.
5. Landlock rulesets (Phase 2 remainder): apply rules from-process instead
   of returning None; tier pill in sidebar `BackendStatus`.
6. User verification of surfaces against Ox (reasoning effort fix) and an
   `exec.run` smoke prompt ("run ls in my workspace").
7. Then: merge branch → `main` per ADR-0008 gates; unblocks
   `feature/type-level-tcb` (Phase 1.1 AuthorizationProof).
