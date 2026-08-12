# Packages and Updates Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The packages and updates specialist owns the package and update domain:
package installation, activation, update, revocation, and rollback. It
reports package state through bounded tools. It does not expose shell
execution or unrestricted file access.

Package installation, activation, update, revocation, and rollback are
privileged lifecycle operations (agent-packages.md). A package update must
not silently broaden an existing agent's capabilities.

## Matching

The package matches package resources and the package registry. A resource
without an unambiguous match remains read-only and is not assigned a
privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_package` | 0 | Read package, version, and signature state |
| `diagnose_fault` | 0 | Compare observations with package invariants |
| `stage_update` | 2 | Checkpoint current state and stage a candidate update |
| `request_rollback` | 4 | Roll back a failed update to a known-good state |

`stage_update` uses the action state machine: checkpoint → stage → health
check → commit or rollback. A failed health check always rolls back. Updates
are automatic for low-risk changes; kernel, boot, security, and irreversible
changes require approval (architecture §12). `request_rollback` is risk 4,
requires a scoped approval, and never bypasses the Guardian.

## Invariants

- `PKG-001`: packages are present, signed, and versioned.
- `PKG-002`: an update does not silently broaden an agent's capabilities.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its package resources with `owns`. Each package
retains `depends_on` edges to the components it affects when discovery can
verify them. Observed relationships are advisory and are never used as a
substitute for broker capability checks.

## Recovery

v0.1 is module/service-level only (ADR-0001: no boot-level changes).
Checkpoints capture the state needed to restore a staged update. A rollback
failure leaves the action in `Failed` and retains the checkpoint for manual
recovery.