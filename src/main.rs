use aios::action::FileActionStore;
use aios::broker::{Broker, BrokerClient, build_request};
use aios::capability::{
    Capability, Clearance, Operation, PrincipalId, ResourceId, ResourceState, RiskLevel,
    ToolDefinition,
};
use aios::executor::StagedExecutor;
use aios::graph::{
    EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, ProvenanceSource,
    SystemGraph, TrustLevel,
};
use aios::guardian::Guardian;
use aios::mocks::{
    MockPlanner, MockVerificationAgent, MockWifiDriver, storage_specialist, wifi_specialist,
};
use aios::protocol::{
    ActionPlan, Approval, ApprovalScope, ApprovedAction, DataClassification, MessageEnvelope,
    MessageType, PlannedAction, RiskAssessment, ToolData, ToolResult, ToolStatus, now,
};
use std::sync::atomic::Ordering;

const WIFI: &str = "device:wifi0";
const NVME: &str = "device:nvme0";

fn register_tools(broker: &Broker) {
    let tools = vec![
        ToolDefinition {
            tool_id: "wifi.observe_device".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Observe,
            }],
            description: "read wifi device state".into(),
        },
        ToolDefinition {
            tool_id: "wifi.diagnose_fault".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Diagnose,
            }],
            description: "diagnose a wifi fault".into(),
        },
        ToolDefinition {
            tool_id: "wifi.stage_driver".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::Staged,
            required_capabilities: vec![Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Stage,
            }],
            description: "stage a wifi driver change".into(),
        },
        ToolDefinition {
            tool_id: "wifi.load_module".into(),
            specialist_package: "wifi.specialist".into(),
            risk_level: RiskLevel::Critical,
            required_capabilities: vec![Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::KernelModule,
            }],
            description: "load a kernel driver module".into(),
        },
        ToolDefinition {
            tool_id: "storage.observe_drive".into(),
            specialist_package: "storage.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![Capability {
                resource: ResourceId(NVME.into()),
                operation: Operation::Observe,
            }],
            description: "read nvme drive state".into(),
        },
        ToolDefinition {
            tool_id: "storage.check_smart".into(),
            specialist_package: "storage.specialist".into(),
            risk_level: RiskLevel::ReadOnly,
            required_capabilities: vec![Capability {
                resource: ResourceId(NVME.into()),
                operation: Operation::Query,
            }],
            description: "query nvme smart data".into(),
        },
    ];
    for tool in tools {
        broker.register_tool(tool);
    }
}

fn register_principals(broker: &Broker) {
    broker.register_principal(
        PrincipalId::agent("planner", "planner-001"),
        vec![
            Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Observe,
            },
            Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Diagnose,
            },
            Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Stage,
            },
            Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::KernelModule,
            },
            Capability {
                resource: ResourceId(NVME.into()),
                operation: Operation::Observe,
            },
            Capability {
                resource: ResourceId(NVME.into()),
                operation: Operation::Query,
            },
        ],
        Clearance(RiskLevel::Critical),
    );
    broker.register_principal(
        PrincipalId::agent("wifi.specialist", "wifi0"),
        vec![
            Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Observe,
            },
            Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Diagnose,
            },
            Capability {
                resource: ResourceId(WIFI.into()),
                operation: Operation::Stage,
            },
        ],
        Clearance(RiskLevel::Staged),
    );
    broker.set_resource_state(ResourceId(WIFI.into()), ResourceState::Available);
    broker.set_resource_state(ResourceId(NVME.into()), ResourceState::Available);
    broker.set_resource_owner(
        ResourceId(WIFI.into()),
        PrincipalId::agent("wifi.specialist", "wifi0"),
    );
}

fn setup_guardian() -> Guardian {
    let mut guardian = Guardian::new();
    guardian.mark_driver_tested("iwlwifi-next");
    guardian
}

fn spawn_specialists(broker: &Broker) {
    for (tool, handler) in [
        ("wifi.observe_device", wifi_specialist as fn(_) -> _),
        ("wifi.diagnose_fault", wifi_specialist),
        ("storage.observe_drive", storage_specialist),
        ("storage.check_smart", storage_specialist),
    ] {
        broker.spawn_specialist(tool, std::sync::Arc::new(handler));
    }
}

fn describe(result: &ToolResult) -> String {
    match result.status {
        ToolStatus::Success => match &result.data {
            Some(ToolData::DeviceState { state, metrics }) => {
                let m: Vec<String> = metrics
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                format!("success state={state:?} {}", m.join(" "))
            }
            Some(ToolData::Diagnosis { findings, confidence }) => format!(
                "success confidence={confidence} findings=[{}]",
                findings.join(" | ")
            ),
            Some(ToolData::QueryResult { data }) => format!("success data={data}"),
            Some(ToolData::CommitResult { .. }) => "success committed".into(),
            Some(ToolData::StagedChange { id, .. }) => format!("success staged {id}"),
            _ => "success".into(),
        },
        ToolStatus::Denied => format!(
            "denied: {}",
            result.error.as_ref().map(|e| e.message.as_str()).unwrap_or("policy")
        ),
        ToolStatus::RolledBack => format!(
            "rolled back: {}",
            result.error.as_ref().map(|e| e.message.as_str()).unwrap_or("health failed")
        ),
        _ => format!(
            "failed: {}",
            result.error.as_ref().map(|e| e.message.as_str()).unwrap_or("error")
        ),
    }
}

