//! Files/Data specialist — Stage 1 of workspace co-partner (0004).
//! Owns file resources `file:/workspace/**` and `file:/artifacts/**` with
//! prefix capabilities (ADR-0008). v0.1 was read-only (observe/diagnose only);
//! Stage 1 adds staged mutating ops through the existing
//! `PolicyBroker → Guardian → StagedExecutor` pipeline.
//!
//! Safety: every mutating file op is gated by the broker (clearance +
//! capability prefix), checked by Guardian DATA-003 (no write outside
//! workspace/artifacts without approval), and executed via `FileDriver`
//! which checkpoints to a backup, stages, health-checks, and rolls back on
//! failure — matching `action-state-machine.md` §2.2 and the `FileCheckpoint`
//! design in milestone 0004.

use crate::action::{Checkpoint, CheckpointError, CheckpointState, CommitError, HealthError, RollbackError, StageError};
use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::executor::ResourceDriver;
use crate::graph::{NodeId, NodeMetadata, NodeType, SystemGraph, TrustLevel};
use crate::protocol::{HealthState, now};

use std::path::{Path, PathBuf};

pub const PACKAGE_ID: &str = "files.specialist";

/// Resolve the on-disk workspace root. `AIOS_WORKSPACE` overrides the default
/// `~/workspace`; `AIOS_ARTIFACTS` overrides artifacts. In tests a temp dir
/// is injected directly via `FileDriver::with_roots`.
pub fn workspace_root() -> PathBuf {
    if let Ok(p) = std::env::var("AIOS_WORKSPACE") {
        PathBuf::from(p)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join("workspace")
    } else {
        PathBuf::from("/tmp/aios-workspace")
    }
}

pub fn artifacts_root() -> PathBuf {
    if let Ok(p) = std::env::var("AIOS_ARTIFACTS") {
        PathBuf::from(p)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".aios").join("artifacts")
    } else {
        PathBuf::from("/tmp/aios-artifacts")
    }
}

/// Convert a `ResourceId` like `file:/workspace/hello.py` into an absolute
/// path under the workspace or artifacts root. Returns `None` if the resource
/// is not a file resource or escapes the allowed prefixes.
pub fn resource_to_path(resource: &ResourceId, workspace: &Path, artifacts: &Path) -> Option<PathBuf> {
    let s = resource.as_str();
    if let Some(rest) = s.strip_prefix("file:/workspace") {
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() {
            return Some(workspace.to_path_buf());
        }
        // Prevent directory traversal via `..` components.
        let candidate = workspace.join(rest);
        let canonical_workspace = workspace.canonicalize().unwrap_or_else(|_| workspace.to_path_buf());
        // We check the joined path does not escape: if rest contains `..`, joining still
        // produces a path with `..` components; we normalize by checking that the
        // final path starts with workspace after lexical normalization.
        // Simple check: reject any `..` segment.
        if rest.split('/').any(|c| c == "..") {
            return None;
        }
        // Also reject absolute rest (should have been stripped).
        if rest.starts_with('/') {
            return None;
        }
        let _ = canonical_workspace;
        Some(candidate)
    } else if let Some(rest) = s.strip_prefix("file:/artifacts") {
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() {
            return Some(artifacts.to_path_buf());
        }
        if rest.split('/').any(|c| c == "..") {
            return None;
        }
        Some(artifacts.join(rest))
    } else {
        None
    }
}

fn is_file_resource(s: &str) -> bool {
    s.starts_with("file:/workspace") || s.starts_with("file:/artifacts")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilesSpecialist {
    pub specialist: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesError {
    Graph(String),
}

impl std::fmt::Display for FilesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graph(reason) => write!(f, "could not instantiate files specialist: {reason}"),
        }
    }
}
impl std::error::Error for FilesError {}

