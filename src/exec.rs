//! Exec specialist — `exec.run` (workspace co-partner full co-user).
//! Runs any shell command via PolicyBroker → Guardian (Staged) with audit.
//! No prefix cap — any command allowed, but gated as Staged so Guardian logs and health checks exit code.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::files::{artifacts_root, workspace_root};
use crate::graph::{NodeId, NodeMetadata, NodeType, SystemGraph, TrustLevel};
use crate::protocol::{
    DataClassification, HealthState, MessageEnvelope, MessageType, ToolData, ToolParameters,
    ToolStatus,
};
use crate::sandbox::{self, SandboxSpec, SandboxTier};
use std::sync::OnceLock;

pub const PACKAGE_ID: &str = "exec.specialist";
pub const EXEC_RESOURCE: &str = "exec:command";

/// Sandbox detected once per process (ADR-0010 §3.1: detection at boot).
static SANDBOX: OnceLock<Box<dyn sandbox::Sandbox>> = OnceLock::new();

fn sandbox() -> &'static dyn sandbox::Sandbox {
    SANDBOX.get_or_init(sandbox::detect_sandbox).as_ref()
}

/// ADR-0010 §3.2 Guardian pattern list — defense-in-depth, NOT the safety
/// boundary. A match denies before the sandbox spawns; a miss does not imply
/// safety. EXEC-P-005 (secret leakage) carries real safety semantics.
fn guardian_pattern_check(command: &str) -> Result<(), String> {
    let lower = command.to_ascii_lowercase();
    // EXEC-P-001: rm on system roots.
    for root in [
        "/boot", "/etc", "/usr", "/var", "/root", "/sys", "/proc", "/lib", "/lib64", "/bin",
        "/sbin",
    ] {
        if lower.contains("rm ")
            && (lower.contains(&format!("rm -rf {root}"))
                || lower.contains(&format!("rm -fr {root}"))
                || lower.contains(&format!("rm -r {root}"))
                || lower.contains(&format!("rm {root}")))
        {
            return Err(format!("EXEC-P-001: rm on system root {root}"));
        }
    }
    // EXEC-P-002: block-device writers co-occurring with a device path.
    let block_writers = [
        "dd",
        "mkfs.",
        "wipefs",
        "shred",
        "blkdiscard",
        "sgdisk",
        "parted",
        "fdisk",
        "cfdisk",
        "sfdisk",
    ];
    let device_paths = [
        "/dev/sd",
        "/dev/nvme",
        "/dev/mmcblk",
        "/dev/vd",
        "/dev/xvd",
        "/dev/dm-",
    ];
    if block_writers.iter().any(|w| lower.contains(w))
        && device_paths.iter().any(|d| lower.contains(d))
    {
        return Err("EXEC-P-002: block-device write".into());
    }
    // EXEC-P-003: firmware writes through the exec path.
    if lower.contains("flashrom")
        || lower.contains("fwupdmgr install")
        || lower.contains("dfu-util")
        || lower.contains(".ucode")
    {
        return Err("EXEC-P-003: firmware write via exec path".into());
    }
    // EXEC-P-004: redirects into kernel/security/firmware pseudo-filesystems.
    for target in [
        "/sys/firmware",
        "/sys/kernel/security",
        "/proc/sys/kernel",
        "/dev/mem",
        "/dev/kmem",
    ] {
        if (lower.contains(">") || lower.contains("tee ")) && lower.contains(target) {
            return Err(format!("EXEC-P-004: redirect into {target}"));
        }
    }
    Ok(())
}

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
            required_capabilities: vec![Capability {
                resource: ResourceId(EXEC_RESOURCE.into()),
                operation: Operation::Execute,
            }],
            description: "run any shell command, capture stdout/stderr and exit code".into(),
        }]
    }

    pub fn handle_exec(
        &self,
        request: &crate::protocol::ToolRequest,
    ) -> crate::protocol::ToolResult {
        let cmd = match &request.parameters {
            ToolParameters::Execute { command } => command.clone(),
            _ => return err_result(request, "exec.run expects Execute { command }"),
        };
        if cmd.trim().is_empty() {
            return err_result(request, "empty command");
        }
        // Defense-in-depth first (ADR-0010 §3.2): deny before spawn so the
        // audit log shows a clear typed reason.
        if let Err(pattern) = guardian_pattern_check(&cmd) {
            return err_result(request, &format!("denied: {pattern}"));
        }
        // The sandbox is the boundary (ADR-0010 §3.1). No unconfined fallback.
        let workspace = workspace_root();
        let artifacts = artifacts_root();
        if let Err(e) = sandbox::prepare_roots(&[workspace.clone(), artifacts.clone()]) {
            return err_result(request, &format!("sandbox roots unavailable: {e}"));
        }
        let spec = SandboxSpec {
            writable_roots: vec![workspace.clone(), artifacts.clone()],
            workdir: workspace,
        };
        let tier = sandbox().tier();
        let output = match sandbox::run_confined(sandbox(), &spec, &cmd) {
            Ok(out) => out,
            Err(e) => return err_result(request, &e),
        };
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let status = output.status.code().unwrap_or(-1);
        let mut text = format!(
            "exec exit={status} tier={tier:?} cmd={cmd:?}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
        if text.len() > 8000 {
            text.truncate(8000);
            text.push_str("\n...truncated");
        }
        let data = serde_json::json!({ "command": cmd, "exit_code": status, "stdout": stdout, "stderr": stderr, "combined": text, "sandbox_tier": format!("{tier:?}") });
        success_result(request, ToolData::QueryResult { data })
    }
}

