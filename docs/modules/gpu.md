# GPU Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The GPU specialist owns the GPU layer of the graphics domain: the graphics
processing unit and its driver. The GPU is second only to the CPU in hardware
importance. It reports GPU, driver, and compute state through bounded tools.
It does not expose shell execution or unrestricted file access.

It is a child of the Graphics specialist (architecture §6 hierarchy). The
display layer above it is owned by the Display specialist; the session layer
is owned by the Session specialist.

## Matching

The package matches `Device` nodes that are GPUs (e.g. `device:gpu0`). A
resource without an unambiguous match remains read-only and is not assigned a
privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_gpu` | 0 | Read GPU, driver, and compute state |
| `diagnose_fault` | 0 | Compare observations with GPU invariants |

v0.1 is read-only. Mutating operations (GPU reset, clock/power control) are
deferred to a later iteration and will require a specific operation with a
defined risk level, passing through the action state machine and the
Guardian.

## Invariants

- `GPU-001`: the GPU is present and reports state.
- `GPU-002`: the GPU reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its GPU with `owns`. The GPU retains `depends_on`
edges to its bus and driver when discovery can verify them. Observed
relationships are advisory and are never used as a substitute for broker
capability checks.

## Recovery

v0.1 is read-only. When mutating operations are added, checkpoints capture
the state needed to restore a staged change; a rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.