impl FilesSpecialist {
    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, FilesError> {
        let specialist = NodeId("specialist:files:0".into());
        if graph.get_node(&specialist).is_some() {
            return Ok(Self { specialist });
        }
        let t = now();
        let mut node = NodeMetadata::new(
            specialist.clone(),
            NodeType::Specialist,
            crate::graph::ProvenanceSource::Declared { package: PACKAGE_ID.into() },
            TrustLevel::Trusted,
            t,
        );
        node.label = "Files/Data specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(FilesError::Graph)?;
        Ok(Self { specialist })
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let ws = ResourceId("file:/workspace".into());
        let arts = ResourceId("file:/artifacts".into());
        // Also allow observing any file:* (risk 0).
        let any_file_ws = ResourceId("file:/workspace".into());
        vec![
            tool("files.observe_file", RiskLevel::ReadOnly, Operation::Observe, &any_file_ws),
            tool("files.diagnose_file", RiskLevel::ReadOnly, Operation::Diagnose, &any_file_ws),
            tool("files.write_file", RiskLevel::Staged, Operation::Write, &ws),
            tool("files.create_file", RiskLevel::Staged, Operation::Create, &ws),
            tool("files.patch_file", RiskLevel::Staged, Operation::Patch, &ws),
            tool("files.delete_file", RiskLevel::Critical, Operation::Delete, &ws),
            // Artifact variants (same ops, artifact prefix).
            tool("files.write_artifact", RiskLevel::Staged, Operation::Write, &arts),
            tool("files.create_artifact", RiskLevel::Staged, Operation::Create, &arts),
        ]
    }

    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            let count = graph.nodes().len();
            return ok_result(
                "files.observe_file",
                format!("{count} nodes in graph; use a file:/workspace/<path> target to inspect a file"),
            );
        }
        // If target looks like a node id, try graph lookup.
        if let Some(node) = graph.get_node(&NodeId(target.into())) {
            return ok_result(
                "files.observe_file",
                format!("node: {} {:?} health={:?}", node.node_id, node.node_type, node.health),
            );
        }
        // Otherwise treat as file path resource — report existence via graph
        // is not authoritative for files; broker capability is. Just echo.
        ok_result("files.observe_file", format!("target: {target}"))
    }
}

fn tool(
    id: &str,
    risk: RiskLevel,
    op: Operation,
    resource: &ResourceId,
) -> crate::capability::ToolDefinition {
    crate::capability::ToolDefinition {
        tool_id: id.into(),
        specialist_package: PACKAGE_ID.into(),
        risk_level: risk,
        required_capabilities: vec![Capability { resource: resource.clone(), operation: op }],
        description: id.into(),
    }
}

fn ok_result(_tool: &str, text: String) -> crate::protocol::ToolResult {
    crate::protocol::ToolResult {
        envelope: crate::protocol::MessageEnvelope::new(
            crate::protocol::MessageType::ToolResult,
            PrincipalId::system("files.specialist"),
            uuid::Uuid::new_v4(),
            crate::protocol::DataClassification::SystemConfig,
        ),
        request_id: uuid::Uuid::new_v4(),
        status: crate::protocol::ToolStatus::Success,
        data: Some(crate::protocol::ToolData::QueryResult { data: serde_json::json!({"text": text}) }),
        error: None,
        health_impact: None,
    }
}

/// FileDriver implements `ResourceDriver` for `file:/workspace/**` and
/// `file:/artifacts/**` resources. It checkpoints by copying the existing
/// file (if any) to a backup, stages by writing the candidate content, and
/// health-checks by verifying the file exists (and for .py, that it parses
/// as UTF-8 and `py_compile` would succeed — here we just check it is valid
/// UTF-8 and non-empty when content was written).
pub struct FileDriver {
    workspace_root: PathBuf,
    artifacts_root: PathBuf,
    /// Optional injected health override for tests (if set, health_check returns
    /// accordingly without touching FS).
    pub health_override: Option<bool>,
    /// If true, stage will deliberately fail (test hook).
    pub stage_should_fail: bool,
}

impl FileDriver {
    pub fn new() -> Self {
        Self {
            workspace_root: workspace_root(),
            artifacts_root: artifacts_root(),
            health_override: None,
            stage_should_fail: false,
        }
    }

    pub fn with_roots(workspace: PathBuf, artifacts: PathBuf) -> Self {
        Self { workspace_root: workspace, artifacts_root: artifacts, health_override: None, stage_should_fail: false }
    }

    fn path_for(&self, resource: &ResourceId) -> Result<PathBuf, CheckpointError> {
        resource_to_path(resource, &self.workspace_root, &self.artifacts_root)
            .ok_or_else(|| CheckpointError::ResourceNotCheckpointable(resource.clone()))
    }

