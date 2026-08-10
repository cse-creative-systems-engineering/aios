# Aios Observability

**Status:** Draft — frozen for M1  
**Depends on:** architecture.md, requirements.md, security-model.md, capability-model.md, message-protocol.md, system-graph.md, action-state-machine.md, decisions/0003-fail-fast-no-silent-fallbacks.md

## Purpose

Define what Aios records, how it records it, what it must not record, and how
traces propagate across the system.

### Design principles

1. **If it happened, it's logged.** Every action, decision, approval, tool
   call, and rollback is recorded. No unaudited operations.
2. **If it's a secret, it's not logged.** Credentials, tokens, and
   cryptographic material are never recorded. Redacted in all outputs.
3. **If it's chain-of-thought, it's not logged.** Model reasoning traces are
   not persisted. Only the inputs, outputs, and decisions are recorded.
4. **Every event is traceable.** Correlation IDs link related events across
   the entire action lifecycle.
5. **Staleness is visible.** Health values carry source, timestamp,
   freshness, and confidence. Missing data appears as `UNKNOWN` or `STALE`,
   never silently as healthy.
6. **Audit log failure stops the system.** If the audit log cannot be
   written, no actions proceed. No unaudited operations (fail-closed).

---

## 1. Audit Log

### 1.1 What is recorded

| Event type | What's logged | When |
|---|---|---|
| `ToolRequestReceived` | request_id, principal, resource, operation, risk_level | Broker receives request |
| `PolicyDecision` | request_id, decision (Allow/Deny), reason | Broker makes decision |
| `GuardianDecision` | request_id, verdict, affected_systems, rule_references | Guardian reviews |
| `ToolResult` | request_id, status, error (if any), health_impact | Specialist returns result |
| `ApprovalGranted` | approval_id, plan_id, plan_hash, scope, expires_at | User approves plan |
| `ApprovalExpired` | approval_id, plan_id | Approval expires |
| `ActionStateChange` | action_id, from_state, to_state, reason | State machine transitions |
| `CheckpointCreated` | checkpoint_id, action_id, resource | Before staging |
| `ActionCommitted` | action_id, resource, checkpoint_id (consumed) | After successful commit |
| `ActionRolledBack` | action_id, reason, checkpoint_id (consumed) | After rollback |
| `ActionFailed` | action_id, reason, checkpoint_id (retained) | After failure |
| `AgentStarted` | principal, package_id, version, resource | Agent instantiated |
| `AgentTerminated` | principal, reason | Agent stopped |
| `PackageActivated` | package_id, version | Package loaded |
| `PackageRevoked` | package_id, reason | Package revoked |
| `PackageQuarantined` | package_id, reason | Package quarantined |
| `ModelProviderSelected` | task_id, provider, model_id, connectivity_state | Router selects provider |
| `ModelProviderFailed` | provider, error | Provider health failure |
| `ConnectivityChanged` | from_state, to_state | Connectivity changes |
| `GraphNodeAdded` | node_id, node_type, source | Graph updated |
| `GraphNodeRemoved` | node_id, reason | Graph updated |
| `GraphConflictDetected` | node_id, conflict_description | Graph conflict |
| `SecretAccessed` | secret_id, principal, purpose (not the secret value) | Broker accesses secret store |

### 1.2 What is NOT recorded

- Model chain-of-thought or reasoning traces
- Secret values (credentials, tokens, keys, passwords) — replaced with `[REDACTED:secret]`
- Full message payloads containing secret references
- Personal memory at `INFO` level (logged at `DEBUG` or `TRACE` only)
- Model prompt contents (inputs are summarized, not verbatim)
- Internal agent memory or working state

### 1.3 Audit entry structure

```rust
pub struct AuditEntry {
    pub entry_id: AuditEntryId,
    pub timestamp: Timestamp,
    pub correlation_id: CorrelationId,
    pub event_type: AuditEventType,
    pub origin: PrincipalId,
    pub summary: AuditSummary,
    pub data_classification: DataClassification,
    pub previous_entry_hash: [u8; 32],  // Hash chaining
    pub entry_hash: [u8; 32],           // This entry's hash
}
```

### 1.4 Storage

| Version | Storage | Integrity |
|---|---|---|
| v0.1 | Local file (`/var/lib/aios/audit.log`) | Hash chaining — each entry includes the hash of the previous entry |
| v0.2+ | Embedded database (SQLite) with signed entries | Tamper-evident with external verification |

### 1.5 Hash chaining

