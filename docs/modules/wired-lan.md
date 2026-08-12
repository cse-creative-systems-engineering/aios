# Wired/LAN Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The wired/LAN specialist owns the wired network transport: ethernet
interfaces and their link state. It reports interface, link, and
network-service state through bounded tools. It does not expose shell
execution or unrestricted file access.

It is a child of the Network specialist (architecture §6 hierarchy). The
Wi-Fi and Bluetooth transports are sibling specialists under the same Network
parent.

## Matching

The package matches discovered `Device` nodes that are wired network
interfaces (from `sys/class/net`, e.g. `device:net-eth1`), excluding wireless
interfaces (owned by the Wi-Fi specialist). An interface without an
unambiguous match remains read-only and is not assigned a privileged
specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_device` | 0 | Read interface, link, and network-service state |
| `diagnose_fault` | 0 | Compare observations with wired-network invariants |

v0.1 is read-only. Mutating operations are deferred to a later iteration and
will require a specific operation with a defined risk level, passing through
the action state machine and the Guardian.

## Invariants

- `WIRED-001`: the wired interface is present and reports link state.
- `WIRED-002`: the interface reaches the required link state after a staged
  change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its wired interface with `owns`. The interface
retains `depends_on` edges to its bus, driver, and network service when
discovery can verify them. Observed relationships are advisory and are never
used as a substitute for broker capability checks.

## Recovery

v0.1 is read-only. Checkpoints capture the state needed to restore a staged
change when mutating operations are added. A rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.