use crate::capability::{CapabilityToken, Operation, PrincipalId, ResourceId, RiskLevel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub type MessageId = uuid::Uuid;
pub type CorrelationId = uuid::Uuid;
pub type RequestId = uuid::Uuid;
pub type PlanId = uuid::Uuid;
pub type ActionId = uuid::Uuid;
pub type ApprovalId = uuid::Uuid;
pub type AuditEntryId = uuid::Uuid;
pub type StagedChangeId = uuid::Uuid;
pub type InstanceId = uuid::Uuid;
pub type PackageId = String;
pub type ToolId = String;
pub type CheckpointRef = String;
pub type InvariantId = String;
pub type PlanHash = [u8; 32];
pub type Timestamp = u64;
pub type Duration = u64;

pub fn now() -> Timestamp {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolVersion {
    V1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataClassification {
    Public,
    PersonalMemory,
    SystemConfig,
    Protected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

impl MessageEnvelope {
    pub fn new(
        message_type: MessageType,
        origin: PrincipalId,
        correlation_id: CorrelationId,
        data_classification: DataClassification,
    ) -> Self {
        Self {
            version: ProtocolVersion::V1,
            message_type,
            message_id: uuid::Uuid::new_v4(),
            correlation_id,
            origin,
            timestamp: now(),
            deadline: None,
            data_classification,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlannedAction {
    pub action_id: ActionId,
    pub tool_request: Box<ToolRequest>,
    pub description: String,
    pub risk_level: RiskLevel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InvariantSeverity {
    Safety,
    Boot,
    Availability,
    Performance,
    Experience,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub resource: ResourceId,
    pub risk: String,
    pub severity: InvariantSeverity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionPlan {
    pub envelope: MessageEnvelope,
    pub plan_id: PlanId,
    pub user_intent: String,
    pub actions: Vec<PlannedAction>,
    pub affected_systems: Vec<ResourceId>,
    pub expected_risks: Vec<RiskAssessment>,
    pub rollback_state: Option<CheckpointRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationVerdict {
    Approve,
    ApproveWithConditions(Vec<String>),
    Reject(String),
    InsufficientInformation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub envelope: MessageEnvelope,
    pub plan_id: PlanId,
    pub verdict: VerificationVerdict,
    pub concerns: Vec<String>,
    pub missing_information: Vec<String>,
    pub recommended_tests: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolParameters {
    Observe { fields: Vec<String> },
    Diagnose { symptom: String },
    Query { query: String },
    Restart { graceful: bool },
    Configure { changes: serde_json::Value },
    Stage { change: serde_json::Value },
    Commit { staged_change_id: StagedChangeId },
    FirmwareWrite { firmware_ref: String },
    BootConfig { changes: serde_json::Value },
    KernelModule { action: String, module: String },
    Reset { to_known_good: bool },
    Quarantine { reason: String },
    Rollback { checkpoint: CheckpointRef },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolRequest {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub principal: PrincipalId,
    pub resource: ResourceId,
    pub operation: Operation,
    pub tool_id: ToolId,
    pub capability_token: CapabilityToken,
    pub parameters: ToolParameters,
    pub plan_hash: Option<PlanHash>,
    pub action_id: Option<ActionId>,
    pub nonce: u64,
}

impl ToolRequest {
    pub fn new(
        principal: PrincipalId,
        resource: ResourceId,
        operation: Operation,
        tool_id: impl Into<String>,
        capability_token: CapabilityToken,
        parameters: ToolParameters,
        correlation_id: CorrelationId,
        data_classification: DataClassification,
        deadline_secs_from_now: Duration,
    ) -> Self {
        Self {
            envelope: MessageEnvelope {
                version: ProtocolVersion::V1,
                message_type: MessageType::ToolRequest,
                message_id: uuid::Uuid::new_v4(),
                correlation_id,
                origin: principal.clone(),
                timestamp: now(),
                deadline: Some(now() + deadline_secs_from_now),
                data_classification,
            },
            request_id: uuid::Uuid::new_v4(),
            principal,
            resource,
            operation,
            tool_id: tool_id.into(),
            capability_token,
            parameters,
            plan_hash: None,
            action_id: None,
            nonce: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolStatus {
    Success,
    Denied,
    Failed,
    RolledBack,
    PartialSuccess,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
    Stale,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolData {
    DeviceState {
        state: crate::capability::ResourceState,
        metrics: HashMap<String, String>,
    },
    Diagnosis {
        findings: Vec<String>,
        confidence: f64,
    },
    QueryResult {
        data: serde_json::Value,
    },
    StagedChange {
        id: StagedChangeId,
        checkpoint: CheckpointRef,
    },
    CommitResult {
        committed: bool,
        health_verified: bool,
    },
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolError {
    pub code: ToolErrorCode,
    pub message: String,
    pub recoverable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthImpact {
    pub resource: ResourceId,
    pub before: HealthState,
    pub after: HealthState,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub status: ToolStatus,
    pub data: Option<ToolData>,
    pub error: Option<ToolError>,
    pub health_impact: Option<HealthImpact>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EventPayload {
    DeviceAdded {
        bus: String,
        id: String,
        class: String,
    },
    DeviceRemoved {
        id: String,
        reason: String,
    },
    LinkStateChanged {
        device: String,
        state: String,
        reason: String,
    },
    TemperatureWarning {
        device: String,
        celsius: f64,
    },
    MemoryEccError {
        bank: u32,
        corrected: bool,
    },
    ServiceStateChanged {
        service: String,
        state: String,
    },
    ResourceHealthChanged {
        resource: ResourceId,
        state: HealthState,
    },
    AgentStarted {
        principal: PrincipalId,
        package: PackageId,
    },
    AgentTerminated {
        principal: PrincipalId,
        reason: String,
    },
    PackageActivated {
        package: PackageId,
        version: u32,
    },
    PackageRevoked {
        package: PackageId,
        reason: String,
    },
    ProgressUpdate {
        request_id: RequestId,
        percent: u8,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub envelope: MessageEnvelope,
    pub event_type: EventType,
    pub payload: EventPayload,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedAction {
    pub action_id: ActionId,
    pub resource: ResourceId,
    pub operation: Operation,
    pub tool_id: ToolId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalScope {
    pub actions: Vec<ApprovedAction>,
    pub resources: Vec<ResourceId>,
    pub operations: Vec<Operation>,
}

impl ApprovalScope {
    pub fn contains(
        &self,
        action_id: &ActionId,
        resource: &ResourceId,
        operation: &Operation,
        tool_id: &ToolId,
    ) -> bool {
        let action_ok = self
            .actions
            .iter()
            .any(|a| &a.action_id == action_id && &a.tool_id == tool_id);
        action_ok && self.resources.contains(resource) && self.operations.contains(operation)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Approval {
    pub envelope: MessageEnvelope,
    pub approval_id: ApprovalId,
    pub plan_id: PlanId,
    pub plan_hash: PlanHash,
    pub approved_by: PrincipalId,
    pub granted_at: Timestamp,
    pub expires_at: Timestamp,
    pub scope: ApprovalScope,
}

impl Approval {
    pub fn is_valid_at(&self, t: Timestamp) -> bool {
        self.granted_at <= t && t < self.expires_at
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Freshness {
    pub last_observed: Timestamp,
    pub ttl: Duration,
    pub is_stale: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HealthReport {
    pub envelope: MessageEnvelope,
    pub resource: ResourceId,
    pub state: HealthState,
    pub source: PrincipalId,
    pub freshness: Freshness,
    pub confidence: f64,
    pub metrics: HashMap<String, String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardianVerdict {
    Allow,
    Block(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardianDecision {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub decision: GuardianVerdict,
    pub affected_systems: Vec<ResourceId>,
    pub rule_references: Vec<InvariantId>,
    pub explanation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyVerdict {
    Allow,
    Deny(crate::capability::DenyReason),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub envelope: MessageEnvelope,
    pub request_id: RequestId,
    pub decision: PolicyVerdict,
    pub audit_entry_id: AuditEntryId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserDecision {
    Approved,
    Rejected(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserResponse {
    pub envelope: MessageEnvelope,
    pub approval_request_id: MessageId,
    pub decision: UserDecision,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub envelope: MessageEnvelope,
    pub in_response_to: MessageId,
    pub error: ProtocolError,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

impl Message {
    pub fn message_type(&self) -> MessageType {
        match self {
            Message::ActionPlan(_) => MessageType::ActionPlan,
            Message::VerificationReport(_) => MessageType::VerificationReport,
            Message::ToolRequest(_) => MessageType::ToolRequest,
            Message::ToolResult(_) => MessageType::ToolResult,
            Message::Event(_) => MessageType::Event,
            Message::Approval(_) => MessageType::Approval,
            Message::HealthReport(_) => MessageType::HealthReport,
            Message::GuardianDecision(_) => MessageType::GuardianDecision,
            Message::PolicyDecision(_) => MessageType::PolicyDecision,
            Message::ApprovalRequest(_) => MessageType::ApprovalRequest,
            Message::UserResponse(_) => MessageType::UserResponse,
            Message::ErrorResponse(_) => MessageType::ErrorResponse,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Capability, Clearance, Provenance, RiskLevel};

    fn test_token(principal: &PrincipalId) -> CapabilityToken {
        CapabilityToken {
            principal: principal.clone(),
            capability: Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Observe,
            },
            clearance: Clearance(RiskLevel::ReadOnly),
            granted_at: 1000,
            expires_at: 2000,
            provenance: Provenance {
                granted_by: PrincipalId::system("policy-broker"),
                package_id: "wifi.specialist".into(),
                package_version: 1,
                signature_verified: true,
            },
        }
    }

    #[test]
    fn envelope_round_trips_through_json() {
        let envelope = MessageEnvelope::new(
            MessageType::ToolRequest,
            PrincipalId::agent("planner", "p1"),
            uuid::Uuid::new_v4(),
            DataClassification::Protected,
        );
        let json = serde_json::to_string(&envelope).unwrap();
        let back: MessageEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope.message_type, back.message_type);
        assert_eq!(envelope.origin, back.origin);
        assert_eq!(envelope.message_id, back.message_id);
        assert_eq!(envelope.correlation_id, back.correlation_id);
        assert_eq!(envelope.data_classification, back.data_classification);
    }

    #[test]
    fn tool_request_defaults_are_safe() {
        let principal = PrincipalId::agent("planner", "p1");
        let token = test_token(&principal);
        let req = ToolRequest::new(
            principal.clone(),
            ResourceId("device:wifi0".into()),
            Operation::Observe,
            "wifi.observe_device",
            token,
            ToolParameters::Observe { fields: vec![] },
            uuid::Uuid::new_v4(),
            DataClassification::SystemConfig,
            3600,
        );
        assert_eq!(req.principal, principal);
        assert_eq!(req.nonce, 0);
        assert!(req.plan_hash.is_none());
        assert!(req.action_id.is_none());
        assert!(req.envelope.deadline.is_some());
        assert_eq!(req.envelope.message_type, MessageType::ToolRequest);
    }

    #[test]
    fn message_type_matches_variant() {
        let principal = PrincipalId::agent("planner", "p1");
        let token = test_token(&principal);
        let plan = ActionPlan {
            envelope: MessageEnvelope::new(
                MessageType::ActionPlan,
                principal,
                uuid::Uuid::new_v4(),
                DataClassification::Protected,
            ),
            plan_id: uuid::Uuid::new_v4(),
            user_intent: "test".into(),
            actions: vec![],
            affected_systems: vec![],
            expected_risks: vec![],
            rollback_state: None,
        };
        assert_eq!(
            Message::ActionPlan(plan.clone()).message_type(),
            MessageType::ActionPlan
        );
        let request = ToolRequest::new(
            crate::capability::PrincipalId::agent("planner", "p1"),
            ResourceId("device:wifi0".into()),
            Operation::Observe,
            "wifi.observe_device",
            token,
            ToolParameters::Observe { fields: vec![] },
            uuid::Uuid::new_v4(),
            DataClassification::SystemConfig,
            3600,
        );
        assert_eq!(
            Message::ToolRequest(request).message_type(),
            MessageType::ToolRequest
        );
    }

    #[test]
    fn approval_scope_contains_is_strict() {
        let scope = ApprovalScope {
            actions: vec![ApprovedAction {
                action_id: uuid::Uuid::new_v4(),
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Restart,
                tool_id: "wifi.restart".into(),
            }],
            resources: vec![ResourceId("device:wifi0".into())],
            operations: vec![Operation::Restart],
        };
        let action_id = scope.actions[0].action_id;
        let tool_id = "wifi.restart".to_string();
        assert!(scope.contains(
            &action_id,
            &ResourceId("device:wifi0".into()),
            &Operation::Restart,
            &tool_id
        ));
        assert!(!scope.contains(
            &action_id,
            &ResourceId("device:other".into()),
            &Operation::Restart,
            &tool_id
        ));
        assert!(!scope.contains(
            &action_id,
            &ResourceId("device:wifi0".into()),
            &Operation::Observe,
            &tool_id
        ));
        assert!(!scope.contains(
            &uuid::Uuid::new_v4(),
            &ResourceId("device:wifi0".into()),
            &Operation::Restart,
            &tool_id
        ));
    }

    #[test]
    fn approval_is_valid_only_inside_window() {
        let mut approval = Approval {
            envelope: MessageEnvelope::new(
                MessageType::Approval,
                PrincipalId::user(),
                uuid::Uuid::new_v4(),
                DataClassification::Protected,
            ),
            approval_id: uuid::Uuid::new_v4(),
            plan_id: uuid::Uuid::new_v4(),
            plan_hash: [0u8; 32],
            approved_by: PrincipalId::user(),
            granted_at: 100,
            expires_at: 200,
            scope: ApprovalScope {
                actions: vec![],
                resources: vec![],
                operations: vec![],
            },
        };
        assert!(!approval.is_valid_at(50));
        assert!(approval.is_valid_at(100));
        assert!(approval.is_valid_at(199));
        assert!(!approval.is_valid_at(200));
        approval.expires_at = 100;
        assert!(!approval.is_valid_at(100));
    }
}
