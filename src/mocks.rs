use crate::action::{
    Checkpoint, CheckpointError, CheckpointState, CommitError, HealthError, RollbackError,
    StageError,
};
use crate::broker::{BrokerClient, LocalBroker, build_request};
use crate::capability::{
    Capability, CapabilityToken, Operation, PrincipalId, ResourceId, ResourceState,
};
use crate::executor::ResourceDriver;
use crate::protocol::{
    ActionPlan, DataClassification, HealthState, MessageEnvelope, MessageType, ToolData, ToolError,
    ToolErrorCode, ToolParameters, ToolRequest, ToolResult, ToolStatus, VerificationReport,
    VerificationVerdict, now,
};

fn ok_result(request: &ToolRequest, data: ToolData) -> ToolResult {
    ToolResult {
        envelope: MessageEnvelope::new(
            MessageType::ToolResult,
            PrincipalId::system("mock-specialist"),
            request.envelope.correlation_id,
            request.envelope.data_classification,
        ),
        request_id: request.request_id,
        status: ToolStatus::Success,
        data: Some(data),
        error: None,
        health_impact: None,
    }
}

fn err_result(request: &ToolRequest, code: ToolErrorCode, message: String) -> ToolResult {
    ToolResult {
        envelope: MessageEnvelope::new(
            MessageType::ToolResult,
            PrincipalId::system("mock-specialist"),
            request.envelope.correlation_id,
            request.envelope.data_classification,
        ),
        request_id: request.request_id,
        status: ToolStatus::Failed,
        data: None,
        error: Some(ToolError {
            code,
            message,
            recoverable: false,
        }),
        health_impact: None,
    }
}

pub fn wifi_specialist(request: ToolRequest) -> ToolResult {
    match &request.parameters {
        ToolParameters::Observe { fields } => {
            let mut metrics = std::collections::HashMap::new();
            for field in fields {
                let value = match field.as_str() {
                    "state" => "up".into(),
                    "link_rate" => "866 Mbps".into(),
                    "ssid" => "home-net".into(),
                    "signal_dbm" => "-45".into(),
                    _ => "unknown".into(),
                };
                metrics.insert(field.clone(), value);
            }
            ok_result(&request, ToolData::DeviceState {
                state: ResourceState::Available,
                metrics,
            })
        }
        ToolParameters::Diagnose { symptom } => ok_result(&request, ToolData::Diagnosis {
            findings: vec![
                format!("observed symptom: {symptom}"),
                "link quality fluctuating on channel 6".into(),
                "no driver error counters in the last hour".into(),
            ],
            confidence: 0.72,
        }),
        other => err_result(
            &request,
            ToolErrorCode::OperationNotSupported,
            format!("wifi specialist does not handle {other:?}"),
        ),
    }
}

pub fn storage_specialist(request: ToolRequest) -> ToolResult {
    match &request.parameters {
        ToolParameters::Observe { fields } => {
            let mut metrics = std::collections::HashMap::new();
            for field in fields {
                let value = match field.as_str() {
                    "temperature_c" => "38".into(),
                    "nvme_completions" => "145930021".into(),
                    "writes_gb" => "412".into(),
                    _ => "unknown".into(),
                };
                metrics.insert(field.clone(), value);
            }
            ok_result(&request, ToolData::DeviceState {
                state: ResourceState::Available,
                metrics,
            })
        }
        ToolParameters::Query { query } => ok_result(
            &request,
            ToolData::QueryResult {
                data: serde_json::json!({
                    "query": query,
                    "smart": {
                        "media_errors": 0,
                        "percent_life_used": 12,
                        "overall": "pass"
                    }
                }),
            },
        ),
        other => err_result(
            &request,
            ToolErrorCode::OperationNotSupported,
            format!("storage specialist does not handle {other:?}"),
        ),
    }
}

pub struct MockPlanner {
    broker: LocalBroker,
    principal: PrincipalId,
    tokens: Vec<CapabilityToken>,
    nonce: u64,
}

impl MockPlanner {
    pub fn new(broker: LocalBroker) -> Self {
        let principal = PrincipalId::agent("planner", "planner-001");
        let tokens = broker.capability_tokens(&principal);
        Self {
            broker,
            principal,
            tokens,
            nonce: 0,
        }
    }

    pub fn principal(&self) -> &PrincipalId {
        &self.principal
    }

    fn token_for(&self, capability: &Capability) -> Option<CapabilityToken> {
        self.tokens
            .iter()
            .find(|t| &t.capability == capability)
            .cloned()
    }

