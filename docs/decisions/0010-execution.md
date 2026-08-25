# ADR-0010: Typed Execution Primitive and Approval Modes

**Status:** Accepted
**Date:** 2026-08-22
**Decides:** docs/milestones/0005-session-day-buckets.md (execution scope), extension of ADR-0008
**Amends:** docs/decisions/0008-workspace-co-partner-branch-and-scope.md §Consequences (`run_any_command` rejection)

## Context

ADR-0008 §Consequences rejected `run_any_command` and stated: *"If a future task needs broader execution, it requires a new ADR and a sandbox design — not a scope extension of this milestone."*

Since then, the co-partner surface has proven that typed file/web ops are not enough to make Aios a real workspace partner. Every non-trivial user request — "install numpy", "start the dev server", "run the test suite", "check what's on port 3005", "reload systemd" — requires *invoking a program*, not writing a file. Continuing to add one typed specialist per verb (`packages.install`, `service.start`, `port.check`, `test.run`, …) has a combinatorial ceiling and pushes complexity into the broker registry with no matching safety gain: each of those specialists would ultimately shell out anyway.

At the same time, `run_any_command` in its naive form (untyped shell string, no policy) *is* the exact bypass ADR-0008 rejected. It cannot be added by scope creep.

This ADR resolves the tension by introducing a single **typed execution primitive**, `exec.run`, that:

1. Passes through the same `capability × clearance → Guardian → Staged executor → Audit` pipeline as every other risk-2 tool. No new TCB path.
2. Runs the child inside a real user-space sandbox (`bubblewrap` primary; `landlock`-only fallback; no-sandbox forces `Default` mode). The sandbox is the actual safety boundary; Guardian pattern checks (`EXEC-P-001..005`) are defense-in-depth and audit-log clarity, not the boundary.
3. Introduces three user-selectable **approval modes** — `Default`, `Auto`, `YOLO` — that trade convenience for friction without ever weakening the sandbox or the audit log.
4. Standardises **`sudo -n`** semantics and a minimal, allowlisted `/etc/sudoers.d/aios` provisioned at install time, so privilege-escalation is deterministic and never interactive from the model's side.

The primitive is **not** a shell tool in the ADR-0008 sense: the parameter is a command string, but the command is treated as an opaque resource whose invocation is brokered, checkpointed, health-checked, and audited. The user, not the model, decides whether a given session runs in Default, Auto, or YOLO.

## Decision

### 1. Branch rule (unchanged)

Code lands on `feature/session-day-buckets` (per ADR-0009). This ADR and the cascading doc updates land on `main` as docs-only. Merges to `main` still require the ADR-0009 gates plus a new **exec regression gate** (see §7).

### 2. `exec.run` — the typed execution primitive

- **Tool id:** `exec.run`
- **Specialist:** `exec.specialist` (`src/exec.rs`)
- **Resource:** `exec:command` (single pseudo-resource, not per-command, to keep token cost bounded — same pattern as `web:fetch`)
- **Operation:** `Operation::Execute` (new; risk-mapped to `RiskLevel::Staged` = 2)
- **Parameters:** `ToolParameters::Execute { command: String }` — the literal command line, invoked as `sh -c "$command"` inside the sandboxed working directory (`AIOS_WORKSPACE`, default `~/workspace`).
- **Health probe:** exit code. `exit == 0` → `Committed`. `exit != 0` → `RolledBack` (no side-effect on the file checkpoint because exec is not a file mutation; the checkpoint is the working-directory snapshot).
- **Result:** `ToolData::QueryResult { command, exit_code, stdout, stderr, combined }` truncated to 8 KiB with `...truncated` suffix. Provenance: `principal = exec.specialist`, `resource = exec:command`, `nonce`, `deadline` — the standard broker envelope.

`exec.run` is **the only** execution primitive. `Operation::Serve` is reserved for a future `serve.specialist` that supervises long-running processes (Stage 4 of milestone 0006); it is out of scope for this ADR beyond the enum entry.

### 3. Safety boundary and defense-in-depth

**The safety boundary for `exec.run` is a real user-space sandbox, not a regex.** ADR-0010 v1 (superseded by this section) proposed a Guardian regex denylist as the primary safety mechanism. That framing is dishonest: any regex denylist is trivially defeated by variable indirection (`X=/etc; rm -rf $X`), full paths (`/bin/rm -rf /etc`), shell quoting (`rm -rf /e''tc`), or execution via another interpreter (`python -c "import shutil; shutil.rmtree('/etc')"`). Calling such a filter a "safety floor" would invite the same complacency that makes ad-hoc `sudo` policies dangerous.

The actual safety architecture has three layers, in this order:

#### 3.1 Primary safety boundary: the sandbox (Phase 2)

`exec.run` executes inside a user-space sandbox that restricts what the child process can see and write. Three implementation tiers, detected at boot and displayed in the sidebar:

