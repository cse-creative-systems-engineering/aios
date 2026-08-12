# Graphics Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The graphics specialist owns the graphics and session domain. It is the
umbrella parent of the graphics specialists: GPU, Display, and Session. It
coordinates cross-layer concerns — rendering health, display state, and
session state — while each child owns its own resource class.

The graphics children form a stack, like storage: a session runs on a
display, which renders on a GPU. The hierarchy is for organization and
delegation; the dependency graph captures the stack (architecture §6).

The GPU is second only to the CPU in hardware importance, so this is a core
hardware domain, not a low-priority one.

This specialist owns the *hardware* (GPU, display, session). It is separate
from the Aios UI itself (docs/ui.md), which is an interface-layer concern: how
Aios is present on the screen, owns screen space, and sees the screen.

Ownership is per-resource: each GPU, display, or session has exactly one
owning specialist. The hierarchy is not a substitute for the dependency graph.

## Matching

The package matches the graphics domain as a whole. Graphics-specific
resources are owned by the children:
- GPU owns GPUs (`device:gpu0`).
- Display owns displays and the display service.
- Session owns user/desktop sessions.

A resource without an unambiguous match remains read-only and is not assigned
a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_graphics` | 0 | Read GPU, display, and session state |
| `diagnose_fault` | 0 | Compare observations with graphics invariants |

v0.1 is read-only. Mutating operations (display configuration, GPU reset) are
deferred to a later iteration and will require a specific operation with a
defined risk level, passing through the action state machine and the
Guardian.

## Invariants

- `GFX-001`: the GPU/display is present and reports state.
- `GFX-002`: the session reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its children (GPU, display, session) with `owns`
or `controls` edges. Each child retains `depends_on` edges to the layer below
it (session → display → GPU → bus/driver) when discovery can verify them.
Observed relationships are advisory and are never used as a substitute for
broker capability checks.

## Recovery

v0.1 is read-only. When mutating operations are added, checkpoints capture
the state needed to restore a staged change; a rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.