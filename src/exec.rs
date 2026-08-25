//! Exec specialist — `exec.run` (workspace co-partner full co-user).
//! Runs any shell command via PolicyBroker → Guardian (Staged) with audit.
//! No prefix cap — any command allowed, but gated as Staged so Guardian logs and health checks exit code.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{NodeId, NodeMetadata, NodeType, SystemGraph, TrustLevel};
use crate::protocol::{DataClassification, HealthState, MessageEnvelope, MessageType, ToolData, ToolStatus, ToolParameters};

pub const PACKAGE_ID: &str = "exec.specialist";
pub const EXEC_RESOURCE: &str = "exec:command";

pub struct ExecSpecialist {
    pub specialist: NodeId,
}

impl ExecSpecialist {
    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, String> {
        let id = format!("exec-{}", uuid::Uuid::new_v4());
        let node = NodeMetadata::new(
            NodeId(format!("exec:{id}").into()),
            NodeType::Service,
            crate::graph::ProvenanceSource::Discovered { via: "exec".into() },
            TrustLevel::Provisional,
            crate::protocol::now(),
        );
        let mut n = node;
        n.label = "exec".into();
        n.health = HealthState::Healthy;
        let nid = n.node_id.clone();
        graph.add_node(n).map_err(|e| e.to_string())?;
        Ok(Self { specialist: nid })
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        vec![crate::capability::ToolDefinition {
            tool_id: "exec.run".into(),
            specialist_package: PACKAGE_ID.into(),
            risk_level: RiskLevel::Staged,
            required_capabilities: vec![Capability { resource: ResourceId(EXEC_RESOURCE.into()), operation: Operation::Execute }],
            description: "run any shell command, capture stdout/stderr and exit code".into(),
        }]
    }

    pub fn handle_exec(&self, request: &crate::protocol::ToolRequest) -> crate::protocol::ToolResult {
        let cmd = match &request.parameters {
            ToolParameters::Execute { command } => command.clone(),
            _ => return err_result(request, "exec.run expects Execute { command }"),
        };
        if cmd.trim().is_empty() {
            return err_result(request, "empty command");
        }
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let status = out.status.code().unwrap_or(-1);
                let mut text = format!("exec exit={status} cmd={cmd:?}\nstdout:\n{stdout}\nstderr:\n{stderr}");
                if text.len() > 8000 { text.truncate(8000); text.push_str("\n...truncated"); }
                let data = serde_json::json!({ "command": cmd, "exit_code": status, "stdout": stdout, "stderr": stderr, "combined": text });
                success_result(request, ToolData::QueryResult { data })
            },
            Err(e) => err_result(request, &format!("exec failed: {e}")),
        }
    }
}

fn success_result(request: &crate::protocol::ToolRequest, data: ToolData) -> crate::protocol::ToolResult {
    crate::protocol::ToolResult {
        envelope: MessageEnvelope::new(MessageType::ToolResult, PrincipalId::system(PACKAGE_ID), request.envelope.correlation_id, DataClassification::Public),
        request_id: request.request_id,
        status: ToolStatus::Success,
        data: Some(data),
        error: None,
        health_impact: None,
    }
}
fn err_result(request: &crate::protocol::ToolRequest, msg: &str) -> crate::protocol::ToolResult {
    crate::protocol::ToolResult {
        envelope: MessageEnvelope::new(MessageType::ToolResult, PrincipalId::system(PACKAGE_ID), request.envelope.correlation_id, DataClassification::Public),
        request_id: request.request_id,
        status: ToolStatus::Failed,
        data: None,
        error: Some(crate::protocol::ToolError { code: crate::protocol::ToolErrorCode::Internal, message: msg.into(), recoverable: false }),
        health_impact: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Capability, ResourceId, Operation};
    use crate::protocol::{MessageEnvelope, MessageType, DataClassification, ToolParameters, ToolStatus};
    use crate::protocol::now;
    #[test]
    fn exec_runs_echo() {
        let mut g = SystemGraph::new();
        let sp = ExecSpecialist::instantiate(&mut g).unwrap();
        let req = crate::protocol::ToolRequest {
            envelope: MessageEnvelope::new(MessageType::ToolRequest, PrincipalId::system("test"), uuid::Uuid::new_v4(), DataClassification::SystemConfig),
            request_id: uuid::Uuid::new_v4(),
            principal: PrincipalId::system("test"),
            resource: ResourceId(EXEC_RESOURCE.into()),
            operation: Operation::Execute,
            tool_id: "exec.run".into(),
            capability_token: crate::capability::CapabilityToken {
                principal: PrincipalId::system("test"),
                capability: Capability { resource: ResourceId(EXEC_RESOURCE.into()), operation: Operation::Execute },
                clearance: crate::capability::Clearance(crate::capability::RiskLevel::Staged),
                granted_at: now(),
                expires_at: now()+100000,
                provenance: crate::capability::Provenance { granted_by: PrincipalId::system("policy-broker"), package_id: PACKAGE_ID.into(), package_version: 1, signature_verified: true },
            },
            parameters: ToolParameters::Execute { command: "echo hello-exec".into() },
            plan_hash: None,
            action_id: None,
            nonce: 0,
        };
        let r = sp.handle_exec(&req);
        assert_eq!(r.status, ToolStatus::Success);
        let data = r.data.unwrap();
        if let ToolData::QueryResult { data } = data {
            assert!(data.get("stdout").unwrap().as_str().unwrap().contains("hello-exec"));
        } else { panic!("wrong data"); }
    }
}
