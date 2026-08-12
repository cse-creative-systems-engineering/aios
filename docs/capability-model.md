# Aios Capability Model

**Status:** Draft — frozen for M1  
**Depends on:** architecture.md, glossary.md, requirements.md, security-model.md, decisions/0001-v01-runs-above-linux.md, decisions/0002-rust-as-implementation-language.md, decisions/0003-fail-fast-no-silent-fallbacks.md, decisions/0004-two-dimensional-authorization.md

## Purpose

Define the authorization system that gates all operations in Aios. This is
the linchpin of the architecture — every safety property depends on it. This
document is concrete enough to implement: principals, resource identifiers,
operation scopes, capability tokens, clearance levels, the broker's decision
algorithm, and Rust type definitions.

### Design principles

1. **Token cost is not a design constraint.** Per-resource granularity and
   per-stage validation are chosen for safety, not efficiency. The capability
   model should be as precise as the safety requirements demand.
2. **Capabilities are static.** An agent's capabilities are determined at
   instantiation from its package manifest. No dynamic capability requests.
   If an agent lacks a capability, that is a design gap to fix at the package
   level, not a runtime problem.
3. **No delegation.** Every agent gets capabilities directly from the broker.
   No agent can grant capabilities to another agent.
4. **Per-stage validation.** A capability authorizes a request. The action
   state machine, Guardian, and staged executor each validate independently at
   every transition. A capability is necessary but not sufficient.
5. **Two-dimensional authorization.** An agent needs both a valid capability
   (resource + operation) and sufficient clearance (tool risk level) to
   execute. See ADR-0004.
6. **Fail-closed.** Missing, ambiguous, or expired capabilities result in
   denial, not allowance. See ADR-0003, REQ-SAF-002.

---

## 1. Principals

A principal is an authenticated identity that can request actions, hold
capabilities, and be held accountable.

### 1.1 Principal types

| Type | Description | v0.1 scope |
|---|---|---|
| **User** | The human operator. Single-user for v0.1. Has `Clearance(Recovery)` = 4 and all capabilities. | One user, full authority |
| **AgentInstance** | A running agent created from a signed Agent Package. Identified by package ID + instance ID. | All agents |
| **SystemService** | A deterministic service that operates within the enforcement plane (e.g., the staged executor). | Broker, Guardian, executor |

**Tension note:** The user principal has "full authority" (all capabilities,
max clearance), while the threat model treats user input and the conversational
facade as untrusted boundaries. This is reconciled by: (1) the user's *input*
is untrusted (prompt injection defense), but the user's *approvals* are
authenticated via a dedicated user-input channel the broker reads directly;
(2) the facade may only produce proposals, not action plans; (3) user approval
is bound to a plan hash, not to the facade's rendering of intent. The user has
authority to approve, but cannot bypass invariants or the capability system.

### 1.2 Principal identity

```text
PrincipalId {
    type: PrincipalType,
    package_id: Option<PackageId>,   // None for user, Some for agents
    instance_id: Option<InstanceId>,  // None for user, Some for agents
}
```

- User principal: `PrincipalId { type: User, package_id: None, instance_id: None }`
- Agent principal: `PrincipalId { type: AgentInstance, package_id: Some("aios.specialist.network.wifi"), instance_id: Some("wifi0-instance-001") }`
- System principal: `PrincipalId { type: SystemService, package_id: None, instance_id: Some("policy-broker") }`

### 1.3 Authentication (v0.1)

v0.1 is in-process. Principal identity is established at instantiation:

- User: the process owner. No additional authentication for v0.1.
- Agent: the broker assigns an instance ID at creation time from a signed
  package. The instance ID is immutable for the agent's lifetime.
- System: hardcoded identities for broker, Guardian, and executor.

v0.2 will add IPC-based authentication (Unix socket credentials or signed
tokens) when agents move to separate processes.

---

## 2. Resources

A resource is any addressable system component that an agent can observe or
mutate.

### 2.1 Resource identifier format

```text
<resource-type>:<resource-name>
```

