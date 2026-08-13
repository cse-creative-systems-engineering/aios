use crate::action::{
    Checkpoint, CheckpointError, CheckpointState, CommitError, HealthError, ResetError,
    RollbackError, StageError,
};
use crate::capability::ResourceId;
use crate::executor::ResourceDriver;
use crate::protocol::{ActionId, HealthState};
use std::path::PathBuf;

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

/// A `DriverControl` backed by the live system: kernel module and link state
/// are read from sysfs, and the mutating commands are dry-run by default
/// (execute = false) so Aios never touches the running kernel on its own.
/// The planned commands are recorded and can be executed by the user on the
/// wired-connected machine (safety boundary, modules/wifi.md). Execution is
/// opt-in per instance so tests stay hermetic against a fake sysfs root.
pub struct LinuxDriverControl {
    root: PathBuf,
    interface: String,
    execute: bool,
    planned: Vec<String>,
}

impl LinuxDriverControl {
    pub fn new(interface: impl Into<String>) -> Self {
        Self {
            root: PathBuf::from("/"),
            interface: interface.into(),
            execute: false,
            planned: Vec::new(),
        }
    }

    pub fn with_root(mut self, root: PathBuf) -> Self {
        self.root = root;
        self
    }

    pub fn with_execute(mut self, execute: bool) -> Self {
        self.execute = execute;
        self
    }

    /// The commands recorded since construction (or since the last clear).
    pub fn planned(&self) -> &[String] {
        &self.planned
    }

    fn link_path(&self) -> PathBuf {
        self.root
            .join("sys")
            .join("class")
            .join("net")
            .join(&self.interface)
    }

    fn read_first_line(&self, path: PathBuf) -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        Some(text.trim().to_string())
    }

    fn run(command: &str, args: &[&str]) -> Result<(), String> {
        let output = std::process::Command::new(command)
            .args(args)
            .output()
            .map_err(|e| format!("{command} unavailable: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "{command} {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        }
    }
}

impl DriverControl for LinuxDriverControl {
    fn active_module(&self) -> String {
        // The interface's PCI/USB device owns the driver, which owns the
        // module: sys/class/net/<iface>/device/driver/module/name.
        self.read_first_line(
            self.link_path()
                .join("device")
                .join("driver")
                .join("module")
                .join("name"),
        )
        .unwrap_or_else(|| "unknown".into())
    }

    fn module_version(&self, module: &str) -> Option<String> {
        self.read_first_line(self.root.join("sys").join("module").join(module).join("version"))
    }

    fn load_module(&mut self, module: &str) -> Result<(), String> {
        let plan = self.plan_load(module);
        self.planned.push(plan.clone());
        if !self.execute {
            return Ok(());
        }
        // modprobe may need to be run with privileges; when it cannot, the
        // error is surfaced to the staged executor for rollback.
        Self::run("modprobe", &[module])
    }

    fn unload_module(&mut self, module: &str) -> Result<(), String> {
        let plan = self.plan_unload(module);
        self.planned.push(plan.clone());
        if !self.execute {
            return Ok(());
        }
        Self::run("modprobe", &["-r", module])
    }

    fn link_state_up(&self) -> bool {
        // carrier is the kernel's ground truth for a live link (NETWORK-002);
        // fall back to operstate for interfaces without carrier reporting.
        match self.read_first_line(self.link_path().join("carrier")) {
            Some(value) => value == "1",
            None => self
                .read_first_line(self.link_path().join("operstate"))
                .map(|state| state == "up")
                .unwrap_or(false),
        }
    }

    fn reset_device(&mut self) -> Result<(), String> {
        // A device reset re-binds the module (unload + load); still planned
        // unless execute is opted in, same safety boundary as load/unload.
        let active = self.active_module();
        let plan = format!("{} && {}", self.plan_unload(&active), self.plan_load(&active));
        self.planned.push(plan);
        if !self.execute {
            return Ok(());
        }
        Self::run("modprobe", &["-r", &active])?;
        Self::run("modprobe", &[&active])
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

    // Build a fake sysfs root describing one interface with a driver module.
    fn fake_sysfs() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("sysfs tempdir");
        let link = dir
            .path()
            .join("sys/class/net/wlp1s0/device/driver/module");
        std::fs::create_dir_all(&link).expect("module dir");
        std::fs::write(link.join("name"), "mt7921e\n").expect("module name");
        let module_dir = dir.path().join("sys/module/mt7921e");
        std::fs::create_dir_all(&module_dir).expect("module dir");
        std::fs::write(module_dir.join("version"), "2.3.4\n").expect("module version");
        dir
    }

    fn live_control(dir: &std::path::Path) -> LinuxDriverControl {
        LinuxDriverControl::new("wlp1s0")
            .with_root(dir.to_path_buf())
            .with_execute(false)
    }

    #[test]
    fn live_control_reads_module_and_version_from_sysfs() {
        let dir = fake_sysfs();
        let control = live_control(dir.path());
        assert_eq!(control.active_module(), "mt7921e");
        assert_eq!(control.module_version("mt7921e").as_deref(), Some("2.3.4"));
        assert_eq!(control.module_version("other"), None);
    }

    #[test]
    fn live_control_unknown_without_device_path() {
        let dir = tempfile::tempdir().expect("sysfs tempdir");
        let control = live_control(dir.path());
        assert_eq!(control.active_module(), "unknown");
        assert_eq!(control.module_version("mt7921e"), None);
    }

    #[test]
    fn live_link_state_reads_carrier() {
        let dir = fake_sysfs();
        let up = dir.path().join("sys/class/net/wlp1s0/carrier");
        std::fs::write(&up, "1\n").expect("carrier up");
        assert!(live_control(dir.path()).link_state_up());
        std::fs::write(&up, "0\n").expect("carrier down");
        assert!(!live_control(dir.path()).link_state_up());
    }

    #[test]
    fn live_link_state_falls_back_to_operstate() {
        let dir = fake_sysfs();
        let state = dir.path().join("sys/class/net/wlp1s0/operstate");
        std::fs::create_dir_all(state.parent().expect("net dir")).expect("net dir");
        std::fs::write(&state, "up\n").expect("operstate");
        assert!(live_control(dir.path()).link_state_up());
        std::fs::write(&state, "down\n").expect("operstate");
        assert!(!live_control(dir.path()).link_state_up());
    }

    #[test]
    fn live_control_plans_mutations_without_executing() {
        let dir = fake_sysfs();
        let mut control = live_control(dir.path());
        control.load_module("iwlwifi").expect("plan-only load");
        control.unload_module("mt7921e").expect("plan-only unload");
        control.reset_device().expect("plan-only reset");
        assert_eq!(
            control.planned(),
            vec![
                "modprobe iwlwifi",
                "modprobe -r mt7921e",
                "modprobe -r mt7921e && modprobe mt7921e",
            ]
        );
        // The kernel was not touched: reads still report the sysfs truth.
        assert_eq!(control.active_module(), "mt7921e");
    }

    #[test]
    fn live_execute_load_failure_maps_to_error() {
        let dir = fake_sysfs();
        let mut control = live_control(dir.path()).with_execute(true);
        // A module name that cannot exist: modprobe fails (or is unavailable,
        // e.g. unprivileged) and the error must surface, never panic.
        assert!(control.load_module("aios_definitely_not_a_module").is_err());
    }
}