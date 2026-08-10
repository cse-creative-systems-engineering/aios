# Aios Security Model

**Status:** Draft — frozen for M1  
**Depends on:** architecture.md, glossary.md, requirements.md, decisions/0001-v01-runs-above-linux.md, decisions/0002-rust-as-implementation-language.md, decisions/0003-fail-fast-no-silent-fallbacks.md

## Purpose

Define the threat model, trust boundaries, and trusted computing base (TCB)
for Aios v0.1. State what is trusted, what is replaceable, what is
recoverable, and what happens when each component is compromised or
unavailable.

This document focuses on the **AI-safety threat** — the risk that
probabilistic agents make unsafe decisions or are manipulated into bypassing
safety controls. External-attacker hardening (network intrusion, privilege
escalation by an external adversary, side-channel attacks) is a separate
concern that will be addressed in a future security workstream. For v0.1, the
Linux OS provides the external-attacker boundary; Aios provides the
AI-safety boundary.

---

## 1. Trusted Computing Base

The TCB is the minimal set of components that must be correct and uncompromised
for Aios's safety properties to hold. If any TCB component fails, the safety
model cannot be guaranteed.

### 1.1 TCB for v0.1

```text
┌─────────────────────────────────────────────────────────┐
│                    Aios Process                          │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │              TCB (v0.1)                          │   │
│  │                                                 │   │
│  │  ┌───────────┐  ┌──────────┐  ┌─────────────┐ │   │
│  │  │  Policy   │  │ Guardian │  │  Staged     │ │   │
│  │  │  Broker   │  │ (read-   │  │  Executor   │ │   │
│  │  │           │  │  only)   │  │  + Rollback │ │   │
│  │  └───────────┘  └──────────┘  └─────────────┘ │   │
│  │                                                 │   │
│  │  ┌─────────────────────────────────────────┐   │   │
│  │  │  Capability Token Verification           │   │   │
│  │  └─────────────────────────────────────────┘   │   │
│  │                                                 │   │
│  │  ┌─────────────────────────────────────────┐   │   │
  │  │  Audit Log (append-only, hash-chained)   │   │   │
  │  └─────────────────────────────────────────┘   │   │
  │                                                 │   │
  │  ┌─────────────────────────────────────────┐   │   │
  │  │  Agent Package Loader + Signature Verifier│   │   │
│  │  └─────────────────────────────────────────┘   │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Non-TCB (agents, specialists, graph, models)    │   │
│  │  These may fail or be compromised without        │   │
│  │  breaking the safety model.                     │   │
│  └─────────────────────────────────────────────────┘   │
│                                                         │
└─────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────┐
│  Linux Kernel + OS (external TCB)                       │
│  Provides process isolation, filesystem, device access  │
└─────────────────────────────────────────────────────────┘
```

### 1.2 TCB components

| Component | Role in TCB | Why it is trusted | What happens if compromised |
|---|---|---|---|
| **Policy Broker** | Sole authority for capability validation and action gating | Deterministic Rust code, no probabilistic logic, small enough to audit completely | Safety model is broken. All safety properties depend on the broker. |
| **Infrastructure Guardian** | Read-only inspection and veto of critical actions | Deterministic rule checks against the Operational Contract. No write capability. The broker is **required** to call the Guardian for risk level 2+ and **required** to honor a denial. Skipping the Guardian call is TCB failure. In v0.1, the Guardian is a code module within the broker process, not a separate process boundary. It becomes an independent TCB component at v0.2 (process isolation). | Unsafe actions may not be blocked. The broker still gates execution, but critical-system vetoes are missing. |
| **Staged Transaction Executor** | Checkpoint, stage, health-verify, commit or rollback | Deterministic state machine with durable checkpoints. The health-verify step is a **deterministic predicate** — the executor must honor the result (fail → rollback). The executor does not decide whether to commit; it commits if and only if the health predicate passes. The commit decision is mechanical, not discretionary. | Changes may be applied without rollback capability. Partial failures could leave the system in an inconsistent state. |
| **Capability Token Verification** | Verification that a capability is valid, unexpired, and unrevoked | v0.1: Rust type safety — tokens are broker-owned opaque handles, not reconstructible structs. Agents receive a `BrokerClient` handle, not the token bytes. The broker maintains the revocation set. v0.2: cryptographic signature verification for IPC. | Agents could present forged or expired capabilities. The broker would accept invalid authority. |
| **Audit Log** | Append-only record of all decisions, approvals, and actions | Append-only storage with hash-chained integrity | Actions could be taken without traceability. Repudiation becomes possible. |
| **Agent Package Loader + Signature Verifier** | Verifies package signatures before instantiation | Deterministic signature verification, no network dependency | Unsigned or modified packages could load with elevated capabilities. |