| Resource type | Examples | Description |
|---|---|---|
| `device` | `device:wifi0`, `device:nvme0`, `device:gpu0` | Physical or virtual hardware devices |
| `service` | `service:networkd`, `service:systemd-resolved` | OS services |
| `file` | `file:/etc/resolv.conf`, `file:/etc/fstab` | Configuration files |
| `driver` | `driver:iwlwifi`, `driver:nvme` | Kernel drivers |
| `firmware` | `firmware:iwlwifi-ucode` | Firmware blobs |
| `boot` | `boot:grub`, `boot:systemd-boot` | Boot configuration |
| `network` | `network:wlan0`, `network:eth0` | Network interfaces |
| `process` | `process:aios-broker` | Running processes |
| `graph` | `graph:system` | The System Graph itself |
| `secret` | `secret:wifi-credentials` | Secrets in the secret store |

### 2.2 Resource ownership

Each resource has exactly one owning specialist. The System Graph records
ownership via `owns` edges. Other agents may request information or actions
from the owner, but two agents do not independently control the same resource.

### 2.3 Resource lifecycle

```text
Discovered → Available → Degraded → Quarantined → Removed
                ↑                                       │
                └───────────── recovery ────────────────┘
```

| State | Meaning | Capability implications |
|---|---|---|
| Discovered | Detected but not yet attested | Read-only observation only |
| Available | Active and healthy | Full capabilities per agent's grants |
| Degraded | Functioning but with issues | Mutating operations may be restricted by Guardian |
| Quarantined | Isolated for safety | Only recovery operations (clearance 4) |
| Removed | No longer present | All capabilities for this resource are invalid |

---

## 3. Operations

Operations are the typed actions an agent can request on a resource.

### 3.1 Operation classes

| Operation | Description | Default risk level |
|---|---|---|
| `observe` | Read device state, health, or telemetry | 0 |
| `diagnose` | Analyze a fault or condition | 0 |
| `query` | Query a service or configuration | 0 |
| `restart` | Restart a service or device | 1 |
| `configure` | Change non-destructive configuration | 1 |
| `stage` | Stage a change for testing (driver, config, firmware) | 2 |
| `commit` | Commit a staged change to production | 2 |
| `firmware_write` | Write firmware to a device | 3 |
| `boot_config` | Modify boot configuration | 3 |
| `kernel_module` | Load or unload a kernel module | 3 |
| `reset` | Reset a device to known state | 4 |
| `quarantine` | Quarantine a device or service | 4 |
| `rollback` | Roll back to a previous checkpoint | 4 |

### 3.2 Capability scope

A capability is a (resource, operation) pair:

```text
Capability {
    resource: ResourceId,    // e.g., "device:wifi0"
    operation: Operation,     // e.g., Stage
}
```

A capability authorizes an agent to *request* that operation on that specific
resource. It does not authorize execution — that requires clearance and
per-stage validation (see sections 4 and 5).

### 3.3 Per-resource granularity

Capabilities are per-resource, not per-resource-class. A Wi-Fi specialist
with `observe` on `device:wifi0` does not automatically have `observe` on
`device:wifi1`. A new device requires a new capability grant at instantiation
or package revision.

This is a deliberate choice: precision over efficiency. Token cost is not a
design constraint for safety systems.

---

## 4. Tool Risk Levels and Clearance

### 4.1 Tool risk levels

Every tool operation has a risk level assigned at design time in the
specialist package manifest. The risk level determines what additional gates
the request must pass:

| Level | Name | Gates required | Examples |
|---|---|---|---|
| **0** | Read-only | Capability only | `observe`, `diagnose`, `query` |
| **1** | Routine | Capability + broker validation | `restart`, `configure` (non-destructive) |
| **2** | Staged mutation | Capability + broker + Guardian + staging | `stage`, `commit` |
| **3** | Critical mutation | Capability + broker + Guardian + user approval + staging | `firmware_write`, `boot_config`, `kernel_module` |
| **4** | Recovery | Capability + broker + Guardian + user approval (staging may be skipped only if the Guardian authorizes it; a checkpoint is still created first) | `reset`, `quarantine`, `rollback` |

### 4.2 Agent clearance

Each Agent Package declares a maximum clearance level in its manifest:

```text
package: aios.specialist.network.wifi
clearance: 4
capabilities:
  - resource: "device:wifi0"
    operations: [observe, diagnose, query, restart, stage, commit, reset]
```

The broker grants or denies clearance at instantiation. An agent with
clearance 1 cannot use level 2+ tools, even if it has the resource capability.