| Tier | Implementation | Coverage | Availability |
|---|---|---|---|
| **Full** | `bubblewrap` (`bwrap`) with fresh mount / network / PID / user / IPC namespaces; `~/workspace` and `~/.aios/artifacts` bind-mounted rw; everything else ro or hidden; `/dev/sd*`, `/dev/nvme*`, `/dev/mem`, `/sys/firmware`, `/proc/sys/kernel` masked; no network by default. | Filesystem + network + PID + user + IPC | Any Linux ≥3.8 with `bubblewrap` installed. Ships as `.deb Depends: bubblewrap`, `.rpm Requires: bubblewrap`. |
| **Filesystem only** | Landlock LSM syscall API; writes outside `~/workspace` and `~/.aios/artifacts` denied by kernel. No network, PID, or process-tree isolation. | Filesystem only | Linux ≥5.13; used only when `bwrap` is missing. |
| **None** | No sandbox available. | — | Very old kernel with no `bwrap`. |

Approval-mode interaction with tier:

- **Full** — all three approval modes (`Default`, `Auto`, `YOLO`) available.
- **Filesystem only** — `Default` unrestricted; `Auto`/`YOLO` require a one-time per-session confirmation banner noting that network and process isolation are not enforced.
- **None** — `Auto` and `YOLO` are **forcibly disabled**; the approval mode is locked to `Default` and every `exec.run` requires an explicit `ApprovalRequest`. This is the fail-closed path (ADR-0003): we never run un-sandboxed shell in an auto-approving mode.

The sandbox implementation lives behind `trait Sandbox` (`src/sandbox.rs`, Phase 2) with `BubblewrapSandbox`, `LandlockSandbox`, and `NullSandbox` implementations. Detection runs once at boot and is cached in `BackendStatus`. The desktop displays the current tier as a status pill (green / amber / red). Aios never auto-installs a dependency; if `bwrap` is missing on a manual build, `aios doctor` prints a one-line remediation.

#### 3.2 Defense-in-depth: Guardian pattern list

Above the sandbox, the Guardian evaluates every `exec.run` against a pattern list. These patterns are **not the safety boundary** — they are a comprehensibility layer that:

1. Catches obvious bad output early so the audit log and the user's UI show a clear reason (`Deny(EXEC-P-001: rm on system root)`) rather than a generic sandbox failure.
2. Fails the request *before* the sandbox even spawns, saving CPU and clarifying intent.
3. Serves as evidence of model output quality over time (repeated hits on a pattern indicate a prompt/routing issue).

A match returns `Deny(GuardianPatternMatch { id, pattern, cmd_excerpt })` regardless of approval mode. A miss does **not** imply safety — the sandbox is still the boundary.

| ID | Pattern (regex, case-insensitive) | What it catches |
|---|---|---|
| `EXEC-P-001` | `\brm\s+(-[rRfF]+\s+)?(/|/boot|/etc|/usr|/var|/root|/sys|/proc|/lib|/lib64|/bin|/sbin)(/|\s|$)` | Naive `rm -rf` on system roots (obvious model mistakes). |
| `EXEC-P-002` | `\b(dd|mkfs\.\w+|wipefs|shred|blkdiscard|sgdisk|parted|fdisk|cfdisk|sfdisk)\b` co-occurring with a block-device path (`/dev/sd*`, `/dev/nvme*`, `/dev/mmcblk*`, `/dev/vd*`, `/dev/xvd*`, `/dev/dm-*`). | Naive block-device writes. |
| `EXEC-P-003` | `\b(flashrom|fwupdmgr\s+install|dfu-util)\b`, or an `.ucode` firmware path in an argument. | Firmware writes attempted through the exec path instead of a risk-3 firmware tool. |
| `EXEC-P-004` | Redirects (`>`, `tee`) targeting `/sys/firmware`, `/sys/kernel/security`, `/proc/sys/kernel/`, `/dev/mem`, `/dev/kmem`. | Naive kernel/security tampering. |
| `EXEC-P-005` | Any expanded `command` string, resolved environment, or embedded heredoc contains a value classified `DataPolicy::Secret` in the current session. | Secret leaking into the process table / `ps` / audit stdout. This one *is* authoritative even inside the sandbox, because the sandbox does not scrub `/proc/*/cmdline`. |

`EXEC-P-005` is the only pattern that carries safety semantics on its own; the rest are UX and log-quality aids. All five run in every mode. Additions to this list are commits, not ADRs — they don't change the safety architecture, they add clarity to the audit log.

#### 3.3 Post-hoc: audit log

Every `exec.run` invocation records: command, sandbox tier, approval mode, approval type (`Explicit` / `AutoApproved(mode=…)`), verifier status, Guardian pattern verdict, exit code, and a truncated stdout/stderr. The audit log is append-only and hash-chained (`security-model.md §1.2`). This is the reviewable record; combined with the sandbox, it lets a user answer "what did Aios do on my machine last Tuesday" precisely.

