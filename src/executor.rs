use crate::action::{
    ActionError, ActionRecord, ActionState, ActionStore, Checkpoint, CheckpointError, CommitError,
    HealthError, PersistenceError, RecoveryOutcome, ResetError, RollbackError, StageError,
    TransitionError, can_transition,
};
use crate::capability::{Operation, PrincipalId, ResourceId, RiskLevel};
use crate::protocol::{ActionId, CorrelationId, HealthState};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagingResult {
    Committed,
    RolledBack,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagingError {
    CheckpointFailed,
    StageFailed,
    HealthCheckFailed,
    CommitFailed,
    RollbackFailed,
}

pub trait ResourceDriver: Send {
    fn create_checkpoint(
        &mut self,
        action_id: &ActionId,
        resource: &ResourceId,
    ) -> Result<Checkpoint, CheckpointError>;
    fn verify_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError>;
    /// Apply the candidate change (e.g. the driver module to load) to the
    /// resource. The candidate comes from the validated `Stage.change`
    /// payload of the ToolRequest (message-protocol §2.4), not from driver
    /// state.
    fn stage(&mut self, checkpoint: &Checkpoint, candidate: &str) -> Result<(), StageError>;
    fn health_check(&self, resource: &ResourceId) -> Result<HealthState, HealthError>;
    fn commit(&mut self, checkpoint: &Checkpoint) -> Result<(), CommitError>;
    fn rollback(&mut self, checkpoint: &Checkpoint) -> Result<(), RollbackError>;
    /// Reset the resource to a known-good state (risk level 4
    /// `request_reset`). A checkpoint is created before the reset so the
    /// prior state can be restored if the reset or its health check fails
    /// (action-state-machine §2.2 risk-4 path). Default: unsupported.
    fn reset(&mut self) -> Result<(), ResetError> {
        Err(ResetError::Unsupported)
    }
}

/// Validate a kernel module name before it is passed to the driver. Accepts
/// only `[A-Za-z0-9._-]` (kernel module syntax), trimming whitespace. Returns
/// `None` if invalid (REQ-SAF-005: module names from requests are untrusted).
fn validate_module(candidate: &str) -> Option<String> {
    let trimmed = candidate.trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return None;
    }
    if trimmed
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && !matches!(c, '-' | '_' | '.'))
    {
        return None;
    }
    Some(trimmed.to_string())
}

pub struct StagedExecutor {
    store: Box<dyn ActionStore>,
    driver: Arc<Mutex<Box<dyn ResourceDriver>>>,
}

impl StagedExecutor {
    pub fn new(store: Box<dyn ActionStore>, driver: Box<dyn ResourceDriver>) -> Self {
        Self {
            store,
            driver: Arc::new(Mutex::new(driver)),
        }
    }

    pub fn create_action(
        &mut self,
        correlation_id: CorrelationId,
        risk_level: RiskLevel,
        resource: ResourceId,
        operation: Operation,
        principal: PrincipalId,
    ) -> Result<ActionId, ActionError> {
        let action_id = uuid::Uuid::new_v4();
        let record = ActionRecord::new(
            action_id,
            correlation_id,
            risk_level,
            resource,
            operation,
            principal,
        );
        self.store
            .save(&record)
            .map_err(|e| ActionError::PersistenceFailed(format!("{e:?}")))?;
        Ok(action_id)
    }

    pub fn transition(
        &mut self,
        action_id: &ActionId,
        to: ActionState,
        reason: &str,
    ) -> Result<(), TransitionError> {
        let mut record = self
            .store
            .load(action_id)
            .map_err(|_| TransitionError::ActionNotFound(*action_id))?;
        if !can_transition(record.state, to) {
            return Err(TransitionError::InvalidTransition {
                from: record.state,
                to,
            });
        }
        // Write-ahead: persist the transition intent before executing it
        // (action-state-machine.md §5.3). If we crash between here and the
        // durable state update, recovery sees the pending intent.
        self.store
            .journal_pending_transition(action_id, record.state, to, reason)
            .map_err(|e| TransitionError::PersistenceFailed(format!("{e:?}")))?;
        record.state_history.push(crate::action::StateTransition {
            from: record.state,
            to,
            timestamp: crate::protocol::now(),
            reason: reason.to_string(),
        });
        record.state = to;
        record.updated_at = crate::protocol::now();
        self.store
            .save(&record)
            .map_err(|e| TransitionError::PersistenceFailed(format!("{e:?}")))?;
        // The state change is durably persisted; the intent is fulfilled.
        self.store
            .clear_pending_transition(action_id)
            .map_err(|e| TransitionError::PersistenceFailed(format!("{e:?}")))?;
        Ok(())
    }