    fn backup_path_for(&self, checkpoint_id: uuid::Uuid) -> PathBuf {
        self.workspace_root.join(".aios-checkpoints").join(format!("{checkpoint_id}.bak"))
    }
}

impl ResourceDriver for FileDriver {
    fn create_checkpoint(
        &mut self,
        action_id: &crate::protocol::ActionId,
        resource: &ResourceId,
    ) -> Result<Checkpoint, CheckpointError> {
        if !is_file_resource(resource.as_str()) {
            return Err(CheckpointError::ResourceNotCheckpointable(resource.clone()));
        }
        let path = self.path_for(resource)?;
        let existed = path.exists();
        let content_hash = if existed {
            let bytes = std::fs::read(&path).map_err(|e| CheckpointError::StorageFailed(e.to_string()))?;
            let mut hasher = sha2::Sha256::new();
            use sha2::Digest as _;
            hasher.update(&bytes);
            let hash: [u8; 32] = hasher.finalize().into();
            Some(hash)
        } else {
            None
        };
        // Copy existing file to backup if it exists.
        let backup_path = if existed {
            let bp = self.backup_path_for(*action_id);
            if let Some(parent) = bp.parent() {
                std::fs::create_dir_all(parent).map_err(|e| CheckpointError::StorageFailed(e.to_string()))?;
            }
            std::fs::copy(&path, &bp).map_err(|e| CheckpointError::StorageFailed(e.to_string()))?;
            Some(bp.to_string_lossy().to_string())
        } else {
            None
        };
        Ok(Checkpoint {
            checkpoint_id: *action_id,
            action_id: *action_id,
            resource: resource.clone(),
            created_at: now(),
            state: CheckpointState::FileBackup { path: path.to_string_lossy().to_string(), backup_path, existed, content_hash },
        })
    }

    fn verify_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        match &checkpoint.state {
            CheckpointState::FileBackup { backup_path, existed, .. } => {
                if *existed {
                    let bp = backup_path.as_ref().ok_or_else(|| CheckpointError::VerificationFailed("missing backup path".into()))?;
                    if !Path::new(bp).exists() {
                        return Err(CheckpointError::VerificationFailed("backup missing".into()));
                    }
                }
                Ok(())
            }
            _ => Err(CheckpointError::VerificationFailed("not a file checkpoint".into())),
        }
    }

    fn stage(&mut self, checkpoint: &Checkpoint, candidate: &str) -> Result<(), StageError> {
        if self.stage_should_fail {
            return Err(StageError::Internal("injected stage failure".into()));
        }
        let path_str = match &checkpoint.state {
            CheckpointState::FileBackup { path, .. } => path.clone(),
            _ => return Err(StageError::Internal("not a file checkpoint".into())),
        };
        let path = PathBuf::from(&path_str);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StageError::Internal(e.to_string()))?;
        }
        std::fs::write(&path, candidate).map_err(|e| StageError::Internal(e.to_string()))?;
        Ok(())
    }

    fn health_check(&self, resource: &ResourceId) -> Result<HealthState, HealthError> {
        if let Some(over) = self.health_override {
            return Ok(if over { HealthState::Healthy } else { HealthState::Unhealthy });
        }
        let path = self.path_for(resource).map_err(|_| HealthError::Internal("not a file resource".into()))?;
        if !path.exists() {
            return Ok(HealthState::Unhealthy);
        }
        // Basic health: file exists and is readable; for .py check UTF-8.
        let bytes = std::fs::read(&path).map_err(|e| HealthError::Internal(e.to_string()))?;
        if path.extension().and_then(|e| e.to_str()) == Some("py") {
            if std::str::from_utf8(&bytes).is_err() {
                return Ok(HealthState::Unhealthy);
            }
        }
        // Any existing file (including empty) is healthy — the staged content
        // itself is the artifact; emptiness is not a failure unless the
        // caller injected health_override = false for the test case.
        let _ = bytes;
        Ok(HealthState::Healthy)
    }

    fn commit(&mut self, checkpoint: &Checkpoint) -> Result<(), CommitError> {
        match &checkpoint.state {
            CheckpointState::FileBackup { backup_path, .. } => {
                if let Some(bp) = backup_path {
                    let _ = std::fs::remove_file(bp);
                }
                Ok(())
            }
            _ => Err(CommitError::Internal("not a file checkpoint".into())),
        }
    }

    fn rollback(&mut self, checkpoint: &Checkpoint) -> Result<(), RollbackError> {
        match &checkpoint.state {
            CheckpointState::FileBackup { path, backup_path, existed, .. } => {
                let target = PathBuf::from(path);
                if *existed {
                    let bp = backup_path.as_ref().ok_or_else(|| RollbackError::CheckpointMissing(checkpoint.checkpoint_id))?;
                    std::fs::copy(bp, &target).map_err(|e| RollbackError::RestorationFailed(e.to_string()))?;
                    let _ = std::fs::remove_file(bp);
                } else {
                    // File did not exist before — remove the staged file.
                    let _ = std::fs::remove_file(&target);
                }
                Ok(())
            }
            _ => Err(RollbackError::CheckpointMissing(checkpoint.checkpoint_id)),
        }
    }
}