```text
entry_1.hash = SHA256(entry_1.contents)
entry_2.previous_entry_hash = entry_1.hash
entry_2.hash = SHA256(entry_2.contents + entry_2.previous_entry_hash)
entry_3.previous_entry_hash = entry_2.hash
...
```

This makes tampering detectable: modifying or deleting an entry breaks the
chain. It does not prevent a compromised broker from rewriting the entire
log — that requires external storage (v0.2+).

### 1.6 Retention

| Data class | Retention |
|---|---|
| Safety-relevant events (decisions, approvals, rollbacks) | Indefinite |
| Routine operations (observations, queries) | 30 days |
| Model routing events | 7 days |
| Graph updates | 7 days |
| Debug/trace level logs | 24 hours |

Retention is configurable. Safety-relevant events are never automatically
deleted.

### 1.7 Audit log failure

If the audit log cannot be written (disk full, permission denied, I/O error):

- The broker denies all actions. No unaudited operations.
- The System State panel shows `Audit: FAILED`.
- The user is notified immediately.
- The system enters a read-only mode (observations may continue if they
  don't require audit logging, but mutations are blocked).

---

## 2. Trace Propagation

### 2.1 Correlation IDs

Every message carries a `correlation_id` that links all events in a single
action lifecycle:

```text
User intent: "Update Wi-Fi driver"
  → correlation_id: corr-1234

  Events with corr-1234:
    ActionPlan created (plan-5678)
    VerificationReport received (verdict: Approve)
    ToolRequest: stage_driver on wifi0 (req-9182)
    PolicyDecision: Allow
    GuardianDecision: Allow
    CheckpointCreated: cp-1234
    ActionStateChange: Staged → HealthVerified
    ActionStateChange: HealthVerified → Committed
    ActionCommitted: wifi0, cp-1234 consumed
```

### 2.2 Trace structure

```text
Intent
  └── Plan
        ├── Verification
        ├── ToolRequest
        │     ├── PolicyDecision
        │     ├── GuardianDecision
        │     ├── ToolResult
        │     └── ActionStateChange(s)
        ├── Approval (if required)
        ├── Checkpoint
        └── Commit or Rollback
```

### 2.3 Trace query

```rust
pub trait TraceQuery {
    /// Get all events for a correlation ID
    fn get_trace(&self, correlation_id: &CorrelationId) -> Vec<AuditEntry>;

    /// Get all events for an action
    fn get_action_trace(&self, action_id: &ActionId) -> Vec<AuditEntry>;

    /// Get all events for a time range
    fn get_events_in_range(&self, start: Timestamp, end: Timestamp) -> Vec<AuditEntry>;

    /// Get all events of a specific type
    fn get_events_by_type(&self, event_type: AuditEventType) -> Vec<AuditEntry>;
}
```

---

## 3. Metrics

### 3.1 Metric categories

| Category | Metrics |
|---|---|
| **Per-agent** | CPU usage, memory usage, latency, token usage, error rate, tool call count |
| **Per-specialist** | Tool call count, success rate, health check results, average response time |
| **Per-action** | Stage duration, health check duration, rollback rate, commit rate |
| **Per-provider** | Request count, latency, error rate, cost (external providers) |
| **System** | Model provider, connectivity state, graph size, active agent count, active action count |

### 3.2 Metric storage

| Version | Storage |
|---|---|
| v0.1 | In-memory counters and gauges, logged periodically to audit log |
| v0.2+ | Time-series database (embedded Prometheus or similar) |

### 3.3 Metric collection

Metrics are collected by the components themselves and reported through
`Event` messages or direct calls to a metrics collector. The metrics
collector aggregates and exposes them to the System State panel.

---

## 4. Health Read Model

### 4.1 State aggregator

The health read model is a state aggregator that sits between raw telemetry
events and the System State panel. It validates events, tracks freshness,
reconciles conflicts, and publishes a stable read model.

```text
Raw telemetry (Events)
        │
        ▼
  State Aggregator
  ├── Validates event provenance
  ├── Tracks freshness (last_observed, TTL)
  ├── Reconciles conflicts
  ├── Calculates health state
  └── Publishes read model
        │
        ▼
  System State Panel
```

### 4.2 Health state calculation

```rust
pub fn calculate_health(
    reports: &[HealthReport],
    now: Timestamp,
) -> HealthState {
    if reports.is_empty() {
        return HealthState::Unknown;
    }

    // Check freshness
    let any_stale = reports.iter().any(|r| r.freshness.is_stale(now));
    if any_stale {
        return HealthState::Stale;
    }

    // Check for conflicts
    if has_conflicting_reports(reports) {
        // Prefer the owning specialist's report
        if let Some(owner_report) = reports.iter().find(|r| r.is_owner) {
            return owner_report.state;
        }
        return HealthState::Unknown;
    }

    // All reports agree
    reports[0].state
}
```

### 4.3 Health report structure

```rust
pub struct HealthReadModel {
    pub resource: ResourceId,
    pub state: HealthState,
    pub source: PrincipalId,
    pub last_observed: Timestamp,
    pub freshness: Freshness,
    pub confidence: f64,
    pub metrics: HashMap<String, String>,
    pub warnings: Vec<String>,
    pub active_operations: Vec<ActionId>,
}

pub struct Freshness {
    pub last_observed: Timestamp,
    pub ttl: Duration,
    pub is_stale: bool,
    pub stale_since: Option<Timestamp>,
}
```

### 4.4 Freshness rules

| Condition | Display state |
|---|---|
| `last_observed + ttl > now` | Actual health state (Healthy/Degraded/Unhealthy) |
| `last_observed + ttl <= now` | `STALE` |
| No reports ever received | `UNKNOWN` |
| Conflicting reports, no owner | `UNKNOWN` |

The System State panel never shows `Healthy` for stale or unknown data.

---

## 5. Privacy

### 5.1 Data classification on log entries

Every audit entry carries a `data_classification`:

| Classification | Logging behavior |
|---|---|
| `Public` | Full content logged |
| `PersonalMemory` | Summarized at `INFO`, full at `DEBUG`/`TRACE` |
| `SystemConfig` | Full content logged |
| `Protected` | Full content logged locally, never forwarded externally |

Note: Secret values are never logged regardless of classification.
They are replaced with `[REDACTED:secret]` by the redaction layer.
Secret is NOT a `DataClassification` variant — it is a data-handling rule
enforced by the broker (see message-protocol.md §5.3 and security-model.md §5).

### 5.2 Redaction rules

| Data type | Redaction rule |
|---|---|
| Credentials, tokens, keys | `[REDACTED:secret]` in all logs and traces |
| API keys in model requests | Not logged (injected by gateway, not visible) |
| Passwords in tool results | Never returned to agents. Broker handles injection. |
| Personal memory in logs | Summarized, not verbatim, at `INFO` level |
| Model prompts | Summarized (intent + parameters), not verbatim |
| Model chain-of-thought | Never logged |

### 5.3 Redaction layer

The redaction layer sits in the logging pipeline, between the component and
the audit log:

```text
Component → RedactionLayer → AuditLog
```

- The redaction layer inspects every log entry before it's written.
- Payloads containing secret references (`SecretRef`) are replaced with
  `[REDACTED:secret]` before writing to the audit log.
- The redaction layer is part of the TCB — it cannot be bypassed by agents.
- Redaction rules are deterministic and testable.

### 5.4 Visibility (v0.1: single-user)

v0.1 is single-user. All logs and metrics are visible to the user. No
per-user filtering needed.

v0.2+ will define per-user visibility:
- Each user sees their own session's events.
- Administrators see all events.
- Remote sessions have restricted visibility (no secrets, no protected data).

---

## 6. Rust Types

```rust
use crate::protocol::{Timestamp, CorrelationId, PrincipalId, ResourceId, ActionId};
use std::collections::HashMap;

// ── Audit ──

pub type AuditEntryId = Uuid;

#[derive(Clone, Debug)]
pub struct AuditEntry {
    pub entry_id: AuditEntryId,
    pub timestamp: Timestamp,
    pub correlation_id: CorrelationId,
    pub event_type: AuditEventType,
    pub origin: PrincipalId,
    pub summary: AuditSummary,
    pub data_classification: DataClassification,
    pub previous_entry_hash: [u8; 32],
    pub entry_hash: [u8; 32],
}

#[derive(Clone, Debug)]
pub enum AuditEventType {
    ToolRequestReceived,
    PolicyDecision,
    GuardianDecision,
    ToolResult,
    ApprovalGranted,
    ApprovalExpired,
    ActionStateChange,
    CheckpointCreated,
    ActionCommitted,
    ActionRolledBack,
    ActionFailed,
    AgentStarted,
    AgentTerminated,
    PackageActivated,
    PackageRevoked,
    PackageQuarantined,
    ModelProviderSelected,
    ModelProviderFailed,
    ConnectivityChanged,
    GraphNodeAdded,
    GraphNodeRemoved,
    GraphConflictDetected,
    SecretAccessed,
}

#[derive(Clone, Debug)]
pub enum AuditSummary {
    ToolRequestReceived { request_id: String, resource: ResourceId, operation: String, risk_level: String },
    PolicyDecision { request_id: String, decision: String, reason: String },
    GuardianDecision { request_id: String, verdict: String, rules: Vec<String> },
    ToolResult { request_id: String, status: String, error: Option<String> },
    ApprovalGranted { approval_id: String, plan_id: String, scope: String },
    ActionStateChange { action_id: ActionId, from: String, to: String, reason: String },
    ActionCommitted { action_id: ActionId, resource: ResourceId },
    ActionRolledBack { action_id: ActionId, reason: String },
    ActionFailed { action_id: ActionId, reason: String },
    AgentStarted { principal: PrincipalId, package: String },
    AgentTerminated { principal: PrincipalId, reason: String },
    ModelProviderSelected { task_id: String, provider: String, model: String },
    ConnectivityChanged { from: String, to: String },
    SecretAccessed { secret_id: String, principal: PrincipalId, purpose: String },
    Other(String),
}

// ── Audit log ──

pub trait AuditLog {
    fn write(&mut self, entry: AuditEntry) -> Result<(), AuditError>;
    fn get_trace(&self, correlation_id: &CorrelationId) -> Vec<AuditEntry>;
    fn get_action_trace(&self, action_id: &ActionId) -> Vec<AuditEntry>;
    fn get_events_in_range(&self, start: Timestamp, end: Timestamp) -> Vec<AuditEntry>;
}

#[derive(Debug)]
pub enum AuditError {
    WriteFailed(String),
    DiskFull,
    PermissionDenied,
}

// ── Health read model ──

#[derive(Clone, Debug)]
pub struct HealthReadModel {
    pub resource: ResourceId,
    pub state: HealthState,
    pub source: PrincipalId,
    pub last_observed: Timestamp,
    pub freshness: Freshness,
    pub confidence: f64,
    pub metrics: HashMap<String, String>,
    pub warnings: Vec<String>,
    pub active_operations: Vec<ActionId>,
}

#[derive(Clone, Debug)]
pub struct Freshness {
    pub last_observed: Timestamp,
    pub ttl: u64,
    pub is_stale: bool,
    pub stale_since: Option<Timestamp>,
}

// ── Redaction ──

pub trait RedactionLayer {
    fn redact(&self, entry: &mut AuditEntry);
}

pub struct DefaultRedaction;

impl RedactionLayer for DefaultRedaction {
    fn redact(&self, entry: &mut AuditEntry) {
        // Secret values are never logged. The redaction layer inspects
        // payload content for secret references (SecretRef) and replaces
        // them with [REDACTED:secret]. This is not keyed off a
        // DataClassification variant — secrets are a data-handling rule,
        // not a message classification.
        if entry.contains_secret_ref() {
            entry.redact_secrets();
        }
    }
}
```

---

## 7. Open questions

1. **Log rotation.** How should the audit log be rotated for long-running
   systems? (Recommendation: rotate by size with configurable limit, but
   never delete safety-relevant entries.)
2. **External log shipping.** Should audit logs be shipped to an external
   system (e.g., syslog, journald)? (Recommendation: yes for v0.2 — integrate
   with journald for Linux-native logging.)
3. **Metric visualization.** Should metrics be visualized in the System State
   panel or a separate dashboard? (Recommendation: integrated for v0.1,
   separate for v0.2+ if complexity warrants.)
4. **Distributed tracing.** When Aios spans multiple processes (v0.2+),
   should it use OpenTelemetry or a custom tracing format?
   (Recommendation: OpenTelemetry for v0.2+ — standard, well-supported.)
5. **Audit log encryption.** Should the audit log be encrypted at rest?
   (Recommendation: yes for v0.2 — protects personal memory and system
   config if the filesystem is compromised.)

---

## References

- `docs/architecture.md` — section 6 (System State panel), section 15 (gaps:
  observability, health read model)
- `docs/security-model.md` — section 5 (secrets management), section 3.2
  (repudiation defenses)
- `docs/message-protocol.md` — section 6 (audit trail), section 5.3 (data
  classification)
- `docs/system-graph.md` — section 4 (graph integrity, staleness)
- `docs/requirements.md` — REQ-OBS-001, REQ-OBS-002, REQ-OBS-003, REQ-UX-003
- `docs/decisions/0003-fail-fast-no-silent-fallbacks.md` — audit log failure
  stops the system