    fn request(
        &mut self,
        resource: ResourceId,
        operation: Operation,
        tool_id: &str,
        parameters: ToolParameters,
    ) -> Option<ToolRequest> {
        let capability = Capability {
            resource: resource.clone(),
            operation,
        };
        let token = self.token_for(&capability)?;
        self.nonce += 1;
        Some(build_request(
            self.principal.clone(),
            resource,
            operation,
            tool_id,
            &token,
            parameters,
            uuid::Uuid::new_v4(),
            self.nonce,
        ))
    }

    fn send(&mut self, request: Option<ToolRequest>) -> ToolResult {
        match request {
            None => ToolResult {
                envelope: MessageEnvelope::new(
                    MessageType::ToolResult,
                    self.principal.clone(),
                    uuid::Uuid::new_v4(),
                    DataClassification::SystemConfig,
                ),
                request_id: uuid::Uuid::new_v4(),
                status: ToolStatus::Denied,
                data: None,
                error: Some(ToolError {
                    code: ToolErrorCode::CapabilityDenied,
                    message: "planner holds no token for this tool".into(),
                    recoverable: false,
                }),
                health_impact: None,
            },
            Some(req) => match self.broker.request_tool(req.clone()) {
                Ok(result) => result,
                Err(e) => err_result(
                    &req,
                    ToolErrorCode::Internal,
                    format!("broker unreachable: {e:?}"),
                ),
            },
        }
    }

    pub fn observe_wifi(&mut self) -> ToolResult {
        let req = self.request(
            ResourceId("device:wifi0".into()),
            Operation::Observe,
            "wifi.observe_device",
            ToolParameters::Observe {
                fields: vec!["state".into(), "link_rate".into(), "ssid".into()],
            },
        );
        self.send(req)
    }

    pub fn diagnose_wifi(&mut self) -> ToolResult {
        let req = self.request(
            ResourceId("device:wifi0".into()),
            Operation::Diagnose,
            "wifi.diagnose_fault",
            ToolParameters::Diagnose {
                symptom: "intermittent drops".into(),
            },
        );
        self.send(req)
    }

    pub fn stage_driver(&mut self, module: &str) -> ToolResult {
        let req = self.request(
            ResourceId("device:wifi0".into()),
            Operation::Stage,
            "wifi.stage_driver",
            ToolParameters::Stage {
                change: serde_json::json!({ "module": module }),
            },
        );
        self.send(req)
    }

    pub fn query_storage(&mut self) -> ToolResult {
        let req = self.request(
            ResourceId("device:nvme0".into()),
            Operation::Query,
            "storage.check_smart",
            ToolParameters::Query {
                query: "smart status".into(),
            },
        );
        self.send(req)
    }
}

pub struct MockVerificationAgent;impl MockVerificationAgent {
    pub fn review(&self, plan: &ActionPlan) -> VerificationReport {
        VerificationReport {
            envelope: MessageEnvelope::new(
                MessageType::VerificationReport,
                PrincipalId::agent("verifier", "verifier-001"),
                plan.envelope.correlation_id,
                DataClassification::Protected,
            ),
            plan_id: plan.plan_id,
            verdict: VerificationVerdict::Approve,
            concerns: Vec::new(),
            missing_information: Vec::new(),
            recommended_tests: vec!["post-change health check".into()],
        }
    }
}

pub struct MockWifiDriver {
    pub module: String,
    pub version: String,
    pub health_ok: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl MockWifiDriver {
    pub fn new() -> Self {
        Self {
            module: "iwlwifi".into(),
            version: "1.0.0".into(),
            health_ok: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
        }
    }
}

impl Default for MockWifiDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceDriver for MockWifiDriver {
    fn create_checkpoint(
        &mut self,
        action_id: &crate::protocol::ActionId,
        resource: &ResourceId,
    ) -> Result<Checkpoint, CheckpointError> {
        Ok(Checkpoint {
            checkpoint_id: uuid::Uuid::new_v4(),
            action_id: *action_id,
            resource: resource.clone(),
            created_at: now(),
            state: CheckpointState::DriverBackup {
                module: self.module.clone(),
                version: self.version.clone(),
                backup_path: format!("/var/lib/aios/backups/{resource}.tar"),
            },
        })
    }

    fn verify_checkpoint(&self, _checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        Ok(())
    }

    fn stage(&mut self, _checkpoint: &Checkpoint) -> Result<(), StageError> {
        self.version = "9.9.9".into();
        Ok(())
    }

    fn health_check(&self, _resource: &ResourceId) -> Result<HealthState, HealthError> {
        Ok(if self
            .health_ok
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            HealthState::Healthy
        } else {
            HealthState::Unhealthy
        })
    }

    fn commit(&mut self, _checkpoint: &Checkpoint) -> Result<(), CommitError> {
        Ok(())
    }

    fn rollback(&mut self, _checkpoint: &Checkpoint) -> Result<(), RollbackError> {
        self.version = "1.0.0".into();
        Ok(())
    }
}
