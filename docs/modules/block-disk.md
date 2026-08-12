# Block/Disk Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The block/disk specialist owns the block-device layer of the storage domain:
NVMe, SATA, and USB storage devices. It reports device health, capacity, and
driver state through bounded tools. It does not expose shell execution or
unrestricted file access.

It is a child of the Storage specialist (architecture §6 hierarchy). The
filesystem layer above it is owned by the Filesystem specialist; file-level
operations are owned by the Files/Data specialist.

## Matching

The package matches discovered `Device` nodes that are block devices (from
`sys/class/block`, e.g. `device:nvme0`, `device:sda`). A device without an
unambiguous match remains read-only and is not assigned a privileged
specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_device` | 0 | Read device, capacity, health, and driver state |
| `diagnose_fault` | 0 | Compare observations with block-device invariants |

v0.1 is read-only. Mutating operations (partitioning, formatting, device
reset) are deferred to a later iteration and will require a specific
operation with a defined risk level, passing through the action state machine
and the Guardian.

## Invariants

- `BLOCK-001`: the block device is present, readable, and reports capacity.
- `BLOCK-002`: the device reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its block device with `owns`. The device retains
`depends_on` edges to its bus and driver when discovery can verify them.
Observed relationships are advisory and are never used as a substitute for
broker capability checks.

## Recovery

v0.1 is filesystem/service-level only (ADR-0001: no boot-level changes).
Checkpoints capture the state needed to restore a staged change. A rollback
failure leaves the action in `Failed` and retains the checkpoint for manual
recovery.