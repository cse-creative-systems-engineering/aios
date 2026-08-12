use crate::action::ActionState;
use crate::capability::{
    Capability, CapabilityToken, Clearance, DenyReason, Operation, PrincipalId, Provenance,
    ResourceId, ResourceState, RiskLevel, ToolRegistry,
};
use crate::executor::{StagedExecutor, StagingError, StagingResult};
use crate::guardian::Guardian;
use crate::protocol::{
    Approval, DataClassification, MessageEnvelope, MessageType, PolicyDecision, PolicyVerdict,
    Timestamp, ToolError, ToolErrorCode, ToolRequest, ToolResult, ToolStatus, now,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

pub type SpecialistCall = (ToolRequest, oneshot::Sender<ToolResult>);

pub type SpecialistHandler = Arc<dyn Fn(ToolRequest) -> ToolResult + Send + Sync>;

#[derive(Debug)]
pub enum BrokerError {
    ChannelClosed(String),
    Internal(String),
}

impl std::fmt::Display for BrokerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub struct PolicyBroker {
    capabilities: HashMap<PrincipalId, Vec<CapabilityToken>>,
    clearances: HashMap<PrincipalId, Clearance>,
    tool_registry: ToolRegistry,
    revoked: HashSet<CapabilityToken>,
    approvals: HashMap<[u8; 32], Approval>,
    resource_states: HashMap<ResourceId, ResourceState>,
    resource_owners: HashMap<ResourceId, PrincipalId>,
    replay_log: HashSet<(PrincipalId, u64)>,
    guardian: Option<Box<dyn crate::capability::GuardianClient>>,
    audit_entries: Vec<PolicyDecision>,
    audit_broken: bool,
    clock: Box<dyn Fn() -> Timestamp + Send + Sync>,
}

impl PolicyBroker {
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
            clearances: HashMap::new(),
            tool_registry: ToolRegistry::new(),
            revoked: HashSet::new(),
            approvals: HashMap::new(),
            resource_states: HashMap::new(),
            resource_owners: HashMap::new(),
            replay_log: HashSet::new(),
            guardian: None,
            audit_entries: Vec::new(),
            audit_broken: false,
            clock: Box::new(now),
        }
    }

    pub fn register_tool(&mut self, definition: crate::capability::ToolDefinition) {
        self.tool_registry.register(definition);
    }

    pub fn register_principal(
        &mut self,
        principal: PrincipalId,
        capabilities: Vec<Capability>,
        clearance: Clearance,
    ) {
        let t = (self.clock)();
        let package_id = principal
            .package_id
            .clone()
            .unwrap_or_else(|| "user".to_string());
        let tokens = capabilities
            .into_iter()
            .map(|capability| CapabilityToken {
                principal: principal.clone(),
                capability,
                clearance,
                granted_at: t,
                expires_at: t + 10_000_000,
                provenance: Provenance {
                    granted_by: PrincipalId::system("policy-broker"),
                    package_id: package_id.clone(),
                    package_version: 1,
                    signature_verified: true,
                },
            })
            .collect();
        self.clearances.insert(principal.clone(), clearance);
        self.capabilities.insert(principal, tokens);
    }

    pub fn grant_capability(&mut self, principal: &PrincipalId, capability: Capability) {
        let t = (self.clock)();
        let package_id = principal
            .package_id
            .clone()
            .unwrap_or_else(|| "user".to_string());
        let clearance = self
            .clearances
            .get(principal)
            .copied()
            .unwrap_or(Clearance(RiskLevel::ReadOnly));
        let tokens = self.capabilities.entry(principal.clone()).or_default();
        tokens.push(CapabilityToken {
            principal: principal.clone(),
            capability,
            clearance,
            granted_at: t,
            expires_at: t + 10_000_000,
            provenance: Provenance {
                granted_by: PrincipalId::system("policy-broker"),
                package_id,
                package_version: 1,
                signature_verified: true,
            },
        });
    }

    pub fn set_resource_state(&mut self, resource: ResourceId, state: ResourceState) {
        self.resource_states.insert(resource, state);
    }

    pub fn set_resource_owner(&mut self, resource: ResourceId, owner: PrincipalId) {
        self.resource_owners.insert(resource, owner);
    }

    pub fn apply_resource_event(
        &mut self,
        from: &PrincipalId,
        resource: &ResourceId,
        state: ResourceState,
    ) -> Result<(), DenyReason> {
        let owner = self
            .resource_owners
            .get(resource)
            .ok_or(DenyReason::AmbiguousCapability)?;
        if owner != from {
            return Err(DenyReason::MissingCapability);
        }
        self.resource_states.insert(resource.clone(), state);
        Ok(())
    }

    pub fn add_approval(&mut self, approval: Approval) {
        self.approvals.insert(approval.plan_hash, approval);
    }

    pub fn set_guardian(&mut self, guardian: Guardian) {
        self.guardian = Some(Box::new(guardian));
    }

    pub fn set_audit_broken(&mut self, broken: bool) {
        self.audit_broken = broken;
    }

    pub fn set_clock(&mut self, clock: impl Fn() -> Timestamp + Send + Sync + 'static) {
        self.clock = Box::new(clock);
    }

    pub fn has_capability(&self, principal: &PrincipalId, capability: &Capability) -> bool {
        self.capabilities
            .get(principal)
            .map(|tokens| {
                tokens
                    .iter()
                    .any(|t| &t.capability == capability && &t.principal == principal)
            })
            .unwrap_or(false)
    }

    pub fn get_capabilities(&self, principal: &PrincipalId) -> Vec<Capability> {
        self.capabilities
            .get(principal)
            .map(|tokens| tokens.iter().map(|t| t.capability.clone()).collect())
            .unwrap_or_default()
    }

    pub fn capability_tokens(&self, principal: &PrincipalId) -> Vec<CapabilityToken> {
        self.capabilities
            .get(principal)
            .map(|tokens| tokens.clone())
            .unwrap_or_default()
    }

    pub fn get_clearance(&self, principal: &PrincipalId) -> Option<Clearance> {
        self.clearances.get(principal).copied()
    }

    pub fn audit_entries(&self) -> &[PolicyDecision] {
        &self.audit_entries
    }

    pub fn evaluate(&mut self, request: &ToolRequest) -> PolicyVerdict {
        let now = (self.clock)();

        let tool = match self.tool_registry.get(&request.tool_id) {
            Some(t) => t.clone(),
            None => return PolicyVerdict::Deny(DenyReason::UnknownTool),
        };
        let risk = tool.risk_level;

        let deadline = match request.envelope.deadline {
            Some(d) => d,
            None => return PolicyVerdict::Deny(DenyReason::MissingDeadline),
        };
        if now > deadline {
            return PolicyVerdict::Deny(DenyReason::RequestExpired);
        }

        if !self
            .replay_log
            .insert((request.principal.clone(), request.nonce))
        {
            return PolicyVerdict::Deny(DenyReason::ReplayDetected);
        }

        if !self.capabilities.contains_key(&request.principal) {
            return PolicyVerdict::Deny(DenyReason::UnknownPrincipal);
        }

        for required in &tool.required_capabilities {
            if !self.has_capability(&request.principal, required) {
                return PolicyVerdict::Deny(DenyReason::MissingCapability);
            }
        }

        match self.resource_states.get(&request.resource) {
            None => return PolicyVerdict::Deny(DenyReason::AmbiguousCapability),
            Some(ResourceState::Removed) => {
                return PolicyVerdict::Deny(DenyReason::ResourceUnavailable(ResourceState::Removed));
            }
            Some(ResourceState::Quarantined) => {
                if risk != RiskLevel::Recovery {
                    return PolicyVerdict::Deny(DenyReason::ResourceQuarantined);
                }
            }
            Some(ResourceState::Discovered) => {
                if !matches!(
                    request.operation,
                    Operation::Observe | Operation::Diagnose | Operation::Query
                ) {
                    return PolicyVerdict::Deny(DenyReason::AmbiguousCapability);
                }
            }
            Some(ResourceState::Available | ResourceState::Degraded) => {}
        }

        let token = &request.capability_token;
        if &token.principal != &request.principal {
            return PolicyVerdict::Deny(DenyReason::MissingCapability);
        }
        if !tool.required_capabilities.contains(&token.capability) {
            return PolicyVerdict::Deny(DenyReason::MissingCapability);
        }
        if &token.capability.resource != &request.resource
            || token.capability.operation != request.operation
        {
            return PolicyVerdict::Deny(DenyReason::MissingCapability);
        }
        if now > token.expires_at {
            return PolicyVerdict::Deny(DenyReason::ExpiredToken);
        }
        if self.revoked.contains(token) {
            return PolicyVerdict::Deny(DenyReason::RevokedToken);
        }

        let clearance = match self.clearances.get(&request.principal) {
            Some(c) => *c,
            None => return PolicyVerdict::Deny(DenyReason::UnknownPrincipal),
        };
        if clearance < Clearance(risk) {
            return PolicyVerdict::Deny(DenyReason::InsufficientClearance);
        }

        if risk.requires_guardian() {
            match &self.guardian {
                None => return PolicyVerdict::Deny(DenyReason::GuardianUnavailable),
                Some(g) => match g.review(request) {
                    crate::protocol::GuardianVerdict::Allow => {}
                    crate::protocol::GuardianVerdict::Block(reason) => {
                        return PolicyVerdict::Deny(DenyReason::GuardianBlocked(reason));
                    }
                },
            }
        }

        if risk.requires_approval() {
            let hash = match request.plan_hash {
                Some(h) => h,
                None => return PolicyVerdict::Deny(DenyReason::NoUserApproval),
            };
            let action_id = match request.action_id {
                Some(id) => id,
                None => return PolicyVerdict::Deny(DenyReason::NoUserApproval),
            };
            let approval = match self.approvals.get(&hash) {
                Some(a) => a,
                None => return PolicyVerdict::Deny(DenyReason::NoUserApproval),
            };
            if !approval.is_valid_at(now) {
                return PolicyVerdict::Deny(DenyReason::NoUserApproval);
            }
            if !approval.scope.contains(
                &action_id,
                &request.resource,
                &request.operation,
                &request.tool_id,
            ) {
                return PolicyVerdict::Deny(DenyReason::ApprovalScopeExceeded);
            }
        }

        let verdict = PolicyVerdict::Allow;

        if self.audit_broken {
            return PolicyVerdict::Deny(DenyReason::AuditLogFailure);
        }
        self.audit_entries.push(PolicyDecision {
            envelope: MessageEnvelope::new(
                MessageType::PolicyDecision,
                PrincipalId::system("policy-broker"),
                request.envelope.correlation_id,
                DataClassification::Protected,
            ),
            request_id: request.request_id,
            decision: verdict.clone(),
            audit_entry_id: uuid::Uuid::new_v4(),
        });

        verdict
    }
}

