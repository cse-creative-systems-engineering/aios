# ADR-0005: Freeze triage — decided, undeveloped

**Status:** Accepted  
**Date:** 2026-08-09  

## Context

Before freezing the doc set for M1, four open P0/P1 issues needed
one-line triage. These are decisions, not discoveries — M1 will test
them, but the fork must be picked now so M1 doesn't start with
contradictions.

## Decisions

### P0-2: Commit-approval state

**Decision:** The `Approved` state is non-terminal. User denial and
approval timeout both transition to `Rejected` (already in the table).
The broker holds approval state internally — there is no separate
"pending approval" state in the action state machine. The action sits
in `GuardianChecked` until approval arrives or expires.

### P0-3: Facade trust channel

**Decision:** The facade renders plans for the user but does NOT carry
approvals. User approval flows through a dedicated, broker-owned input
channel that the broker reads directly. The facade cannot mint, modify,
or relay approvals. This is a threat-model judgment: the facade handles
untrusted input (user text, model output) and cannot be trusted with
authority. The approval channel is separate, authenticated, and
broker-internal.

### P0-4: Capability token

**Decision:** v0.1 uses Rust type safety — tokens are broker-owned
opaque handles, not reconstructible structs. No cryptographic signature.
Agents receive a `BrokerClient` handle, not token bytes. v0.2 adds
cryptographic signatures for IPC. This is decided, not pending.

### P1-5: Resource-state authority

**Decision:** The broker maintains its own resource-state registry,
updated only by signed events from trusted specialists (the owning
specialist for each resource). The System Graph is advisory. The broker
does not trust graph state for permission decisions. If the broker's
registry has no state for a resource, it treats the resource as
`Unknown` and denies (fail-closed).

## What M1 tests

M1 will test whether these decisions are implementable and whether they
hold up under simulation. If M1 finds a decision is wrong, the decision
is revised — but M1 starts with these as the assumed truth.

## Related

- `docs/human-interaction.md` — details the approval channel (P0-3)
- `docs/capability-model.md` — token model (P0-4), resource state (P1-5)
- `docs/action-state-machine.md` — approval state (P0-2)
