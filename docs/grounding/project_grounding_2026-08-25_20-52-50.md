# Grounding Snapshot: W1: Windows MSVC library gate green

## Current State

ADR-0011 Phase W1 is functionally complete on
`feature/windows/core-build`: the library compiles and the test suite is
green natively on Windows MSVC (rustc 1.98.0, x86_64-pc-windows-msvc) —
**345 passed, 0 failed, 1 ignored** via `cargo test --lib`. Linux behavior
is unchanged by construction: every change is a `#[cfg]` split whose Unix
side is byte-identical to `main @ 45deeb6`, plus one added test. Linux must
still be re-baselined on CI before merge.

What made this possible beyond code:

- Windows toolchain installed and verified end to end (VS Build Tools 2022
  C++ workload, rustup stable-msvc, CMake, LLVM/libclang, Node LTS, Python
  3.12). llama.cpp compiles natively through `llama-cpp-2`; the earlier
  "cannot find libclang" failure is solved by `LIBCLANG_PATH` (persisted as
  a user env var).
- `.gitattributes` now pins `* text=auto eol=lf` with binary exceptions and
  CRLF only for `.bat/.cmd/.ps1` (ADR-0011 §4). The index was renormalized;
  history stays LF-only.
- CI gained a `windows-latest` job running `cargo test --lib` with
  `LIBCLANG_PATH` set for bindgen.

Known platform posture after W1 (all fail-closed by design):

- Discovery is Linux-only; `Coordinator::boot` on Windows fails with
  `Discovery("scan failed: no sysfs or procfs tree at /")` until W3.
- `exec.run` has no sandbox tier on Windows (`NullSandbox`), so
  `run_confined` refuses to spawn — never runs unconfined — until W4
  (Job Objects / AppContainer behind the existing `Sandbox` trait).
- Filesystem usage evidence returns `None` on Windows rather than fabricated
  numbers.

## Relevant Paths

- `src/discovery.rs` — `filesystem_usage` split (statvfs on Unix; `None` on
  Windows); `mock_symlink` cfg-split helper; 13 mock-sysfs/systemctl tests
  scoped `#[cfg(unix)]` (fixture needs symlinks + colon-bearing PCI dir
  names that Windows forbids). Note: this file also absorbed rustfmt reflow
  from the newer toolchain.
- `src/action.rs` — `sync_dir` split: directory fsync on Unix; `Ok(())` on
  Windows (NTFS metadata journaling covers rename durability; data still
  flushed via `write_file_synced`). This unblocked executor/broker/files
  staged-action tests on Windows.
- `src/session.rs` — `SessionStore::today()` no longer shells out to
  `date -u +%F` (silently degraded to `day-N` on Windows); pure civil-from-
  days math in `date_from_days`, unit-tested against known UTC dates.
- `src/exec.rs` — `exec_runs_echo` scoped `#[cfg(unix)]` (needs a
  spawn-capable sandbox tier; W4 brings one).
- `src/main.rs` — M1 demo's live-discovery tail scoped `#[cfg(unix)]`;
  `cargo run` now completes cleanly on Windows (enforcement-plane section
  only), instead of panicking at the sysfs scan.
- `src/facade.rs`, `src/coordinator/tests.rs` — boot-dependent test modules
  scoped to Unix (19 + 41 tests) pending the W3 scanner.
- `.gitattributes`, `.github/workflows/ci.yml` — LF policy; windows job.
- `README.md` — Windows build prerequisites (winget one-liners).
- `docs/test-ledger.md` — W1 entry: +1 test, 74 Unix-scoped (with per-group
  rationale), platform splits documented.

## Open Work

1. **Linux re-baseline** — run `cargo test --lib` + fmt/clippy gates on
   Linux CI for this branch; expected count = prior baseline + 1 (the new
   date test). Merge gate per ADR-0011 requires green on both platforms.
2. **Pre-existing CI drift (not from W1):** current-stable rustfmt/clippy
   flag ~53 lints across ~22 untouched files (`project.rs`, `planner.rs`,
   `broker.rs`, …) and fmt drift in e.g. `stub_provider.rs`. `main`'s CI is
   presumably red under rustc 1.98 already. Needs a separate housekeeping
   commit (`cargo fmt --all` + clippy fixes) so the CI gate means something.
3. **W2 (shell):** target-gate gtk/gdkx11/gtk-layer-shell deps in
   `src-tauri/Cargo.toml`; WebView2 baseline; sidebar without layer-shell.
4. **W3 (discovery):** `SystemScanner` trait + WMI backend; un-gate the 60
   boot-dependent tests against an injected scanner; real filesystem usage
   via LogicalDisk.
5. **W4 (sandbox):** `JobObjectSandbox`/`AppContainerSandbox` behind the
   existing `Sandbox` trait; tier detection wired into approval-mode lock.
