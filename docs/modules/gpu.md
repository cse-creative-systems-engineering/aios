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

## Live runtime observations

The live-state collector uses a compiled-in, read-only adapter catalog. On
Linux, the NVIDIA adapter is enabled only when `nvidia-smi` is available and
successfully responds. AMD and Intel adapters are enabled only when a matching
PCI vendor is present in Linux DRM/sysfs. All are sampled at most once every
five seconds. They publish stable projection keys for each available value,
including `gpu.<id>.temperature_c`, `gpu.<id>.utilization_percent`, memory,
and power. NVIDIA additionally publishes GPU-associated process memory.
NVIDIA IDs retain their numeric index; DRM IDs are vendor-qualified (for
example, `amd-card0` and `intel-card1`) so mixed-driver machines cannot
overwrite each other's observations. Missing commands, unsupported fields, and
`N/A` values publish no fact rather than an invented zero or a healthy status.

The state store can produce a bounded temporal-overlap finding when a GPU's
temperature rose by at least 2 C, a process held memory on that same GPU, and
a non-loopback host interface carried traffic within 15 seconds. This is only
possible for an adapter that truthfully reports GPU-associated process memory;
the AMD and Intel DRM adapters do not manufacture that attribution. The finding
names all source keys, carries its rule and freshness, and explicitly says it
is not causal attribution: host-interface traffic cannot prove which process
caused it.

## Graph relationships

The specialist is linked to its GPU with `owns`. The GPU retains `depends_on`
edges to its bus and driver when discovery can verify them. Observed
relationships are advisory and are never used as a substitute for broker
capability checks.

## Recovery

v0.1 is read-only. When mutating operations are added, checkpoints capture
the state needed to restore a staged change; a rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.