### 4. Approval modes

The user selects one mode per session via the toggle beside the composer. The mode is stored in `~/.aios/config.toml` and per-session in `sessions/{date}.json` (ADR-0009 §3). The mode is **user-owned**; the model cannot request or observe a mode change.

| Mode | Behaviour for `exec.run` (risk 2) | Behaviour for risk 3–4 | Guardian denylist |
|---|---|---|---|
| **Default** | Every call issues an `ApprovalRequest` bound to `plan_hash`. `expires_at` = 10 min. No approval → `Rejected` after timeout, same as ADR-0004 risk-3 flow. | Unchanged (risk-3+ always requires approval). | Enforced. |
| **Auto** | Auto-approved *if and only if* the command does not match the Guardian denylist and does not touch a resource classified `Protected` or `Secret`. Approval is recorded in the audit log as `AutoApproved(mode=Auto)`. Denylist / classified match → falls back to Default (explicit approval). | Unchanged — risk-3+ always requires explicit approval, `Auto` cannot escalate. | Enforced. |
| **YOLO** | Auto-approved for all risk-2 calls including denylist-adjacent ones that don't literally match, recorded as `AutoApproved(mode=YOLO)`. | Unchanged — risk-3+ still requires explicit approval, YOLO cannot escalate. | Enforced. YOLO does **not** disable the denylist; it only removes the risk-2 approval prompt. |

Rationale:

- **Default** is the ADR-0004 status quo. Zero regression.
- **Auto** is what a developer wants for an interactive session where the model is scaffolding files, running tests, and starting servers in `~/workspace`. Guardian still stops the box-brickers.
- **YOLO** is for demo/experimentation where the user has accepted responsibility for anything short of the denylist. The name is deliberate — this is not a hidden setting.

The mode is displayed in the sidebar's `BackendStatus` rail at all times. Switching modes takes effect on the *next* prompt, never mid-plan (mid-plan switching would let the model choose a mode by prompt-injection).

### 5. Verifier toggle

The `Verifier` (`src/verifier.rs`) currently adds one `ModelGateway` turn per plan (~0.8–2 s with an Ox reasoning model). It is not TCB — its verdict is advisory input to the Broker. The user may toggle it off for latency-sensitive sessions.

- Toggle stored in `~/.aios/config.toml` (`verifier_enabled: bool`, default `true`) and per-session in `sessions/{date}.json`.
- When disabled, `Coordinator` skips the `Verifier::review` call and passes `VerifierVerdict::Skipped` to the Broker. The Broker treats `Skipped` identically to a passing verdict *for risk levels ≤ 2 only*. Risk-3+ requires a passing verifier verdict regardless of the toggle (defense-in-depth for approval-gated actions).
- The audit log records `verifier_skipped=true` on every skipped call.
- The toggle is displayed alongside the approval mode.

### 6. `sudo` semantics

`exec.run` never runs interactively. If a command requires elevation, it must succeed via `sudo -n` (non-interactive). Interactive `sudo` prompts are treated as failures.

- **Installer contract:** the `.deb`/first-run bootstrap writes `/etc/sudoers.d/aios` via `pkexec` with the following literal contents (no `NOPASSWD:ALL`):

  ```text
  # Managed by aios installer. Do not edit by hand.
  %aios ALL=(root) NOPASSWD: /usr/bin/apt, /usr/bin/apt-get, \
      /usr/bin/systemctl, /usr/bin/journalctl, /usr/bin/dpkg, \
      /usr/bin/snap, /usr/bin/flatpak, /usr/bin/pkexec
  ```

  The user is added to group `aios` at install time. The allowlist is **package-management and service-control only**. Editors, shells, network, filesystem, and firmware tools are deliberately excluded.

- **Runtime detection:** on first `exec.run` per session, `src/exec.rs` runs `sudo -n true` once and caches the result in the session. If it fails (`sudo: a password is required` or `sudo: sorry, you must have a tty`), any subsequent `exec.run` that contains a leading `sudo` returns `ToolError { code: OperationNotSupported, message: "sudo requires /etc/sudoers.d/aios (run aios --install-sudoers)", recoverable: false }`. No prompt, no fallback to `askpass`.
- **`--install-sudoers`:** a `src/bin/install_sudoers.rs` helper that re-runs the `pkexec`-authenticated write, for users who dismissed the installer prompt.
- **No cached credentials.** `sudo -n` is stateless from Aios's perspective; the kernel/`sudo` handles the timestamp.

### 7. Regression gates

The gates from ADR-0008 §1 and ADR-0009 §1 still apply. This ADR adds (tracked in `docs/test-ledger.md`):

