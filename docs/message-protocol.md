# Aios Message Protocol

**Status:** Draft — frozen for M1  
**Depends on:** architecture.md, glossary.md, requirements.md, security-model.md, capability-model.md, decisions/0001-v01-runs-above-linux.md, decisions/0003-fail-fast-no-silent-fallbacks.md, decisions/0004-two-dimensional-authorization.md

## Purpose

Specify the versioned, typed internal protocol for all inter-agent and
agent-to-service communication in Aios. This document is RFC-style with
concrete Rust schemas — not prose descriptions.

### Design principles

1. **Typed, not free-form.** Every message is a Rust enum variant with
   defined fields. No agent sends or receives raw strings, arbitrary JSON,
   or untyped payloads.
2. **Fail-fast on unknown or malformed messages.** Per ADR-0003, an unknown
   message type, a missing required field, or a failed validation causes an
   immediate error. No silent dropping or defaulting.
3. **Every message carries provenance.** Origin, correlation ID, timestamp,
   and deadline are mandatory on every message. No anonymous messages.
4. **Capability tokens travel with requests.** Every `ToolRequest` carries
   the principal's capability token. The broker verifies it; no message is
   trusted by origin alone.
5. **Data classification labels on payloads.** Every message payload carries
   a data classification so downstream components and the audit log know how
   to handle it.
6. **Secrets never appear in messages.** Secrets are injected by the broker
   directly into operations, never serialized into messages visible to agents.

### Canonical type ownership

This document is the **single source of truth** for all message-bearing types.
Other documents import these types; they do not redefine them.

| Type | Defined here | Used by |
|---|---|---|
| `ToolRequest` | §2.4 | capability-model.md, action-state-machine.md, testing-strategy.md |
| `ToolResult` | §2.5 | capability-model.md, testing-strategy.md |
| `ToolStatus` | §2.5 | capability-model.md |
| `ToolParameters` | §2.4 | capability-model.md |
| `GuardianVerdict` | §2.9 | capability-model.md |
| `PolicyDecision` (enveloped message) | §2.10 | capability-model.md |
| `PolicyVerdict` (bare return type) | §2.10 | capability-model.md |
| `DataClassification` | §5.3 | model-routing.md, observability.md |
| `Approval` | §2.7 | capability-model.md |
| `ApprovalScope` | §2.7 | capability-model.md |
| `ApprovalRequest` | §2.11 | capability-model.md |
| `UserResponse` | §2.12 | capability-model.md |
| `Message` | §7 | all docs |
| `MessageEnvelope` | §1.2 | all docs |

Types defined in `observability.md` (audit types):
`AuditEntry`, `AuditEventType`, `AuditSummary`, `AuditLog`, `RedactionLayer`.
These are the canonical definitions; message-protocol.md §6 references them.

Types defined in `capability-model.md` (authorization types, not messages):
`Capability`, `CapabilityToken`, `Clearance`, `DenyReason`, `RiskLevel`,
`Operation`, `PrincipalId`, `ResourceId`, `ResourceState`, `Provenance`,
`PolicyBroker`, `GuardianClient`, `ExecutorClient`.

---

## 1. Protocol Overview

### 1.1 Transport

| Version | Transport | Notes |
|---|---|---|
| v0.1 | In-process channels (Tokio `mpsc` / `oneshot`) | Typed Rust values, no serialization on the wire |
| v0.2 | Unix domain sockets | Serialized via serde, authenticated via socket credentials |
| v0.3+ | Optional Redis or network IPC | For distributed or multi-process deployments |

In v0.1, messages are Rust structs passed through channels. Serialization
is only needed for the audit log and persistence — not for transport. This
means the protocol types are the wire types; there is no separate schema.

The `BrokerClient` trait (defined in `capability-model.md`) is the agent's
only interface to the broker. It abstracts the transport so that v0.2 can
swap channels for sockets without changing agent code.

### 1.2 Message envelope

Every message carries a standard envelope:

```rust
pub struct MessageEnvelope {
    pub version: ProtocolVersion,
    pub message_type: MessageType,
    pub message_id: MessageId,
    pub correlation_id: CorrelationId,
    pub origin: PrincipalId,
    pub timestamp: Timestamp,
    pub deadline: Option<Timestamp>,
    pub data_classification: DataClassification,
}
```

| Field | Description |
|---|---|
| `version` | Protocol version. v0.1 uses `ProtocolVersion::V1`. Unknown versions → fail-fast. |
| `message_type` | Discriminant for the message body. |
| `message_id` | Unique ID for this message. Used for deduplication and audit. |
| `correlation_id` | Links related messages across a conversation or action. Shared by all messages in one action lifecycle. |
| `origin` | The principal that sent this message. Set by the broker from channel identity (not by the agent). |
| `timestamp` | When the message was created. Used for replay detection (v0.2+). |
| `deadline` | When the message expires. If `None`, no deadline. If past, message is rejected. |
| `data_classification` | Classification of the payload (Public, PersonalMemory, SystemConfig, Protected). Secret values never appear in messages. |

### 1.3 Versioning

```rust
pub enum ProtocolVersion {
    V1,  // v0.1
}
```

- Unknown versions cause fail-fast rejection.
- Future versions are additive — new message types are added, existing types
  are not removed or renamed. Fields may be added as `Option<T>` for backward
  compatibility.
- The protocol version is checked before any message processing.

---

## 2. Message Types

### 2.1 Message type registry

```rust
pub enum MessageType {
    ActionPlan,
    VerificationReport,
    ToolRequest,
    ToolResult,
    Event,
    Approval,
    HealthReport,
    GuardianDecision,
    PolicyDecision,
    ApprovalRequest,
    UserResponse,
    ErrorResponse,
}
```

### 2.2 ActionPlan

A structured proposal from the Planner Agent describing one or more actions
to achieve a user intent.

```rust
pub struct ActionPlan {
    pub envelope: MessageEnvelope,
    pub plan_id: PlanId,
    pub user_intent: String,
    pub actions: Vec<PlannedAction>,
    pub affected_systems: Vec<ResourceId>,
    pub expected_risks: Vec<RiskAssessment>,
    pub rollback_state: Option<CheckpointRef>,
}

pub struct PlannedAction {
    pub action_id: ActionId,
    pub tool_request: ToolRequest,
    pub description: String,
    pub risk_level: RiskLevel,  // Advisory/display only — broker resolves authoritative risk from ToolRegistry
}

pub struct RiskAssessment {
    pub resource: ResourceId,
    pub risk: String,
    pub severity: InvariantSeverity,
}

pub enum InvariantSeverity {
    Safety,      // Level 0 — most severe
    Boot,        // Level 1
    Availability, // Level 2
    Performance, // Level 3
    Experience,  // Level 4 — least severe
}

// Note: InvariantSeverity describes how severe an invariant violation is.
// This is the OPPOSITE ordering from RiskLevel (where 4 = most dangerous).
// RiskLevel describes how much authority an operation needs.
// Do not confuse the two scales.
```

### 2.3 VerificationReport

Independent review from the Verification Agent.

```rust
pub struct VerificationReport {
    pub envelope: MessageEnvelope,
    pub plan_id: PlanId,
    pub verdict: VerificationVerdict,
    pub concerns: Vec<String>,
    pub missing_information: Vec<String>,
    pub recommended_tests: Vec<String>,
}

pub enum VerificationVerdict {
    Approve,
    ApproveWithConditions(Vec<String>),
    Reject(String),
    InsufficientInformation,
}
```

### 2.4 ToolRequest

A request from an agent to a specialist tool, routed through the broker.

**This is the single source of truth for `ToolRequest`.** The capability model
imports this type; it does not redefine it.

