# Network Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The network specialist owns the network domain. It is the umbrella parent of
the transport specialists: Wi-Fi, Wired/LAN, and Bluetooth (architecture §6
hierarchy). It coordinates cross-transport concerns — connectivity, routing,
and network-service state — while each transport child owns its own resource
class.

Ownership is per-resource: each interface or controller has exactly one
owning specialist. The hierarchy is for organization and delegation, not a
substitute for the dependency graph (architecture §6).

## Matching

The package matches the network domain as a whole. Transport-specific
resources are owned by the transport children:
- Wi-Fi owns wireless interfaces (`device:net-wlp*`).
- Wired/LAN owns ethernet interfaces (`device:net-eth*`).
- Bluetooth owns bluetooth controllers (`device:usb-*` with Bluetooth class).

A resource without an unambiguous transport match remains read-only and is
not assigned a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_network` | 0 | Read connectivity, routing, and network-service state |
| `diagnose_fault` | 0 | Compare observations with network invariants |

v0.1 is read-only. Mutating operations are deferred to a later iteration and
will require a specific operation with a defined risk level, passing through
the action state machine and the Guardian.

## Invariants

- `NETWORK-001`: the network domain is present and reports connectivity.
- `NETWORK-002`: a transport reaches the required link state after a staged
  change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The network specialist is linked to its transport children with `owns` or
`controls` edges. Each transport retains `depends_on` edges to its bus,
driver, and network service when discovery can verify them. Observed
relationships are advisory and are never used as a substitute for broker
capability checks.

## Recovery

v0.1 is read-only. Checkpoints capture the state needed to restore a staged
change when mutating operations are added. A rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.