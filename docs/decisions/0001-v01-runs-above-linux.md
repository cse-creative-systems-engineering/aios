# ADR-0001: Aios v0.1 runs above Linux in user space

**Status:** Accepted  
**Date:** 2026-08-09  

## Context

Aios is ultimately intended to be an AI-native operating environment that may
integrate at the kernel, firmware, or hardware level. However, the architecture
document (section 16) leaves open whether the first implementation should run
above Linux, use a microkernel, or eventually use a custom kernel.

Building directly on or below the kernel from the start would require solving
kernel development, driver signing, boot chain integrity, and hardware control
problems simultaneously with the agent coordination, capability, and protocol
problems. That conflates two large risk areas and slows validation of the
concepts that are novel to Aios: the dual-agent bridge, capability broker,
staged execution, System Graph, and Agent Package model.

## Decision

**Aios v0.1 is a user-space prototype that runs above an existing Linux
distribution.** It does not modify the kernel, boot chain, or firmware. It
interacts with the OS through standard Linux interfaces (sysfs, procfs, udev,
D-Bus, command-line tools) exposed as typed tools behind the policy broker.

The first prototype validates the agent, specialist, graph, message, policy,
and dashboard concepts. It does not take responsibility for lower-level
operating-system behavior.

## Consequences

**Positive:**

- The novel Aios concepts can be prototyped and tested without kernel
  development risk.
- Linux provides mature hardware discovery (udev, sysfs), telemetry (procfs,
  sysfs), and device control interfaces.
- The Rust ecosystem has strong Linux user-space libraries.
- Recovery and rollback can use existing Linux mechanisms (snapshots, A/B boot
  images) rather than requiring custom solutions.
- The prototype can run on any Linux machine without special installation.

**Negative:**

- Aios cannot enforce true capability isolation at the hardware level. A
  compromised or buggy agent could potentially bypass the broker if it has
  direct OS access. This is acceptable for a prototype but not for production.
- Some hardware control operations will be limited to what Linux user-space
  interfaces expose.
- The trust plane is not truly independent of the OS — it relies on Linux's
  own kernel and boot integrity.

**Neutral:**

- This decision is reversible. A later version may move below Linux or to a
  custom kernel. The agent, protocol, capability, and graph designs should be
  OS-agnostic at their specification level.

## Related

- `docs/architecture.md` section 16 (open questions)
- `docs/architecture.md` section 18 (implementation strategy)
- Future ADR: when and whether to move below Linux