### 1.3 Non-TCB components

These components may fail, produce incorrect output, or be compromised without
breaking the safety model. The TCB must handle their failure gracefully:

| Component | Failure mode | TCB response |
|---|---|---|
| **Planner Agent** | Produces unsafe or hallucinated plan | Verification Agent reviews; broker validates capabilities; Guardian checks invariants |
| **Verification Agent** | Fails to catch a bad plan | Guardian checks invariants; broker enforces capabilities; staged execution provides rollback |
| **Subsystem Specialists** | Returns incorrect diagnosis or faulty tool result | Broker validates the tool request against capabilities; staged execution tests before commit |
| **System Graph** | Stale, poisoned, or incomplete | Treated as advisory. Broker does not trust graph state for permission decisions. The broker maintains its own resource state registry, updated by signed events from trusted specialists. Missing data → fail-closed. |
| **Model Gateway / Providers** | Unavailable, compromised, or returns manipulated output | Agents lose reasoning capability. Broker and Guardian remain deterministic. Recovery path does not require models. |
| **Message Bus** | Fails or is compromised | System becomes less coordinated. TCB components must remain functional without the bus (see section 6.3). |
| **Conversational Facade** | Compromised input channel or intent reframing | All input is untrusted. The facade may only produce *proposals* — it may not produce or modify action plans. User approval (for level 3+) is bound to a plan hash produced by the Planner, not to the facade's rendering of intent. The facade's rendering is display-only. |

### 1.4 TCB evolution

| Version | TCB scope | Key addition |
|---|---|---|
| v0.1 | In-process Rust modules | Type-level API isolation; broker owns all tool handles |
| v0.2 | Broker + Guardian in separate process | OS-level process isolation; broker is sole privilege holder |
| v0.3+ | Broker in TEE or separate execution domain | Hardware-assisted isolation; survives kernel compromise |

The v0.1 TCB relies on Rust's type system for isolation. The `BrokerClient`
interface that agents use must be designed as if it were already an IPC
boundary, so that the v0.2 split is a transport change, not a redesign.

---

## 2. Trust Boundaries

### 2.1 Boundary diagram

```text
                 ┌──────────────────────────────────┐
                 │          External World           │
                 │  (internet, model providers,      │
                 │   external data, files, devices)  │
                 └───────────────┬──────────────────┘
                                 │
                    Boundary A: Untrusted input
                    All external data is adversarial.
                    Prompt injection defense here.
                                 │
                                 ▼
┌────────────────────────────────────────────────────────┐
│                    Agent Plane                          │
│                                                        │
│  Conversational Facade                                 │
│  Planner Agent    Verification Agent                   │
│  Subsystem Specialists                                 │
│  System Graph (advisory)                               │
│  Model Gateway                                         │
│                                                        │
└───────────────────────┬────────────────────────────────┘
                        │
           Boundary B: Advisory → Authoritative
           Agents propose. Broker decides.
           No agent crosses this boundary
           with execution authority.
                        │
                        ▼
┌────────────────────────────────────────────────────────┐
│                 Enforcement Plane (TCB)                 │
│                                                        │
│  Policy Broker    Infrastructure Guardian              │
│  Capability Verification                               │
│  Staged Transaction Executor                           │
│  Audit Log                                             │
│                                                        │
└───────────────────────┬────────────────────────────────┘
                        │
           Boundary C: Validated → Executed
           Only broker-approved actions with
           valid capabilities cross this boundary.
           Staged execution provides rollback.
                        │
                        ▼
┌────────────────────────────────────────────────────────┐
│              OS / Hardware (Linux)                      │
│                                                        │
│  Kernel, drivers, filesystems, devices                 │
│                                                        │
└───────────────────────┬────────────────────────────────┘
                        │
           Boundary D: OS → Trust Plane
           Recovery and integrity verification.
           Must survive OS or agent failure.
                        │
                        ▼
┌────────────────────────────────────────────────────────┐
│                   Trust Plane                           │
│                                                        │
│  Known-good images, watchdogs, recovery supervisor     │
│  (v0.1: Linux-level recovery; full trust plane later)  │
│                                                        │
└────────────────────────────────────────────────────────┘
```