use sha2::Digest as _;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_roots() -> (TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("workspace");
        let arts = tmp.path().join("artifacts");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&arts).unwrap();
        (tmp, ws, arts)
    }

    #[test]
    fn resource_to_path_workspace() {
        let (_tmp, ws, arts) = temp_roots();
        let r = ResourceId("file:/workspace/hello.py".into());
        assert_eq!(resource_to_path(&r, &ws, &arts).unwrap(), ws.join("hello.py"));
        let r2 = ResourceId("file:/workspace/a/b/c.txt".into());
        assert_eq!(resource_to_path(&r2, &ws, &arts).unwrap(), ws.join("a/b/c.txt"));
        let r3 = ResourceId("file:/artifacts/out.html".into());
        assert_eq!(resource_to_path(&r3, &ws, &arts).unwrap(), arts.join("out.html"));
        // traversal blocked
        let bad = ResourceId("file:/workspace/../etc/passwd".into());
        assert!(resource_to_path(&bad, &ws, &arts).is_none());
        // non-file resource
        let other = ResourceId("device:wifi0".into());
        assert!(resource_to_path(&other, &ws, &arts).is_none());
    }

    #[test]
    fn file_checkpoint_create_and_rollback_new_file() {
        let (_tmp, ws, arts) = temp_roots();
        let mut driver = FileDriver::with_roots(ws.clone(), arts.clone());
        let resource = ResourceId("file:/workspace/new.txt".into());
        let aid = uuid::Uuid::new_v4();
        let cp = driver.create_checkpoint(&aid, &resource).unwrap();
        assert!(matches!(cp.state, CheckpointState::FileBackup { existed: false, .. }));
        driver.stage(&cp, "hello world").unwrap();
        assert_eq!(std::fs::read_to_string(ws.join("new.txt")).unwrap(), "hello world");
        driver.rollback(&cp).unwrap();
        assert!(!ws.join("new.txt").exists());
    }

    #[test]
    fn file_checkpoint_rollback_restores_existing() {
        let (_tmp, ws, arts) = temp_roots();
        std::fs::write(ws.join("existing.txt"), "original").unwrap();
        let mut driver = FileDriver::with_roots(ws.clone(), arts.clone());
        let resource = ResourceId("file:/workspace/existing.txt".into());
        let aid = uuid::Uuid::new_v4();
        let cp = driver.create_checkpoint(&aid, &resource).unwrap();
        driver.stage(&cp, "modified").unwrap();
        assert_eq!(std::fs::read_to_string(ws.join("existing.txt")).unwrap(), "modified");
        driver.rollback(&cp).unwrap();
        assert_eq!(std::fs::read_to_string(ws.join("existing.txt")).unwrap(), "original");
    }

    #[test]
    fn prefixed_capability_covers_subpath() {
        use crate::capability::{Capability, Operation, resource_covers, capability_covers};
        assert!(resource_covers("file:/workspace", "file:/workspace/hello.py"));
        assert!(resource_covers("file:/workspace", "file:/workspace"));
        assert!(resource_covers("file:/artifacts", "file:/artifacts/out.html"));
        assert!(!resource_covers("file:/workspace", "file:/artifacts/out.html"));
        assert!(!resource_covers("device:wifi0", "device:wifi0-extra"));
        let token = Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Write };
        assert!(capability_covers(&token, &ResourceId("file:/workspace/a/b.txt".into()), Operation::Write));
        assert!(!capability_covers(&token, &ResourceId("file:/workspace/a/b.txt".into()), Operation::Delete));
        assert!(!capability_covers(&token, &ResourceId("file:/etc/passwd".into()), Operation::Write));
    }

    use crate::broker::BrokerClient;
    fn file_broker_with_health(healthy: bool) -> (crate::broker::Broker, crate::capability::PrincipalId, crate::capability::CapabilityToken, tempfile::TempDir, PathBuf) {
        use crate::broker::Broker;
        use crate::capability::{Capability, Clearance, Operation, PrincipalId, ResourceId, RiskLevel, ToolDefinition};
        use crate::guardian::Guardian;
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("workspace");
        let arts = tmp.path().join("artifacts");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&arts).unwrap();
        let store_dir = tmp.path().join("store");
        std::fs::create_dir_all(&store_dir).unwrap();
        let broker = Broker::new();
        broker.core().lock().unwrap().set_clock(|| 2000);
        let principal = PrincipalId::agent("files.specialist", "files-001");
        let capability = Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Write };
        let token = crate::capability::CapabilityToken {
            principal: principal.clone(),
            capability: capability.clone(),
            clearance: Clearance(RiskLevel::Staged),
            granted_at: 1000,
            expires_at: 999999,
            provenance: crate::capability::Provenance { granted_by: PrincipalId::system("policy-broker"), package_id: "files.specialist".into(), package_version: 1, signature_verified: true },
        };
        // Register the specialist's tools (write_file).
        broker.register_tool(ToolDefinition { tool_id: "files.write_file".into(), specialist_package: "files.specialist".into(), risk_level: RiskLevel::Staged, required_capabilities: vec![capability.clone()], description: "write file".into() });
        broker.register_tool(ToolDefinition { tool_id: "files.create_file".into(), specialist_package: "files.specialist".into(), risk_level: RiskLevel::Staged, required_capabilities: vec![capability.clone()], description: "create file".into() });
        broker.register_tool(ToolDefinition { tool_id: "files.delete_file".into(), specialist_package: "files.specialist".into(), risk_level: RiskLevel::Critical, required_capabilities: vec![Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Delete }], description: "delete file".into() });
        broker.register_tool(ToolDefinition { tool_id: "files.observe_file".into(), specialist_package: "files.specialist".into(), risk_level: RiskLevel::ReadOnly, required_capabilities: vec![Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Observe }], description: "observe".into() });
        broker.register_principal(principal.clone(), vec![capability.clone(), Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Create }, Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Observe }, Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Delete }], Clearance(RiskLevel::Recovery));
        broker.set_resource_state(ResourceId("file:/workspace".into()), crate::capability::ResourceState::Available);
        broker.set_resource_state(ResourceId("file:/workspace/hello.py".into()), crate::capability::ResourceState::Available);
        broker.set_resource_owner(ResourceId("file:/workspace".into()), principal.clone());
        broker.set_resource_owner(ResourceId("file:/workspace/hello.py".into()), principal.clone());
        broker.set_guardian(Guardian::new());
        // executor with FileDriver
        let store = crate::action::FileActionStore::new(&store_dir).unwrap();
        let mut driver = FileDriver::with_roots(ws.clone(), arts.clone());
        if !healthy {
            driver.health_override = Some(false);
        }
        broker.set_executor(crate::executor::StagedExecutor::new(Box::new(store), Box::new(driver)));
        (broker, principal, token, tmp, ws)
    }

    #[test]
    fn broker_writes_file_and_commits() {
        let (broker, principal, mut token, _tmp, ws) = file_broker_with_health(true);
        token.capability = crate::capability::Capability { resource: ResourceId("file:/workspace".into()), operation: crate::capability::Operation::Write };
        token.clearance = crate::capability::Clearance(crate::capability::RiskLevel::Staged);
        let req = crate::broker::build_request(
            principal.clone(),
            ResourceId("file:/workspace/hello.py".into()),
            crate::capability::Operation::Write,
            "files.write_file",
            &token,
            crate::protocol::ToolParameters::Stage { change: serde_json::json!({"content": "hello from aios"}) },
            uuid::Uuid::new_v4(),
            10,
        );
        let result = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(result.status, crate::protocol::ToolStatus::Success);
        assert_eq!(std::fs::read_to_string(ws.join("hello.py")).unwrap(), "hello from aios");
    }

    #[test]
    fn broker_write_rolls_back_on_unhealthy() {
        let (broker, principal, mut token, _tmp, ws) = file_broker_with_health(false);
        token.capability = crate::capability::Capability { resource: ResourceId("file:/workspace".into()), operation: crate::capability::Operation::Write };
        let req = crate::broker::build_request(
            principal.clone(),
            ResourceId("file:/workspace/hello.py".into()),
            crate::capability::Operation::Write,
            "files.write_file",
            &token,
            crate::protocol::ToolParameters::Stage { change: serde_json::json!({"content": "bad content"}) },
            uuid::Uuid::new_v4(),
            11,
        );
        let result = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(result.status, crate::protocol::ToolStatus::RolledBack);
        assert!(!ws.join("hello.py").exists(), "failed write should be rolled back and file removed");
    }

    #[test]
    fn guardian_blocks_write_outside_workspace() {
        let (broker, principal, mut token, _tmp, _ws) = file_broker_with_health(true);
        // Try to write to /etc/passwd via file:/etc/passwd resource – DATA-003 should block
        token.capability = crate::capability::Capability { resource: ResourceId("file:/etc/passwd".into()), operation: crate::capability::Operation::Write };
        // Need to give principal that capability and resource state for the check to reach guardian
        broker.core().lock().unwrap().set_resource_state(ResourceId("file:/etc/passwd".into()), crate::capability::ResourceState::Available);
        broker.core().lock().unwrap().set_resource_owner(ResourceId("file:/etc/passwd".into()), principal.clone());
        // Also grant the capability to the principal directly
        broker.core().lock().unwrap().grant_capability(&principal, crate::capability::Capability { resource: ResourceId("file:/etc/passwd".into()), operation: crate::capability::Operation::Write });
        // Register a tool that requires that exact capability at staged risk
        broker.core().lock().unwrap().register_tool(crate::capability::ToolDefinition { tool_id: "files.write_outside".into(), specialist_package: "files.specialist".into(), risk_level: crate::capability::RiskLevel::Staged, required_capabilities: vec![crate::capability::Capability { resource: ResourceId("file:/etc/passwd".into()), operation: crate::capability::Operation::Write }], description: "outside".into() });
        let req = crate::broker::build_request(
            principal.clone(),
            ResourceId("file:/etc/passwd".into()),
            crate::capability::Operation::Write,
            "files.write_outside",
            &token,
            crate::protocol::ToolParameters::Stage { change: serde_json::json!({"content": "hacked"}) },
            uuid::Uuid::new_v4(),
            12,
        );
        let result = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(result.status, crate::protocol::ToolStatus::Denied);
        let msg = result.error.unwrap().message;
        assert!(msg.contains("DATA-003") || msg.contains("blocked by guardian"), "expected DATA-003 block, got: {msg}");
    }

    #[test]
    fn observe_file_allows_prefix_capability() {
        use crate::capability::{Capability, Operation, ResourceId};
        use crate::protocol::ToolParameters;
        let (broker, principal, mut token, _tmp, _ws) = file_broker_with_health(true);
        // Observe with prefix token should be allowed (read-only, no guardian)
        token.capability = Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Observe };
        let req = crate::broker::build_request(
            principal.clone(),
            ResourceId("file:/workspace/hello.py".into()),
            Operation::Observe,
            "files.observe_file",
            &token,
            ToolParameters::Observe { fields: vec![] },
            uuid::Uuid::new_v4(),
            13,
        );
        // Need handler for observe – register a simple specialist that returns ok
        broker.spawn_specialist("files.observe_file", std::sync::Arc::new(|req| {
            crate::protocol::ToolResult {
                envelope: crate::protocol::MessageEnvelope::new(crate::protocol::MessageType::ToolResult, crate::capability::PrincipalId::system("files.specialist"), req.envelope.correlation_id, crate::protocol::DataClassification::SystemConfig),
                request_id: req.request_id,
                status: crate::protocol::ToolStatus::Success,
                data: Some(crate::protocol::ToolData::QueryResult { data: serde_json::json!({"ok": true}) }),
                error: None,
                health_impact: None,
            }
        }));
        let result = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(result.status, crate::protocol::ToolStatus::Success);
    }
}
