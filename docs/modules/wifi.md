# Wi-Fi Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The Wi-Fi specialist owns one discovered wireless device at a time. It reports
device, driver, firmware, link, and network-service state through bounded
tools. It does not expose shell execution or unrestricted file access.

It is a child of the Network specialist (architecture §6 hierarchy). The
wired/LAN and Bluetooth transports are sibling specialists under the same
Network parent.

## Matching

The package matches a discovered `Device` whose attributes identify a wireless
PCI or USB controller. A device without an unambiguous match remains
read-only and is not assigned a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_device` | 0 | Read device, driver, firmware, and link state |
| `diagnose_fault` | 0 | Compare observations with Wi-Fi invariants |
| `stage_driver` | 2 | Checkpoint the current module and stage a candidate |
| `request_reset` | 4 | Request a device reset to known-good state |

`stage_driver` uses the action state machine: checkpoint → stage → health
check → commit or rollback. A failed health check always rolls back. A reset
requires a scoped approval and never bypasses the Guardian.

## Invariants

- `DRIVER-001`: the active driver is present, loadable, and attached to the
  discovered device.
- `NETWORK-002`: the interface reaches the required link state after a staged
  change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its device with `owns`. The device retains
`depends_on` edges to its bus, driver, firmware, and network service when
discovery can verify them. Observed relationships are advisory and are never
used as a substitute for broker capability checks.

## Recovery

v0.1 is module-level only. Checkpoints contain the current driver module and
configuration needed to restore it. The package does not modify the boot chain
or firmware. A rollback failure leaves the action in `Failed` and retains the
checkpoint for manual recovery.
