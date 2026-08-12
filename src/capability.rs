use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

pub use crate::protocol::{PackageId, Timestamp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrincipalType {
    User,
    AgentInstance,
    SystemService,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrincipalId {
    pub r#type: PrincipalType,
    pub package_id: Option<PackageId>,
    pub instance_id: Option<String>,
}

impl PrincipalId {
    pub fn user() -> Self {
        Self {
            r#type: PrincipalType::User,
            package_id: None,
            instance_id: None,
        }
    }

    pub fn agent(package_id: impl Into<String>, instance_id: impl Into<String>) -> Self {
        Self {
            r#type: PrincipalType::AgentInstance,
            package_id: Some(package_id.into()),
            instance_id: Some(instance_id.into()),
        }
    }

    pub fn system(instance_id: impl Into<String>) -> Self {
        Self {
            r#type: PrincipalType::SystemService,
            package_id: None,
            instance_id: Some(instance_id.into()),
        }
    }
}

impl fmt::Display for PrincipalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.package_id, &self.instance_id) {
            (Some(pkg), Some(inst)) => write!(f, "{pkg}#{inst}"),
            (_, Some(inst)) => write!(f, "{inst}"),
            _ => write!(f, "user"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub String);

impl ResourceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceState {
    Discovered,
    Available,
    Degraded,
    Quarantined,
    Removed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl Operation {
    pub fn default_risk_level(self) -> RiskLevel {
        match self {
            Operation::Observe | Operation::Diagnose | Operation::Query => RiskLevel::ReadOnly,
            Operation::Restart | Operation::Configure => RiskLevel::Routine,
            Operation::Stage | Operation::Commit => RiskLevel::Staged,
            Operation::FirmwareWrite | Operation::BootConfig | Operation::KernelModule => {
                RiskLevel::Critical
            }
            Operation::Reset | Operation::Quarantine | Operation::Rollback => RiskLevel::Recovery,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum RiskLevel {
    ReadOnly,
    Routine,
    Staged,
    Critical,
    Recovery,
}

impl RiskLevel {
    pub fn as_u8(self) -> u8 {
        match self {
            RiskLevel::ReadOnly => 0,
            RiskLevel::Routine => 1,
            RiskLevel::Staged => 2,
            RiskLevel::Critical => 3,
            RiskLevel::Recovery => 4,
        }
    }

    pub fn requires_guardian(self) -> bool {
        self.as_u8() >= 2
    }

    pub fn requires_approval(self) -> bool {
        self.as_u8() >= 3
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Clearance(pub RiskLevel);

impl Clearance {
    pub fn max() -> Self {
        Clearance(RiskLevel::Recovery)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    pub resource: ResourceId,
    pub operation: Operation,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Provenance {
    pub granted_by: PrincipalId,
    pub package_id: PackageId,
    pub package_version: u32,
    pub signature_verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub principal: PrincipalId,
    pub capability: Capability,
    pub clearance: Clearance,
    pub granted_at: Timestamp,
    pub expires_at: Timestamp,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DenyReason {
    UnknownPrincipal,
    UnknownTool,
    MissingCapability,
    AmbiguousCapability,
    InsufficientClearance,
    GuardianBlocked(String),
    GuardianUnavailable,
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

impl fmt::Display for DenyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DenyReason::UnknownPrincipal => f.write_str("unknown principal"),
            DenyReason::UnknownTool => f.write_str("unknown tool"),
            DenyReason::MissingCapability => f.write_str("missing capability"),
            DenyReason::AmbiguousCapability => f.write_str("ambiguous capability"),
            DenyReason::InsufficientClearance => f.write_str("insufficient clearance"),
            DenyReason::GuardianBlocked(rule) => write!(f, "blocked by guardian: {rule}"),
            DenyReason::GuardianUnavailable => f.write_str("guardian unavailable"),
            DenyReason::NoUserApproval => f.write_str("no user approval"),
            DenyReason::PlanHashMismatch => f.write_str("plan hash mismatch"),
            DenyReason::ApprovalScopeExceeded => f.write_str("approval scope exceeded"),
            DenyReason::ResourceUnavailable(state) => {
                write!(f, "resource unavailable: {state:?}")
            }
            DenyReason::ResourceQuarantined => f.write_str("resource quarantined"),
            DenyReason::AuditLogFailure => f.write_str("audit log failure"),
            DenyReason::ExpiredToken => f.write_str("expired token"),
            DenyReason::RevokedToken => f.write_str("revoked token"),
            DenyReason::RequestExpired => f.write_str("request expired"),
            DenyReason::MissingDeadline => f.write_str("missing deadline"),
            DenyReason::ReplayDetected => f.write_str("replay detected"),
            DenyReason::StagingFailure => f.write_str("staging failure"),
            DenyReason::HealthCheckFailure => f.write_str("health check failure"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolDefinition {
    pub tool_id: crate::protocol::ToolId,
    pub specialist_package: PackageId,
    pub risk_level: RiskLevel,
    pub required_capabilities: Vec<Capability>,
    pub description: String,
}

#[derive(Clone, Debug, Default)]
pub struct ToolRegistry {
    pub tools: HashMap<crate::protocol::ToolId, ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, definition: ToolDefinition) {
        let id = definition.tool_id.clone();
        if self.tools.insert(id.clone(), definition).is_some() {
            panic!("tool {id} registered twice");
        }
    }

    pub fn get(&self, tool_id: &str) -> Option<&ToolDefinition> {
        self.tools.get(tool_id)
    }
}

pub trait GuardianClient {
    fn review(&self, request: &crate::protocol::ToolRequest)
        -> crate::protocol::GuardianVerdict;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_risk_mapping() {
        assert_eq!(Operation::Observe.default_risk_level(), RiskLevel::ReadOnly);
        assert_eq!(Operation::Diagnose.default_risk_level(), RiskLevel::ReadOnly);
        assert_eq!(Operation::Query.default_risk_level(), RiskLevel::ReadOnly);
        assert_eq!(Operation::Restart.default_risk_level(), RiskLevel::Routine);
        assert_eq!(Operation::Configure.default_risk_level(), RiskLevel::Routine);
        assert_eq!(Operation::Stage.default_risk_level(), RiskLevel::Staged);
        assert_eq!(Operation::Commit.default_risk_level(), RiskLevel::Staged);
        assert_eq!(Operation::FirmwareWrite.default_risk_level(), RiskLevel::Critical);
        assert_eq!(Operation::BootConfig.default_risk_level(), RiskLevel::Critical);
        assert_eq!(Operation::KernelModule.default_risk_level(), RiskLevel::Critical);
        assert_eq!(Operation::Reset.default_risk_level(), RiskLevel::Recovery);
        assert_eq!(Operation::Quarantine.default_risk_level(), RiskLevel::Recovery);
        assert_eq!(Operation::Rollback.default_risk_level(), RiskLevel::Recovery);
    }

    #[test]
    fn risk_level_gates_and_ordering() {
        assert_eq!(RiskLevel::ReadOnly.as_u8(), 0);
        assert_eq!(RiskLevel::Recovery.as_u8(), 4);
        assert!(!RiskLevel::Routine.requires_guardian());
        assert!(RiskLevel::Staged.requires_guardian());
        assert!(RiskLevel::Critical.requires_guardian());
        assert!(!RiskLevel::Routine.requires_approval());
        assert!(RiskLevel::Critical.requires_approval());
        assert!(RiskLevel::Recovery.requires_approval());
        assert!(RiskLevel::ReadOnly < RiskLevel::Recovery);
        assert!(Clearance(RiskLevel::ReadOnly) < Clearance(RiskLevel::Staged));
        assert_eq!(Clearance::max(), Clearance(RiskLevel::Recovery));
    }

    #[test]
    fn principal_display_forms() {
        assert_eq!(
            PrincipalId::agent("wifi.specialist", "wifi0").to_string(),
            "wifi.specialist#wifi0"
        );
        assert_eq!(PrincipalId::system("guardian").to_string(), "guardian");
        assert_eq!(PrincipalId::user().to_string(), "user");
    }

    #[test]
    fn tool_registry_round_trips() {
        let mut registry = ToolRegistry::new();
        let definition = ToolDefinition {
            tool_id: "wifi.observe_device".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Observe,
            }],
            description: "observe".into(),
        };
        registry.register(definition);
        let got = registry.get("wifi.observe_device").expect("registered");
        assert_eq!(got.risk_level, RiskLevel::ReadOnly);
        assert_eq!(registry.get("wifi.missing"), None);
    }

    #[test]
    #[should_panic(expected = "registered twice")]
    fn tool_registry_rejects_duplicates() {
        let mut registry = ToolRegistry::new();
        registry.register(ToolDefinition {
            tool_id: "wifi.observe_device".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![],
            description: String::new(),
        });
        registry.register(ToolDefinition {
            tool_id: "wifi.observe_device".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![],
            description: String::new(),
        });
    }
}