fn build_demo_plan() -> ActionPlan {
    ActionPlan {
        envelope: MessageEnvelope::new(
            MessageType::ActionPlan,
            PrincipalId::agent("planner", "planner-001"),
            uuid::Uuid::new_v4(),
            DataClassification::SystemConfig,
        ),
        plan_id: uuid::Uuid::new_v4(),
        user_intent: "wifi is flaky, stage a newer driver".into(),
        actions: vec![PlannedAction {
            action_id: uuid::Uuid::new_v4(),
            tool_request: Box::new(build_request(
                PrincipalId::agent("planner", "planner-001"),
                ResourceId(WIFI.into()),
                Operation::Stage,
                "wifi.stage_driver",
                &stage_token(),
                aios::protocol::ToolParameters::Stage {
                    change: serde_json::json!({ "module": "iwlwifi-next" }),
                },
                uuid::Uuid::new_v4(),
                1,
            )),
            description: "stage iwlwifi-next".into(),
            risk_level: RiskLevel::Staged,
        }],
        affected_systems: vec![ResourceId(WIFI.into())],
        expected_risks: vec![RiskAssessment {
            resource: ResourceId(WIFI.into()),
            risk: "driver swap may drop link".into(),
            severity: aios::protocol::InvariantSeverity::Availability,
        }],
        rollback_state: None,
    }
}

fn stage_token() -> aios::capability::CapabilityToken {
    aios::capability::CapabilityToken {
        principal: PrincipalId::agent("planner", "planner-001"),
        capability: Capability {
            resource: ResourceId(WIFI.into()),
            operation: Operation::Stage,
        },
        clearance: Clearance(RiskLevel::Staged),
        granted_at: 0,
        expires_at: u64::MAX,
        provenance: aios::capability::Provenance {
            granted_by: PrincipalId::system("policy-broker"),
            package_id: "planner".into(),
            package_version: 1,
            signature_verified: true,
        },
    }
}

fn build_graph() -> SystemGraph {
    let t = now();
    let mut graph = SystemGraph::new();
    let wifi_node = NodeId("device:wifi0".into());
    let driver_node = NodeId("driver:iwlwifi".into());
    let firmware_node = NodeId("firmware:iwlwifi-46".into());
    let bus_node = NodeId("bus:pci0000:00".into());
    let service_node = NodeId("service:networkd".into());
    let planner_node = NodeId("agent:planner".into());
    let verifier_node = NodeId("agent:verifier".into());

    for (id, kind, _label) in [
        (wifi_node.clone(), NodeType::Device, "wifi0 (Intel AX210)".to_string()),
        (driver_node.clone(), NodeType::Driver, "iwlwifi".to_string()),
        (firmware_node.clone(), NodeType::Firmware, "iwlwifi-46.ucode".to_string()),
        (bus_node.clone(), NodeType::Bus, "PCIe 00:14.3".to_string()),
        (service_node.clone(), NodeType::Service, "systemd-networkd".to_string()),
        (planner_node.clone(), NodeType::PlannerAgent, "planner".to_string()),
        (verifier_node.clone(), NodeType::VerificationAgent, "verifier".to_string()),
    ] {
        graph
            .add_node(NodeMetadata::new(
                id,
                kind,
                ProvenanceSource::Declared {
                    package: "wifi.specialist".into(),
                },
                TrustLevel::Trusted,
                t,
            ))
            .expect("node added");
    }
    for (from, to, edge_type) in [
        (wifi_node.clone(), driver_node.clone(), EdgeType::DependsOn),
        (wifi_node.clone(), firmware_node.clone(), EdgeType::DependsOn),
        (wifi_node.clone(), bus_node.clone(), EdgeType::DependsOn),
        (wifi_node.clone(), service_node.clone(), EdgeType::DependsOn),
        (planner_node.clone(), wifi_node.clone(), EdgeType::Observes),
        (verifier_node.clone(), wifi_node.clone(), EdgeType::Observes),
    ] {
        graph
            .add_edge(EdgeMetadata {
                edge_id: aios::graph::EdgeId::new(),
                edge_type,
                source_node: from,
                target_node: to,
                provenance: EdgeProvenance::Declared {
                    declared_by: PrincipalId::system("mock-discovery"),
                    package: "wifi.specialist".into(),
                },
                created_at: t,
                last_observed: t,
                expires_at: None,
                attributes: Default::default(),
            })
            .expect("edge added");
    }
    graph
}

