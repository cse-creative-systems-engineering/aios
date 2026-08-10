# ADR-0002: Rust as the implementation language

**Status:** Accepted  
**Date:** 2026-08-09  

## Context

Aios is a safety-critical system where the separation of decision from
execution is enforced at the type level. The implementation language must
support encoding capability boundaries, state machine transitions, and
protocol schemas in a way that makes violations unrepresentable rather than
merely discouraged.

Candidate languages considered:

- **Rust** — memory-safe, no GC, expressive type system, strong Linux
  user-space ecosystem, native model inference bindings.
- **C/C++** — maximum control and kernel proximity, but no memory safety and
  high defect risk in a system where safety is the primary goal.
- **Go** — good concurrency and simplicity, but weaker type system for
  encoding invariants; GC pauses may matter near real-time paths.
- **Python** — fast prototyping and ML ecosystem, but unsuitable as the
  enforcement layer for a safety-critical system.

## Decision

**Aios is implemented in Rust.** The core system — policy broker, capability
model, message protocol, state machine, System Graph, agent runtime, and
specialists — is Rust. Model inference uses Rust bindings to native runtimes
(`llama.cpp`, `candle`, or `mistral.rs`).

The System State panel may use a Rust-native GUI framework (`egui`, `tauri`)
or a web-based frontend communicating with the Rust backend, to be decided in
a future ADR.

## Consequences

**Positive:**

- Capability boundaries, state machine transitions, and protocol schemas can
  be encoded as types that make invalid states unrepresentable.
- Ownership and borrowing model capability transfer and revocation naturally.
- `Send`/`Sync` traits provide compile-time concurrency safety for
  multi-agent coordination.
- No GC pauses — important for paths that approach real-time-adjacent
  behavior.
- Single static binary distribution; no runtime dependency.
- Mature Linux user-space crates: `udev`, `nix`, `zbus`, `sysinfo`, `tokio`.
- Native model inference without leaving the language ecosystem.

**Negative:**

- Compile times will slow as the codebase grows. Acceptable for a system
  where correctness outweighs iteration speed.
- Steeper learning curve for contributors compared to Go or Python.
- Some ML ecosystem tooling is Python-first; model evaluation scripts may
  need Python alongside the Rust core.

**Neutral:**

- Rust can interoperate with C and Python via FFI if needed for specific
  kernel interfaces or ML tooling.
- The choice is compatible with a future move below Linux, as Rust can target
  `no_std` and kernel modules.

## Related

- ADR-0001 (runs above Linux)
- `docs/capability-model.md` (where the type system earns its keep)
- `docs/message-protocol.md` (serde-based serialization)