impl Default for PolicyBroker {
    fn default() -> Self {
        Self::new()
    }
}

pub trait BrokerClient {
    fn request_tool(&self, request: ToolRequest) -> Result<ToolResult, BrokerError>;
    fn get_capabilities(&self, principal: &PrincipalId) -> Vec<Capability>;
    fn get_clearance(&self, principal: &PrincipalId) -> Option<Clearance>;
    fn capability_tokens(&self, principal: &PrincipalId) -> Vec<CapabilityToken>;
}

pub struct Broker {
    core: Arc<Mutex<PolicyBroker>>,
    specialists: Arc<Mutex<HashMap<String, mpsc::Sender<SpecialistCall>>>>,
    executor: Arc<Mutex<Option<Arc<Mutex<StagedExecutor>>>>>,
    resource_locks: Arc<Mutex<HashMap<ResourceId, Arc<Mutex<()>>>>>,
    runtime: tokio::runtime::Runtime,
}

impl Broker {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("failed to build broker runtime");
        Self {
            core: Arc::new(Mutex::new(PolicyBroker::new())),
            specialists: Arc::new(Mutex::new(HashMap::new())),
            executor: Arc::new(Mutex::new(None)),
            resource_locks: Arc::new(Mutex::new(HashMap::new())),
            runtime,
        }
    }

    pub fn core(&self) -> &Arc<Mutex<PolicyBroker>> {
        &self.core
    }

    pub fn register_tool(&self, definition: crate::capability::ToolDefinition) {
        self.core.lock().expect("broker lock").register_tool(definition);
    }

    pub fn register_principal(
        &self,
        principal: PrincipalId,
        capabilities: Vec<Capability>,
        clearance: Clearance,
    ) {
        self.core
            .lock()
            .expect("broker lock")
            .register_principal(principal, capabilities, clearance);
    }

    pub fn set_resource_state(&self, resource: ResourceId, state: ResourceState) {
        self.core
            .lock()
            .expect("broker lock")
            .set_resource_state(resource, state);
    }

    pub fn set_resource_owner(&self, resource: ResourceId, owner: PrincipalId) {
        self.core
            .lock()
            .expect("broker lock")
            .set_resource_owner(resource, owner);
    }

    pub fn add_approval(&self, approval: Approval) {
        self.core
            .lock()
            .expect("broker lock")
            .add_approval(approval);
    }

    pub fn set_guardian(&self, guardian: Guardian) {
        self.core.lock().expect("broker lock").set_guardian(guardian);
    }

    pub fn set_executor(&self, executor: StagedExecutor) {
        *self.executor.lock().expect("executor lock") = Some(Arc::new(Mutex::new(executor)));
    }

    pub fn spawn_specialist(&self, tool_id: impl Into<String>, handler: SpecialistHandler) {
        let tool_id = tool_id.into();
        let (tx, mut rx) = mpsc::channel::<SpecialistCall>(16);
        self.runtime.spawn(async move {
            while let Some((request, reply)) = rx.recv().await {
                let result = handler(request);
                let _ = reply.send(result);
            }
        });
        self.specialists
            .lock()
            .expect("specialists lock")
            .insert(tool_id, tx);
    }

    pub fn client(&self, _principal: PrincipalId) -> LocalBroker {
        LocalBroker {
            core: self.core.clone(),
            specialists: self.specialists.clone(),
            executor: self.executor.clone(),
            resource_locks: self.resource_locks.clone(),
        }
    }
}

