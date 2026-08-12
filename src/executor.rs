use crate::action::{
    ActionError, ActionRecord, ActionState, ActionStore, Checkpoint, CheckpointError,
    CheckpointState, CommitError, HealthError, PersistenceError, RecoveryOutcome, RollbackError,
    StageError, TransitionError, can_transition,
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
    fn stage(&mut self, checkpoint: &Checkpoint) -> Result<(), StageError>;
    fn health_check(&self, resource: &ResourceId) -> Result<HealthState, HealthError>;
    fn commit(&mut self, checkpoint: &Checkpoint) -> Result<(), CommitError>;
    fn rollback(&mut self, checkpoint: &Checkpoint) -> Result<(), RollbackError>;
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
        Ok(())
    }

    pub fn create_checkpoint(&mut self, action_id: &ActionId) -> Result<Checkpoint, CheckpointError> {
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
        Ok(checkpoint)
    }

    pub fn verify_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        self.driver
            .lock()
            .map_err(|e| CheckpointError::VerificationFailed(e.to_string()))?
            .verify_checkpoint(checkpoint)
    }

    pub fn stage(&mut self, checkpoint: &Checkpoint) -> Result<(), StageError> {
        self.driver
            .lock()
            .map_err(|e| StageError::Internal(e.to_string()))?
            .stage(checkpoint)
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
    ) -> Result<StagingResult, StagingError> {
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
        self.verify_checkpoint(&checkpoint)
            .map_err(|_| StagingError::CheckpointFailed)?;

        if self.stage(&checkpoint).is_err() {
            self.transition(action_id, ActionState::Failed, "stage failed")
                .map_err(|_| StagingError::StageFailed)?;
            return Err(StagingError::StageFailed);
        }

        let healthy = self
            .health_check(&record.resource)
            .map(|h| matches!(h, HealthState::Healthy | HealthState::Degraded))
            .unwrap_or(false);

        if !healthy {
            return self.do_rollback(action_id, &checkpoint);
        }

        self.transition(action_id, ActionState::HealthVerified, "health check passed")
            .map_err(|_| StagingError::HealthCheckFailed)?;

        match self.commit(&checkpoint) {
            Ok(()) => {
                self.transition(action_id, ActionState::Committed, "commit succeeded")
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
                Ok(StagingResult::RolledBack)
            }
            Err(_) => {
                self.transition(action_id, ActionState::Failed, "rollback failed")
                    .map_err(|_| StagingError::RollbackFailed)?;
                Err(StagingError::RollbackFailed)
            }
        }
    }

    pub fn recover(&mut self) -> Vec<RecoveryOutcome> {
        let records = match self.store.load_all() {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };
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
                | ActionState::GuardianChecked => {
                    (ActionState::Rejected, "interrupted before staging".to_string())
                }
                ActionState::Approved => {
                    (ActionState::Rejected, "interrupted before staging".to_string())
                }
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
        outcomes
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
        let checkpoint = Checkpoint {
            checkpoint_id,
            action_id: *action_id,
            resource: record.resource.clone(),
            created_at: record.created_at,
            state: CheckpointState::Empty,
        };
        let healthy = self
            .health_check(&record.resource)
            .map(|h| matches!(h, HealthState::Healthy | HealthState::Degraded))
            .unwrap_or(false);
        if healthy {
            match self.commit(&checkpoint) {
                Ok(()) => {
                    let _ = self.transition(action_id, ActionState::Committed, "recovered commit");
                    (ActionState::Committed, "health passed on recovery, committed".into())
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
        let checkpoint = record
            .checkpoint_id
            .map(|checkpoint_id| Checkpoint {
                checkpoint_id,
                action_id: *action_id,
                resource: record.resource.clone(),
                created_at: record.created_at,
                state: CheckpointState::Empty,
            })
            .ok_or_else(|| (ActionState::Failed, "checkpoint missing".to_string()));
        let checkpoint = match checkpoint {
            Ok(cp) => cp,
            Err(e) => return e,
        };
        match self.commit(&checkpoint) {
            Ok(()) => {
                let _ = self.transition(action_id, ActionState::Committed, "recovered commit");
                (ActionState::Committed, "commit retried and succeeded".into())
            }
            Err(_) => self.recover_rolling_back(action_id),
        }
    }

    fn recover_rolling_back(&mut self, action_id: &ActionId) -> (ActionState, String) {
        let record = match self.store.load(action_id) {
            Ok(r) => r,
            Err(_) => return (ActionState::Failed, "record unreadable".into()),
        };
        let checkpoint = match record.checkpoint_id.map(|checkpoint_id| Checkpoint {
            checkpoint_id,
            action_id: *action_id,
            resource: record.resource.clone(),
            created_at: record.created_at,
            state: CheckpointState::Empty,
        }) {
            Some(cp) => cp,
            None => return (ActionState::Failed, "checkpoint missing".into()),
        };
        match self.rollback(&checkpoint) {
            Ok(()) => {
                let _ = self.transition(action_id, ActionState::RollingBack, "recovered rollback");
                let _ = self.transition(action_id, ActionState::RolledBack, "recovered rollback");
                (ActionState::RolledBack, "rollback retried and succeeded".into())
            }
            Err(_) => {
                let _ = self.transition(action_id, ActionState::Failed, "rollback failed");
                (ActionState::Failed, "rollback failed during recovery".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDriver {
        state: u64,
        health_ok: bool,
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
            Ok(())
        }

        fn stage(&mut self, _cp: &Checkpoint) -> Result<(), StageError> {
            self.state += 1;
            Ok(())
        }

        fn health_check(&self, _r: &ResourceId) -> Result<HealthState, HealthError> {
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
            self.state = self.snapshot().saturating_sub(1);
            Ok(())
        }
    }

    fn fresh(dir: &tempfile::TempDir, health_ok: bool) -> (StagedExecutor, u64) {
        let store = Box::new(
            crate::action::FileActionStore::new(dir.path()).expect("store init"),
        );
        let driver = MockDriver {
            state: 10,
            health_ok,
        };
        let current = driver.state;
        let executor = StagedExecutor::new(store, Box::new(driver));
        (executor, current)
    }

    fn advance_to(
        executor: &mut StagedExecutor,
        action_id: &ActionId,
        target: ActionState,
    ) {
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
            executor.stage_and_commit(&action_id).unwrap(),
            StagingResult::Committed
        );
        let record = executor.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::Committed);
        assert!(record.state_history.len() >= 3);
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
            executor.stage_and_commit(&action_id).unwrap(),
            StagingResult::RolledBack
        );
        let record = executor.load_record(&action_id).unwrap();
        assert_eq!(record.state, ActionState::RolledBack);
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
            }),
        );
        let outcomes = recovered.recover();
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
            }),
        );
        let outcomes = recovered.recover();
        assert_eq!(outcomes[0].to_state, ActionState::Rejected);
    }
}