Note: the Wi-Fi specialist declares clearance 4 because the M6 vertical
slice requires `request_reset` (risk level 4). Compare the identical manifest
declaration in `agent-packages.md` §1.1. A package that does not need
recovery-level operations would declare a lower clearance.

### 4.3 Clearance is static

Clearance is set at instantiation and fixed for the agent's lifetime. If an
agent needs higher clearance, that is a package revision — not a runtime
request. Same principle as static capabilities: design-time decision, not
runtime.

### 4.4 Risk level assignment

Risk levels are assigned by the package author and verified at package
signing time. The broker does not trust agent-reported risk levels. A
compromised agent cannot lower a tool's risk level to bypass the Guardian.

---

## 5. Policy Broker

### 5.1 Role

The Policy Broker is the sole authority for capability validation and action
gating. It is deterministic, small, and fully audited. No `unsafe` code. No
external dependencies in the decision path. No probabilistic logic.

### 5.2 Decision algorithm

```text
Input: ToolRequest { principal, resource, operation, tool_id, plan_hash, action_id, nonce }

0. Resolve tool from registry
   - Look up tool_id in ToolRegistry
   - Unknown tool → DENY(UnknownTool)
   - Get authoritative risk_level from tool definition (NOT from the request)
   - Get required_capabilities from tool definition

0.5 Check deadline
   - If envelope.deadline is None → DENY(MissingDeadline)
   - If now > deadline → DENY(RequestExpired)

0.6 Check nonce (anti-replay)
   - If (principal, nonce) in replay_log → DENY(ReplayDetected)
   - Add (principal, nonce) to replay_log

1. Validate principal identity
   - Unknown principal → DENY(UnknownPrincipal)

2. Validate capability
   - For each required_capability in tool definition:
     - Principal has (resource, operation) capability?
     - No → DENY(MissingCapability)
   - Check resource state:
     - If resource is Discovered → only observe/diagnose/query allowed
     - If resource is Quarantined → only level 4 operations allowed
     - If resource is Removed → DENY(ResourceUnavailable)
   - Ambiguous or missing → DENY (fail-closed)

2.5 Validate token
   - If token is expired (now > token.expires_at) → DENY(ExpiredToken)
   - If token is in revoked set → DENY(RevokedToken)

3. Validate clearance
   - Principal clearance >= tool risk_level (from registry, not request)?
   - No → DENY(InsufficientClearance)

4. For risk level >= 2: Guardian review
   - Guardian unavailable → DENY(GuardianUnavailable) (fail-closed)
   - Guardian returns Allow or Deny
   - Deny → DENY(GuardianBlocked)
   - (Guardian Escalate is collapsed to Deny per ADR-0003 — see human-interaction.md §5)

5. For risk level >= 3: user approval
   - No valid, unexpired approval for this plan → DENY(NoUserApproval)
   - Approval exists but plan hash mismatch → DENY(PlanHashMismatch)
   - Approval exists but request's (action_id, resource, operation, tool_id)
     is not within the approval scope → DENY(ApprovalScopeExceeded)
   - Approval exists, valid, and request is within scope → proceed

6. For risk level >= 2: authorize staged execution
   - Emit StagingPlan to Staged Executor (separate principal)
   - Broker does NOT execute staging itself — it authorizes it
   - Await StagingResult from executor
   - On success → Allow
   - On failure → Deny(StagingFailure) or trigger rollback

7. Audit log entry (always — including denials)
   - If audit log write fails → DENY(AuditLogFailure)

Output: PolicyVerdict { Allow, Deny(reason) }
```

**Key changes from adversarial review:**
- Step 0: Broker resolves `tool_id` from `ToolRegistry` to get the
  authoritative `risk_level`. The request does NOT carry `tool_risk_level`.
- Step 0.5: Deadline is enforced.
- Step 0.6: Nonce prevents replay.
- Step 2: Broker checks `required_capabilities` from the tool definition,
  not just a single (resource, operation) pair. Resource state is checked.
- Step 2.5: Token expiration and revocation are checked.
- Step 4: Guardian `Escalate` is collapsed to `Deny(GuardianEscalation)` (fail-closed per ADR-0003). The `Escalate` variant is removed from v0.1 types. See human-interaction.md §5.
- Step 6: Broker authorizes staging but does NOT execute it. The Staged
  Executor (separate principal) performs checkpoint/stage/health/commit.

### 5.3 Fail-closed behavior

