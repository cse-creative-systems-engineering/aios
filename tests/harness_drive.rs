use aios::config::AiosConfig;
use aios::coordinator::Coordinator;
use std::path::PathBuf;

/// Test harness that drives Aios without the UI - reproduces the exact
/// chat error you hit and lets me iterate locally.
/// Run: cargo test --test harness_drive -- --nocapture
/// Or:  AIOS_WORKSPACE=/tmp/ws cargo test --test harness_drive -- --nocapture
#[test]
fn harness_repro_file_write() {
    let ws = std::env::var("AIOS_WORKSPACE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            PathBuf::from(home).join("workspace")
        });
    std::fs::create_dir_all(&ws).unwrap();
    let target = ws.join("hello.py");
    let _ = std::fs::remove_file(&target);
    let cfg = AiosConfig::load().unwrap_or_default();
    let coord = Coordinator::boot_with(cfg).expect("Coordinator boot");
    // Exact path that failed in UI: natural language
    let r = coord.run_tool(
        "files.write_file",
        "file:/workspace/hello.py hello from aios",
    );
    match &r {
        Ok(ok) => println!("run_tool OK: {}", ok.text),
        Err(e) => println!("run_tool ERR: {}", e),
    }
    assert!(r.is_ok(), "files.write_file should succeed, got {:?}", r);
    let text = r.unwrap().text;
    assert!(
        text.contains("committed=true"),
        "expected committed, got {}",
        text
    );
    assert!(target.exists(), "file not on disk at {:?}", target);
    let content = std::fs::read_to_string(&target).unwrap();
    assert!(
        content.contains("hello from aios"),
        "content mismatch {}",
        content
    );
    println!(
        "HARNESS OK: staged->committed file:/workspace/hello.py -> {:?}",
        target
    );
}

/// Read-only file tools must have a runtime handler registered at boot
/// (regression: observe_file used to fail with "no specialist for
/// files.observe_file" because only the tool definition was registered).
#[test]
fn harness_files_observe_has_handler() {
    let cfg = AiosConfig::load().unwrap_or_default();
    let coord = Coordinator::boot_with(cfg).expect("Coordinator boot");
    let r = coord.run_tool("files.observe_file", "file:/workspace/hello.py");
    assert!(r.is_ok(), "files.observe_file should succeed, got {:?}", r);
    let text = r.unwrap().text;
    assert!(!text.contains("no specialist"), "handler missing: {}", text);
    assert!(
        text.contains("target:"),
        "unexpected observe output: {}",
        text
    );
}

/// Direct broker-level harness for when you don't want Coordinator boot at all
#[test]
fn harness_direct_broker() {
    use aios::broker::{Broker, BrokerClient};
    use aios::capability::{Capability, Clearance, Operation, PrincipalId, ResourceId, RiskLevel};
    use aios::files::FileDriver;
    use aios::guardian::Guardian;
    use tempfile::TempDir;
    let tmp = TempDir::new().unwrap();
    let ws = tmp.path().join("workspace");
    let arts = tmp.path().join("artifacts");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::create_dir_all(&arts).unwrap();
    let store_dir = tmp.path().join("store");
    std::fs::create_dir_all(&store_dir).unwrap();
    let broker = Broker::new();
    broker.core().lock().unwrap().set_clock(|| 2000);
    let principal = PrincipalId::agent("files.specialist", "harness");
    let cap = Capability {
        resource: ResourceId("file:/workspace".into()),
        operation: Operation::Write,
    };
    let token = aios::capability::CapabilityToken {
        principal: principal.clone(),
        capability: cap.clone(),
        clearance: Clearance(RiskLevel::Staged),
        granted_at: 1000,
        expires_at: 999999,
        provenance: aios::capability::Provenance {
            granted_by: PrincipalId::system("policy-broker"),
            package_id: "files.specialist".into(),
            package_version: 1,
            signature_verified: true,
        },
    };
    broker.register_tool(aios::capability::ToolDefinition {
        tool_id: "files.write_file".into(),
        specialist_package: "files.specialist".into(),
        risk_level: RiskLevel::Staged,
        required_capabilities: vec![cap.clone()],
        description: "write".into(),
    });
    broker.register_principal(
        principal.clone(),
        vec![cap.clone()],
        Clearance(RiskLevel::Recovery),
    );
    broker.set_resource_state(
        ResourceId("file:/workspace".into()),
        aios::capability::ResourceState::Available,
    );
    broker.set_resource_owner(ResourceId("file:/workspace".into()), principal.clone());
    broker.set_guardian(Guardian::new());
    let store = aios::action::FileActionStore::new(&store_dir).unwrap();
    broker.set_executor(aios::executor::StagedExecutor::new(
        Box::new(store),
        Box::new(FileDriver::with_roots(ws.clone(), arts)),
    ));
    let req = aios::broker::build_request(
        principal.clone(),
        ResourceId("file:/workspace/hello.py".into()),
        Operation::Write,
        "files.write_file",
        &token,
        aios::protocol::ToolParameters::Stage {
            change: serde_json::json!({"content": "harness content"}),
        },
        uuid::Uuid::new_v4(),
        10,
    );
    let res = broker.client(principal).request_tool(req).unwrap();
    assert_eq!(res.status, aios::protocol::ToolStatus::Success);
    assert!(ws.join("hello.py").exists());
    println!("HARNESS_DIRECT OK");
}
