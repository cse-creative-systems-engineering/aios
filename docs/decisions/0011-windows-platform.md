# ADR-0011: Windows Platform Support

**Status:** Accepted
**Date:** 2026-08-25
**Amends:** ADR-0001 (Aios v0.1 runs above Linux in user space)
**Related:** security-model.md §1 (TCB), capability-model.md, milestone 0004+ (co-partner)

## Context

ADR-0001 scoped v0.1 to Linux deliberately: the hard problems were authorization and
staging, and Linux offered sysfs/systemd/landlock/bubblewrap to build them against.
That foundation now exists — broker, guardian, executor, sessions, surfaces, sandboxed
exec — and is ~80% platform-neutral Rust.

The owner develops on Windows (ASUS ROG Strix) inside WSL2 today. WSL works but adds
friction (file I/O latency, display bridging, connection drops), and the product goal
has always included Windows. The owner intends to clone the repo natively on Windows
and develop there once the port's scaffolding exists.

This ADR decides how Windows support is structured so that:

1. `main` stays shippable on Linux at every point in the port.
2. Platform-specific code has typed boundaries, not scattered `#[cfg]` soup.
3. The safety architecture (broker × clearance → Guardian → staged executor → audit)
   survives the port unchanged. No Windows path bypasses the TCB.

## Decision

### 1. Phased port, each phase merged early

Windows work happens in small phases that merge back to `main` behind
`#[cfg(windows)]` boundaries. No long-lived divergent branch (lesson from the
session-day-buckets merge).

| Phase | Scope | Gate | Where developed |
|---|---|---|---|
| **W1: Core builds** | Library compiles + 424 lib tests pass on Windows MSVC. Linux-only modules (`discovery`, `sandbox`, `local` llama backend, `wifi_driver`) stubbed behind traits with `#[cfg(windows)]` no-op or error implementations that fail closed. | `cargo test --lib` green on both platforms; CI gains a windows-latest job | Started from WSL via `cargo check --target x86_64-pc-windows-msvc`; finished/verified natively |
| **W2: Shell** | Tauri on WebView2; sidebar as AppBar (or floating window first); canvas overlay reworked for Windows input-region semantics; frontend unchanged where possible. | Desktop baseline (CPU/RAM surface, drag, click-through) passes on Windows 11 | Native Windows only |
| **W3: Discovery/specialists** | Windows discovery backend (WMI / WinAPI / ETW) feeding the same `SystemGraph` node types. Specialists map: storage→LogicalDisk/PhysicalDisk, network→MSFT_NetAdapter, processes→Win32_Process, power→thermal zones, packages→installed-programs + winget, boot→BCD, security→Defender/LSA. Wi-Fi → native Wi-Fi API. Graphics → DXGI/D3DKMT. | Each specialist lands with observe/diagnose through the broker, same contracts as M7 | Native Windows |
| **W4: Execution sandbox** | Windows equivalent of ADR-0010 tiers: Job Objects (mandatory baseline confinement) + restricted tokens; optional AppContainer or Windows Sandbox integration for a Full tier. Approval-mode interaction identical: no-sandbox tier locks mode to Default (fail-closed). | Same §7 regression matrix as ADR-0010, adapted | Native Windows |

Phases W1–W2 unblock development; W3 makes it *Aios*; W4 makes it safe to let the
model run commands.

### 2. Trait boundaries are the port mechanism

Where Linux code reaches for the OS directly, the port introduces (or confirms) a
trait with per-platform implementations selected by `#[cfg]`. Candidates already
identified:

- `ConnectivityProbe` (exists)
- `Sandbox` (exists — gains `JobObjectSandbox`, `AppContainerSandbox`)
- Discovery: new `SystemScanner` trait wrapping sysfs/systemctl vs WMI
- Model runtime: `local.rs` llama.cpp stays (llama.cpp builds on Windows); GPU via
  Vulkan/CUDA builds of llama.cpp, not DirectML initially

No specialist logic may call OS APIs directly; it goes through its scanner/driver
abstraction. This keeps the broker/guardian/executor stack byte-identical across
platforms — which is what lets us say the safety model is unchanged.

### 3. What does NOT change

- The TCB: broker, guardian, executor, audit remain single-sourced, shared code.
- Capability/clearance model, risk levels, approval semantics (ADR-0004).
- Session persistence format (ADR-0009) — JSON paths move to `%APPDATA%`/dirs crate.
- Groundless surface generation and fidelity gating (ADR-0007).
- Fail-closed discipline (ADR-0003): a missing Windows discovery source renders
  `Unknown`, never silently healthy.

### 4. Repository and tooling

- Development moves natively to Windows (`C:\dev\aios` clone, Zed for Windows) at the
  start of W1 verification / W2. Until then all work continues in WSL.
- `.gitattributes` audited before the first Windows commit: sources pinned LF;
  CRLF never enters the history.
- CI: add `windows-latest` job running `cargo test --lib` at W1; extend with
  frontend build at W2. Desktop e2e remains manual-baseline until a Windows runner
  story exists.
- Build prerequisites documented in README (MSVC toolchain, Node, CMake for
  llama.cpp; vcpkg if needed).

### 5. Branch rule

Same discipline as ADR-0008: short-lived branches per phase
(`feature/windows/core-build`, `feature/windows/shell`, …), merged to `main` when
their gate passes. `main` must build on BOTH platforms after W1 lands — enforced by
CI, not convention.

## Consequences

- ADR-0001 is amended: Aios runs above a host OS in user space; v0.x targets Linux,
  Windows follows. macOS remains out of scope (no decision either way).
- Roughly half the remaining specialist work (W3) is comparable in scope to the
  entire M7 effort — this is stated plainly so the port is resourced honestly.
- Some Linux capabilities have no exact Windows analogue (landlock, namespaces).
  Where isolation is weaker (Job Objects vs bwrap), the approval-mode fail-closed
  rule compensates: weaker tier ⇒ stricter human gating. This mirrors ADR-0010 §3.1.
- WSL remains a supported dev environment for Linux-side work indefinitely.

## What this ADR does NOT authorize

- No shipping of exec.run on Windows before W4 lands (no unconfined shell, ever).
- No divergence of the enforcement plane per platform.
- No dropping of Linux-first behavior: sysfs/systemd discovery stays the reference
  implementation that Windows discovery is validated against.

## Related

- docs/decisions/0001-v01-runs-above-linux.md — amended
- docs/decisions/0010-execution.md — sandbox tier model extended to Windows
- docs/security-model.md §1 — TCB unchanged by this port