| Condition | Result |
|---|---|
| Unknown principal | DENY |
| Missing capability | DENY |
| Ambiguous capability (e.g., resource in `Discovered` state, not `Available`) | DENY |
| Insufficient clearance | DENY |
| Guardian unavailable | DENY(GuardianUnavailable) (cannot skip Guardian for level 2+) |
| Guardian returns Deny | DENY(GuardianBlocked) |
| Resource quarantined (non-level-4 operation) | DENY(ResourceQuarantined) |
| Approval exists but plan hash mismatch | DENY(PlanHashMismatch) |
| Request not within approval scope | DENY(ApprovalScopeExceeded) |
| Unknown tool | DENY(UnknownTool) |
| Missing deadline | DENY(MissingDeadline) |
| Request expired | DENY(RequestExpired) |
| Replay detected | DENY(ReplayDetected) |
| No user approval for level 3+ | DENY(NoUserApproval) |
| Staging or health check failure | ROLLBACK |
| Audit log write failure | DENY (no unaudited actions) |

### 5.4 Broker API (v0.1)

The broker exposes a `BrokerClient` interface to agents. This interface is
designed as if it were already an IPC boundary, so the v0.2 process split is
a transport change, not a redesign.

```rust
pub trait BrokerClient {
    fn request_tool(&self, request: ToolRequest) -> Result<ToolResult, BrokerError>;
    fn get_capabilities(&self, principal: &PrincipalId) -> Vec<Capability>;
    fn get_clearance(&self, principal: &PrincipalId) -> Clearance;
}
```

Agents do not have direct access to tools, capability tokens, or the broker's
internal state. They can only send `ToolRequest` messages and receive
`ToolResult` responses.

---

## 6. Capability Tokens

### 6.1 Purpose

A capability token is proof that a principal holds a specific capability. It
is carried with every `ToolRequest` and verified by the broker.

### 6.2 Token structure

```text
CapabilityToken {
    principal: PrincipalId,
    capability: Capability,          // (resource, operation)
    clearance: Clearance,            // max risk level
    granted_at: Timestamp,
    expires_at: Timestamp,
    provenance: Provenance,          // who granted it, from which package
    // Note: no `revoked: bool` field. Revocation is a broker-side fact
    // maintained in the broker's revocation set. Tokens are immutable.
}
```

### 6.3 Token lifecycle

```text
Package instantiated
  → Broker reads manifest
  → Broker grants capabilities (creates tokens)
  → Agent holds tokens for its lifetime
  → Tokens expire when agent is terminated or package is revoked
```

Tokens are static — granted at instantiation, valid for the agent's lifetime,
revoked when the agent is terminated or the package is revoked. No renewal,
no re-grant, no dynamic tokens.

### 6.4 Revocation

| Trigger | Action |
|---|---|
| Agent terminated | All tokens for that principal are revoked |
| Package revoked | All tokens for all instances of that package are revoked |
| Resource removed | All tokens for that resource are invalid |
| Resource quarantined | Mutating tokens for that resource are suspended; recovery tokens (level 4) remain valid |

Revocation is immediate in v0.1 (in-process). The broker maintains a
revocation set and checks it on every request.

### 6.5 No delegation

Agents cannot grant, transfer, or delegate capability tokens. The broker is
the sole grantor. If a specialist needs authority, its package declares it
and the broker grants it directly at instantiation.

---

## 7. Tool Registry

### 7.1 Purpose

The tool registry is a broker-managed catalog of all available tools, their
risk levels, and which specialist packages expose them. Agents query the
registry to discover what tools they can request.

### 7.2 Registry structure

```text
ToolRegistry {
    tools: Map<ToolId, ToolDefinition>
}

ToolDefinition {
    tool_id: ToolId,                    // e.g., "wifi.observe_device"
    specialist_package: PackageId,      // who provides this tool
    risk_level: RiskLevel,              // 0-4, assigned at package signing
    required_capabilities: Vec<Capability>,
    description: String,
}
```

### 7.3 v0.1: static tools only

In v0.1, tools are registered at package load time. The registry is
populated when specialist packages are instantiated and does not change
during the session.

### 7.4 Future: composed tools (not in v0.1)

The registry is designed to support composed tools in a future version:

- An agent requests a composition of approved primitives.
- The broker verifies every primitive is from a signed package.
- The broker computes the composed tool's risk level as the maximum of its
  constituents.
