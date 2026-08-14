# Storage Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The storage specialist owns the storage domain. It is the umbrella parent of
the storage specialists: Block/Disk, Filesystem, and Files/Data. It
coordinates cross-layer concerns — capacity, health, and data safety — while
each child owns its own resource class.

Unlike the network domain (whose children are parallel transports), the
storage children are a **stack**: a file lives on a filesystem, which lives on
a block device. The hierarchy is for organization and delegation; the
dependency graph captures the stack (architecture §6).

Ownership is per-resource: each block device, filesystem, or file has exactly
one owning specialist. The hierarchy is not a substitute for the dependency
graph.

## Matching

The package matches the storage domain as a whole. Storage-specific resources
are owned by the children:
- Block/Disk owns block devices (`device:nvme0`, `device:sda`).
- Filesystem owns mounted filesystems (`Filesystem` nodes).
- Files/Data owns file-level operations (copy, move, read, write).

A resource without an unambiguous match remains read-only and is not assigned
a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_storage` | 0 | Read capacity, health, and cross-layer state |
| `diagnose_fault` | 0 | Compare observations with storage invariants |

`observe_storage` (target `all` for the whole domain) reports `disk_N` rows
per block device — reads/writes, sector and latency counters from
`/sys/block/<n>/stat`, plus rotational, scheduler, and logical/physical block
size from `queue/*` — and `fs_N` rows per mounted filesystem: fstype, mount,
backing device, mount options, read-only state, and statvfs usage (total,
used, available, used percent). A `ro` mount or an error filesystem is
reported as Degraded and counts against `degraded`. Per-filesystem usage is
gated on the live root, so hermetic discovery tests never touch real mounts.
I/O rate deltas (reads/sec, writes/sec) are deferred to the shared
window-sampling util that lands with the network pass; observe emits
cumulative counters only.

v0.1 is read-only. Mutating operations (partitioning, formatting, device
reset) are deferred to a later iteration and will require a specific
operation with a defined risk level, passing through the action state machine
and the Guardian. Read-only observation is the default (architecture §5).

## Invariants

- `STORAGE-001`: the block device is present, readable, and reports capacity.
- `STORAGE-002`: the filesystem reaches the required state after a staged
  change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The storage specialist is linked to its children with `owns` or `controls`
edges. Each child retains `depends_on` edges to the layer below it (files →
filesystem → block device → bus/driver) when discovery can verify them.
Observed relationships are advisory and are never used as a substitute for
broker capability checks.

## Recovery

v0.1 is filesystem/service-level only (ADR-0001: no boot-level changes).
Checkpoints capture the state needed to restore a staged change. A rollback
failure leaves the action in `Failed` and retains the checkpoint for manual
recovery.