impl Default for Broker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub struct LocalBroker {
    core: Arc<Mutex<PolicyBroker>>,
    specialists: Arc<Mutex<HashMap<String, mpsc::Sender<SpecialistCall>>>>,
    executor: Arc<Mutex<Option<Arc<Mutex<StagedExecutor>>>>>,
    resource_locks: Arc<Mutex<HashMap<ResourceId, Arc<Mutex<()>>>>>,
}

impl LocalBroker {
    fn forward_to_specialist(&self, request: ToolRequest) -> Result<ToolResult, BrokerError> {
        let tx = self
            .specialists
            .lock()
            .map_err(|_| BrokerError::Internal("specialists lock poisoned".into()))?
            .get(&request.tool_id)
            .cloned()
            .ok_or_else(|| BrokerError::Internal(format!("no specialist for {}", request.tool_id)))?;
        let (reply_tx, reply_rx) = oneshot::channel();
        tx.blocking_send((request, reply_tx))
            .map_err(|_| BrokerError::ChannelClosed("specialist channel".into()))?;
        reply_rx
            .blocking_recv()
            .map_err(|_| BrokerError::ChannelClosed("specialist reply".into()))
    }

    fn run_staged(&self, request: ToolRequest) -> Result<ToolResult, BrokerError> {
        let executor = self
            .executor
            .lock()
            .map_err(|_| BrokerError::Internal("executor lock poisoned".into()))?
            .clone()
            .ok_or_else(|| BrokerError::Internal("no executor configured".into()))?;
        let mut ex = executor
            .lock()
            .map_err(|_| BrokerError::Internal("executor poisoned".into()))?;

        let risk = {
            let core = self
                .core
                .lock()
                .map_err(|_| BrokerError::Internal("broker lock poisoned".into()))?;
            match core.tool_registry.get(&request.tool_id) {
                Some(t) => t.risk_level,
                None => {
                    return Ok(ToolResult {
                        envelope: result_envelope(&request),
                        request_id: request.request_id,
                        status: ToolStatus::Failed,
                        data: None,
                        error: Some(ToolError {
                            code: ToolErrorCode::OperationNotSupported,
                            message: format!("tool {} unknown", request.tool_id),
                            recoverable: false,
                        }),
                        health_impact: None,
                    });
                }
            }
        };

        let steps: [(ActionState, &str); 5] = [
            (ActionState::ImpactAnalyzed, "impact analyzed"),
            (ActionState::Reviewed, "reviewed by planner and verifier"),
            (ActionState::PolicyValidated, "policy validated"),
            (ActionState::GuardianChecked, "guardian checked"),
            (ActionState::Approved, "approved by user"),
        ];
        let needed = if risk.requires_approval() {
            5
        } else if risk.requires_guardian() {
            4
        } else {
            3
        };

        let action_id = match ex.create_action(
            request.envelope.correlation_id,
            risk,
            request.resource.clone(),
            request.operation,
            request.principal.clone(),
        ) {
            Ok(id) => id,
            Err(e) => {
                return Ok(ToolResult {
                    envelope: result_envelope(&request),
                    request_id: request.request_id,
                    status: ToolStatus::Failed,
                    data: None,
                    error: Some(ToolError {
                        code: ToolErrorCode::Internal,
                        message: format!("create action failed: {e:?}"),
                        recoverable: false,
                    }),
                    health_impact: None,
                });
            }
        };

        for (i, (state, reason)) in steps.iter().enumerate() {
            if i >= needed {
                break;
            }
            if let Err(e) = ex.transition(&action_id, *state, reason) {
                return Ok(ToolResult {
                    envelope: result_envelope(&request),
                    request_id: request.request_id,
                    status: ToolStatus::Failed,
                    data: None,
                    error: Some(ToolError {
                        code: ToolErrorCode::Internal,
                        message: format!("transition failed: {e:?}"),
                        recoverable: false,
                    }),
                    health_impact: None,
                });
            }
        }

        match ex.stage_and_commit(&action_id) {
            Ok(StagingResult::Committed) => Ok(ToolResult {
                envelope: result_envelope(&request),
                request_id: request.request_id,
                status: ToolStatus::Success,
                data: Some(crate::protocol::ToolData::CommitResult {
                    committed: true,
                    health_verified: true,
                }),
                error: None,
                health_impact: None,
            }),
            Ok(StagingResult::RolledBack) => Ok(ToolResult {
                envelope: result_envelope(&request),
                request_id: request.request_id,
                status: ToolStatus::RolledBack,
                data: None,
                error: Some(ToolError {
                    code: ToolErrorCode::HealthCheckFailed,
                    message: "health check failed after staging, change rolled back".into(),
                    recoverable: true,
                }),
                health_impact: None,
            }),
            Err(e) => {
                let code = match e {
                    StagingError::CheckpointFailed | StagingError::StageFailed => {
                        ToolErrorCode::StagingFailed
                    }
                    StagingError::HealthCheckFailed => ToolErrorCode::HealthCheckFailed,
                    StagingError::CommitFailed | StagingError::RollbackFailed => {
                        ToolErrorCode::Internal
                    }
                };
                Ok(ToolResult {
                    envelope: result_envelope(&request),
                    request_id: request.request_id,
                    status: ToolStatus::Failed,
                    data: None,
                    error: Some(ToolError {
                        code,
                        message: format!("staging error: {e:?}"),
                        recoverable: false,
                    }),
                    health_impact: None,
                })
            }
        }
    }
}

