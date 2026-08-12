# Security and Identity Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md, security-model.md

## Scope

The security and identity specialist owns the security domain: identity,
credentials, trust boundaries, and anomaly response. It reports security
state through bounded tools. It does not expose shell execution or
unrestricted file access.

Secrets never leave the local trust boundary (security-model §5). Credentials
and keys are never sent to models, never recorded in logs, and never accepted
as agent input. The broker handles credential injection directly.

## Matching

The package matches identity, credential, and security-boundary resources. A
resource without an unambiguous match remains read-only and is not assigned a
privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_security` | 0 | Read identity, trust, and security state |
| `diagnose_fault` | 0 | Compare observations with security invariants |
| `quarantine` | 4 | Quarantine a capability while preserving evidence |

`quarantine` is the bounded containment response to a security anomaly
(architecture §12). It is risk 4, requires a scoped approval, and never
bypasses the Guardian. It preserves evidence and is reversible (un-quarantine).
Deletion, credential rotation, and broader isolation require human approval.

## Invariants

- `SEC-001`: identity and trust boundaries are present and verified.
- `SEC-002`: no secret leaves the local trust boundary.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its identity and security resources with `owns`.
Each resource retains `depends_on` edges to the components it protects when
discovery can verify them. Observed relationships are advisory and are never
used as a substitute for broker capability checks.

## Recovery

v0.1 is read-only plus quarantine. Quarantine is risk 4, approval-gated, and
preserves evidence. Checkpoints capture the state needed to restore a
quarantined capability. A rollback failure leaves the action in `Failed` and
retains the checkpoint for manual recovery.