# Aios Requirements

**Status:** Draft  
**Depends on:** architecture.md, glossary.md, decisions/0001-v01-runs-above-linux.md

This document defines what Aios v0.1 must do, must not do, and what qualities
it must have. Requirements are traceable to architecture principles. Each
requirement has a unique ID for reference in design docs, tests, and ADRs.

Requirement IDs use the format `REQ-<category>-<number>`.

Categories:

| Code | Category |
|---|---|
| SAF | Safety and security |
| FUNC | Functional |
| PERF | Performance |
| REL | Reliability and recovery |
| OBS | Observability |
| UX | User experience |
| COMP | Compatibility |

---

## Safety and Security

### REQ-SAF-001
**No agent may both decide and execute.** Every mutating action must pass
through the Policy Broker before execution. No agent has direct, unrestricted
OS authority.

**Source:** Architecture section 1 (core idea), section 5 (specialists as
tools).

### REQ-SAF-002
**Fail-closed by default.** If the Policy Broker cannot determine whether an
action is permitted, the action is denied. Missing capabilities, stale
telemetry, or ambiguous graph state result in denial, not allowance.

**Source:** Architecture section 13 (risks), glossary (fail-closed).

### REQ-SAF-003
**The Infrastructure Guardian must be able to block critical actions.** The
block is enforced by the Policy Broker, not by the Guardian alone. A Guardian
denial cannot be bypassed by any agent through the normal interface.

**Source:** Architecture section 7 (Infrastructure Guardian).

### REQ-SAF-004
**User approval does not bypass safety invariants.** Approval is scoped to a
specific plan, expires, and does not override fundamental invariants or the
capability system.

**Source:** Architecture section 9 (critical action lifecycle).

### REQ-SAF-005
**External data is untrusted.** All input from files, devices, web content, or
user messages is treated as potentially adversarial. Prompt injection must not
elevate authority. Context never grants capability.

**Source:** Architecture section 13 (risks), section 5 (specialists as tools).

### REQ-SAF-006
**Secrets never leave the local trust boundary.** Credentials, tokens, and
cryptographic material are never sent to external model providers. The Policy
Broker enforces this regardless of general private-memory consent.

**Source:** Architecture section 11 (setup and data-sharing consent).

### REQ-SAF-007
**The system must lose intelligence before it loses the ability to recover.**
If the Agent Plane, Message Bus, or model providers fail, the Trust Plane and
recovery mechanisms must remain functional.

**Source:** Architecture section 3 (trust plane).

---

## Functional

### REQ-FUNC-001
**Single conversational interface.** The user interacts with Aios through one
conversational facade. Internally, the system coordinates multiple agents, but
the user sees one coherent response.

**Source:** Architecture section 1 (core idea).

### REQ-FUNC-002
**Dual-agent bridge.** A Planner Agent produces action plans. A Verification
Agent independently reviews them. Both are advisory until accepted by the
Enforcement Plane.

**Source:** Architecture section 4 (dual-agent bridge).

### REQ-FUNC-003
**Typed tool interfaces.** Specialists expose bounded operations (e.g.,
`observe_device`, `diagnose_fault`, `stage_change`). No specialist exposes
unrestricted operations like `run_any_command()` or
`write_any_memory_address()`.

**Source:** Architecture section 5 (specialists as tools).

### REQ-FUNC-004
**Staged, reversible actions.** Critical changes are checkpointed, staged,
health-checked, and either committed or rolled back automatically.

**Source:** Architecture section 9 (critical action lifecycle).

### REQ-FUNC-005
**Live System Graph.** Aios maintains a typed graph of hardware, OS resources,
agents, capabilities, and recovery paths. The graph is used for impact
analysis, routing, and health — not as the authority for permissions.

**Source:** Architecture section 6 (System Graph).

### REQ-FUNC-006
**Agent Package instantiation.** Runtime agents are created from versioned,
signed Agent Packages. Discovery maps graph nodes to packages. Unknown hardware
receives read-only inspection or quarantine — never an invented privileged
agent.

**Source:** Architecture section 6 (Agent Packages).

### REQ-FUNC-007
**Model routing based on connectivity state.** Aios selects model providers
deterministically from configured tiers: local (offline) → LAN gateway →
internet provider. Selection considers health, data policy, and task type.

**Source:** Architecture section 11 (model routing).

### REQ-FUNC-008
**Hardware discovery.** Aios discovers hardware through deterministic Linux
interfaces (udev, sysfs, procfs). Discovery does not require AI and builds the
initial System Graph.

**Source:** ADR-0001, architecture section 6 (discovery).