```rust
pub struct ToolRequest {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub principal: PrincipalId,
    pub resource: ResourceId,
    pub operation: Operation,
    pub tool_id: ToolId,               // Broker looks up risk level from registry by this
    pub capability_token: CapabilityToken,
    pub parameters: ToolParameters,
    pub plan_hash: Option<PlanHash>,    // Required for risk level 3+ (links to approval)
    pub action_id: Option<ActionId>,    // Required for risk level 3+ (links to approval scope)
    pub nonce: u64,                      // Anti-replay: monotonically increasing per principal
}
```

**Key changes from adversarial review:**
- `tool_risk_level` removed from the request — the broker looks up the
  authoritative risk level from the `ToolRegistry` by `tool_id`. An agent
  cannot influence its own risk level.
- `tool_id` added — the broker resolves the tool definition (including risk
  level and required capabilities) from the registry.
- `plan_hash` and `action_id` added — required for risk level 3+ to link
  the request to a specific approved plan and check approval scope.
- `nonce` added — anti-replay protection. The broker rejects duplicate
  `(principal, nonce)` pairs.
- `deadline` removed from top-level — use `envelope.deadline` (must be
  `Some` for `ToolRequest`).

```rust
pub enum ToolParameters {
    Observe { fields: Vec<String> },
    Diagnose { symptom: String },
    Query { query: String },
    Restart { graceful: bool },
    Configure { changes: ConfigChanges },
    Stage { change: StagedChange },
    Commit { staged_change_id: StagedChangeId },
    FirmwareWrite { firmware_ref: FirmwareRef },
    BootConfig { changes: ConfigChanges },
    KernelModule { action: ModuleAction, module: String },
    Reset { to_known_good: bool },
    Quarantine { reason: String },
    Rollback { checkpoint: CheckpointRef },
}
```

**Validation:** The broker validates that the `ToolParameters` variant
matches the `Operation` variant. A mismatch (e.g., `Operation::Observe`
with `ToolParameters::FirmwareWrite`) is rejected with
`ProtocolError::ValidationFailed` (fail-fast per ADR-0003).

### 2.5 ToolResult

Response from a specialist tool, returned through the broker.

```rust
pub struct ToolResult {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub status: ToolStatus,
    pub data: Option<ToolData>,
    pub error: Option<ToolError>,
    pub health_impact: Option<HealthImpact>,
}

pub enum ToolStatus {
    Success,
    Denied,
    Failed,
    RolledBack,
    PartialSuccess,  // For multi-step operations
}

pub enum ToolData {
    DeviceState { state: ResourceState, metrics: HashMap<String, String> },
    Diagnosis { findings: Vec<String>, confidence: f64 },
    QueryResult { data: serde_json::Value },
    StagedChange { id: StagedChangeId, checkpoint: CheckpointRef },
    CommitResult { committed: bool, health_verified: bool },
    Empty,
}

pub struct ToolError {
    pub code: ToolErrorCode,
    pub message: String,
    pub recoverable: bool,
}

pub enum ToolErrorCode {
    ResourceUnavailable,
    OperationNotSupported,
    CapabilityDenied,
    GuardianBlocked,
    StagingFailed,
    HealthCheckFailed,
    Timeout,
    Internal,
}

pub struct HealthImpact {
    pub resource: ResourceId,
    pub before: HealthState,
    pub after: HealthState,
}
```

### 2.6 Event

Telemetry or system state change, published on the message bus.

```rust
pub struct Event {
    pub envelope: MessageEnvelope,
    pub event_type: EventType,
    pub payload: EventPayload,
}

pub enum EventType {
    DeviceAdded,
    DeviceRemoved,
    LinkStateChanged,
    TemperatureWarning,
    MemoryEccError,
    ServiceStateChanged,
    ResourceHealthChanged,
    AgentStarted,
    AgentTerminated,
    PackageActivated,
    PackageRevoked,
    ProgressUpdate,
}

pub enum EventPayload {
    DeviceAdded { bus: String, id: String, class: String },
    DeviceRemoved { id: String, reason: String },
    LinkStateChanged { device: String, state: String, reason: String },
    TemperatureWarning { device: String, celsius: f64 },
    MemoryEccError { bank: u32, corrected: bool },
    ServiceStateChanged { service: String, state: String },
    ResourceHealthChanged { resource: ResourceId, state: HealthState },
    AgentStarted { principal: PrincipalId, package: PackageId },
    AgentTerminated { principal: PrincipalId, reason: String },
    PackageActivated { package: PackageId, version: u32 },
    PackageRevoked { package: PackageId, reason: String },
    ProgressUpdate { request_id: RequestId, percent: u8, message: String },
}
```

