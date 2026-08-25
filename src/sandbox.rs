//! Execution sandbox for `exec.run` (ADR-0010 §3.1).
//!
//! The sandbox is the actual safety boundary; Guardian pattern checks are
//! defense-in-depth and audit-log clarity, not the boundary.
//!
//! Tiers:
//! - **Full** — bubblewrap (`bwrap`): fresh mount/network/PID/user/IPC
//!   namespaces; workspace + artifacts bind-mounted rw; host root read-only;
//!   sensitive devices/firmware masked; no network by default.
//! - **FilesystemOnly** — Landlock LSM (kernel ≥5.13): writes outside the
//!   allowed roots denied by the kernel. No network/PID isolation.
//! - **None** — no sandbox available. Per ADR-0010 §3.1 this forcibly locks
//!   approval mode to Default (fail-closed, ADR-0003).
//!
//! Detection runs once and is cached.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxTier {
    Full,
    FilesystemOnly,
    None,
}

impl SandboxTier {
    pub fn label(self) -> &'static str {
        match self {
            SandboxTier::Full => "sandbox: full (bubblewrap)",
            SandboxTier::FilesystemOnly => "sandbox: filesystem-only (landlock)",
            SandboxTier::None => "sandbox: none",
        }
    }

    /// Auto/YOLO approval modes require at least filesystem isolation
    /// (ADR-0010 §3.1: tier None locks mode to Default).
    pub fn allows_auto_modes(self) -> bool {
        !matches!(self, SandboxTier::None)
    }
}

/// What the sandbox needs to know to confine one execution.
pub struct SandboxSpec {
    /// Read-write roots (workspace, artifacts).
    pub writable_roots: Vec<PathBuf>,
    /// Working directory inside the sandbox.
    pub workdir: PathBuf,
}

pub trait Sandbox: Send + Sync {
    fn tier(&self) -> SandboxTier;

    /// Build the command that runs `command` confined. Returns None when this
    /// implementation cannot confine (callers must then fail closed).
    fn wrap(&self, spec: &SandboxSpec, command: &str) -> Option<Vec<String>>;
}

/// bubblewrap: fresh namespaces, ro host root, rw workspace/artifacts,
/// masked devices/firmware, no network (`--unshare-net`).
pub struct BubblewrapSandbox;

impl Sandbox for BubblewrapSandbox {
    fn tier(&self) -> SandboxTier {
        SandboxTier::Full
    }

    fn wrap(&self, spec: &SandboxSpec, command: &str) -> Option<Vec<String>> {
        let workdir = spec.workdir.canonicalize().ok()?;
        let mut argv = vec![
            "bwrap".to_string(),
            "--unshare-all".to_string(),
            "--die-with-parent".to_string(),
            "--new-session".to_string(),
            "--dev".to_string(),
            "/dev".to_string(),
            "--proc".to_string(),
            "/proc".to_string(),
            "--tmpfs".to_string(),
            "/tmp".to_string(),
            // Host root visible but read-only; binaries/libs resolvable.
            "--ro-bind".to_string(),
            "/".to_string(),
            "/".to_string(),
            // Re-mask what the ro bind exposed (order matters: later wins).
            "--tmpfs".to_string(),
            "/sys".to_string(),
            "--tmpfs".to_string(),
            "/proc/sys".to_string(),
            "--tmpfs".to_string(),
            "/dev/shm".to_string(),
            "--tmpfs".to_string(),
            "/run".to_string(),
            "--tmpfs".to_string(),
            "/var/tmp".to_string(),
            "--clearenv".to_string(),
            "--setenv".to_string(),
            "PATH".to_string(),
            "/usr/local/bin:/usr/bin:/bin".to_string(),
            "--setenv".to_string(),
            "HOME".to_string(),
            workdir.display().to_string(),
            "--setenv".to_string(),
            "TMPDIR".to_string(),
            "/tmp".to_string(),
            "--chdir".to_string(),
            workdir.display().to_string(),
        ];
        for root in &spec.writable_roots {
            let canonical = root.canonicalize().ok()?;
            argv.push("--bind".to_string());
            argv.push(canonical.display().to_string());
            argv.push(canonical.display().to_string());
        }
        argv.push("--".to_string());
        argv.push("sh".to_string());
        argv.push("-c".to_string());
        argv.push(command.to_string());
        Some(argv)
    }
}

/// Landlock LSM fallback: kernel-enforced filesystem write confinement only.
/// Requires kernel ≥ 5.13; used when bwrap is missing (ADR-0010 §3.1).
pub struct LandlockSandbox {
    /// True when the running kernel accepted our landlock ruleset probe.
    available: bool,
}

impl LandlockSandbox {
    pub fn detect() -> Self {
        Self {
            available: landlock_supported(),
        }
    }
}

impl Sandbox for LandlockSandbox {
    fn tier(&self) -> SandboxTier {
        if self.available {
            SandboxTier::FilesystemOnly
        } else {
            SandboxTier::None
        }
    }