impl BrokerClient for LocalBroker {
    fn request_tool(&self, request: ToolRequest) -> Result<ToolResult, BrokerError> {
        let lock = {
            let mut locks = self
                .resource_locks
                .lock()
                .map_err(|_| BrokerError::Internal("resource locks poisoned".into()))?;
            locks
                .entry(request.resource.clone())
                .or_insert_with(|| Arc::new(Mutex::new(())))
                .clone()
        };
        let _guard = lock
            .lock()
            .map_err(|_| BrokerError::Internal("resource lock poisoned".into()))?;

        let risk = {
            let core = self
                .core
                .lock()
                .map_err(|_| BrokerError::Internal("broker lock poisoned".into()))?;
            match core.tool_registry.get(&request.tool_id) {
                Some(t) => t.risk_level,
                None => {
                    return Ok(ToolResult {
                        envelope: result_envelope(&request),
                        request_id: request.request_id,
                        status: ToolStatus::Failed,
                        data: None,
                        error: Some(ToolError {
                            code: ToolErrorCode::OperationNotSupported,
                            message: format!("tool {} not registered", request.tool_id),
                            recoverable: false,
                        }),
                        health_impact: None,
                    });
                }
            }
        };

        let verdict = {
            let mut core = self
                .core
                .lock()
                .map_err(|_| BrokerError::Internal("broker lock poisoned".into()))?;
            core.evaluate(&request)
        };

        match verdict {
            PolicyVerdict::Deny(reason) => Ok(denied_result(&request, reason)),
            PolicyVerdict::Allow => {
                if risk.as_u8() <= 1 {
                    self.forward_to_specialist(request)
                } else {
                    self.run_staged(request)
                }
            }
        }
    }