### 2.2 What crosses each boundary

| Boundary | What crosses | Direction | Validation |
|---|---|---|---|
| **A: External → Agent** | User messages, file contents, device data, web content, model responses | Inbound | All treated as untrusted. No external data may carry authority. Data classification labels applied at ingestion. |
| **B: Agent → Enforcement** | Action plans, tool requests, verification reports | Inbound | Broker validates capabilities. Guardian checks invariants. No agent output is trusted as proof of safety. |
| **C: Enforcement → OS** | Typed tool operations (stage, commit, reset) | Outbound | Only broker-approved actions with valid capability tokens. Staged execution with checkpoint and rollback. |
| **D: OS → Trust** | Health state, boot status, recovery signals | Bidirectional | Trust plane verifies OS integrity. Recovery path independent of agent plane. |

### 2.3 Data classification at boundaries

| Data class | Boundary A (inbound) | Boundary B (agent→broker) | Boundary C (broker→OS) | External model routing |
|---|---|---|---|---|
| **Public** | Labeled, passed through | Passed in tool requests | Normal execution | Any approved provider |
| **Personal memory** | Labeled at ingestion | Passed with data label | Normal execution | Local or trusted gateway only |
| **System config** | Labeled | Passed with data label | Normal execution | Local by default |
| **Credentials/keys** | Never accepted as agent input | Blocked by broker | Never sent to OS tools | Never sent to any model |
| **Kernel/security state** | Labeled | Passed with data label | Normal execution | Local or tightly trusted gateway |

---

## 3. Threat Model

### 3.1 Threat prioritization

For v0.1, the AI-safety threat is primary. External-attacker threats are
secondary and rely on Linux OS-level protections.

| Priority | Threat category | Addressed in v0.1? |
|---|---|---|
| **P0** | Agent makes unsafe decision (hallucination, bad reasoning) | Yes — dual-agent review, Guardian, broker, staged execution |
| **P0** | Prompt injection from external data | Yes — all external data untrusted, authority separate from context |
| **P0** | Capability escalation (agent attempts unauthorized action) | Yes — broker enforces, fail-closed |
| **P1** | Agent compromise (memory corruption, code injection) | Partial — type-level isolation in v0.1, process isolation in v0.2 |
| **P1** | Model provider compromise (data leakage, manipulated output) | Yes — data classification, consent, local fallback |
| **P1** | Graph state poisoning (false telemetry, stale data) | Yes — graph is advisory, fail-closed on missing data |
| **P2** | Message bus failure or compromise | Partial — fail-fast in dev, designed fallback in production |
| **P2** | External network attacker | No — deferred to future security workstream |
| **P2** | Side-channel, physical access | No — deferred to future security workstream |

### 3.2 STRIDE analysis

#### Spoofing

| Threat | Vector | Defense |
|---|---|---|
| Agent impersonation | A component claims to be the Planner or a Specialist | Agent instances are created from signed Agent Packages with unique instance IDs. The broker verifies identity on every tool request. |
| Message origin spoofing | A message claims to be from a different agent | Messages carry authenticated origin (v0.1: in-process identity; v0.2: IPC authentication). Broker rejects messages from unknown or unauthorized origins. |
| Model gateway spoofing | A rogue endpoint claims to be an approved LAN gateway | Gateways require explicit pairing. Discovery does not establish trust. Gateway identity verified on every request. |
| User impersonation | A process claims to act on behalf of the user | v0.1 is single-user. The conversational facade is the only input channel. User approval is authenticated via a dedicated user-input channel that the broker reads directly (not via the message bus). Agents cannot mint approvals — the approval store is broker-internal. Future: user authentication and session tokens. |
| Facade intent reframing | The facade misinterprets or reframes user intent to elevate authority | The facade may only produce proposals, not action plans. User approval is bound to the Planner's plan hash, not the facade's rendering. The broker validates the plan hash regardless of facade output. |