### 2.7 Approval

User authorization for a specific action plan.

```rust
pub struct Approval {
    pub envelope: MessageEnvelope,
    pub approval_id: ApprovalId,
    pub plan_id: PlanId,
    pub plan_hash: PlanHash,        // Hash of the approved plan
    pub approved_by: PrincipalId,   // Always User for v0.1
    pub granted_at: Timestamp,
    pub expires_at: Timestamp,
    pub scope: ApprovalScope,
}

pub struct ApprovalScope {
    pub actions: Vec<ApprovedAction>,  // Which actions are approved
    pub resources: Vec<ResourceId>,    // Which resources may be touched
    pub operations: Vec<Operation>,    // Which operations are permitted
}

pub struct ApprovedAction {
    pub action_id: ActionId,
    pub resource: ResourceId,
    pub operation: Operation,
    pub tool_id: ToolId,
}

pub type PlanHash = [u8; 32];  // SHA-256 of the plan
pub type ActionId = Uuid;

// Approval does not bypass invariants or capabilities.
// The broker still validates every request against the approval scope.
// If the request is not within the approved scope, it is denied.
//
// The scope is derived from the plan at approval time:
// - actions: all action IDs in the approved plan
// - resources: all resources the plan touches
// - operations: all operations the plan requests
//
// The user approves the whole plan by seeing it. The scope constrains
// execution so an agent cannot use a plan approval as blanket authority
// for operations not in the approved plan.
//
// If the plan is modified after approval, the plan_hash will not match
// and the approval is invalid.
```

### 2.8 HealthReport

Subsystem health status, published periodically or on change.

```rust
pub struct HealthReport {
    pub envelope: MessageEnvelope,
    pub resource: ResourceId,
    pub state: HealthState,
    pub source: PrincipalId,         // Who reported this health
    pub freshness: Freshness,
    pub confidence: f64,
    pub metrics: HashMap<String, String>,
    pub warnings: Vec<String>,
}

pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
    Stale,
}

pub struct Freshness {
    pub last_observed: Timestamp,
    pub ttl: Duration,               // Time-to-live for this health report
    pub is_stale: bool,
}
```

### 2.9 GuardianDecision

The Infrastructure Guardian's verdict on a proposed action.

```rust
pub struct GuardianDecision {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub decision: GuardianVerdict,
    pub affected_systems: Vec<ResourceId>,
    pub rule_references: Vec<InvariantId>,
    pub explanation: String,
}

pub enum GuardianVerdict {
    Allow,
    Block(String),
}

// Note: GuardianVerdict is defined here as the authoritative version.
// capability-model.md imports it. The Block variant carries a reason string.
// Escalate variant removed in v0.1 (Guardian Escalate is collapsed to
// Deny by the broker per ADR-0003). See human-interaction.md §5.

// The block is enforced by the Policy Broker, not by the Guardian alone.
// The Guardian is read-only. It cannot execute or prevent execution directly.
```

### 2.10 PolicyDecision

The Policy Broker's decision on a tool request. The broker's `evaluate`
function returns a bare `PolicyVerdict`; the enveloped `PolicyDecision`
message is emitted to the audit log and any listeners.

