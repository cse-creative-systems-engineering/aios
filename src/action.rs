use crate::capability::{Operation, PrincipalId, ResourceId, RiskLevel};
use crate::protocol::{ActionId, CorrelationId, Timestamp, now};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub type CheckpointId = uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionState {
    Proposed,
    ImpactAnalyzed,
    Reviewed,
    PolicyValidated,
    GuardianChecked,
    Approved,
    Staged,
    HealthVerified,
    RollingBack,
    Committed,
    RolledBack,
    Rejected,
    Failed,
}

impl ActionState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            ActionState::Committed
                | ActionState::RolledBack
                | ActionState::Rejected
                | ActionState::Failed
        )
    }

    pub fn label(&self) -> &'static str {
        match self {
            ActionState::Proposed => "proposed",
            ActionState::ImpactAnalyzed => "impact-analyzed",
            ActionState::Reviewed => "reviewed",
            ActionState::PolicyValidated => "policy-validated",
            ActionState::GuardianChecked => "guardian-checked",
            ActionState::Approved => "approved",
            ActionState::Staged => "staged",
            ActionState::HealthVerified => "health-verified",
            ActionState::RollingBack => "rolling-back",
            ActionState::Committed => "committed",
            ActionState::RolledBack => "rolled-back",
            ActionState::Rejected => "rejected",
            ActionState::Failed => "failed",
        }
    }
}

pub fn can_transition(from: ActionState, to: ActionState) -> bool {
    if from.is_terminal() {
        return false;
    }
    use ActionState::*;
    matches!(
        (from, to),
        (Proposed, ImpactAnalyzed)
            | (Proposed, Rejected)
            | (ImpactAnalyzed, Reviewed)
            | (ImpactAnalyzed, Rejected)
            | (Reviewed, PolicyValidated)
            | (Reviewed, Rejected)
            | (PolicyValidated, GuardianChecked)
            | (PolicyValidated, Committed)
            | (PolicyValidated, Rejected)
            | (GuardianChecked, Approved)
            | (GuardianChecked, Staged)
            | (GuardianChecked, Rejected)
            | (Approved, Staged)
            | (Approved, Committed)
            | (Approved, RollingBack)
            | (Approved, Failed)
            | (Approved, Rejected)
            | (Staged, HealthVerified)
            | (Staged, RollingBack)
            | (Staged, Failed)
            | (HealthVerified, Committed)
            | (HealthVerified, RollingBack)
            | (RollingBack, RolledBack)
            | (RollingBack, Failed)
    )
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: ActionState,
    pub to: ActionState,
    pub timestamp: Timestamp,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub action_id: ActionId,
    pub correlation_id: CorrelationId,
    pub state: ActionState,
    pub risk_level: RiskLevel,
    pub resource: ResourceId,
    pub operation: Operation,
    pub principal: PrincipalId,
    pub checkpoint_id: Option<CheckpointId>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub state_history: Vec<StateTransition>,
}

impl ActionRecord {
    pub fn new(
        action_id: ActionId,
        correlation_id: CorrelationId,
        risk_level: RiskLevel,
        resource: ResourceId,
        operation: Operation,
        principal: PrincipalId,
    ) -> Self {
        let t = now();
        Self {
            action_id,
            correlation_id,
            state: ActionState::Proposed,
            risk_level,
            resource,
            operation,
            principal,
            checkpoint_id: None,
            created_at: t,
            updated_at: t,
            state_history: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointState {
    ConfigBackup {
        path: String,
        content_hash: [u8; 32],
    },
    DriverBackup {
        module: String,
        version: String,
        backup_path: String,
    },
    ServiceState {
        service: String,
        state: String,
    },
    FirmwareBackup {
        device: String,
        firmware_ref: String,
    },
    BootConfigBackup {
        config_path: String,
        content_hash: [u8; 32],
    },
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub checkpoint_id: CheckpointId,
    pub action_id: ActionId,
    pub resource: ResourceId,
    pub created_at: Timestamp,
    pub state: CheckpointState,
}

#[derive(Clone, Debug)]
pub struct RecoveryOutcome {
    pub action_id: ActionId,
    pub from_state: ActionState,
    pub to_state: ActionState,
    pub reason: String,
}

#[derive(Debug)]
pub enum ActionError {
    InvalidRequest,
    PersistenceFailed(String),
}

#[derive(Debug)]
pub enum TransitionError {
    InvalidTransition { from: ActionState, to: ActionState },
    ActionNotFound(ActionId),
    PersistenceFailed(String),
}

#[derive(Debug)]
pub enum StageError {
    CheckpointMissing,
    ResourceUnavailable,
    Internal(String),
}

#[derive(Debug)]
pub enum HealthError {
    SubsystemUnavailable,
    CheckFailed(String),
    Internal(String),
}

#[derive(Debug)]
pub enum CommitError {
    AlreadyCommitted,
    PartiallyApplied,
    Internal(String),
}

#[derive(Debug)]
pub enum PersistenceError {
    RecordNotFound,
    RecordCorrupted(String),
    StorageFailed(String),
}

#[derive(Debug)]
pub enum CheckpointError {
    ResourceNotCheckpointable(ResourceId),
    StorageFailed(String),
    VerificationFailed(String),
}

#[derive(Debug)]
pub enum RollbackError {
    CheckpointMissing(CheckpointId),
    CheckpointCorrupted(CheckpointId),
    RestorationFailed(String),
    HealthCheckFailed(String),
}

pub trait ActionStore {
    fn save(&self, record: &ActionRecord) -> Result<(), PersistenceError>;
    fn load(&self, action_id: &ActionId) -> Result<ActionRecord, PersistenceError>;
    fn load_all(&self) -> Result<Vec<ActionRecord>, PersistenceError>;
    fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), PersistenceError>;
    fn load_checkpoint(&self, checkpoint_id: &CheckpointId)
    -> Result<Checkpoint, PersistenceError>;
    fn delete_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<(), PersistenceError>;
}

pub struct FileActionStore {
    dir: PathBuf,
}

impl FileActionStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self, PersistenceError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)
            .map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        Ok(Self { dir })
    }

    fn path_for(&self, action_id: &ActionId) -> PathBuf {
        self.dir.join(format!("{action_id}.json"))
    }
}