#### Tampering

| Threat | Vector | Defense |
|---|---|---|
| Action plan tampering | A plan is modified between Planner and broker | Plans are structured messages with integrity. Broker re-validates capabilities regardless of plan content. |
| Tool result tampering | A specialist returns falsified results | Results carry provenance. Staged execution verifies health independently before commit. |
| Graph state poisoning | False telemetry or events corrupt the graph | Graph is advisory. Declared vs observed vs attested edges are distinguished. Fail-closed on conflicting or stale data. |
| Audit log tampering | Log entries are modified or deleted | Append-only storage with hash chaining (SHA-256, forward-chained). Each entry includes the hash of the previous entry. Tampering is detectable. Residual risk: a compromised broker can *omit* entries even if it cannot forge past ones. Omission detection (sequence number gaps) is a v0.2 concern. |
| Agent Package tampering | A package is modified after signing | Packages are signed. Signature verified at load time. Unsigned or modified packages are rejected. |

#### Repudiation

| Threat | Vector | Defense |
|---|---|---|
| Action repudiation | An agent denies making a request | Every tool request carries origin, correlation ID, and is logged in the audit log. |
| Approval repudiation | A user denies approving an action | Approvals are scoped, timestamped, and recorded in the audit log with the specific plan hash. |
| Decision repudiation | The broker denies making a decision | Every policy decision is logged with principal, resource, operation, decision, and reason. |

#### Information disclosure

| Threat | Vector | Defense |
|---|---|---|
| Secret leakage to models | Credentials or keys are included in model prompts | Secrets are a separate data class. Broker blocks secrets from model requests regardless of general consent. Redaction rules applied to all outbound data. |
| Secret leakage in logs | Credentials appear in audit log or telemetry | Redaction rules in the logging layer. Secrets never recorded. Data classification labels on log entries. |
| Private memory disclosure | Personal data sent to unapproved provider | Data classification + consent records. Provider must match consent scope. Task pinning prevents mid-task provider switching. |
| System state disclosure | Kernel or security state exposed to external model | Classified as protected data. Local or tightly trusted gateway only. |

#### Denial of service

| Threat | Vector | Defense |
|---|---|---|
| Message bus failure | Bus becomes unavailable | TCB components must function without the bus (v0.1: in-process; v0.2: direct IPC to broker). System degrades to less coordinated, not unsafe. |
| Model provider failure | No model available | Agents lose reasoning. Broker and Guardian remain deterministic. Recovery path does not require models. |
| Specialist crash | A specialist process fails | Broker detects failure (timeout, channel close). Action fails fast. No silent fallback. |
| Resource exhaustion | Agent consumes excessive CPU/memory | Resource budgets in Agent Packages. Broker enforces deadlines. Runaway agents are terminated. |
| Audit log exhaustion | Log storage fills | Fail-fast: system stops if audit log cannot write. No action proceeds without audit. |

#### Elevation of privilege

| Threat | Vector | Defense |
|---|---|---|
| Capability escalation | Agent attempts an operation beyond its capabilities | Broker validates every request. Fail-closed on missing or ambiguous capability. |
| Package capability expansion | A package update silently broadens authority | Package updates do not expand existing capabilities. Capability changes require explicit review and ADR. |
| Context-as-authority | Agent uses system context (device info, state) to imply permission | Context never grants capability. Authority comes only from the broker. |
| Guardian bypass | Agent attempts to execute without Guardian review | Broker requires Guardian sign-off for critical actions. Guardian is in the TCB. No path around it. |
| Broker compromise | The broker itself is corrupted | This is TCB failure. v0.1: mitigated by small, auditable Rust code. v0.2: process isolation. v0.3+: TEE. |

---

## 4. Compromise Scenarios

Each scenario describes what an adversary can achieve, what stops them, and
what the system does.

### 4.1 Agent compromised

**Scenario:** An agent instance (Planner, Specialist, etc.) is fully
compromised — it produces malicious output or attempts unauthorized actions.

