# Files/Data Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The files/data specialist owns the file-level layer of the storage domain:
copy, move, read, write, and delete operations on files. It reports file and
data state through bounded tools. It does not expose shell execution or
unrestricted file access.

It is a child of the Storage specialist (architecture §6 hierarchy). The
filesystem below it is owned by the Filesystem specialist; the block device is
owned by the Block/Disk specialist.

## Matching

The package matches file and data resources within the storage domain. A
resource without an unambiguous match remains read-only and is not assigned a
privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_file` | 0 | Read file and data state |
| `diagnose_fault` | 0 | Compare observations with data invariants |

v0.1 is read-only. Mutating operations (copy, move, write, delete) are
deferred to a later iteration and will require a specific operation with a
defined risk level, passing through the action state machine and the
Guardian.

## Invariants

- `DATA-001`: the file is present and readable.
- `DATA-002`: the data reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its file resource with `owns`. The file retains
`depends_on` edges to its filesystem when discovery can verify them. Observed
relationships are advisory and are never used as a substitute for broker
capability checks.

## Recovery

v0.1 is filesystem/service-level only (ADR-0001: no boot-level changes).
Checkpoints capture the state needed to restore a staged change. A rollback
failure leaves the action in `Failed` and retains the checkpoint for manual
recovery.