fn success_result(
    request: &crate::protocol::ToolRequest,
    data: ToolData,
) -> crate::protocol::ToolResult {
    crate::protocol::ToolResult {
        envelope: MessageEnvelope::new(
            MessageType::ToolResult,
            PrincipalId::system(PACKAGE_ID),
            request.envelope.correlation_id,
            DataClassification::Public,
        ),
        request_id: request.request_id,
        status: ToolStatus::Success,
        data: Some(data),
        error: None,
        health_impact: None,
    }
}
fn err_result(request: &crate::protocol::ToolRequest, msg: &str) -> crate::protocol::ToolResult {
    crate::protocol::ToolResult {
        envelope: MessageEnvelope::new(
            MessageType::ToolResult,
            PrincipalId::system(PACKAGE_ID),
            request.envelope.correlation_id,
            DataClassification::Public,
        ),
        request_id: request.request_id,
        status: ToolStatus::Failed,
        data: None,
        error: Some(crate::protocol::ToolError {
            code: crate::protocol::ToolErrorCode::Internal,
            message: msg.into(),
            recoverable: false,
        }),
        health_impact: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Capability, Operation, ResourceId};
    use crate::protocol::now;
    use crate::protocol::{
        DataClassification, MessageEnvelope, MessageType, ToolParameters, ToolStatus,
    };
    /// Unix-only: requires a sandbox tier that can spawn. On Windows no
    /// sandbox exists yet (W4: Job Objects / AppContainer per ADR-0011), and
    /// `run_confined` correctly refuses to execute unconfined, so a success
    /// assertion cannot hold there.
    #[test]
    #[cfg(unix)]
    fn exec_runs_echo() {
        let mut g = SystemGraph::new();
        let sp = ExecSpecialist::instantiate(&mut g).unwrap();
        let req = crate::protocol::ToolRequest {
            envelope: MessageEnvelope::new(
                MessageType::ToolRequest,
                PrincipalId::system("test"),
                uuid::Uuid::new_v4(),
                DataClassification::SystemConfig,
            ),
            request_id: uuid::Uuid::new_v4(),
            principal: PrincipalId::system("test"),
            resource: ResourceId(EXEC_RESOURCE.into()),
            operation: Operation::Execute,
            tool_id: "exec.run".into(),
            capability_token: crate::capability::CapabilityToken {
                principal: PrincipalId::system("test"),
                capability: Capability {
                    resource: ResourceId(EXEC_RESOURCE.into()),
                    operation: Operation::Execute,
                },
                clearance: crate::capability::Clearance(crate::capability::RiskLevel::Staged),
                granted_at: now(),
                expires_at: now() + 100000,
                provenance: crate::capability::Provenance {
                    granted_by: PrincipalId::system("policy-broker"),
                    package_id: PACKAGE_ID.into(),
                    package_version: 1,
                    signature_verified: true,
                },
            },
            parameters: ToolParameters::Execute {
                command: "echo hello-exec".into(),
            },
            plan_hash: None,
            action_id: None,
            nonce: 0,
        };
        let r = sp.handle_exec(&req);
        assert_eq!(r.status, ToolStatus::Success);
        let data = r.data.unwrap();
        if let ToolData::QueryResult { data } = data {
            assert!(
                data.get("stdout")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .contains("hello-exec")
            );
        } else {
            panic!("wrong data");
        }
    }

    fn request_with(command: &str) -> crate::protocol::ToolRequest {
        crate::protocol::ToolRequest {
            envelope: MessageEnvelope::new(
                MessageType::ToolRequest,
                PrincipalId::system("test"),
                uuid::Uuid::new_v4(),
                DataClassification::SystemConfig,
            ),
            request_id: uuid::Uuid::new_v4(),
            principal: PrincipalId::system("test"),
            resource: ResourceId(EXEC_RESOURCE.into()),
            operation: Operation::Execute,
            tool_id: "exec.run".into(),
            capability_token: crate::capability::CapabilityToken {
                principal: PrincipalId::system("test"),
                capability: Capability {
                    resource: ResourceId(EXEC_RESOURCE.into()),
                    operation: Operation::Execute,
                },
                clearance: crate::capability::Clearance(crate::capability::RiskLevel::Staged),
                granted_at: now(),
                expires_at: now() + 100000,
                provenance: crate::capability::Provenance {
                    granted_by: PrincipalId::system("policy-broker"),
                    package_id: PACKAGE_ID.into(),
                    package_version: 1,
                    signature_verified: true,
                },
            },
            parameters: ToolParameters::Execute {
                command: command.into(),
            },
            plan_hash: None,
            action_id: None,
            nonce: 0,
        }
    }

    /// ADR-0010 §7: `sandbox_bubblewrap_confines_write` — a write to /etc
    /// fails with a filesystem permission error from inside the sandbox, not
    /// because the regex caught it. The sandbox is the boundary.
    #[test]
    fn sandbox_confines_write_outside_workspace() {
        let mut g = SystemGraph::new();
        let sp = ExecSpecialist::instantiate(&mut g).unwrap();
        if sandbox().tier() != SandboxTier::Full {
            // Landlock/None tiers refuse to spawn; that is also fail-closed.
            let r = sp.handle_exec(&request_with("touch /etc/aios-sandbox-probe"));
            assert_eq!(r.status, ToolStatus::Failed);
            return;
        }
        let r = sp.handle_exec(&request_with("touch /etc/aios-sandbox-probe"));
        assert_eq!(
            r.status,
            ToolStatus::Success,
            "command ran; exit code carries the failure"
        );
        let data = match r.data.unwrap() {
            ToolData::QueryResult { data } => data,
            other => panic!("wrong data {other:?}"),
        };
        let exit = data.get("exit_code").unwrap().as_i64().unwrap();
        assert_ne!(exit, 0, "/etc write must fail inside the sandbox");
        let stderr = data.get("stderr").unwrap().as_str().unwrap();
        assert!(
            stderr.contains("Read-only")
                || stderr.contains("Permission")
                || stderr.contains("EACCES")
                || stderr.contains("EROFS"),
            "expected filesystem denial, got: {stderr}"
        );
    }

    /// ADR-0010 §7: workspace writes succeed through the rw bind.
    #[test]
    fn sandbox_allows_workspace_write() {
        let mut g = SystemGraph::new();
        let sp = ExecSpecialist::instantiate(&mut g).unwrap();
        if sandbox().tier() != SandboxTier::Full {
            return; // tier without bwrap: skip (fail-closed path tested above)
        }
        let r = sp.handle_exec(&request_with(
            "echo sandbox-ok > proof.txt && cat proof.txt",
        ));
        assert_eq!(r.status, ToolStatus::Success);
        let data = match r.data.unwrap() {
            ToolData::QueryResult { data } => data,
            other => panic!("wrong data {other:?}"),
        };
        let stdout = data.get("stdout").unwrap().as_str().unwrap();
        assert!(stdout.contains("sandbox-ok"), "{stdout}");
    }

    /// ADR-0010 §7: `guardian_pattern_matches_are_pre_sandbox` — pattern hits
    /// deny before spawn with a typed reason.
    #[test]
    fn guardian_patterns_deny_before_spawn() {
        let mut g = SystemGraph::new();
        let sp = ExecSpecialist::instantiate(&mut g).unwrap();
        for cmd in [
            "rm -rf /etc",
            "dd if=/dev/zero of=/dev/sda",
            "flashrom -w bios.rom",
            "echo x > /sys/firmware/foo",
        ] {
            let r = sp.handle_exec(&request_with(cmd));
            assert_eq!(r.status, ToolStatus::Failed, "{cmd} must be denied");
            let msg = r.error.unwrap().message;
            assert!(msg.starts_with("denied: EXEC-P-"), "{cmd}: {msg}");
        }
    }

    /// Variable indirection defeats regexes but not the sandbox — this is why
    /// the sandbox is the boundary and patterns are only clarity (§3.2).
    #[test]
    fn indirect_rm_still_confined_by_sandbox() {
        let mut g = SystemGraph::new();
        let sp = ExecSpecialist::instantiate(&mut g).unwrap();
        if sandbox().tier() != SandboxTier::Full {
            return;
        }
        // Not caught by EXEC-P-001 (indirection), but the sandbox confines it.
        let r = sp.handle_exec(&request_with("X=/etc; rm -rf $X/passwd"));
        let data = match r.data.unwrap() {
            ToolData::QueryResult { data } => data,
            other => panic!("wrong data {other:?}"),
        };
        let exit = data.get("exit_code").unwrap().as_i64().unwrap();
        assert_ne!(exit, 0, "indirect rm must still fail inside the sandbox");
    }
}
