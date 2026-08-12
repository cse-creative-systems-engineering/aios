# Session Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The session specialist owns the session layer of the graphics domain: user
and desktop sessions. It reports session state and lifecycle through bounded
tools. It does not expose shell execution or unrestricted file access.

It is a child of the Graphics specialist (architecture §6 hierarchy). The
display below it is owned by the Display specialist; the GPU is owned by the
GPU specialist.

This specialist owns the *session* (user/desktop session lifecycle). It is
separate from the Aios UI itself (docs/ui.md), which is about how Aios is
present on the screen.

## Matching

The package matches user and desktop session resources. A session without an
unambiguous match remains read-only and is not assigned a privileged
specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_session` | 0 | Read user and desktop session state |
| `diagnose_fault` | 0 | Compare observations with session invariants |

v0.1 is read-only. Mutating operations (session lifecycle changes) are
deferred to a later iteration and will require a specific operation with a
defined risk level, passing through the action state machine and the
Guardian.

## Invariants

- `SESS-001`: the user session is present and reports state.
- `SESS-002`: the session reaches the required state after a staged change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its session with `owns`. The session retains
`depends_on` edges to its display and session service when discovery can
verify them. Observed relationships are advisory and are never used as a
substitute for broker capability checks.

## Recovery

v0.1 is read-only. When mutating operations are added, checkpoints capture
the state needed to restore a staged change; a rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.