**What the adversary can do:**
- Produce malicious action plans or tool requests
- Attempt to call tools it does not have capabilities for
- Include prompt injection payloads in its output
- Send messages to other agents

**What stops them:**
- The broker validates every tool request against capabilities. No capability,
  no execution.
- The Guardian checks every critical action against invariants.
- The Verification Agent independently reviews plans (if the compromised agent
  is the Planner).
- Staged execution tests changes before commit.
- The audit log records every attempt, including denied ones.

**What the system does:**
- Unauthorized requests are denied and logged.
- If the agent's behavior is detected as anomalous (repeated denials,
  malformed messages), it can be quarantined.
- The system continues operating. One compromised agent does not compromise
  the safety model.

### 4.2 Policy broker compromised

**Scenario:** The broker's code or state is corrupted.

**What the adversary can do:**
- Approve unauthorized capabilities
- Bypass Guardian vetoes
- Skip staged execution
- Suppress audit log entries

**What stops them:**
- v0.1: Nothing within Aios. This is TCB failure.
- v0.2: Process isolation makes compromise harder (separate address space,
  minimal attack surface).
- v0.3+: TEE or hardware isolation.

**What the system does:**
- v0.1: The safety model is broken. This is accepted risk for a prototype.
  Mitigation: the broker is small, deterministic, and fully audited. No
  `unsafe` code. No external dependencies in the broker's decision path.
- The broker is the most hardened component. It has the smallest code surface,
  the fewest dependencies, and the most tests.

### 4.3 Message bus compromised

**Scenario:** The message bus (in-process channel for v0.1, Redis/IPC later)
is compromised — messages are injected, replayed, or dropped.

**What the adversary can do:**
- Inject fake messages between agents
- Replay old messages
- Drop messages to disrupt coordination

**What stops them:**
- v0.1: In-process channels. No external bus to compromise. Messages are
  typed Rust values, not serialized data on a wire.
- v0.2+: Messages carry authentication, correlation IDs, and deadlines.
  Replay is detected via correlation ID and timestamp. Injection is detected
  via origin verification.
- The broker does not trust message content for safety decisions. Even a
  forged tool request must pass capability validation.

**What the system does:**
- Detected anomalies (replay, unknown origin) cause fail-fast rejection.
- The bus is transport, not authority. Compromising the bus degrades
  coordination but does not grant execution authority.

### 4.4 Model provider compromised

**Scenario:** An external model provider returns manipulated output, leaks
data, or becomes unavailable.

**What the adversary can do:**
- Feed manipulated plans or reasoning to agents
- Retain or leak data sent to the provider
- Return subtly wrong diagnoses

**What stops them:**
- Data classification prevents sensitive data from reaching unapproved
  providers.
- The Verification Agent provides independent review (especially important
  when using the same provider for both roles).
- The broker and Guardian do not trust model output as proof of safety.
- Staged execution tests changes before commit.
- Local model fallback ensures the system can operate offline.

**What the system does:**
- If a provider is detected as compromised or unreliable, it is marked
  unhealthy and removed from the routing pool.
- Tasks pinned to the compromised provider fail and are retried on a
  fallback provider (within configured policy).
- No data is re-sent to a provider that has been marked compromised.

### 4.5 Graph state poisoned

**Scenario:** False telemetry, stale events, or malicious reports corrupt the
System Graph.

**What the adversary can do:**
- Make the graph show incorrect dependencies, ownership, or health
- Cause agents to route requests to the wrong specialist
- Hide a failing component as healthy

**What stops them:**
- The graph is advisory, not authoritative. The broker does not use graph
  state for permission decisions.
- Declared, attested, and observed edges are distinguished. Observed edges
  have provenance and freshness.
- Stale or conflicting data is surfaced as `STALE` or `UNKNOWN`, not
  silently treated as healthy.
- Fail-closed: if the graph cannot provide reliable information about a
  resource, actions affecting that resource are denied.

**What the system does:**
- Graph conflicts trigger reconciliation.
- Unreliable graph regions are marked and not used for routing.
- The system becomes less efficient (poorer routing) but not less safe.

### 4.6 Linux kernel compromised

**Scenario:** The Linux kernel is compromised.

