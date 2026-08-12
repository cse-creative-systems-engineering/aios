# Power and Thermal Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The power and thermal specialist owns the power and thermal domain:
temperature sensors, fan state, and power/battery state. It reports thermal
and power health through bounded tools. It does not expose shell execution or
unrestricted file access.

AI is not placed in real-time control loops for voltage, thermal safety, or
similar functions (architecture §5). Aios may diagnose, predict, or recommend;
deterministic controllers enforce hard limits.

## Matching

The package matches discovered `Sensor` nodes from `sys/class/hwmon` (e.g.
`sensor:hwmon0-temp1` for temperature, `sensor:hwmon1-fan1` for fan RPM) and
power/battery resources. A sensor without an unambiguous match remains
read-only and is not assigned a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_thermal` | 0 | Read temperature, fan, and power state |
| `diagnose_fault` | 0 | Compare observations with thermal invariants |

v0.1 is read-only. Bounded workload changes (e.g. throttling a workload,
adjusting a fan curve) are deferred to a later iteration. They will require a
specific operation with a defined risk level, passing through the action
state machine and the Guardian, and will stay within deterministic hard
limits (architecture §5).

## Invariants

- `THERMAL-001`: temperature sensors are present and report within limits.
- `THERMAL-002`: fan/power state reaches the required state after a staged
  change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its sensor nodes with `owns`. Each sensor retains
`depends_on` edges to the device it monitors when discovery can verify them.
Observed relationships are advisory and are never used as a substitute for
broker capability checks.

## Recovery

v0.1 is read-only. Deterministic controllers, not AI, enforce hard thermal
limits. When bounded workload changes are added, checkpoints capture the
state needed to restore a staged change; a rollback failure leaves the action
in `Failed` and retains the checkpoint for manual recovery.