fn main() {
    let state_dir = std::env::temp_dir().join("aios-demo-state");
    let _ = std::fs::remove_dir_all(&state_dir);
    let store = FileActionStore::new(&state_dir).expect("action store init");

    let driver = MockWifiDriver::new();
    let health_ok = driver.health_ok.clone();
    let executor = StagedExecutor::new(Box::new(store), Box::new(driver));

    let broker = Broker::new();
    register_tools(&broker);
    broker.set_guardian(setup_guardian());
    broker.set_executor(executor);
    spawn_specialists(&broker);
    register_principals(&broker);

    let client = broker.client(PrincipalId::agent("planner", "planner-001"));
    let mut planner = MockPlanner::new(client);

    println!("== aios demo: mock planner drives the broker ==");

    let observe = planner.observe_wifi();
    println!("observe wifi0      -> {}", describe(&observe));

    let diagnose = planner.diagnose_wifi();
    println!("diagnose wifi0     -> {}", describe(&diagnose));

    let smart = planner.query_storage();
    println!("query nvme0 smart  -> {}", describe(&smart));

    let plan = build_demo_plan();
    let verifier = MockVerificationAgent;
    let report = verifier.review(&plan);
    println!(
        "verifier reviews plan -> {:?} ({} test)",
        report.verdict,
        report.recommended_tests.join(", ")
    );

    let staged = planner.stage_driver("iwlwifi-next");
    println!("stage driver        -> {}", describe(&staged));

    health_ok.store(false, Ordering::Relaxed);
    let failed = planner.stage_driver("iwlwifi-next");
    println!("stage + bad health  -> {}", describe(&failed));
    health_ok.store(true, Ordering::Relaxed);

    let untested = kernel_module_request(&broker, "iwlwifi-bleeding-edge", None);
    println!("load untested module-> {}", describe(&untested));

    let action_id = uuid::Uuid::new_v4();
    let plan_hash = [0x42u8; 32];
    let approval = Approval {
        envelope: MessageEnvelope::new(
            MessageType::Approval,
            PrincipalId::user(),
            uuid::Uuid::new_v4(),
            DataClassification::Protected,
        ),
        approval_id: uuid::Uuid::new_v4(),
        plan_id: uuid::Uuid::new_v4(),
        plan_hash,
        approved_by: PrincipalId::user(),
        granted_at: now(),
        expires_at: now() + 3600,
        scope: ApprovalScope {
            actions: vec![ApprovedAction {
                action_id,
                resource: ResourceId(WIFI.into()),
                operation: Operation::KernelModule,
                tool_id: "wifi.load_module".into(),
            }],
            resources: vec![ResourceId(WIFI.into())],
            operations: vec![Operation::KernelModule],
        },
    };
    broker.add_approval(approval);
    let approved = kernel_module_request(&broker, "iwlwifi-next", Some((plan_hash, action_id)));
    println!("load approved module-> {}", describe(&approved));

    let graph = build_graph();
    let wifi_node = NodeId("device:wifi0".into());
    let subgraph = graph.get_subgraph(&wifi_node, 1).expect("subgraph");
    println!(
        "system graph subgraph of wifi0 -> {} nodes, {} edges",
        subgraph.nodes.len(),
        subgraph.edges.len()
    );
    let deps: Vec<String> = graph
        .get_dependencies(&wifi_node)
        .into_iter()
        .map(|n| n.node_id.to_string())
        .collect();
    println!("wifi0 dependencies -> {}", deps.join(", "));
    if let Some(impact) = graph.analyze_impact(&wifi_node) {
        println!("wifi0 impact report -> {}", impact.risk_assessment);
    }

    let audit_count = broker
        .core()
        .lock()
        .expect("broker lock")
        .audit_entries()
        .len();
    println!("policy decisions logged -> {audit_count}");
    println!("== done ==");
}

fn kernel_module_request(
    broker: &Broker,
    module: &str,
    approval: Option<([u8; 32], uuid::Uuid)>,
) -> aios::protocol::ToolResult {
    let principal = PrincipalId::agent("planner", "planner-001");
    let token = broker
        .client(principal.clone())
        .capability_tokens(&principal)
        .into_iter()
        .find(|t| t.capability.operation == Operation::KernelModule)
        .expect("kernel module token");
    let mut req = build_request(
        principal.clone(),
        ResourceId(WIFI.into()),
        Operation::KernelModule,
        "wifi.load_module",
        &token,
        aios::protocol::ToolParameters::KernelModule {
            action: "load".into(),
            module: module.into(),
        },
        uuid::Uuid::new_v4(),
        9000 + module.len() as u64,
    );
    if let Some((hash, action_id)) = approval {
        req.plan_hash = Some(hash);
        req.action_id = Some(action_id);
    }
    broker
        .client(principal)
        .request_tool(req)
        .expect("kernel module request")
}
