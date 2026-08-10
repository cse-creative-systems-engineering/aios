# Aios Human Interaction Model

**Status:** Draft — frozen for M1  
**Depends on:** capability-model.md, message-protocol.md, action-state-machine.md, security-model.md, decisions/0005-freeze-triage.md

## Purpose

Own the human-in-the-loop story end-to-end: who can approve, what the
channel is, what's exempt, what the UI must show, and what happens when
the user denies or walks away. This doc consolidates the approval,
escalation, facade trust, and rollback authorization concepts that were
previously scattered across five docs.

### Design principles

1. **The facade renders, it does not authorize.** The conversational
   facade presents plans to the user but cannot mint, modify, or relay
   approvals. User approval flows through a dedicated, broker-owned input
   channel.
2. **The user sees the full scope, not a summary.** The UI must show the
   complete plan — every action, every resource, every operation, every
   risk — not a summarized version. The user approves what they see.
3. **Denial and abandonment are first-class outcomes.** User denial and
   approval timeout both transition to `Rejected`. The system does not
   hang waiting for a response that may never come.
4. **Approval is scoped and expiring.** An approval covers a specific
   plan (hash-verified), specific actions, specific resources, and
   specific operations. It expires. It does not grant blanket authority.
5. **The user cannot bypass invariants.** User approval authorizes risk,
   but the Guardian and capability system still enforce fundamental
   safety invariants regardless of approval.

---

## 1. Approval Channel

### 1.1 Architecture

```text
User input (untrusted)
      │
      ▼
  Conversational Facade
  (renders plans, parses intent)
      │
      │  facade CANNOT carry approvals
      │
      ▼
  Broker-owned approval channel
  (dedicated, authenticated, broker-internal)
      │
      ▼
  Policy Broker
  (validates approval against plan hash and scope)
```

### 1.2 Why the facade cannot carry approvals

The facade handles untrusted input: raw user text, model output, and
external data. It is the most exposed component to prompt injection and
manipulation. If the facade could relay approvals, a compromised facade
could:

- Reframe a user's "no" as "yes"
- Modify the plan after the user saw it but before approval reaches the broker
- Mint approvals for plans the user never saw

The dedicated approval channel is separate from the facade's input
channel. In v0.1 (in-process), this is a broker-internal data structure
that only the broker can write to. In v0.2+, it will be a separate IPC
path with user authentication.

### 1.3 What the channel carries

```rust
// The approval channel carries only Approval messages.
// The broker creates the ApprovalRequest; the user responds via
// the dedicated channel; the broker stores the Approval internally.
// Agents never see the approval store and cannot write to it.

// ApprovalRequest (from message-protocol.md §2.11):
//   plan_id, plan_hash, plan_summary, affected_systems,
//   expected_risks, rollback_state, expires_at

// UserResponse (from message-protocol.md §2.12):
//   approval_request_id, decision (Approved/Rejected/Modified),
//   modifications
```

### 1.4 v0.1 implementation

In v0.1, the "dedicated channel" is a broker-internal `HashMap<PlanId,
Approval>` that only the broker can write to. The user's approval is
collected through a terminal prompt that the broker controls directly —
not through the facade's conversational interface. The facade can
*display* the approval request, but the user's yes/no response goes
through the broker's own input reader.

This is a code-level convention in v0.1, not a process boundary. v0.2
moves the approval channel to a separate process with user
authentication.

---

## 2. What the UI Must Show

### 2.1 The full-scope contract

When requesting approval for a risk level 3+ action, the UI must show:

| Element | Required | Why |
|---|---|---|
| Plan hash | Yes | Binds the approval to the exact plan |
| Every action in the plan | Yes | The user must see all operations, not a summary |
| Every resource affected | Yes | The user must know what will be touched |
| Every operation requested | Yes | The user must know what will be done |
| Risk level per action | Yes | The user must understand the danger |
| Rollback state | Yes | The user must know what recovery is available |
| Approval expiration | Yes | The user must know the time limit |
| Guardian verdict | Yes | The user must know if the Guardian flagged concerns |
| Verification Agent verdict | Yes | The user must know if the Verifier had concerns |

### 2.2 What the UI must NOT show

- Model chain-of-thought or reasoning traces
- Secret values
- Internal agent memory or working state
- A summarized or redacted version of the plan that hides actions

### 2.3 The plan_summary problem

The `ApprovalRequest` message has a `plan_summary: String` field. This
is for display convenience, not for binding. The approval is bound to
the `plan_hash`, not to the summary. The UI must show the full plan
details (from §2.1), not just the summary. If the UI shows only the
summary and the user approves, the approval is still valid (it's bound
to the hash), but the user may have approved more than they understood.

**Contract:** The UI implementation must display the full plan scope.
The `plan_summary` is a title, not the content. This is a UI contract,
not a protocol contract — the protocol guarantees hash-binding, but
comprehension is the UI's responsibility.

---

## 3. User Denial and Abandonment

### 3.1 Denial

When the user explicitly rejects an approval request:

```text
UserResponse { decision: Rejected(reason) }
  → Broker records denial
  → Action transitions: GuardianChecked → Rejected
  → Audit log entry: ApprovalDenied
  → User notified: "Action rejected. No changes made."
```

### 3.2 Abandonment (timeout)

When the user does not respond within the approval expiration window:

```text
Approval expires (expires_at < now)
  → Broker records timeout
  → Action transitions: GuardianChecked → Rejected
  → Audit log entry: ApprovalExpired
  → User notified: "Approval timed out. No changes made."
```

### 3.3 Default expiration

| Risk level | Default approval window |
|---|---|
| 3 (Critical mutation) | 10 minutes |
| 4 (Recovery) | 5 minutes |

These are configurable. The expiration is set when the `ApprovalRequest`
is created and cannot be extended. If the user needs more time, a new
approval request must be issued (with a fresh plan hash if the plan
changed).

### 3.4 User unavailability

If the user is unavailable (not at the machine, no response):

- The action is rejected after timeout.
- No action proceeds without approval for risk level 3+.
- The system does not auto-approve under any circumstance.
- Recovery operations (risk level 4) that are time-critical (e.g.,
  thermal shutdown) are handled by deterministic controllers, not by
  the approval flow. The approval flow is for operations that require
  human judgment, not for emergency fail-safe responses.

---

## 4. Approval Scope

### 4.1 What approval covers

An approval covers exactly:

- The specific `plan_id` and `plan_hash`
- The specific actions in `ApprovalScope.actions`
- The specific resources in `ApprovalScope.resources`
- The specific operations in `ApprovalScope.operations`

### 4.2 What approval does NOT cover

- Actions not in the plan
- Resources not listed in the scope
- Operations not listed in the scope
- Any plan with a different hash (even if similar)
- Any future action (approval is one-shot, not standing)

### 4.3 Scope checking

The broker checks every `ToolRequest` against the approval scope:

```text
request.plan_hash == approval.plan_hash?
  No → DENY(PlanHashMismatch)

(request.action_id, request.resource, request.operation, request.tool_id)
  ∈ approval.scope.actions?
  No → DENY(ApprovalScopeExceeded)
```

### 4.4 Modification

If the user responds with `UserResponse { decision: Modified(changes) }`:

- The modification is treated as a rejection of the original plan.
- The Planner must create a new plan incorporating the changes.
- The new plan goes through the full lifecycle again (Proposed → ... →
  Approved).
- The original approval is not carried over.

---

## 5. Escalation

### 5.1 Guardian escalation

The Guardian can return `Escalate` for risk level 2+ actions. Per
ADR-0003 (fail-fast), the broker collapses Guardian `Escalate` to
`Deny(GuardianEscalation)`. There is no automatic escalation path in
v0.1.

If the Guardian escalates, the action is denied. The user is notified
with the Guardian's explanation. If the user wants to proceed, a package
revision or plan modification is needed — the same action cannot be
retried with the same plan.

### 5.2 No Escalate variant in v0.1

The `PolicyVerdict::Escalate` and `GuardianVerdict::Escalate` variants
are removed from the v0.1 type definitions (see capability-model.md and
message-protocol.md). If a real escalation path is designed later, the
variants are re-added then — not speculatively now.

---

## 6. Rollback Authorization

### 6.1 Automatic rollback (no approval needed)

Rollback is automatic when:
- Health check fails after staging
- Commit fails after health verification
- Crash recovery detects an interrupted `Staged` action

Automatic rollback does not require user approval. It is a safety
mechanism, not a mutation. The user is notified after rollback completes.

### 6.2 Manual recovery (requires user action)

Manual recovery is required when:
- Rollback fails (checkpoint missing or corrupted)
- Action is in `Failed` state
- System is in an unknown state after a crash

Manual recovery is initiated by the user through a risk level 4
operation. It goes through the full approval flow — the user must
approve the recovery action just like any other risk level 4 operation.

### 6.3 Recovery when user is unavailable

If the system is in `Failed` state and the user is unavailable:

- The system does not auto-recover.
- The `Failed` action is retained with its checkpoint for manual
  recovery.
- The System State panel shows the failure.
- Deterministic safety controllers (thermal, power) continue to operate
  independently.

---

## 7. User Principal

### 7.1 Authority

The user principal has `Clearance(Recovery) = 4` and all capabilities.
This means the user can request any operation on any resource. However:

- The user's *input* (text, messages) is untrusted — prompt injection
  defense applies.
- The user's *approvals* are authenticated via the dedicated approval
  channel.
- The user cannot bypass invariants or the capability system.
- The user cannot bypass the Guardian.

### 7.2 The tension

The user has "full authority" (all capabilities, max clearance) while
the facade is an untrusted boundary. This is reconciled by:

1. The user's *input* flows through the facade (untrusted).
2. The user's *approvals* flow through the dedicated channel (trusted).
3. The facade may only produce proposals, not action plans.
4. User approval is bound to a plan hash, not to the facade's rendering.

The user has authority to approve, but the system enforces safety
invariants regardless of approval.

---

## 8. References

- `docs/capability-model.md` — §5.2 step 5 (approval scope checking),
  §1.1 (user principal)
- `docs/message-protocol.md` — §2.7 (Approval), §2.11 (ApprovalRequest),
  §2.12 (UserResponse)
- `docs/action-state-machine.md` — §3.1 (GuardianChecked → Rejected for
  denial/timeout), §6.1 (crash recovery for Approved)
- `docs/security-model.md` — §3.2 (facade intent reframing), §1.3
  (facade as non-TCB)
- `docs/decisions/0005-freeze-triage.md` — P0-3 (facade trust channel
  decision)
- `docs/requirements.md` — REQ-SAF-004 (approval doesn't bypass
  invariants), REQ-UX-001 (scoped approvals)
