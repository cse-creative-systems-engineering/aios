use crate::action::{
    Checkpoint, CheckpointError, CheckpointState, CommitError, HealthError, ResetError,
    RollbackError, StageError,
};
use crate::capability::ResourceId;
use crate::executor::ResourceDriver;
use crate::protocol::{ActionId, HealthState};

/// The bounded set of Linux operations the Wi-Fi driver resource driver may
/// perform. Abstracted so tests use a mock control and the real system uses a
/// Linux command control — the driver logic is identical either way
/// (REQ-FUNC-003: bounded typed operations, never `run_any_command`).
pub trait DriverControl: Send {
    /// Which kernel module is currently active for the device.
    fn active_module(&self) -> String;
    /// Version string of the active module, if known.
    fn module_version(&self, module: &str) -> Option<String>;
    /// Load a candidate kernel module and bring its interface up.
    fn load_module(&mut self, module: &str) -> Result<(), String>;
    /// Unload a kernel module.
    fn unload_module(&mut self, module: &str) -> Result<(), String>;
    /// Whether the wireless interface is up (link state, NETWORK-002).
    fn link_state_up(&self) -> bool;
    /// Reset the device to a known state (risk level 4).
    fn reset_device(&mut self) -> Result<(), String>;
    /// The raw command(s) that would be executed for `load_module`, for
    /// display before the user executes them (human-interaction full-scope).
    fn plan_load(&self, module: &str) -> String;
    /// The raw command(s) that would be executed for `unload_module`.
    fn plan_unload(&self, module: &str) -> String;
}

/// A `DriverControl` that never touches the kernel: it records the intended
/// operations so tests can verify the staged executor drives the right calls
/// and health checks without real hardware (testing-strategy §1.5).
#[derive(Clone, Debug)]
pub struct MockDriverControl {
    pub active: String,
    pub version: String,
    pub staged_module: Option<String>,
    pub link_up: bool,
    pub load_ok: bool,
    pub unload_ok: bool,
    pub reset_ok: bool,
}

impl MockDriverControl {
    pub fn new() -> Self {
        Self {
            active: "iwlwifi".into(),
            version: "1.0.0".into(),
            staged_module: None,
            link_up: true,
            load_ok: true,
            unload_ok: true,
            reset_ok: true,
        }
    }
}

impl Default for MockDriverControl {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverControl for MockDriverControl {
    fn active_module(&self) -> String {
        self.active.clone()
    }

    fn module_version(&self, _module: &str) -> Option<String> {
        Some(self.version.clone())
    }

    fn load_module(&mut self, module: &str) -> Result<(), String> {
        if !self.load_ok {
            return Err("mock load failed".into());
        }
        self.staged_module = Some(module.to_string());
        self.active = module.to_string();
        Ok(())
    }

    fn unload_module(&mut self, module: &str) -> Result<(), String> {
        if !self.unload_ok {
            return Err("mock unload failed".into());
        }
        if self.active == module {
            self.active = "none".into();
        }
        Ok(())
    }

    fn link_state_up(&self) -> bool {
        self.link_up
    }

    fn reset_device(&mut self) -> Result<(), String> {
        if !self.reset_ok {
            return Err("mock reset failed".into());
        }
        Ok(())
    }

    fn plan_load(&self, module: &str) -> String {
        format!("modprobe {module}")
    }

    fn plan_unload(&self, module: &str) -> String {
        format!("modprobe -r {module}")
    }
}

/// Real Wi-Fi driver `ResourceDriver` used by the staged executor for the
/// `wifi.stage_driver` and `wifi.request_reset` tools (modules/wifi.md).
///
/// Checkpoints capture the currently active module and version
/// (`CheckpointState::DriverBackup`). Staging loads the candidate module
/// (passed from the validated `Stage.change` payload, message-protocol
/// §2.4); the health check verifies the link is up (NETWORK-002); commit
/// keeps the candidate, rollback restores the checkpointed module (v0.1:
/// module-level only — never the boot chain, ADR-0001).
pub struct WifiDriverResourceDriver {
    pub control: Box<dyn DriverControl>,
    pub device: ResourceId,
}

impl WifiDriverResourceDriver {
    pub fn new(control: Box<dyn DriverControl>, device: ResourceId) -> Self {
        Self { control, device }
    }