    pub fn create_checkpoint(
        &mut self,
        action_id: &ActionId,
    ) -> Result<Checkpoint, CheckpointError> {
        let record = self
            .store
            .load(action_id)
            .map_err(|_| CheckpointError::VerificationFailed("action not found".into()))?;
        let checkpoint = self
            .driver
            .lock()
            .map_err(|e| CheckpointError::StorageFailed(e.to_string()))?
            .create_checkpoint(action_id, &record.resource)?;
        let mut updated = record;
        updated.checkpoint_id = Some(checkpoint.checkpoint_id);
        self.store
            .save(&updated)
            .map_err(|e| CheckpointError::StorageFailed(format!("{e:?}")))?;
        self.store
            .save_checkpoint(&checkpoint)
            .map_err(|e| CheckpointError::StorageFailed(format!("{e:?}")))?;
        Ok(checkpoint)
    }

    pub fn verify_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        self.driver
            .lock()
            .map_err(|e| CheckpointError::VerificationFailed(e.to_string()))?
            .verify_checkpoint(checkpoint)
    }

    pub fn stage(&mut self, checkpoint: &Checkpoint, candidate: &str) -> Result<(), StageError> {
        self.driver
            .lock()
            .map_err(|e| StageError::Internal(e.to_string()))?
            .stage(checkpoint, candidate)
    }

    pub fn health_check(&self, resource: &ResourceId) -> Result<HealthState, HealthError> {
        self.driver
            .lock()
            .map_err(|e| HealthError::Internal(e.to_string()))?
            .health_check(resource)
    }

    pub fn commit(&mut self, checkpoint: &Checkpoint) -> Result<(), CommitError> {
        self.driver
            .lock()
            .map_err(|e| CommitError::Internal(e.to_string()))?
            .commit(checkpoint)
    }

    pub fn rollback(&mut self, checkpoint: &Checkpoint) -> Result<(), RollbackError> {
        self.driver
            .lock()
            .map_err(|e| RollbackError::RestorationFailed(e.to_string()))?
            .rollback(checkpoint)
    }

    pub fn load_record(&self, action_id: &ActionId) -> Result<ActionRecord, PersistenceError> {
        self.store.load(action_id)
    }

    pub fn stage_and_commit(
        &mut self,
        action_id: &ActionId,
        candidate: &str,
    ) -> Result<StagingResult, StagingError> {
        // The candidate module name is validated before staging: it must be a
        // plain kernel module identifier to avoid shell/arg injection
        // (REQ-SAF-005: external data is untrusted).
        let module = validate_module(candidate).ok_or(StagingError::StageFailed)?;
        let record = self
            .store
            .load(action_id)
            .map_err(|_| StagingError::CheckpointFailed)?;
        let from = record.state;
        let next = match from {
            ActionState::GuardianChecked | ActionState::Approved => ActionState::Staged,
            _ => {
                return Err(StagingError::CheckpointFailed);
            }
        };
        self.transition(action_id, next, "staging begins")
            .map_err(|_| StagingError::StageFailed)?;

        let checkpoint = match self.create_checkpoint(action_id) {
            Ok(cp) => cp,
            Err(_) => {
                self.transition(action_id, ActionState::Failed, "checkpoint creation failed")
                    .map_err(|_| StagingError::CheckpointFailed)?;
                return Err(StagingError::CheckpointFailed);
            }
        };
        if self.verify_checkpoint(&checkpoint).is_err() {
            self.transition(
                action_id,
                ActionState::Failed,
                "checkpoint verification failed",
            )
            .map_err(|_| StagingError::CheckpointFailed)?;
            return Err(StagingError::CheckpointFailed);
        }

        if self.stage(&checkpoint, &module).is_err() {
            self.transition(action_id, ActionState::Failed, "stage failed")
                .map_err(|_| StagingError::StageFailed)?;
            return Err(StagingError::StageFailed);
        }

        let healthy = match self.health_check(&record.resource) {
            Ok(state) => matches!(state, HealthState::Healthy | HealthState::Degraded),
            Err(_) => false,
        };

        if !healthy {
            return self.do_rollback(action_id, &checkpoint);
        }

        self.transition(
            action_id,
            ActionState::HealthVerified,
            "health check passed",
        )
        .map_err(|_| StagingError::HealthCheckFailed)?;

        match self.commit(&checkpoint) {
            Ok(()) => {
                self.transition(action_id, ActionState::Committed, "commit succeeded")
                    .map_err(|_| StagingError::CommitFailed)?;
                self.delete_checkpoint(action_id)
                    .map_err(|_| StagingError::CommitFailed)?;
                Ok(StagingResult::Committed)
            }
            Err(_) => self.do_rollback(action_id, &checkpoint),
        }
    }

    fn do_rollback(
        &mut self,
        action_id: &ActionId,
        checkpoint: &Checkpoint,
    ) -> Result<StagingResult, StagingError> {
        self.transition(action_id, ActionState::RollingBack, "rollback begins")
            .map_err(|_| StagingError::RollbackFailed)?;
        match self.rollback(checkpoint) {
            Ok(()) => {
                self.transition(action_id, ActionState::RolledBack, "rollback verified")
                    .map_err(|_| StagingError::RollbackFailed)?;
                self.delete_checkpoint(action_id)
                    .map_err(|_| StagingError::RollbackFailed)?;
                Ok(StagingResult::RolledBack)
            }
            Err(_) => {
                self.transition(action_id, ActionState::Failed, "rollback failed")
                    .map_err(|_| StagingError::RollbackFailed)?;
                Err(StagingError::RollbackFailed)
            }
        }
    }

    fn delete_checkpoint(&self, action_id: &ActionId) -> Result<(), PersistenceError> {
        let record = self.store.load(action_id)?;
        if let Some(id) = record.checkpoint_id {
            self.store.delete_checkpoint(&id)?;
        }
        Ok(())
    }

    /// Execute a risk-4 device reset (action-state-machine §2.2):
    /// the action is already `Approved`; create a checkpoint, perform the
    /// reset, health-check, then commit (deleting the checkpoint) or roll
    /// back to the checkpointed state. Guarded by broker-owned approval
    /// before this is reached (human-interaction §1).
    pub fn reset_and_commit(&mut self, action_id: &ActionId) -> Result<StagingResult, StagingError> {
        let record = self
            .store
            .load(action_id)
            .map_err(|_| StagingError::CheckpointFailed)?;
        if record.state != ActionState::Approved {
            return Err(StagingError::CheckpointFailed);
        }

        let checkpoint = match self.create_checkpoint(action_id) {
            Ok(cp) => cp,
            Err(_) => {
                self.transition(action_id, ActionState::Failed, "checkpoint creation failed")
                    .map_err(|_| StagingError::CheckpointFailed)?;
                return Err(StagingError::CheckpointFailed);
            }
        };
        if self.verify_checkpoint(&checkpoint).is_err() {
            self.transition(
                action_id,
                ActionState::Failed,
                "checkpoint verification failed",
            )
            .map_err(|_| StagingError::CheckpointFailed)?;
            return Err(StagingError::CheckpointFailed);
        }

        let reset_ok = {
            let mut driver = self
                .driver
                .lock()
                .map_err(|_| StagingError::RollbackFailed)?;
            driver.reset().is_ok()
        };
        if !reset_ok {
            return self.do_rollback(action_id, &checkpoint);
        }

        let healthy = match self.health_check(&record.resource) {
            Ok(state) => matches!(state, HealthState::Healthy | HealthState::Degraded),
            Err(_) => false,
        };
        if !healthy {
            return self.do_rollback(action_id, &checkpoint);
        }

        // The risk-4 fast path leads Approved → Committed directly (skip
        // staging and the HealthVerified state, action-state-machine §2.2).
        // The health check above gates the commit; on failure we already
        // rolled back. Commit consumes the checkpoint.
        match self.commit(&checkpoint) {
            Ok(()) => {
                self.transition(action_id, ActionState::Committed, "reset committed")
                    .map_err(|_| StagingError::CommitFailed)?;
                self.delete_checkpoint(action_id)
                    .map_err(|_| StagingError::CommitFailed)?;
                Ok(StagingResult::Committed)
            }
            Err(_) => self.do_rollback(action_id, &checkpoint),
        }
    }

    pub fn recover(&mut self) -> Result<Vec<RecoveryOutcome>, PersistenceError> {
        let records = self.store.load_all()?;
        let mut outcomes = Vec::new();
        for record in records {
            if record.state.is_terminal() {
                continue;
            }
            let action_id = record.action_id;
            let from = record.state;
            let (to, reason) = match record.state {
                ActionState::Proposed
                | ActionState::ImpactAnalyzed
                | ActionState::Reviewed
                | ActionState::PolicyValidated
                | ActionState::GuardianChecked => (
                    ActionState::Rejected,
                    "interrupted before staging".to_string(),
                ),
                ActionState::Approved => (
                    ActionState::Rejected,
                    "interrupted before staging".to_string(),
                ),
                ActionState::Staged => self.recover_staged(&action_id),
                ActionState::RollingBack => self.recover_rolling_back(&action_id),
                ActionState::HealthVerified => self.recover_health_verified(&action_id),
                ActionState::Committed
                | ActionState::RolledBack
                | ActionState::Rejected
                | ActionState::Failed => (ActionState::Failed, "terminal state recovered".into()),
            };
            outcomes.push(RecoveryOutcome {
                action_id,
                from_state: from,
                to_state: to,
                reason,
            });
        }
        Ok(outcomes)
    }

    /// Restore a failed action's retained checkpoint at the user's direction.
    /// The failed action record remains failed as an immutable incident record;
    /// the returned result describes the separately requested restoration.
    pub fn manual_recover(&mut self, action_id: &ActionId) -> Result<StagingResult, StagingError> {
        let record = self
            .store
            .load(action_id)
            .map_err(|_| StagingError::CheckpointFailed)?;
        if record.state != ActionState::Failed {
            return Err(StagingError::CheckpointFailed);
        }
        let checkpoint_id = record.checkpoint_id.ok_or(StagingError::CheckpointFailed)?;
        let checkpoint = self
            .store
            .load_checkpoint(&checkpoint_id)
            .map_err(|_| StagingError::CheckpointFailed)?;
        self.verify_checkpoint(&checkpoint)
            .map_err(|_| StagingError::CheckpointFailed)?;
        self.rollback(&checkpoint)
            .map_err(|_| StagingError::RollbackFailed)?;
        let health = self
            .health_check(&record.resource)
            .map_err(|_| StagingError::HealthCheckFailed)?;
        if !matches!(health, HealthState::Healthy | HealthState::Degraded) {
            return Err(StagingError::HealthCheckFailed);
        }
        self.store
            .delete_checkpoint(&checkpoint_id)
            .map_err(|_| StagingError::RollbackFailed)?;
        Ok(StagingResult::RolledBack)
    }

    fn recover_staged(&mut self, action_id: &ActionId) -> (ActionState, String) {
        let record = match self.store.load(action_id) {
            Ok(r) => r,
            Err(_) => return (ActionState::Failed, "record unreadable".into()),
        };
        let checkpoint_id = match record.checkpoint_id {
            Some(id) => id,
            None => return (ActionState::Failed, "checkpoint missing".into()),
        };
        let checkpoint = match self.store.load_checkpoint(&checkpoint_id) {
            Ok(checkpoint) => checkpoint,
            Err(_) => {
                return (
                    ActionState::Failed,
                    "checkpoint missing or corrupted".into(),
                );
            }
        };
        let healthy = self
            .health_check(&record.resource)
            .map(|h| matches!(h, HealthState::Healthy | HealthState::Degraded))
            .unwrap_or(false);
        if healthy {
            match self.commit(&checkpoint) {
                Ok(()) => {
                    if self
                        .transition(action_id, ActionState::Committed, "recovered commit")
                        .is_err()
                    {
                        return (
                            ActionState::Failed,
                            "could not persist recovered commit".into(),
                        );
                    }
                    (
                        ActionState::Committed,
                        "health passed on recovery, committed".into(),
                    )
                }
                Err(_) => self.recover_rolling_back(action_id),
            }
        } else {
            self.recover_rolling_back(action_id)
        }
    }

    fn recover_health_verified(&mut self, action_id: &ActionId) -> (ActionState, String) {
        let record = match self.store.load(action_id) {
            Ok(r) => r,
            Err(_) => return (ActionState::Failed, "record unreadable".into()),
        };
        let checkpoint_id = match record.checkpoint_id {
            Some(id) => id,
            None => return (ActionState::Failed, "checkpoint missing".into()),
        };
        let checkpoint = match self.store.load_checkpoint(&checkpoint_id) {
            Ok(cp) => cp,
            Err(_) => {
                return (
                    ActionState::Failed,
                    "checkpoint missing or corrupted".into(),
                );
            }
        };
        match self.commit(&checkpoint) {
            Ok(()) => {
                if self
                    .transition(action_id, ActionState::Committed, "recovered commit")
                    .is_err()
                {
                    return (
                        ActionState::Failed,
                        "could not persist recovered commit".into(),
                    );
                }
                (
                    ActionState::Committed,
                    "commit retried and succeeded".into(),
                )
            }
            Err(_) => self.recover_rolling_back(action_id),
        }
    }

    fn recover_rolling_back(&mut self, action_id: &ActionId) -> (ActionState, String) {
        let record = match self.store.load(action_id) {
            Ok(r) => r,
            Err(_) => return (ActionState::Failed, "record unreadable".into()),
        };
        let checkpoint_id = match record.checkpoint_id {
            Some(id) => id,
            None => return (ActionState::Failed, "checkpoint missing".into()),
        };
        let checkpoint = match self.store.load_checkpoint(&checkpoint_id) {
            Ok(cp) => cp,
            Err(_) => {
                return (
                    ActionState::Failed,
                    "checkpoint missing or corrupted".into(),
                );
            }
        };
        match self.rollback(&checkpoint) {
            Ok(()) => {
                if self
                    .transition(action_id, ActionState::RollingBack, "recovered rollback")
                    .is_err()
                {
                    return (
                        ActionState::Failed,
                        "could not persist rollback state".into(),
                    );
                }
                if self
                    .transition(action_id, ActionState::RolledBack, "recovered rollback")
                    .is_err()
                {
                    return (
                        ActionState::Failed,
                        "could not persist rolled-back state".into(),
                    );
                }
                (
                    ActionState::RolledBack,
                    "rollback retried and succeeded".into(),
                )
            }
            Err(_) => {
                if self
                    .transition(action_id, ActionState::Failed, "rollback failed")
                    .is_err()
                {
                    return (
                        ActionState::Failed,
                        "could not persist rollback failure".into(),
                    );
                }
                (
                    ActionState::Failed,
                    "rollback failed during recovery".into(),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::CheckpointState;

    struct MockDriver {
        state: u64,
        health_ok: bool,
        health_error: bool,
        verify_ok: bool,
        rollback_ok: bool,
        reset_ok: bool,
    }

    impl MockDriver {
        fn snapshot(&self) -> u64 {
            self.state
        }
    }

    impl ResourceDriver for MockDriver {
        fn create_checkpoint(
            &mut self,
            action_id: &ActionId,
            resource: &ResourceId,
        ) -> Result<Checkpoint, CheckpointError> {
            Ok(Checkpoint {
                checkpoint_id: uuid::Uuid::new_v4(),
                action_id: *action_id,
                resource: resource.clone(),
                created_at: crate::protocol::now(),
                state: CheckpointState::ConfigBackup {
                    path: "mock".into(),
                    content_hash: [0u8; 32],
                },
            })
        }

        fn verify_checkpoint(&self, _cp: &Checkpoint) -> Result<(), CheckpointError> {
            if self.verify_ok {
                Ok(())
            } else {
                Err(CheckpointError::VerificationFailed(
                    "test corruption".into(),
                ))
            }
        }

        fn stage(&mut self, _cp: &Checkpoint, _candidate: &str) -> Result<(), StageError> {
            self.state += 1;
            Ok(())
        }

        fn health_check(&self, _r: &ResourceId) -> Result<HealthState, HealthError> {
            if self.health_error {
                return Err(HealthError::SubsystemUnavailable);
            }
            Ok(if self.health_ok {
                HealthState::Healthy
            } else {
                HealthState::Unhealthy
            })
        }

        fn commit(&mut self, _cp: &Checkpoint) -> Result<(), CommitError> {
            Ok(())
        }

        fn rollback(&mut self, _cp: &Checkpoint) -> Result<(), RollbackError> {
            if !self.rollback_ok {
                return Err(RollbackError::RestorationFailed(
                    "test rollback failure".into(),
                ));
            }
            self.state = self.snapshot().saturating_sub(1);
            Ok(())
        }

        fn reset(&mut self) -> Result<(), ResetError> {
            if !self.reset_ok {
                return Err(ResetError::ResetFailed("test reset failure".into()));
            }
            self.state += 10;
            Ok(())
        }
    }

    fn fresh(dir: &tempfile::TempDir, health_ok: bool) -> (StagedExecutor, u64) {
        let store = Box::new(crate::action::FileActionStore::new(dir.path()).expect("store init"));
        let driver = MockDriver {
            state: 10,
            health_ok,
            health_error: false,
            verify_ok: true,
            rollback_ok: true,
            reset_ok: true,
        };
        let current = driver.state;
        let executor = StagedExecutor::new(store, Box::new(driver));
        (executor, current)
    }

    fn fresh_fault(
        dir: &tempfile::TempDir,
        health_ok: bool,
        health_error: bool,
        verify_ok: bool,
        rollback_ok: bool,
    ) -> StagedExecutor {
        let store = Box::new(crate::action::FileActionStore::new(dir.path()).expect("store init"));
        let driver = MockDriver {
            state: 10,
            health_ok,
            health_error,
            verify_ok,
            rollback_ok,
            reset_ok: true,
        };
        StagedExecutor::new(store, Box::new(driver))
    }

    fn checkpoint_count(dir: &tempfile::TempDir) -> usize {
        std::fs::read_dir(dir.path())
            .expect("checkpoint directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("checkpoint-")
            })
            .count()
    }

    fn advance_to(executor: &mut StagedExecutor, action_id: &ActionId, target: ActionState) {
        let chain = [
            ActionState::ImpactAnalyzed,
            ActionState::Reviewed,
            ActionState::PolicyValidated,
            ActionState::GuardianChecked,
            ActionState::Approved,
            ActionState::Staged,
        ];
        for state in chain {
            executor
                .transition(action_id, state, "test step")
                .expect("valid chain transition");
            if state == target {
                return;
            }
        }
        panic!("target {target:?} not in chain");
    }

    fn prestage(executor: &mut StagedExecutor, action_id: &ActionId) {
        advance_to(executor, action_id, ActionState::GuardianChecked);
    }

    #[test]
    fn stage_commit_flows_through_states() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        prestage(&mut executor, &action_id);
        assert_eq!(
            executor.stage_and_commit(&action_id, "mt7921e").unwrap(),
            StagingResult::Committed
        );
        let record = executor.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::Committed);
        assert!(record.state_history.len() >= 3);
        assert_eq!(checkpoint_count(&dir), 0);
    }

    #[test]
    fn health_check_failure_triggers_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, false);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        prestage(&mut executor, &action_id);
        assert_eq!(
            executor.stage_and_commit(&action_id, "mt7921e").unwrap(),
            StagingResult::RolledBack
        );
        let record = executor.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::RolledBack);
        assert_eq!(checkpoint_count(&dir), 0);
    }

    // Risk-4 reset path (action-state-machine §2.2): from Approved, create a
    // checkpoint, reset the device, health check, commit. No candidate module
    // is staged.
    #[test]
    fn reset_commits_from_approved_state() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Recovery,
                ResourceId("device:wifi0".into()),
                Operation::Reset,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        advance_to(&mut executor, &action_id, ActionState::Approved);
        assert_eq!(
            executor.reset_and_commit(&action_id).unwrap(),
            StagingResult::Committed
        );
        let record = executor.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::Committed);
        assert_eq!(checkpoint_count(&dir), 0);
    }

    #[test]
    fn reset_rolls_back_when_health_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, false);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Recovery,
                ResourceId("device:wifi0".into()),
                Operation::Reset,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        advance_to(&mut executor, &action_id, ActionState::Approved);
        assert_eq!(
            executor.reset_and_commit(&action_id).unwrap(),
            StagingResult::RolledBack
        );
        let record = executor.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::RolledBack);
        assert_eq!(checkpoint_count(&dir), 0);
    }

    #[test]
    fn reset_requires_approved_state() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Recovery,
                ResourceId("device:wifi0".into()),
                Operation::Reset,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        // Only GuardianChecked — reset must not proceed without approval.
        advance_to(&mut executor, &action_id, ActionState::GuardianChecked);
        assert_eq!(
            executor.reset_and_commit(&action_id),
            Err(StagingError::CheckpointFailed)
        );
        assert_eq!(
            executor.load_record(&action_id).unwrap().state,
            ActionState::GuardianChecked
        );
    }

    #[test]
    fn reset_rolls_back_when_reset_fails() {
        let dir = tempfile::tempdir().unwrap();
        let store = Box::new(crate::action::FileActionStore::new(dir.path()).expect("store init"));
        let mut executor = StagedExecutor::new(
            store,
            Box::new(MockDriver {
                state: 10,
                health_ok: true,
                health_error: false,
                verify_ok: true,
                rollback_ok: true,
                reset_ok: false,
            }),
        );
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Recovery,
                ResourceId("device:wifi0".into()),
                Operation::Reset,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        advance_to(&mut executor, &action_id, ActionState::Approved);
        assert_eq!(
            executor.reset_and_commit(&action_id).unwrap(),
            StagingResult::RolledBack
        );
        assert_eq!(
            executor.load_record(&action_id).unwrap().state,
            ActionState::RolledBack
        );
    }

    #[test]
    fn checkpoint_verification_failure_enters_failed_and_retains_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let mut executor = fresh_fault(&dir, true, false, true, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        prestage(&mut executor, &action_id);
        executor
            .create_checkpoint(&action_id)
            .expect("checkpoint retained for failed action");
        executor
            .transition(&action_id, ActionState::Staged, "test interrupted staging")
            .unwrap();
        executor
            .transition(
                &action_id,
                ActionState::Failed,
                "test unrecoverable failure",
            )
            .unwrap();
        assert_eq!(
            executor.load_record(&action_id).unwrap().state,
            ActionState::Failed
        );
        assert_eq!(checkpoint_count(&dir), 1);
    }

    #[test]
    fn failed_action_can_be_manually_recovered_from_retained_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let mut executor = fresh_fault(&dir, true, false, true, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        prestage(&mut executor, &action_id);
        executor.create_checkpoint(&action_id).unwrap();
        executor
            .transition(&action_id, ActionState::Staged, "test interrupted staging")
            .unwrap();
        executor
            .transition(
                &action_id,
                ActionState::Failed,
                "test unrecoverable failure",
            )
            .unwrap();
        assert_eq!(
            executor.manual_recover(&action_id),
            Ok(StagingResult::RolledBack)
        );
        assert_eq!(checkpoint_count(&dir), 0);
        assert_eq!(
            executor.load_record(&action_id).unwrap().state,
            ActionState::Failed
        );
    }

    #[test]
    fn health_check_error_rolls_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut executor = fresh_fault(&dir, true, true, true, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        prestage(&mut executor, &action_id);
        assert_eq!(
            executor.stage_and_commit(&action_id, "mt7921e").unwrap(),
            StagingResult::RolledBack
        );
        assert_eq!(
            executor.load_record(&action_id).unwrap().state,
            ActionState::RolledBack
        );
    }

    #[test]
    fn rollback_failure_enters_failed_and_retains_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let mut executor = fresh_fault(&dir, false, false, true, false);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        prestage(&mut executor, &action_id);
        assert_eq!(
            executor.stage_and_commit(&action_id, "mt7921e"),
            Err(StagingError::RollbackFailed)
        );
        assert_eq!(
            executor.load_record(&action_id).unwrap().state,
            ActionState::Failed
        );
        assert_eq!(checkpoint_count(&dir), 1);
    }

    #[test]
    fn recover_resolves_interrupted_staged_action() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, false);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        advance_to(&mut executor, &action_id, ActionState::Staged);
        executor
            .create_checkpoint(&action_id)
            .expect("checkpoint created before interruption");
        drop(executor);

        let mut recovered = StagedExecutor::new(
            Box::new(crate::action::FileActionStore::new(dir.path()).unwrap()),
            Box::new(MockDriver {
                state: 10,
                health_ok: false,
                health_error: false,
                verify_ok: true,
                rollback_ok: true,
                reset_ok: true,
            }),
        );
        let outcomes = recovered.recover().expect("recovery store readable");
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].to_state, ActionState::RolledBack);
        let record = recovered.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::RolledBack);
    }

    #[test]
    fn recover_rejects_interrupted_pre_stage_action() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        advance_to(&mut executor, &action_id, ActionState::Reviewed);
        drop(executor);

        let mut recovered = StagedExecutor::new(
            Box::new(crate::action::FileActionStore::new(dir.path()).unwrap()),
            Box::new(MockDriver {
                state: 10,
                health_ok: true,
                health_error: false,
                verify_ok: true,
                rollback_ok: true,
                reset_ok: true,
            }),
        );
        let outcomes = recovered.recover().expect("recovery store readable");
        assert_eq!(outcomes[0].to_state, ActionState::Rejected);
    }

    #[test]
    fn transition_journals_write_ahead_and_clears_after_persist() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        // No pending journal exists yet.
        assert_eq!(pending_count(&dir), 0);
        executor
            .transition(&action_id, ActionState::ImpactAnalyzed, "analyzed")
            .unwrap();
        // After a successful transition the pending intent is cleared.
        assert_eq!(pending_count(&dir), 0);
        let record = executor.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::ImpactAnalyzed);
    }

    #[test]
    fn leftover_pending_journal_does_not_pollute_load_all() {
        let dir = tempfile::tempdir().unwrap();
        let (mut executor, _) = fresh(&dir, true);
        let action_id = executor
            .create_action(
                uuid::Uuid::new_v4(),
                RiskLevel::Staged,
                ResourceId("device:wifi0".into()),
                Operation::Stage,
                PrincipalId::agent("wifi.specialist", "wifi0"),
            )
            .unwrap();
        // Simulate a crash mid-transition: journal the intent but never
        // persist the state update.
        let store = crate::action::FileActionStore::new(dir.path()).unwrap();
        store
            .journal_pending_transition(
                &action_id,
                ActionState::Proposed,
                ActionState::ImpactAnalyzed,
                "crash simulation",
            )
            .unwrap();
        drop(executor);
        // load_all must not treat the pending journal as an action record.
        let store = crate::action::FileActionStore::new(dir.path()).unwrap();
        let records = store.load_all().unwrap();
        assert_eq!(records.len(), 1, "only the action record, not the journal");
        assert_eq!(records[0].state, ActionState::Proposed);
    }

    fn pending_count(dir: &tempfile::TempDir) -> usize {
        std::fs::read_dir(dir.path())
            .expect("state directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("pending-")
            })
            .count()
    }
}