**What the adversary can do:**
- Bypass all process-level isolation
- Access all memory and devices
- Forge any OS-level credential

**What stops them:**
- v0.1: Nothing. The kernel is the external TCB. If it falls, Aios falls.
- Future: the trust plane (separate boot verification, recovery supervisor,
  hardware-assisted isolation) would provide a recovery path independent of
  the kernel.

**What the system does:**
- v0.1: Accepted risk. Aios v0.1 does not defend against kernel compromise.
  This is documented in ADR-0001.
- The architecture is designed so that future versions can move trust-critical
  components below the kernel or into a TEE.

---

## 5. Secrets Management

### 5.1 Secret store

| Version | Mechanism |
|---|---|
| v0.1 | Linux keyring (`keyctl` or `secret-service` via D-Bus) |
| v0.2+ | Dedicated secret store with access controlled by the broker |

Secrets are never stored in plaintext, in configuration files, or in agent
memory beyond the minimum necessary.

**v0.1 residual risk:** In v0.1, secret-store access isolation is a code-level
convention, not a process boundary. A compromised in-process component can
call the Linux keyring directly, bypassing the broker. This is accepted v0.1
risk. Mitigation: all keyring access goes through a single Rust module
(`SecretStore` handle) whose API the type system makes hard to bypass (the
handle is not `Clone` and is only held by the broker). v0.2 moves the secret
store behind the broker's process boundary.

### 5.2 Secret access rules

- Only the broker and explicitly authorized components may access the secret
  store.
- Agents do not have direct access to secrets. If an agent needs a credential
  (e.g., a Wi-Fi password), it requests it through a typed tool call, and the
  broker retrieves and injects it into the operation without exposing it to
  the agent's prompt.
- Secrets are never included in model prompts, tool request payloads visible
  to agents, audit logs, or telemetry.

### 5.3 Redaction rules

| Data type | Redaction rule |
|---|---|
| Credentials, tokens, keys | Replaced with `[REDACTED:secret]` in all logs and traces |
| API keys in model requests | Injected by the gateway, not visible to agents |
| Passwords in tool results | Never returned to agents. Broker handles credential injection directly. |
| Private memory in logs | Labeled with data classification. Personal memory not logged at `INFO` level. |

### 5.4 Provenance labels

Data that has touched secrets carries a provenance label so downstream
components know not to forward it:

```text
DataProvenance {
    classification: Secret,        // this is a provenance label, not a DataClassification variant
    source: "keyring:wifi-credentials",
    touched_secret: true,
    forwardable: false,
}
```

### 5.5 Hard boundary

> **Secrets never leave the local trust boundary.**

This is enforced by the broker regardless of any general private-memory
consent. Even if the user has consented to sending personal memory to an
external provider, credentials, tokens, and cryptographic material are
blocked. This is a P0 safety property (REQ-SAF-006).

---

## 6. Recovery Security

### 6.1 Recovery principles

1. **Recovery must not require AI.** If all model providers are down, the
   recovery path must still function deterministically.
2. **Recovery must not require the message bus.** If the bus fails, the
   trust plane and recovery mechanisms must remain operational.
3. **Recovery must not require the agent plane.** If all agents crash, the
   system can still roll back to a known-good state.
4. **Aios should lose intelligence before it loses the ability to recover
   safely.** (REQ-SAF-007)

### 6.2 v0.1 recovery mechanisms

| Mechanism | What it protects | How it works |
|---|---|---|
| **Checkpoint rollback** | System state after staged changes | Staged executor creates a checkpoint before staging. If health verification fails, the checkpoint is restored. |
| **Action state persistence** | In-flight actions | Action state is persisted so partially executed actions can be detected and recovered on restart. |
| **Process restart** | Agent crashes | If an agent crashes, the broker detects it (channel close, timeout). The action fails fast. The system can restart the agent from its package. |
| **Linux-level recovery** | Boot failures, kernel panics | v0.1 relies on Linux's own recovery (GRUB recovery, systemd emergency mode, journalctl). Aios does not manage boot in v0.1. |

### 6.3 Message bus failure behavior