- The broker validates the agent has capabilities and clearance for every
  primitive.
- The composed tool is registered with the broker-assigned risk level.

**Key constraint:** the agent never assigns or influences the risk level.
The broker computes it deterministically. No primitive composition can
produce a tool with a lower risk level than its highest-risk constituent.

This is deferred to v0.2+ and will require its own ADR.

---

## 8. Rust Types (Target)

These types are close to compilable Rust. They will be refined during
implementation but serve as the contract for the capability model.

```rust
use std::collections::{HashMap, HashSet};

// ── Principals ──

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PrincipalType {
    User,
    AgentInstance,
    SystemService,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PrincipalId {
    pub r#type: PrincipalType,
    pub package_id: Option<PackageId>,
    pub instance_id: Option<InstanceId>,
}

// ── Resources ──

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ResourceId(String);  // e.g., "device:wifi0"

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceState {
    Discovered,
    Available,
    Degraded,
    Quarantined,
    Removed,
}

// ── Operations ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operation {
    Observe,
    Diagnose,
    Query,
    Restart,
    Configure,
    Stage,
    Commit,
    FirmwareWrite,
    BootConfig,
    KernelModule,
    Reset,
    Quarantine,
    Rollback,
}

// ── Capabilities ──

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Capability {
    pub resource: ResourceId,
    pub operation: Operation,
}

// ── Risk levels and clearance ──

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    ReadOnly,    // 0
    Routine,     // 1
    Staged,      // 2
    Critical,    // 3
    Recovery,    // 4
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Clearance(pub RiskLevel);  // Agent's max risk level
// Clearance is a distinct newtype, NOT a type alias for RiskLevel.
// This prevents mixing clearance and risk_level at compile time.

// ── Capability tokens ──

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CapabilityToken {
    pub principal: PrincipalId,
    pub capability: Capability,
    pub clearance: Clearance,
    pub granted_at: Timestamp,
    pub expires_at: Timestamp,
    // Note: no `revoked: bool` field. Revocation is a broker-side fact
    // maintained in the broker's revocation set. Tokens are immutable
    // once granted. An agent cannot self-attest non-revocation.
    pub provenance: Provenance,
    // v0.1: no cryptographic signature. Token authenticity is enforced by
    // Rust type safety — tokens are broker-owned opaque handles, not
    // reconstructible structs. Agents receive a BrokerClient handle, not
    // the token bytes. v0.2: add broker signature for IPC.
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Provenance {
    pub granted_by: PrincipalId,
    pub package_id: PackageId,
    pub package_version: u32,
    pub signature_verified: bool,
}

// ── Tool request (imported from message-protocol.md) ──
// ToolRequest is defined in message-protocol.md as the single source of truth.
// It is reproduced here for reference but the protocol doc is authoritative.
//
// pub struct ToolRequest {
//     pub envelope: MessageEnvelope,
//     pub request_id: RequestId,
//     pub principal: PrincipalId,
//     pub resource: ResourceId,
//     pub operation: Operation,
//     pub tool_id: ToolId,
//     pub capability_token: CapabilityToken,
//     pub parameters: ToolParameters,
//     pub plan_hash: Option<PlanHash>,
//     pub action_id: Option<ActionId>,
//     pub nonce: u64,
// }
//
// Note: tool_risk_level is NOT in the request. The broker resolves it
// from the ToolRegistry by tool_id.

// ── Tool result (imported from message-protocol.md §2.5) ──
// ToolResult, ToolStatus, ToolData, ToolError, HealthImpact are defined
// in message-protocol.md as the single source of truth. NOT redefined here.
//
// pub struct ToolResult {
//     pub envelope: MessageEnvelope,
//     pub request_id: RequestId,
//     pub status: ToolStatus,
//     pub data: Option<ToolData>,
//     pub error: Option<ToolError>,
//     pub health_impact: Option<HealthImpact>,
// }
// pub enum ToolStatus { Success, Denied, Failed, RolledBack, PartialSuccess }

// Note: PolicyDecision (the enveloped message) and PolicyVerdict (the bare
// return type) are defined in message-protocol.md §2.10. NOT redefined here.

#[derive(Clone, Debug)]
pub enum DenyReason {
    UnknownPrincipal,
    UnknownTool,
    MissingCapability,
    AmbiguousCapability,
    InsufficientClearance,
    GuardianBlocked(String),
    GuardianUnavailable,
    GuardianEscalation,
    NoUserApproval,
    PlanHashMismatch,
    ApprovalScopeExceeded,
    ResourceUnavailable(ResourceState),
    ResourceQuarantined,
    AuditLogFailure,
    ExpiredToken,
    RevokedToken,
    RequestExpired,
    MissingDeadline,
    ReplayDetected,
    StagingFailure,
    HealthCheckFailure,
}

// ── Guardian verdict (imported from message-protocol.md §2.9) ──
// GuardianVerdict is defined in message-protocol.md as the authoritative version.
// It is NOT redefined here.
//
// pub enum GuardianVerdict {
//     Allow,
//     Block(String),
// }
//
// Note: The Escalate variant is removed in v0.1 (collapsed to
// Deny(GuardianEscalation) by the broker per ADR-0003).

// ── Policy verdict (imported from message-protocol.md §2.10) ──
// PolicyVerdict (bare return type) and PolicyDecision (enveloped message)
// are defined in message-protocol.md as the authoritative versions.
// They are NOT redefined here.
//
// pub enum PolicyVerdict {
//     Allow,
//     Deny(DenyReason),
// }
//
// Note: The Escalate variant is removed in v0.1. Guardian Escalate is
// collapsed to Deny(GuardianEscalation) per ADR-0003 (see
// human-interaction.md §5).

// ── Approval types (defined in message-protocol.md, reproduced for reference) ──
//
// pub struct Approval {
//     pub approval_id: ApprovalId,
//     pub plan_id: PlanId,
//     pub plan_hash: PlanHash,
//     pub approved_by: PrincipalId,
//     pub granted_at: Timestamp,
//     pub expires_at: Timestamp,
//     pub scope: ApprovalScope,
// }
// pub struct ApprovalScope {
//     pub actions: Vec<ApprovedAction>,
//     pub resources: Vec<ResourceId>,
//     pub operations: Vec<Operation>,
// }
// pub struct ApprovedAction {
//     pub action_id: ActionId,
//     pub resource: ResourceId,
//     pub operation: Operation,
//     pub tool_id: ToolId,
// }
// pub type PlanHash = [u8; 32];
// pub type ActionId = Uuid;
//
// EscalationRequirements and the Escalate variant are removed in v0.1.

// ── Broker ──

pub trait BrokerClient {
    fn request_tool(&self, request: ToolRequest) -> Result<ToolResult, BrokerError>;
    fn get_capabilities(&self, principal: &PrincipalId) -> Vec<Capability>;
    fn get_clearance(&self, principal: &PrincipalId) -> Clearance;
}

pub struct PolicyBroker {
    capabilities: HashMap<PrincipalId, Vec<CapabilityToken>>,
    clearances: HashMap<PrincipalId, Clearance>,
    tool_registry: ToolRegistry,
    revoked: HashSet<CapabilityToken>,
    audit_log: AuditLog,
    approvals: HashMap<PlanId, Approval>,        // For step 5
    resource_states: HashMap<ResourceId, ResourceState>,  // For step 2
    replay_log: HashSet<(PrincipalId, u64)>,    // For step 0.6
    // Guardian and Executor are separate principals, accessed via traits
    guardian: Box<dyn GuardianClient>,
    executor: Box<dyn ExecutorClient>,
}

pub trait GuardianClient {
    fn review(&self, request: &ToolRequest) -> GuardianVerdict;
}

pub trait ExecutorClient {
    fn stage_and_commit(&self, action_id: &ActionId, request: &ToolRequest) -> Result<StagingResult, StagingError>;
}

pub enum StagingResult {
    Committed,
    RolledBack,
}

pub enum StagingError {
    CheckpointFailed,
    StageFailed,
    HealthCheckFailed,
    CommitFailed,
    RollbackFailed,
}

impl PolicyBroker {
    pub fn evaluate(&self, request: &ToolRequest) -> PolicyVerdict {
        // 0. Resolve tool from registry (get authoritative risk_level)
        // 0.5 Check deadline
        // 0.6 Check nonce (anti-replay)
        // 1. Validate principal
        // 2. Validate capability (from tool definition's required_capabilities)
        // 2.5 Validate token (expiration, revocation)
        // 3. Validate clearance
        // 4. Guardian review (level 2+)
        // 5. User approval (level 3+)
        // 6. Authorize staged execution (level 2+) — emit to executor
        // 7. Audit log
        // → Allow or Deny
        todo!("implement per section 5.2")
    }
}
```