```rust
pub struct PolicyDecision {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub decision: PolicyVerdict,
    pub audit_entry_id: AuditEntryId,
}

pub enum PolicyVerdict {
    Allow,
    Deny(DenyReason),
}

// Note: Escalate variant removed in v0.1 (Guardian Escalate is
// collapsed to Deny per ADR-0003). See human-interaction.md §5.
// The `Escalate` variant is intentionally NOT present.
// PolicyVerdict is the bare return type from PolicyBroker::evaluate
// PolicyDecision is the enveloped message emitted to audit log and listeners
```

### 2.11 ApprovalRequest

Sent from the broker to the conversational facade when user approval is
required for a risk level 3+ action. The facade presents the plan to the
user and returns a `UserResponse`.

```rust
pub struct ApprovalRequest {
    pub envelope: MessageEnvelope,
    pub plan_id: PlanId,
    pub plan_hash: PlanHash,
    pub plan_summary: String,
    pub affected_systems: Vec<ResourceId>,
    pub expected_risks: Vec<String>,
    pub rollback_state: Option<CheckpointRef>,
    pub expires_at: Timestamp,
}
```

### 2.12 UserResponse

The user's response to an `ApprovalRequest`. Authenticated by the broker
as coming from the user principal (v0.1: in-process user input channel;
v0.2: user authentication).

```rust
pub struct UserResponse {
    pub envelope: MessageEnvelope,
    pub approval_request_id: MessageId,  // References ApprovalRequest's envelope.message_id
    pub decision: UserDecision,
}

pub enum UserDecision {
    Approved,
    Rejected(String),
    // No `Modified` variant in v0.1. A user who wants to change the plan
    // responds `Rejected`; the Planner creates a new plan that goes
    // through the full lifecycle again (see human-interaction.md §4.4).
}
```

### 2.13 ErrorResponse

Returned when a message fails validation or processing.

```rust
pub struct ErrorResponse {
    pub envelope: MessageEnvelope,
    pub in_response_to: MessageId,
    pub error: ProtocolError,
}

pub enum ProtocolError {
    UnknownVersion(ProtocolVersion),
    UnknownMessageType(MessageType),
    MissingField(String),
    ValidationFailed(String),
    UnknownPrincipal(PrincipalId),
    ExpiredDeadline,
    DeserializationFailed(String),
    Internal(String),
}
```

---

## 3. Delivery Semantics

### 3.1 Delivery guarantees

| Message class | Guarantee | Idempotency | Retry |
|---|---|---|---|
| `ToolRequest` | At-least-once | Yes — `request_id` for dedup | Caller retries until deadline |
| `ToolResult` | At-most-once | N/A — matched by `request_id` | No retry |
| `ActionPlan` | At-least-once | Yes — `plan_id` for dedup | Planner retries |
| `Event` | At-most-once | No — transient, loss is acceptable | No retry |
| `Approval` | At-least-once | Yes — `approval_id` for dedup | No retry (user-initiated) |
| `HealthReport` | At-most-once | No — periodic, stale is detectable | No retry |
| `GuardianDecision` | At-least-once | Yes — matched by `request_id` | Broker retries |
| `PolicyDecision` | At-least-once | Yes — matched by `request_id` | No retry (broker is authoritative) |
| `ApprovalRequest` | At-least-once | Yes — matched by `message_id` | No retry (user-initiated) |
| `UserResponse` | At-least-once | Yes — matched by `approval_request_id` | No retry (user-initiated) |
| `ErrorResponse` | At-most-once | No | No retry |

### 3.2 Deadlines

Every `ToolRequest` and `ActionPlan` carries a deadline. Messages past their
deadline are rejected:

```rust
if let Some(deadline) = message.envelope.deadline {
    if deadline < now() {
        return Err(ProtocolError::ExpiredDeadline);
    }
}
```

Deadlines are mandatory for `ToolRequest` and `ActionPlan`. Optional for
`Event` and `HealthReport` (they may be dropped if stale).

### 3.3 Ordering

- **No global ordering guarantee.** Messages from different agents may
  arrive in any order.
- **Per-correlation ordering.** Messages sharing a `correlation_id` are
  processed in the order they arrive at the broker. The broker does not
  reorder.
