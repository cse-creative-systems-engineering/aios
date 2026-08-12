# Memory Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The memory specialist owns the system's memory domain: physical memory,
swap, pressure, and ECC state. It reports memory health and usage through
bounded tools. It does not expose shell execution or unrestricted file
access.

## Matching

The package matches the memory resources the system exposes (e.g. the
`memory` node and per-bank ECC sensors). A system without an unambiguous
memory resource remains read-only and is not assigned a privileged
specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_memory` | 0 | Read total, used, free, swap, and pressure state |
| `diagnose_fault` | 0 | Compare observations with memory invariants |
| `stage_policy` | 2 | Checkpoint current memory policy and stage a candidate |
| `request_reset` | 4 | Request a memory controller reset to known-good state |

`stage_policy` uses the action state machine: checkpoint → stage → health
check → commit or rollback. A failed health check always rolls back. A reset
requires a scoped approval and never bypasses the Guardian.

## Invariants

- `MEMORY-001`: the memory subsystem is present and reports usable capacity.
- `MEMORY-002`: ECC errors are within the tolerated threshold after a staged
  change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its memory resource with `owns`. The resource
retains `depends_on` edges to its controller, bus, and ECC sensors when
discovery can verify them. Observed relationships are advisory and are never
used as a substitute for broker capability checks.

## Recovery

v0.1 is policy-level only. Checkpoints contain the current memory policy and
configuration needed to restore it. The package does not modify the boot chain
or firmware. A rollback failure leaves the action in `Failed` and retains the
checkpoint for manual recovery.