    fn get_capabilities(&self, principal: &PrincipalId) -> Vec<Capability> {
        self.core
            .lock()
            .map(|core| core.get_capabilities(principal))
            .unwrap_or_default()
    }

    fn get_clearance(&self, principal: &PrincipalId) -> Option<Clearance> {
        self.core.lock().ok().and_then(|core| core.get_clearance(principal))
    }

    fn capability_tokens(&self, principal: &PrincipalId) -> Vec<CapabilityToken> {
        self.core
            .lock()
            .map(|core| core.capability_tokens(principal))
            .unwrap_or_default()
    }
}

fn result_envelope(request: &ToolRequest) -> MessageEnvelope {
    let mut e = MessageEnvelope::new(
        MessageType::ToolResult,
        PrincipalId::system("policy-broker"),
        request.envelope.correlation_id,
        request.envelope.data_classification,
    );
    e.timestamp = request.envelope.timestamp;
    e
}

pub fn denied_result(request: &ToolRequest, reason: DenyReason) -> ToolResult {
    ToolResult {
        envelope: result_envelope(request),
        request_id: request.request_id,
        status: ToolStatus::Denied,
        data: None,
        error: Some(ToolError {
            code: error_code_for(&reason),
            message: format!("denied: {reason}"),
            recoverable: false,
        }),
        health_impact: None,
    }
}

