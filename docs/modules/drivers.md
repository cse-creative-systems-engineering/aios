# Drivers and Hardware Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The drivers and hardware specialist is a peer of the domain specialists
(Network, Storage, Graphics — architecture §5). It owns the cross-cutting
device inventory concern: PCI and USB devices as hardware, firmware state, and
loaded kernel modules. It reports device, driver, firmware, and module state
through bounded tools. It does not expose shell execution or unrestricted file
access.

It owns the generic hardware that no domain specialist owns. Domain-specific
devices are owned by their domain: Wi-Fi interfaces by the Network children,
block devices by the Storage children, GPUs by the Graphics children. This
keeps one owner per resource (architecture §5). The Drivers specialist may
implement driver staging, bound to the devices it owns; it does not stage
drivers for devices owned by another specialist.

Device classification is structural, not heuristic. A device is classified by
its bus and interface class (e.g. USB interface class `0xE0` for Bluetooth,
`0x028` for Wi-Fi), not by matching the word "wireless" in a self-reported
name. This keeps a Bluetooth controller on a combo Wi-Fi/Bluetooth module from
being miscounted as a second Wi-Fi device.

## Matching

The package matches discovered `Device` nodes that identify a PCI or USB
controller, and the `Driver` nodes attached to them, **except** those owned by
a domain specialist. A device without an unambiguous match remains read-only
and is not assigned a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_device` | 0 | Read device, driver, firmware, and module state |
| `diagnose_fault` | 0 | Compare observations with driver invariants |
| `stage_driver` | 2 | Checkpoint the current module and stage a candidate |
| `request_reset` | 4 | Request a device reset to known-good state |

`stage_driver` uses the action state machine: checkpoint → stage → health
check → commit or rollback. A failed health check always rolls back. A reset
requires a scoped approval and never bypasses the Guardian.

## Invariants

- `DRIVER-001`: the active driver is present, loadable, and attached to the
  discovered device.
- `DEVICE-002`: the device reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its device with `owns`. The device retains
`depends_on` edges to its bus, driver, and firmware when discovery can verify
them. Observed relationships are advisory and are never used as a substitute
for broker capability checks.

## Recovery

v0.1 is module-level only. Checkpoints contain the current driver module and
configuration needed to restore it. The package does not modify the boot chain
or firmware. A rollback failure leaves the action in `Failed` and retains the
checkpoint for manual recovery.