- `cargo test --lib` includes new tests:
  - `sandbox_bubblewrap_confines_write` — a bwrap-sandboxed `exec.run touch /etc/x` fails with EROFS/EACCES, not with a Guardian denial (the sandbox is the boundary, not the regex).
  - `sandbox_landlock_confines_write` — same, on Landlock-only tier.
  - `sandbox_absent_locks_default_mode` — if no sandbox is available, approval mode is forced to `Default` and Auto/YOLO commands from the frontend are rejected with a typed error.
  - `exec_mode_default_requires_approval`, `exec_mode_auto_auto_approves_safe`, `exec_mode_yolo_still_runs_sandboxed` — approval-mode matrix. Note the renaming from the prior draft: even YOLO runs sandboxed.
  - `guardian_pattern_matches_are_pre_sandbox` — pattern hits deny before spawn.
  - `verifier_skip_records_audit` — audit records `verifier_skipped=true` when the toggle is off for risk ≤ 2.
  - `sudo_missing_returns_unsupported` — `sudo -n` detection returns `ToolError::OperationNotSupported` when `/etc/sudoers.d/aios` is absent.
- `cargo test --test harness_drive` covers a full plan: `files.write_file` → `exec.run cargo test` → health pass → committed, in each of the three modes, with the sandbox intercepting writes outside `~/workspace`.
- Manual desktop baseline adds: switch modes mid-session (takes effect next prompt); attempt `rm -rf /etc` in YOLO → visible `Deny(EXEC-P-001)` before the sandbox even spawns; attempt `python -c "open('/etc/x','w')"` in YOLO → not caught by pattern list, blocked by the sandbox with a clear filesystem-permission error surfaced in the UI (this is the proof that the sandbox is the boundary).

### 8. What this ADR does NOT authorize

- No `bash -i`, no interactive shells, no PTY allocation. The model cannot open a stdin-attached process.
- No `nsenter`, `unshare`, `chroot`, `setpriv`, `capsh` — no privilege-space manipulation outside `sudo` allowlist.
- No fork bombs or unbounded background processes — `exec.run` waits for the child. Long-running processes are the future `serve.specialist`'s job (Milestone 0006, not started).
- No override of ADR-0004 risk-3+ approval. Firmware writes, kernel modules, boot configuration still require explicit user approval in every mode.
- No expansion of the `sudoers.d/aios` allowlist without a new ADR.

## Consequences

- The TCB is unchanged: `Broker + Guardian + StagedExecutor + Audit + Sandbox`. `exec.specialist` is a specialist, not a TCB component. The `Sandbox` trait joins the TCB in v0.1 because its correct enforcement is what makes `exec.run` safe; a compromised `NullSandbox` implementation would break the safety property. This is the *only* TCB addition from ADR-0010.
- `security-model.md` §1.2 gains a `Sandbox` row in the TCB table (Phase 2 doc update).
- `capability-model.md §3.1` gains `execute` and `serve` operations (both risk 2 default).
- `human-interaction.md` gains a new §8 (approval modes) appended before the References section. Existing §1–§7 are unchanged, preserving cross-references from `capability-model.md §5.2` and `message-protocol.md §2.9–10, §2.12` that target §5 and §4.4.
- `implementation-roadmap.md` gains M11 (Execution Primitive) between M10 (Session Day-Buckets) and future work; M11's Phase 2 (`Sandbox` trait + `BubblewrapSandbox` + `LandlockSandbox` + `NullSandbox` + tier detection) is the *primary* deliverable, with the toggles and approval modes building on top.
- ADR-0008's "no shell tool" clause is narrowed: shell strings are permitted **only** through `exec.run`, which is a typed, brokered, Guardian-pattern-checked, sandboxed, staged, audited primitive. Naive `run_any_command` remains rejected.
- `Operation::Serve` is reserved in `capability.rs` but has no specialist yet. Documenting its intent here prevents scope drift later.
- **Honesty note.** The v1 draft of this ADR (superseded by §3 in this revision) framed the Guardian regex list as a "brickable denylist" and "safety floor." That was wrong. Regexes cannot be a shell-safety boundary. §3 as written is what the code will implement; §3 v1 language is retained in git history for traceability but should not be quoted as normative.

## Related

- docs/decisions/0004-two-dimensional-authorization.md — capability × clearance
- docs/decisions/0008-workspace-co-partner-branch-and-scope.md — the clause this ADR narrows
- docs/decisions/0009-session-day-buckets.md — persistence for mode/toggle state
- docs/capability-model.md §3.1 (updated), §5.2 (broker decision)
- docs/human-interaction.md §8 (new — approval modes and verifier toggle)
- docs/action-state-machine.md — staged execution, unchanged
- docs/security-model.md — TCB unchanged
- src/exec.rs, src/guardian.rs, src/broker.rs, src/verifier.rs, src/config.rs