fn error_code_for(reason: &DenyReason) -> ToolErrorCode {
    match reason {
        DenyReason::GuardianBlocked(_) => ToolErrorCode::GuardianBlocked,
        DenyReason::StagingFailure => ToolErrorCode::StagingFailed,
        DenyReason::HealthCheckFailure => ToolErrorCode::HealthCheckFailed,
        _ => ToolErrorCode::CapabilityDenied,
    }
}

pub fn build_request(
    principal: PrincipalId,
    resource: ResourceId,
    operation: Operation,
    tool_id: impl Into<String>,
    token: &CapabilityToken,
    parameters: crate::protocol::ToolParameters,
    correlation_id: uuid::Uuid,
    nonce: u64,
) -> ToolRequest {
    let mut request = ToolRequest::new(
        principal,
        resource,
        operation,
        tool_id,
        token.clone(),
        parameters,
        correlation_id,
        DataClassification::SystemConfig,
        3600,
    );
    request.nonce = nonce;
    request
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::ToolDefinition;
    use crate::protocol::{ToolParameters};

    fn wifi_token(principal: &PrincipalId) -> CapabilityToken {
        CapabilityToken {
            principal: principal.clone(),
            capability: Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Observe,
            },
            clearance: Clearance(RiskLevel::ReadOnly),
            granted_at: 1000,
            expires_at: 5000,
            provenance: Provenance {
                granted_by: PrincipalId::system("policy-broker"),
                package_id: "wifi.specialist".into(),
                package_version: 1,
                signature_verified: true,
            },
        }
    }

    fn basic_broker() -> (PolicyBroker, PrincipalId, CapabilityToken) {
        let mut broker = PolicyBroker::new();
        broker.set_clock(|| 2000);
        broker.register_tool(ToolDefinition {
            tool_id: "wifi.observe_device".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Observe,
            }],
            description: "observe a wifi device".into(),
        });
        broker.register_tool(ToolDefinition {
            tool_id: "wifi.stage_driver".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::Staged,
            required_capabilities: vec![Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Stage,
            }],
            description: "stage a wifi driver".into(),
        });
        let principal = PrincipalId::agent("wifi.specialist", "wifi0-instance-001");
        let token = wifi_token(&principal);
        broker.set_resource_state(ResourceId("device:wifi0".into()), ResourceState::Available);
        broker.set_resource_owner(ResourceId("device:wifi0".into()), principal.clone());
        (broker, principal, token)
    }

    fn request_for(
        principal: &PrincipalId,
        token: &CapabilityToken,
        operation: Operation,
        tool_id: &str,
        parameters: ToolParameters,
        nonce: u64,
    ) -> ToolRequest {
        build_request(
            principal.clone(),
            ResourceId("device:wifi0".into()),
            operation,
            tool_id,
            token,
            parameters,
            uuid::Uuid::new_v4(),
            nonce,
        )
    }

    #[test]
    fn allows_with_valid_capability() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let req = request_for(
            &principal,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            1,
        );
        assert_eq!(broker.evaluate(&req), PolicyVerdict::Allow);
    }

    #[test]
    fn denies_without_capability() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(principal.clone(), vec![], Clearance(RiskLevel::ReadOnly));
        let req = request_for(
            &principal,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            1,
        );
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::MissingCapability)
        ));
    }

    #[test]
    fn denies_with_insufficient_clearance() {
        let (mut broker, principal, token) = basic_broker();
        let stage_token = CapabilityToken {
            capability: Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Stage,
            },
            ..token.clone()
        };
        broker.register_principal(
            principal.clone(),
            vec![stage_token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let req = request_for(
            &principal,
            &stage_token,
            Operation::Stage,
            "wifi.stage_driver",
            ToolParameters::Stage {
                change: serde_json::json!({}),
            },
            1,
        );
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::InsufficientClearance)
        ));
    }

    #[test]
    fn denies_guardian_block() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::KernelModule,
            }],
            Clearance(RiskLevel::Critical),
        );
        broker.register_tool(ToolDefinition {
            tool_id: "wifi.load_module".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::Critical,
            required_capabilities: vec![Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::KernelModule,
            }],
            description: "load a kernel module".into(),
        });
        let mut guardian = Guardian::new();
        guardian.add_rule(crate::guardian::InvariantRule {
            id: "TEST-BLOCK".into(),
            description: "block".into(),
            severity: crate::guardian::InvariantSeverity::Safety,
            check: crate::guardian::InvariantCheck::BlockOperation(Operation::KernelModule),
        });
        broker.set_guardian(guardian);
        let module_token = CapabilityToken {
            capability: Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::KernelModule,
            },
            ..token.clone()
        };
        let req = request_for(
            &principal,
            &module_token,
            Operation::KernelModule,
            "wifi.load_module",
            ToolParameters::KernelModule {
                action: "load".into(),
                module: "iwlwifi-next".into(),
            },
            1,
        );
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::GuardianBlocked(_))
        ));
    }

    #[test]
    fn denies_replay() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let req = request_for(
            &principal,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            7,
        );
        assert_eq!(broker.evaluate(&req), PolicyVerdict::Allow);
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::ReplayDetected)
        ));
    }

    #[test]
    fn denies_unknown_principal() {
        let (mut broker, _principal, token) = basic_broker();
        let stranger = PrincipalId::agent("storage.specialist", "nvme0");
        let req = request_for(
            &stranger,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            1,
        );
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::UnknownPrincipal)
        ));
    }

    #[test]
    fn denies_missing_deadline() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let mut req = request_for(
            &principal,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            1,
        );
        req.envelope.deadline = None;
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::MissingDeadline)
        ));
    }

    #[test]
    fn denies_expired_deadline() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let mut req = request_for(
            &principal,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            1,
        );
        req.envelope.deadline = Some(1000);
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::RequestExpired)
        ));
    }

    #[test]
    fn denies_resource_in_unknown_state() {
        let mut broker = PolicyBroker::new();
        broker.set_clock(|| 2000);
        broker.register_tool(ToolDefinition {
            tool_id: "wifi.observe_device".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Observe,
            }],
            description: "observe a wifi device".into(),
        });
        let principal = PrincipalId::agent("wifi.specialist", "wifi0");
        let token = wifi_token(&principal);
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let req = request_for(
            &principal,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            1,
        );
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::AmbiguousCapability)
        ));
    }

    #[test]
    fn rejects_non_owner_resource_state_claim() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let intruder = PrincipalId::agent("storage.specialist", "nvme0");
        let result = broker.apply_resource_event(
            &intruder,
            &ResourceId("device:wifi0".into()),
            ResourceState::Quarantined,
        );
        assert!(result.is_err());
        assert!(matches!(
            broker.resource_states.get(&ResourceId("device:wifi0".into())),
            Some(ResourceState::Available)
        ));
    }

    #[test]
    fn owner_can_update_resource_state() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        let result = broker.apply_resource_event(
            &principal,
            &ResourceId("device:wifi0".into()),
            ResourceState::Degraded,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn audit_broken_denies_everything() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![token.capability.clone()],
            Clearance(RiskLevel::ReadOnly),
        );
        broker.set_audit_broken(true);
        let req = request_for(
            &principal,
            &token,
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe { fields: vec![] },
            1,
        );
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::AuditLogFailure)
        ));
    }

    #[test]
    fn guardian_unavailable_denies_level2() {
        let (mut broker, principal, token) = basic_broker();
        broker.register_principal(
            principal.clone(),
            vec![Capability {
                resource: ResourceId("device:wifi0".into()),
                operation: Operation::Stage,
            }],
            Clearance(RiskLevel::Staged),
        );
        let mut token2 = token.clone();
        token2.capability = Capability {
            resource: ResourceId("device:wifi0".into()),
            operation: Operation::Stage,
        };
        let req = request_for(
            &principal,
            &token2,
            Operation::Stage,
            "wifi.stage_driver",
            ToolParameters::Stage {
                change: serde_json::json!({}),
            },
            1,
        );
        assert!(matches!(
            broker.evaluate(&req),
            PolicyVerdict::Deny(DenyReason::GuardianUnavailable)
        ));
    }
}
