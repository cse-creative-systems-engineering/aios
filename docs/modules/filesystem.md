# Filesystem Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The filesystem specialist owns the filesystem layer of the storage domain:
mounted filesystems, their usage, and their health. It reports mount state,
capacity, and filesystem health through bounded tools. It does not expose
shell execution or unrestricted file access.

It is a child of the Storage specialist (architecture §6 hierarchy). The block
device below it is owned by the Block/Disk specialist; file-level operations
are owned by the Files/Data specialist.

## Matching

The package matches discovered `Filesystem` nodes (mounted filesystems). A
filesystem without an unambiguous match remains read-only and is not assigned
a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_fs` | 0 | Read mount state, usage, and filesystem health |
| `diagnose_fault` | 0 | Compare observations with filesystem invariants |

v0.1 is read-only. Mutating operations (mount, unmount, fsck, quota) are
deferred to a later iteration and will require a specific operation with a
defined risk level, passing through the action state machine and the
Guardian.

## Invariants

- `FS-001`: the filesystem is mounted and reports usage.
- `FS-002`: the filesystem reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its filesystem with `owns`. The filesystem retains
`depends_on` edges to its block device when discovery can verify them.
Observed relationships are advisory and are never used as a substitute for
broker capability checks.

## Recovery

v0.1 is filesystem/service-level only (ADR-0001: no boot-level changes).
Checkpoints capture the state needed to restore a staged change. A rollback
failure leaves the action in `Failed` and retains the checkpoint for manual
recovery.