- **Per-resource serialization.** The broker processes one `ToolRequest` per
  resource at a time. Concurrent requests for the same resource are queued.

### 3.4 Acknowledgements

v0.1 (in-process): No explicit ack needed. Channel send/recv is synchronous
in the Tokio sense — a sent message is received or the channel is closed
(fail-fast).

v0.2+ (IPC): Explicit ack with `MessageId` for `ToolRequest` and
`ActionPlan`. No ack for `Event` and `HealthReport`.

---

## 4. Error Handling

### 4.1 Error taxonomy

| Layer | Error type | Handling |
|---|---|---|
| Protocol | `UnknownVersion`, `UnknownMessageType`, `MissingField` | Reject immediately, return `ErrorResponse`, fail-fast |
| Protocol | `DeserializationFailed` | Reject, return `ErrorResponse`, log |
| Protocol | `ExpiredDeadline` | Return `ErrorResponse`, log audit entry. Do not silently drop (ADR-0003). |
| Capability | `UnknownPrincipal`, `CapabilityDenied` | Return `PolicyDecision::Deny`, log |
| Guardian | `GuardianBlocked` | Return `GuardianVerdict::Block`, log |
| Tool | `ResourceUnavailable`, `OperationNotSupported` | Return `ToolResult::Failed`, log |
| Tool | `StagingFailed`, `HealthCheckFailed` | Return `ToolResult::RolledBack`, trigger rollback, log |
| Tool | `Timeout` | Return `ToolResult::Failed`, log, cancel operation. Distinguished from `ExpiredDeadline`: the envelope deadline is checked at receipt (§3.2) and is a protocol error; a tool `Timeout` means the operation started but overran its own execution budget. |
| Tool | `Internal` | Return `ToolResult::Failed`, log. No silent escalation in v0.1 — the failure surfaces to the caller and the audit log. |

### 4.2 Retry behavior

- **No automatic retries in v0.1.** Per ADR-0003 (fail-fast), errors surface
  immediately. The caller decides whether to retry.
- If the caller retries, it must use the same `request_id` for deduplication
  or a new `request_id` for a fresh attempt.
- Retries must respect the original deadline. A retry past the deadline is
  rejected.

### 4.3 Dead-letter handling

v0.1: No dead-letter queue. Failed messages are logged to the audit log and
returned to the caller as `ErrorResponse` or `ToolResult::Failed`.

v0.2+: Dead-letter queue for messages that exceed retry limits, for
post-mortem analysis.

---

## 5. Security

### 5.1 Message authentication

| Version | Mechanism |
|---|---|
| v0.1 | In-process identity. The `origin` field is **set by the broker** from the channel identity, not by the sending agent. Each `mpsc::Receiver` is associated with a `PrincipalId` at instantiation time. The broker overwrites whatever `origin` the agent put in the envelope. Agents cannot forge another principal's identity because they do not control the channel-to-principal mapping. |
| v0.2 | Unix socket credentials (`SO_PEERCRED`) or signed tokens. |
| v0.3+ | mTLS or signed message tokens. |

The broker rejects any message where `origin` does not match the sender's
authenticated identity. Unknown origins → fail-fast.

### 5.2 Capability token attachment

Every `ToolRequest` carries a `CapabilityToken`. The broker verifies:

1. The token is valid (not expired, not revoked).
2. The token's principal matches the `origin`.
3. The token's capability matches the requested `(resource, operation)`.
4. The token's clearance is >= the tool's risk level.

Missing or invalid token → `PolicyDecision::Deny(MissingCapability)`.

### 5.3 Data classification labels

Every message envelope carries `DataClassification`:

```rust
pub enum DataClassification {
    Public,
    PersonalMemory,
    SystemConfig,
    Protected,  // Kernel, security, recovery state
    // Note: Secret is NOT a message classification. Secret values never
    // appear in messages. The broker injects secrets directly into
    // operations without exposing them to agents or message payloads.
    // A message that references a secret uses a SecretRef (by ID), not
    // the secret value.
}

pub type SecretRef = String;  // Reference to a secret in the broker's store
```