    fn checkpoint_state(&self) -> CheckpointState {
        CheckpointState::DriverBackup {
            module: self.control.active_module(),
            version: self
                .control
                .module_version(&self.control.active_module())
                .unwrap_or_else(|| "unknown".into()),
            backup_path: format!(
                "/var/lib/aios/backups/{}.driver",
                self.device.as_str()
            ),
        }
    }
}

impl ResourceDriver for WifiDriverResourceDriver {
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
            state: self.checkpoint_state(),
        })
    }

    fn verify_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), CheckpointError> {
        match &checkpoint.state {
            CheckpointState::DriverBackup {
                module, version, ..
            } => {
                if module.is_empty() || version.is_empty() {
                    return Err(CheckpointError::VerificationFailed(
                        "driver checkpoint has no module/version".into(),
                    ));
                }
                Ok(())
            }
            _ => Err(CheckpointError::VerificationFailed(
                "wifi driver checkpoint must be a DriverBackup".into(),
            )),
        }
    }

    fn stage(&mut self, _checkpoint: &Checkpoint, candidate: &str) -> Result<(), StageError> {
        if candidate.trim().is_empty() {
            return Err(StageError::Internal(
                "no candidate driver module set before staging".into(),
            ));
        }
        self.control
            .load_module(candidate)
            .map_err(|e| StageError::Internal(format!("stage failed: {e}")))?;
        Ok(())
    }

    fn health_check(&self, _resource: &ResourceId) -> Result<HealthState, HealthError> {
        Ok(if self.control.link_state_up() {
            HealthState::Healthy
        } else {
            HealthState::Unhealthy
        })
    }

    fn commit(&mut self, _checkpoint: &Checkpoint) -> Result<(), CommitError> {
        // The candidate module is already loaded and healthy; commit is a
        // no-op at the driver layer (the state machine persists it). If a
        // persistence step is later needed (udev rules, config), it lands
        // here.
        Ok(())
    }

    fn rollback(&mut self, checkpoint: &Checkpoint) -> Result<(), RollbackError> {
        // Restore the checkpointed module: unload the candidate, reload the
        // checkpointed module.
        let previous = match &checkpoint.state {
            CheckpointState::DriverBackup {
                module, version, ..
            } => {
                let _ = version;
                module.clone()
            }
            _ => return Err(RollbackError::CheckpointMissing(uuid::Uuid::nil())),
        };
        let candidate_current = self.control.active_module();
        if candidate_current != previous {
            self.control
                .unload_module(&candidate_current)
                .map_err(|e| RollbackError::RestorationFailed(e.to_string()))?;
            self.control
                .load_module(&previous)
                .map_err(|e| RollbackError::RestorationFailed(e.to_string()))?;
        }
        if !self.control.link_state_up() {
            return Err(RollbackError::HealthCheckFailed(
                "link did not come back up after rollback".into(),
            ));
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), ResetError> {
        self.control
            .reset_device()
            .map_err(|e| ResetError::ResetFailed(e.to_string()))?;
        if !self.control.link_state_up() {
            return Err(ResetError::HealthCheckFailed(
                "link did not come back up after reset".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn driver(control: MockDriverControl) -> WifiDriverResourceDriver {
        WifiDriverResourceDriver {
            control: Box::new(control),
            device: ResourceId("device:net-wlp1s0".into()),
        }
    }

    #[test]
    fn checkpoint_captures_active_module() {
        let mut d = driver(MockDriverControl::new());
        let device = d.device.clone();
        let cp = d.create_checkpoint(&uuid::Uuid::new_v4(), &device).unwrap();
        match &cp.state {
            CheckpointState::DriverBackup { module, version, .. } => {
                assert_eq!(module, "iwlwifi");
                assert_eq!(version, "1.0.0");
            }
            other => panic!("expected DriverBackup, got {other:?}"),
        }
        d.verify_checkpoint(&cp).unwrap();
    }

    #[test]
    fn health_check_reflects_link_state() {
        let ok = driver(MockDriverControl::new());
        let bad = MockDriverControl {
            link_up: false,
            ..MockDriverControl::new()
        };
        let bad_d = driver(bad);
        assert_eq!(
            ok.health_check(&ok.device).unwrap(),
            HealthState::Healthy
        );
        assert_eq!(
            bad_d.health_check(&bad_d.device).unwrap(),
            HealthState::Unhealthy
        );
    }

    #[test]
    fn stage_loads_candidate_module() {
        let mut d = driver(MockDriverControl::new());
        let device = d.device.clone();
        let cp = d.create_checkpoint(&uuid::Uuid::new_v4(), &device).unwrap();
        d.stage(&cp, "mt7921e").expect("candidate stages");
        assert_eq!(d.control.active_module(), "mt7921e");
    }

    #[test]
    fn stage_requires_candidate() {
        let mut d = driver(MockDriverControl::new());
        let device = d.device.clone();
        let cp = d.create_checkpoint(&uuid::Uuid::new_v4(), &device).unwrap();
        assert!(d.stage(&cp, "  ").is_err(), "empty candidate must fail");
    }

    #[test]
    fn rollback_restores_checkpointed_module_when_stage_changed_it() {
        let mut d = driver(MockDriverControl::new());
        let device = d.device.clone();
        let cp = d.create_checkpoint(&uuid::Uuid::new_v4(), &device).unwrap();
        d.stage(&cp, "mt7921e").expect("candidate stages");
        d.rollback(&cp).expect("rollback restores checkpoint");
        assert_eq!(d.control.active_module(), "iwlwifi");
    }

    #[test]
    fn rollback_fails_when_link_does_not_return() {
        let mut control = MockDriverControl::new();
        control.link_up = false;
        let mut d = driver(control);
        let device = d.device.clone();
        let cp = d.create_checkpoint(&uuid::Uuid::new_v4(), &device).unwrap();
        d.stage(&cp, "mt7921e").expect("candidate stages");
        assert!(d.rollback(&cp).is_err());
    }

    #[test]
    fn mock_control_plans_commands() {
        let control = MockDriverControl::new();
        assert_eq!(control.plan_load("mt7921e"), "modprobe mt7921e");
        assert_eq!(control.plan_unload("mt7921e"), "modprobe -r mt7921e");
    }
}