---

## 9. Capability model summary

```text
Agent sends ToolRequest to Broker
  │
  ├── 0. Resolve tool from ToolRegistry by tool_id (get authoritative risk_level)
  │      Unknown tool → DENY(UnknownTool)
  │
  ├── 0.5 Check deadline (envelope.deadline)
  │      Missing or expired → DENY
  │
  ├── 0.6 Check nonce (anti-replay)
  │      Duplicate (principal, nonce) → DENY(ReplayDetected)
  │
  ├── 1. Principal identity validated?
  │      No → DENY (fail-closed)
  │
  ├── 2. Capability validated? (from tool definition's required_capabilities)
  │      No → DENY (fail-closed)
  │      Resource state checked (Discovered/Quarantined/Removed)
  │
  ├── 2.5 Token validated? (expiration, revocation)
  │      No → DENY (fail-closed)
  │
  ├── 3. Clearance validated? (agent clearance >= tool risk_level from registry)
  │      No → DENY (fail-closed)
  │
  ├── 4. Risk level >= 2? → Guardian review
  │      Guardian unavailable → DENY (fail-closed)
  │      Guardian Deny → DENY(GuardianBlocked)
  │      Guardian Escalate → DENY(GuardianEscalation) (fail-closed)
  │
  ├── 5. Risk level >= 3? → User approval required
  │      No valid approval → DENY(NoUserApproval)
  │      Plan hash mismatch → DENY(PlanHashMismatch)
  │      Out of scope → DENY(ApprovalScopeExceeded)
  │
  ├── 6. Risk level >= 2? → Authorize staged execution (separate executor)
  │      Checkpoint → Stage → Health check → Commit or Rollback
  │
  ├── 7. Audit log entry (always)
  │      Log write failure → DENY (no unaudited actions)
  │
  └── ALLOW or DENY
```

