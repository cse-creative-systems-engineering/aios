# Bluetooth Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The bluetooth specialist owns the bluetooth transport: bluetooth controllers
and their connected devices. It reports controller, link, and device state
through bounded tools. It does not expose shell execution or unrestricted
file access.

It is a child of the Network specialist (architecture §6 hierarchy). The
Wi-Fi and wired/LAN transports are sibling specialists under the same Network
parent.

## Matching

The package matches discovered `Device` nodes that are bluetooth controllers
(USB devices with Bluetooth interface class `0xE0`, or devices that map to
`/sys/class/bluetooth`). Classification is structural, not heuristic — a
bluetooth controller on a combo Wi-Fi/Bluetooth module is not counted as a
Wi-Fi device. A controller without an unambiguous match remains read-only and
is not assigned a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_device` | 0 | Read controller, link, and device state |
| `diagnose_fault` | 0 | Compare observations with bluetooth invariants |

v0.1 is read-only. Mutating operations are deferred to a later iteration and
will require a specific operation with a defined risk level, passing through
the action state machine and the Guardian.

## Invariants

- `BT-001`: the bluetooth controller is present and reports link state.
- `BT-002`: the controller reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its bluetooth controller with `owns`. The
controller retains `depends_on` edges to its bus and driver when discovery
can verify them. Observed relationships are advisory and are never used as a
substitute for broker capability checks.

## Recovery

v0.1 is read-only. Checkpoints capture the state needed to restore a staged
change when mutating operations are added. A rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.