The classification controls:
- **Logging:** Secret values are never logged (regardless of classification). `PersonalMemory` is not logged at `INFO` level.
- **Model routing:** Secret values and `Protected` data are never sent to external models. `PersonalMemory` requires consent.
- **Forwarding:** Secret values are never forwarded to other agents. `Protected` is only forwarded to trusted gateways.

### 5.4 Redaction in transit

- Secrets are never serialized into messages. The broker injects credentials
  directly into operations.
- If a tool result accidentally contains a secret (e.g., a config file with
  a password), the redaction layer replaces it with `[REDACTED:secret]`
  before the result reaches the requesting agent.
- The redaction layer is part of the broker, not the agent. Agents cannot
  bypass it.

---

## 6. Audit Trail

### 6.1 What is logged

The audit log schema (`AuditEntry`, `AuditEventType`, `AuditSummary`) is
defined in `observability.md` as the canonical source. This section
describes what events are logged; the type definitions live in
observability.md §1.3 and §6.

Every message that passes through the broker generates an audit entry.
The audit entry includes hash chaining (`previous_entry_hash`, `entry_hash`)
for tamper detection, as defined in observability.md §1.4.

Note on `PolicyVerdict` vs `PolicyDecision`: the broker's `evaluate()` returns
a bare `PolicyVerdict` (§2.10) as its function result; the enveloped
`PolicyDecision` message is what gets written to the audit log and emitted to
listeners. The audit entry is derived from the verdict, not a separate
attempt.

**Audit-write failure terminates the flow.** If the audit entry for a request
cannot be written, the broker fails that request (`Deny(AuditLogFailure)`) and
enters a read-only/elevated-fail mode (observability.md §1.7). The broker does
**not** recursively attempt to log the failed audit write — that would
self-loop. Once the audit log is unavailable, further requests are rejected
without fresh audit entries, and the System State panel flags `Audit: FAILED`.

### 6.2 What is NOT logged

- Model chain-of-thought or reasoning traces
- Secret values (replaced with `[REDACTED:secret]`)
- Full message payloads containing secret values
- Personal memory at `INFO` level (logged at `DEBUG` or `TRACE` only)

### 6.3 Audit log integrity

v0.1: Append-only file with hash chaining. Each entry includes the hash of
the previous entry. Tampering is detectable.

v0.2+: External append-only storage or signed log entries.

---

## 7. Rust Type Summary

All protocol types are defined in the `aios::protocol` module. Key types:

```rust
// IDs
pub type MessageId = Uuid;
pub type CorrelationId = Uuid;
pub type RequestId = Uuid;
pub type PlanId = Uuid;
// ActionId and PlanHash are defined in §2.7 — not redefined here.
pub type ApprovalId = Uuid;
pub type AuditEntryId = Uuid;
pub type StagedChangeId = Uuid;
pub type InstanceId = Uuid;
pub type PackageId = String;  // e.g., "aios.specialist.network.wifi"
pub type ToolId = String;     // e.g., "wifi.observe_device"

// References
pub type CheckpointRef = String;
pub type InvariantId = String;  // e.g., "DRIVER-001"
// PlanHash and ActionId are defined in §2.7 — not redefined here.

// Time
pub type Timestamp = u64;  // Unix epoch seconds
pub type Duration = u64;  // Seconds

// Top-level message wrapper
pub enum Message {
    ActionPlan(ActionPlan),
    VerificationReport(VerificationReport),
    ToolRequest(ToolRequest),
    ToolResult(ToolResult),
    Event(Event),
    Approval(Approval),
    HealthReport(HealthReport),
    GuardianDecision(GuardianDecision),
    PolicyDecision(PolicyDecision),
    ApprovalRequest(ApprovalRequest),
    UserResponse(UserResponse),
    ErrorResponse(ErrorResponse),
}
```

