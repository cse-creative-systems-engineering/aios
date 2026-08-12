# Display Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The display specialist owns the display layer of the graphics domain:
monitors, the display service, and framebuffer/compositor state. It reports
display and output state through bounded tools. It does not expose shell
execution or unrestricted file access.

It is a child of the Graphics specialist (architecture §6 hierarchy). The GPU
below it is owned by the GPU specialist; the session layer is owned by the
Session specialist.

## Matching

The package matches display and output resources (monitors, the display
service). A resource without an unambiguous match remains read-only and is not
assigned a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_display` | 0 | Read monitor, output, and display-service state |
| `diagnose_fault` | 0 | Compare observations with display invariants |

v0.1 is read-only. Mutating operations (display configuration, output
changes) are deferred to a later iteration and will require a specific
operation with a defined risk level, passing through the action state machine
and the Guardian.

## Invariants

- `DISP-001`: the display is present and reports output state.
- `DISP-002`: the display reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its display with `owns`. The display retains
`depends_on` edges to its GPU and display service when discovery can verify
them. Observed relationships are advisory and are never used as a substitute
for broker capability checks.

## Recovery

v0.1 is read-only. When mutating operations are added, checkpoints capture
the state needed to restore a staged change; a rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.