impl ActionStore for FileActionStore {
    fn save(&self, record: &ActionRecord) -> Result<(), PersistenceError> {
        let path = self.path_for(&record.action_id);
        let json = serde_json::to_vec_pretty(record)
            .map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        let tmp = self.dir.join(format!("{}.tmp", record.action_id));
        std::fs::write(&tmp, &json).map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        std::fs::rename(&tmp, &path).map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        Ok(())
    }

    fn load(&self, action_id: &ActionId) -> Result<ActionRecord, PersistenceError> {
        let path = self.path_for(action_id);
        if !path.exists() {
            return Err(PersistenceError::RecordNotFound);
        }
        let bytes =
            std::fs::read(&path).map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        serde_json::from_slice(&bytes).map_err(|e| PersistenceError::RecordCorrupted(e.to_string()))
    }

    fn load_all(&self) -> Result<Vec<ActionRecord>, PersistenceError> {
        let mut records = Vec::new();
        let entries = std::fs::read_dir(&self.dir)
            .map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        for entry in entries {
            let path = entry
                .map_err(|e| PersistenceError::StorageFailed(e.to_string()))?
                .path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if file_name.ends_with(".tmp") || file_name.starts_with("checkpoint-") {
                continue;
            }
            let bytes =
                std::fs::read(&path).map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
            let record = serde_json::from_slice(&bytes)
                .map_err(|e| PersistenceError::RecordCorrupted(e.to_string()))?;
            records.push(record);
        }
        Ok(records)
    }

    fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), PersistenceError> {
        let path = self
            .dir
            .join(format!("checkpoint-{}.json", checkpoint.checkpoint_id));
        let tmp = self
            .dir
            .join(format!("checkpoint-{}.tmp", checkpoint.checkpoint_id));
        let bytes = serde_json::to_vec_pretty(checkpoint)
            .map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        std::fs::write(&tmp, bytes).map_err(|e| PersistenceError::StorageFailed(e.to_string()))?;
        std::fs::rename(tmp, path).map_err(|e| PersistenceError::StorageFailed(e.to_string()))
    }

    fn load_checkpoint(
        &self,
        checkpoint_id: &CheckpointId,
    ) -> Result<Checkpoint, PersistenceError> {
        let path = self.dir.join(format!("checkpoint-{}.json", checkpoint_id));
        let bytes = std::fs::read(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PersistenceError::RecordNotFound
            } else {
                PersistenceError::StorageFailed(e.to_string())
            }
        })?;
        serde_json::from_slice(&bytes).map_err(|e| PersistenceError::RecordCorrupted(e.to_string()))
    }

    fn delete_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<(), PersistenceError> {
        let path = self.dir.join(format!("checkpoint-{}.json", checkpoint_id));
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(PersistenceError::StorageFailed(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_states_have_no_outgoing_transitions() {
        for terminal in [
            ActionState::Committed,
            ActionState::RolledBack,
            ActionState::Rejected,
            ActionState::Failed,
        ] {
            for to in [
                ActionState::Proposed,
                ActionState::ImpactAnalyzed,
                ActionState::Reviewed,
            ] {
                assert!(!can_transition(terminal, to));
            }
        }
    }

    #[test]
    fn risk0_fast_path_transitions_are_valid() {
        assert!(can_transition(
            ActionState::Proposed,
            ActionState::ImpactAnalyzed
        ));
        assert!(can_transition(
            ActionState::ImpactAnalyzed,
            ActionState::Reviewed
        ));
        assert!(can_transition(
            ActionState::Reviewed,
            ActionState::PolicyValidated
        ));
        assert!(can_transition(
            ActionState::PolicyValidated,
            ActionState::Committed
        ));
    }

    #[test]
    fn stage_to_commit_transitions_are_valid() {
        assert!(can_transition(
            ActionState::GuardianChecked,
            ActionState::Staged
        ));
        assert!(can_transition(
            ActionState::Staged,
            ActionState::HealthVerified
        ));
        assert!(can_transition(
            ActionState::Staged,
            ActionState::RollingBack
        ));
        assert!(can_transition(
            ActionState::HealthVerified,
            ActionState::Committed
        ));
    }

    #[test]
    fn invalid_transitions_are_rejected() {
        assert!(!can_transition(
            ActionState::Proposed,
            ActionState::Committed
        ));
        assert!(!can_transition(
            ActionState::Reviewed,
            ActionState::Approved
        ));
        assert!(!can_transition(
            ActionState::RollingBack,
            ActionState::Committed
        ));
    }

    #[test]
    fn file_store_round_trips_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileActionStore::new(dir.path()).unwrap();
        let record = ActionRecord::new(
            uuid::Uuid::new_v4(),
            uuid::Uuid::new_v4(),
            RiskLevel::Staged,
            crate::capability::ResourceId("device:wifi0".into()),
            Operation::Stage,
            crate::capability::PrincipalId::agent("wifi.specialist", "wifi0"),
        );
        store.save(&record).unwrap();
        let loaded = store.load(&record.action_id).unwrap();
        assert_eq!(loaded.action_id, record.action_id);
        assert_eq!(loaded.state, ActionState::Proposed);
        assert_eq!(store.load_all().unwrap().len(), 1);
    }
}