Serialization for audit and persistence uses `serde_json` in v0.1. Transport
in v0.1 is in-process (no serialization needed). v0.2 may use `bincode`
for IPC efficiency.

---

## 8. Message Flow Examples

### 8.1 Read-only observation (risk level 0)

```text
User → Facade: "What's the status of wifi0?"
Facade → Planner: create plan
Planner → Broker: ToolRequest { observe, device:wifi0, tool_id: wifi.observe_device }
Broker: resolve tool_id → risk_level 0 from registry
Broker: validate capability ✓, validate clearance ✓
Broker → Wi-Fi Specialist: forward request
Wi-Fi Specialist → Broker: ToolResult { DeviceState }
Broker → Planner: forward result
Planner → Facade: "wifi0 is healthy, signal strength -42 dBm"
```

### 8.2 Staged driver update (risk level 2)

```text
User → Facade: "Update the Wi-Fi driver"
Facade → Planner: create plan
Planner → Broker: ToolRequest { stage_driver, device:wifi0, tool_id: wifi.stage_driver }
Broker: resolve tool_id → risk_level 2 from registry
Broker: validate capability ✓, validate clearance ✓
Broker → Guardian: review request
Guardian → Broker: Allow (no invariant violated)
Broker → Staged Executor: checkpoint, stage driver
Staged Executor → Broker: staged, health check passed
Broker → Planner: ToolResult { StagedChange, committed: true }
Planner → Facade: "Driver updated successfully"
```

### 8.3 Guardian block (risk level 3)

```text
User → Facade: "Write new firmware to wifi0"
Facade → Planner: create plan
Planner → Broker: ToolRequest { firmware_write, device:wifi0, tool_id: wifi.firmware_write }
Broker: resolve tool_id → risk_level 3 from registry
Broker: validate capability ✓, validate clearance ✓
Broker → Guardian: review request
Guardian → Broker: Block("FIRMWARE-001: untested firmware cannot be activated")
Broker → Planner: PolicyDecision::Deny(GuardianBlocked)
Planner → Facade: "Blocked: untested firmware. Rule FIRMWARE-001 requires a tested fallback image."
```

---

## 9. Open questions

1. **Message size limits.** Should there be a maximum message size? Large
   tool results (e.g., full device dumps) may need streaming or pagination.
2. **Backpressure.** How does the broker handle a slow specialist that can't
   keep up with requests? Queue with deadline, or reject?
3. **Message priority.** Should `Event` messages have priority levels for
   critical alerts (e.g., `TemperatureWarning`) vs routine telemetry?
4. **Protocol version negotiation.** v0.1 is single-version. When v0.2
   introduces a new version, how do agents negotiate? (Recommendation:
   broker enforces minimum version; agents declare their version at
   connection.)
5. **Streaming results.** Some operations (e.g., long-running diagnostics)
   may benefit from streaming results rather than a single `ToolResult`.
   **v0.1 decision:** No streaming. Specialists may publish `Event` messages
   for progress on long-running operations. The `ToolResult` is still a single
   message at completion. This uses the `Event` type already defined and
   keeps the protocol simple. If insufficient, add streaming in v0.2+.

---

## References

- `docs/architecture.md` — section 10 (message routing), section 15 (gaps:
  internal protocol)
- `docs/security-model.md` — section 3.2 (STRIDE: spoofing, tampering),
  section 5 (secrets management)
- `docs/capability-model.md` — section 5 (broker decision algorithm),
  section 8 (Rust types)
- `docs/requirements.md` — REQ-OBS-003 (trace propagation), REQ-PERF-001
  (deadlines), REQ-SAF-005 (external data untrusted)
- `docs/decisions/0003-fail-fast-no-silent-fallbacks.md` — unknown messages
  and errors cause immediate failure
- `docs/decisions/0004-two-dimensional-authorization.md` — tool risk levels
  in `ToolRequest`
- `docs/action-state-machine.md` — will define how `ToolRequest` and
  `ToolResult` map to action states
