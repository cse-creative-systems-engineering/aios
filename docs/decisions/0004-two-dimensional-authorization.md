# ADR-0004: Two-dimensional authorization (capability × tool risk level)

**Status:** Accepted  
**Date:** 2026-08-09  

## Context

The capability model as originally conceived has one dimension: a capability
is a (resource, operation) pair that an agent is authorized to request. This
works for gating *which* resources an agent can touch, but it does not encode
*how dangerous* an operation is.

Without a risk dimension, the only way to prevent a Wi-Fi specialist from
writing firmware is to not grant it the `firmware_write` capability. This
requires enumerating every dangerous operation in the capability system and
hoping no new dangerous tool is added without a corresponding capability
denial. It also means the Guardian must review every tool call, because there
is no way to fast-path safe operations.

## Decision

**Aios uses a two-dimensional authorization model:**

1. **Capability** — a (resource, operation) pair that an agent is authorized
   to request. Per-resource granularity (e.g., `device:wifi0`, not
   `device:wifi*`).

2. **Tool risk level** — a 0–4 classification of how dangerous a tool
   operation is. Each Agent Package declares a maximum clearance level. The
   broker grants or denies clearance at instantiation based on the signed
   package manifest.

An agent needs **both** a valid capability and sufficient clearance to
execute a tool operation.

### Tool risk levels

| Level | Name | Examples | Requires |
|---|---|---|---|
| 0 | Read-only | `observe_device`, `get_health`, `diagnose_fault` | Capability only |
| 1 | Routine | Non-destructive config, service restart, query state | Capability + broker validation |
| 2 | Staged mutation | `stage_driver`, config changes with rollback | Capability + broker + Guardian + staging |
| 3 | Critical mutation | Firmware writes, boot config, kernel module loading | Capability + broker + Guardian + user approval + staging |
| 4 | Recovery | Device reset, quarantine, rollback to known-good | Capability + broker + Guardian + user approval (staging may be skipped only if the Guardian authorizes it; a checkpoint is still created first) |

### Agent clearance

Each Agent Package declares a maximum clearance level in its manifest. The
broker grants or denies at instantiation. An agent with clearance 1 cannot
use level 2+ tools, even if it has the resource capability.

Clearance is static — set at instantiation, fixed for the agent's lifetime.
If an agent needs higher clearance, that is a package revision, not a
runtime request.

### Broker decision flow

```text
ToolRequest arrives at broker
  1. Validate agent identity (from signed package)
  2. Validate capability (agent has resource + operation)
  3. Validate clearance (agent clearance >= tool risk level)
  4. For level 2+: Guardian review
  5. For level 3+: user approval
  6. For level 2+: staged execution with rollback
  7. Audit log entry (always, including denials)
  → Execute or Deny
```

## Consequences

**Positive:**

- Separates "what resource" from "how dangerous." New dangerous tools are
  automatically gated by risk level without reconfiguring every agent's
  capabilities.
- Guardian only reviews level 2+ calls. Level 0 and 1 operations are
  fast-pathed through the broker, reducing Guardian load without reducing
  safety for dangerous operations.
- Creates a natural escalation path: higher clearance requires a package
  revision, not a runtime request. Same principle as static capabilities.
- Defense in depth: capability is necessary but not sufficient. Each stage
  (broker, Guardian, staging, approval) is an independent gate.
- Token cost is not a design constraint for safety systems. Per-resource
  granularity and per-stage validation are chosen for safety, not efficiency.

**Negative:**

- More checks per action than a single-gate model. This is intentional and
  aligned with the fail-fast, safety-first principle.
- Adding a new tool requires assigning it a risk level. This is a design-time
  decision documented in the tool's specialist package.

**Neutral:**

- The two-dimensional model is compatible with future extensions (e.g.,
  time-limited clearance, conditional capabilities) without redesigning the
  base model.

## Related

- ADR-0002 (Rust as implementation language — type system enforces clearance)
- ADR-0003 (fail-fast — missing clearance causes immediate failure)
- `docs/security-model.md` section 3.2 (elevation of privilege defenses)
- `docs/capability-model.md` (will define the full model)