---

## 10. Open questions

1. **Capability expiration.** Resolved: tokens have `expires_at` and
   the broker checks it at step 2.5. Agent lifetime is the default
   expiration for v0.1. Explicit expiration for v0.2+.
2. **Resource state transitions.** How does the broker learn that a resource
   moved from `Available` to `Degraded`? Per ADR-0005 P1-5, the broker keeps
   its **own** `resource_states` registry, updated only by signed events from
   the trusted owning specialist — the System Graph is advisory and is not
   the broker's source for permission decisions. M1 must fix the plumbing:
   which message (`Event`?) carries the authoritative `ResourceState`
   transition from the owning specialist to the broker, and how the broker
   rejects state claims from non-owners. Until a resource has an entry in the
   broker's registry, it is treated as `Unknown` and denied (fail-closed).
3. **Guardian unavailability.** If the Guardian is temporarily unavailable,
   should level 2+ requests fail-closed (deny) or queue? (Recommendation:
   fail-closed per ADR-0003.)
4. **Tool registry persistence.** Should the tool registry persist across
   restarts, or is it rebuilt from packages each time? (Recommendation:
   rebuild from packages — packages are the source of truth.)
5. **Composed tool ADR.** When dynamic tool composition is introduced (v0.2+),
   what are the exact constraints on primitive composition and risk level
   computation?

---

## References

- `docs/architecture.md` — sections 5 (specialists as tools), 7 (Guardian),
  15 (gaps: capabilities and identity)
- `docs/security-model.md` — sections 1 (TCB), 3.2 (STRIDE: elevation of
  privilege), 7 (design rules)
- `docs/requirements.md` — REQ-SAF-001, REQ-SAF-002, REQ-SAF-004, REQ-SAF-005
- `docs/decisions/0002-rust-as-implementation-language.md` — type system
  enforces capability and clearance
- `docs/decisions/0003-fail-fast-no-silent-fallbacks.md` — missing capability
  causes immediate failure
- `docs/decisions/0004-two-dimensional-authorization.md` — capability × risk
  level model
- `docs/message-protocol.md` — will define `ToolRequest` and `ToolResult`
  wire format
- `docs/action-state-machine.md` — will define staged execution transitions