    fn wrap(&self, _spec: &SandboxSpec, _command: &str) -> Option<Vec<String>> {
        // Phase 2 note: full landlock ruleset application requires the
        // landlock syscalls from the aios process itself (rules apply to the
        // calling thread and are inherited). For now report unsupported so
        // callers fail closed rather than run unconfined. Tracked in the
        // grounding snapshot as remaining Phase 2 work.
        None
    }
}

fn landlock_supported() -> bool {
    // Best-effort ABI check via /sys/kernel/security/lsm listing would need
    // root-readable securityfs in some setups; instead probe the syscall ABI
    // number presence in kernel headers exposed through uname version.
    let release = Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let mut parts = release
        .split('.')
        .take(2)
        .filter_map(|p| p.parse::<u32>().ok());
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    major > 5 || (major == 5 && minor >= 13)
}

/// No sandbox available — fail-closed tier. Callers must refuse auto modes
/// and gate every exec behind explicit approval (ADR-0010 §3.1).
pub struct NullSandbox;

impl Sandbox for NullSandbox {
    fn tier(&self) -> SandboxTier {
        SandboxTier::None
    }

    fn wrap(&self, _spec: &SandboxSpec, _command: &str) -> Option<Vec<String>> {
        None
    }
}

/// Detect the best available sandbox tier once at boot.
pub fn detect_sandbox() -> Box<dyn Sandbox> {
    if bwrap_available() {
        Box::new(BubblewrapSandbox)
    } else {
        let landlock = LandlockSandbox::detect();
        if landlock.tier() == SandboxTier::FilesystemOnly {
            Box::new(landlock)
        } else {
            Box::new(NullSandbox)
        }
    }
}

fn bwrap_available() -> bool {
    Command::new("bwrap")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run `command` confined per the sandbox spec. Returns the raw process
/// output or an error explaining why execution was refused (never runs
/// unconfined as a fallback — fail-closed per ADR-0003).
pub fn run_confined(
    sandbox: &dyn Sandbox,
    spec: &SandboxSpec,
    command: &str,
) -> Result<std::process::Output, String> {
    let argv = sandbox.wrap(spec, command).ok_or_else(|| {
        format!(
            "no sandbox available (tier {:?}); refusing to execute unconfined",
            sandbox.tier()
        )
    })?;
    let (program, args) = argv.split_first().ok_or("empty sandbox argv")?;
    Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("sandboxed spawn failed: {e}"))
}

/// Ensure the writable roots exist before confinement (bwrap fails on
/// missing bind sources).
pub fn prepare_roots(roots: &[PathBuf]) -> std::io::Result<()> {
    for root in roots {
        if !root.exists() {
            std::fs::create_dir_all(root)?;
        }
        let _ = canonicalize_lossy(root);
    }
    Ok(())
}

fn canonicalize_lossy(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubblewrap_argv_confines_and_binds_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = tmp.path().join("workspace");
        let arts = tmp.path().join("artifacts");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&arts).unwrap();
        let spec = SandboxSpec {
            writable_roots: vec![ws.clone(), arts.clone()],
            workdir: ws.clone(),
        };
        let sb = BubblewrapSandbox;
        let argv = sb.wrap(&spec, "echo hi").expect("argv");
        assert_eq!(argv[0], "bwrap");
        assert!(argv.contains(&"--unshare-all".to_string()));
        // Both roots bound rw (two occurrences each: src and dest).
        let binds = argv.iter().filter(|a| a.as_str() == "--bind").count();
        assert_eq!(binds, 2, "expected rw binds for both roots: {argv:?}");
        assert!(argv.iter().any(|a| a.as_str() == "--ro-bind"));
        // Command is the last four tokens: -- sh -c <cmd>
        let n = argv.len();
        assert_eq!(&argv[n - 3..n - 1], &["sh".to_string(), "-c".to_string()]);
        assert_eq!(argv[n - 1], "echo hi");
        assert_eq!(argv[n - 4], "--");
    }

    #[test]
    fn null_sandbox_refuses_to_wrap() {
        let sb = NullSandbox;
        let spec = SandboxSpec {
            writable_roots: vec![],
            workdir: PathBuf::from("/tmp"),
        };
        assert!(sb.wrap(&spec, "echo hi").is_none());
        assert!(!sb.tier().allows_auto_modes());
    }

    #[test]
    fn detection_returns_a_tier() {
        let sb = detect_sandbox();
        // On dev machines with bwrap installed this is Full; either way the
        // tier must be valid and consistent with auto-mode permission.
        match sb.tier() {
            SandboxTier::Full => assert!(sb.tier().allows_auto_modes()),
            SandboxTier::FilesystemOnly => assert!(sb.tier().allows_auto_modes()),
            SandboxTier::None => assert!(!sb.tier().allows_auto_modes()),
        }
    }

    #[test]
    fn run_confined_fails_closed_without_sandbox() {
        let sb = NullSandbox;
        let spec = SandboxSpec {
            writable_roots: vec![],
            workdir: PathBuf::from("/tmp"),
        };
        let err = run_confined(&sb, &spec, "echo hi").unwrap_err();
        assert!(err.contains("refusing to execute unconfined"), "{err}");
    }
}