### REQ-FUNC-009
**System State panel.** Aios exposes a dashboard showing overall health,
subsystem status, active operations, model connectivity, and recovery state.
Missing or stale telemetry appears as `UNKNOWN` or `STALE`, never silently as
healthy.

**Source:** Architecture section 6 (System State panel).

---

## Performance

### REQ-PERF-001
**Agent operations have deadlines.** Every tool request and action plan
carries a deadline. Operations that exceed their deadline are cancelled or
escalated.

**Source:** Architecture section 10 (message routing).

### REQ-PERF-002
**Resource budgets.** Each agent instance has CPU, memory, storage, latency,
and power budgets defined in its Agent Package. In v0.1, budgets are
advisory (logged but not enforced — in-process, no isolation). In v0.2+,
the system enforces these budgets via process-level resource limits.

**Source:** Architecture section 6 (Agent Packages).

### REQ-PERF-003
**No LLM in real-time control loops.** Language models must not be placed in
memory protection, DMA, interrupt handling, voltage, or thermal safety loops.
Deterministic controllers enforce hard limits.

**Source:** Architecture section 5 (hardware depth).

---

## Reliability and Recovery

### REQ-REL-001
**Automatic rollback on health failure.** If a staged change fails health
verification, the system automatically reverts to the previous checkpoint.

**Source:** Architecture section 9 (critical action lifecycle).

### REQ-REL-002
**Recovery independent of AI.** The recovery path must function when no
language model is available. Aios must retain a deterministic recovery path
even if all model providers are down.

**Source:** Architecture section 11 (model routing).

### REQ-REL-003
**Message bus failure degrades, not breaks.** If the message bus fails, Aios
becomes less coordinated but not unsafe. Critical controls remain independent
of the bus.

**Source:** Architecture section 10 (message routing), section 13 (risks).

### REQ-REL-004
**Durable action state.** Action state survives process crashes and, where
possible, power loss. Partially executed actions can be detected and recovered
on restart.

**Source:** Architecture section 15 (gaps: action state and transactions).

---

## Observability

### REQ-OBS-001
**Audit log.** Aios records intent, plans, evidence, approvals, tool calls,
policy decisions, results, and rollback events in an append-only audit log.

**Source:** Architecture section 5 (enforcement plane).

### REQ-OBS-002
**No secrets or chain-of-thought in logs.** The audit log must not contain
credentials, tokens, cryptographic material, or model chain-of-thought.

**Source:** Architecture section 15 (gaps: observability).

### REQ-OBS-003
**Trace propagation.** Every action, tool request, and policy decision carries
a correlation ID that links related events across the audit log.

**Source:** Architecture section 10 (message routing).

---

## User Experience

### REQ-UX-001
**Scoped approvals.** When user approval is required, Aios presents the
affected systems, required permissions, expected risks, rollback state, and
expiration. Approval is for a specific plan, not blanket authority.

**Source:** Architecture section 9 (critical action lifecycle).

### REQ-UX-002
**Explainable decisions.** When an action is blocked or escalated, Aios
explains which rule caused the decision and what is required to proceed.

**Source:** Architecture section 7 (Infrastructure Guardian).

### REQ-UX-003
**Meaningful health display.** The System State panel prioritizes meaning over
raw metric volume. Health values carry source, timestamp, freshness, and
confidence.

**Source:** Architecture section 6 (System State panel).

---

## Compatibility

### REQ-COMP-001
**Runs on standard Linux.** Aios v0.1 runs on a standard Linux distribution
without kernel modifications, custom bootloaders, or firmware changes.

**Source:** ADR-0001.

### REQ-COMP-002
**OS-agnostic specifications.** The capability model, message protocol, action
state machine, and System Graph specifications are designed to be portable
across future kernel choices. Implementation details may be Linux-specific, but
the contracts should not be.

**Source:** ADR-0001 (consequences).

### REQ-COMP-003
**Signed Agent Packages.** All Agent Packages are signed and versioned.
Package updates do not silently broaden existing agent capabilities.

**Source:** Architecture section 6 (Agent Packages).

---

## v0.1 Scope Boundaries

The following are **explicitly out of scope** for v0.1:

- Kernel modification or custom kernel development
- Boot chain or firmware modification
- True hardware-level capability isolation (relies on Linux OS-level isolation)
- Custom microkernel or hypervisor
- Real-time hardware control loops
- Multi-user identity and session management (single-user for v0.1)
- Full specialist coverage (start with Wi-Fi diagnosis as the first vertical
  slice)
- Production-grade secret store (use OS keyring for v0.1)

These are not permanent exclusions — they are deferred to later versions per
the implementation roadmap.