| Component | Behavior without bus |
|---|---|
| Policy Broker | Must remain functional. v0.1: in-process, no bus dependency. v0.2: direct IPC channel to broker, not through bus. |
| Guardian | Must remain functional. Same as broker. |
| Staged Executor | Must remain functional. Same as broker. |
| Agents | Lose coordination. Cannot send tool requests. Actions in progress fail fast. |
| System Graph | Stops receiving events. Graph becomes stale. Marked `STALE`. |
| System State panel | Shows `DEGRADED` or `UNKNOWN` for affected subsystems. |

The key design rule: **the TCB communicates through direct channels, not
through the bus.** The bus is for agent coordination and telemetry, not for
safety-critical communication.

### 6.4 Future recovery (not in v0.1)

- A/B boot images with watchdog
- Signed recovery image
- Recovery supervisor independent of the main OS
- Firmware and boot chain verification
- Full trust plane as described in architecture section 3

---

## 7. Security-relevant design rules

These rules are derived from the threat model and must be followed by all
downstream design documents and implementations.

| Rule | Source | Enforced by |
|---|---|---|
| No agent has direct access to tools. All tool calls go through the broker. | REQ-SAF-001, section 1 | Type system (v0.1), process isolation (v0.2) |
| Context never grants capability. | REQ-SAF-005, section 3.2 | Broker capability validation |
| All external data is untrusted. | REQ-SAF-005, section 2.2 | Data classification at ingestion, broker validation |
| Secrets never leave the local trust boundary. | REQ-SAF-006, section 5.5 | Broker data policy enforcement |
| Fail-closed on ambiguity. | REQ-SAF-002, ADR-0003 | Broker decision logic |
| Guardian veto enforced by broker, not by Guardian alone. | REQ-SAF-003, section 1.2 | Broker requires Guardian sign-off for critical actions |
| User approval does not bypass invariants. | REQ-SAF-004, section 3.2 | Broker validates invariants regardless of approval |
| No LLM in real-time control loops. | REQ-PERF-003 | Architecture — deterministic controllers only |
| Agent Packages are signed and verified. | REQ-COMP-003, section 3.2 | Package loader |
| Audit log is append-only and never contains secrets. | REQ-OBS-001, REQ-OBS-002 | Logging layer |
| TCB components communicate through direct channels, not the bus. | Section 6.3 | Architecture — broker IPC design |
| Broker is small, deterministic, and fully audited. | Section 4.2 | Code review, test coverage, no `unsafe` in broker |

---

## 8. Open security questions

These are deferred to future design passes:

1. **Agent authentication protocol.** How do agents prove their identity to
   the broker in v0.2 when they are separate processes? (Unix socket
   credentials? mTLS? Signed tokens?)
2. **Audit log integrity.** Hash chaining is decided (SHA-256,
   forward-chained). Remaining question: what prevents a compromised
   broker from *omitting* entries? Omission detection (sequence number
   gaps) and external append-only storage are v0.2 concerns.
3. **Gateway pairing protocol.** How is a LAN gateway paired and
   authenticated? What happens if its certificate expires?
4. **Multi-user security.** v0.1 is single-user. When does multi-user
   identity, session isolation, and per-user consent become necessary?
5. **External-attacker hardening.** Network intrusion detection, privilege
   escalation prevention, service hardening. Deferred to a future security
   workstream.
6. **Hardware-assisted isolation.** When does the broker move into a TEE?
   What TEE technology (Intel SGX, AMD SEV, ARM TrustZone)?
7. **Secret store upgrade path.** When does v0.1's OS keyring become a
   dedicated secret store? What threat does that address?

---

## References

- `docs/architecture.md` — sections 1, 3, 5, 7, 10, 11, 13, 15
- `docs/requirements.md` — REQ-SAF-001 through REQ-SAF-007, REQ-OBS-001,
  REQ-OBS-002
- `docs/decisions/0001-v01-runs-above-linux.md` — v0.1 scope and Linux
  dependency
- `docs/decisions/0002-rust-as-implementation-language.md` — type system as
  isolation mechanism
- `docs/decisions/0003-fail-fast-no-silent-fallbacks.md` — development
  principle
- `docs/capability-model.md` — will define the capability protocol referenced
  throughout this document
- `docs/message-protocol.md` — will define message authentication and
  integrity referenced in section 3.2
