use super::*;

impl Coordinator {
    pub(crate) fn configure_read_only_broker(&mut self) {
        let principal = self.session_principal.clone();
        let resource = ResourceId("system:graph".into());
        self.broker
            .set_resource_state(resource.clone(), ResourceState::Available);
        self.broker
            .set_resource_owner(resource.clone(), PrincipalId::system("discovery"));
        let operations = [
            (
                "observe",
                Operation::Observe,
                "observe discovered system state",
            ),
            (
                "diagnose",
                Operation::Diagnose,
                "diagnose discovered system state",
            ),
            ("query", Operation::Query, "query discovered system state"),
            ("deps", Operation::Query, "query dependencies"),
            ("impact", Operation::Query, "query impact relationships"),
            ("health", Operation::Query, "query graph health"),
        ];
        for (tool, operation, description) in operations {
            self.broker.register_tool(ToolDefinition {
                tool_id: tool.to_string(),
                specialist_package: "aios.discovery.read-only".into(),
                risk_level: operation.default_risk_level(),
                required_capabilities: vec![Capability {
                    resource: resource.clone(),
                    operation,
                }],
                description: description.into(),
            });
            self.broker.spawn_specialist(tool, {
                let graph = self.graph.clone();
                let tool = tool.to_string();
                std::sync::Arc::new(move |request| {
                    let args = tool_arguments(&request.parameters);
                    let graph = graph.read().expect("graph lock");
                    match ToolRegistry::new().run(&graph, &tool, &args) {
                        Ok(value) => crate::protocol::ToolResult {
                            envelope: crate::protocol::MessageEnvelope::new(
                                crate::protocol::MessageType::ToolResult,
                                PrincipalId::system("discovery"),
                                request.envelope.correlation_id,
                                request.envelope.data_classification,
                            ),
                            request_id: request.request_id,
                            status: crate::protocol::ToolStatus::Success,
                            data: Some(crate::protocol::ToolData::QueryResult {
                                data: serde_json::json!({"text": value.text}),
                            }),
                            error: None,
                            health_impact: None,
                        },
                        Err(error) => crate::protocol::ToolResult {
                            envelope: crate::protocol::MessageEnvelope::new(
                                crate::protocol::MessageType::ToolResult,
                                PrincipalId::system("discovery"),
                                request.envelope.correlation_id,
                                request.envelope.data_classification,
                            ),
                            request_id: request.request_id,
                            status: crate::protocol::ToolStatus::Failed,
                            data: None,
                            error: Some(crate::protocol::ToolError {
                                code: crate::protocol::ToolErrorCode::Internal,
                                message: error.to_string(),
                                recoverable: false,
                            }),
                            health_impact: None,
                        },
                    }
                })
            });
        }
        // Static session tokens are granted once at session start
        // (capability-model §6.3). The session holds them for its lifetime;
        // it does not pull tokens from the broker on demand (M1 carry-forward).
        let capabilities: Vec<Capability> = operations
            .into_iter()
            .map(|(_, operation, _)| Capability {
                resource: resource.clone(),
                operation,
            })
            .collect();
        self.broker
            .register_principal(principal.clone(), capabilities, Clearance::max());
        self.session_tokens = self
            .broker
            .client(principal.clone())
            .capability_tokens(&principal);
    }

    /// Instantiate the Aios control-plane nodes so every runtime component in
    /// `docs/ui.md` is a real `SystemGraph` node. Health is the genuine boot
    /// state; later health wiring updates these from runtime signals. A node
    /// whose signal is absent stays `Unknown` — never a silent green.
    pub(crate) fn ensure_control_plane_nodes(&mut self) {
        let t = now();
        let mut graph = self.graph.write().expect("graph lock");
        let definitions: &[(&str, NodeType, &str)] = &[
            ("facade", NodeType::Facade, "Facade"),
            ("coordinator", NodeType::Coordinator, "Coordinator"),
            ("planner", NodeType::PlannerAgent, "Planner"),
            ("verifier", NodeType::VerificationAgent, "Verifier"),
            ("broker", NodeType::Broker, "Broker"),
            ("gateway", NodeType::ModelGateway, "ModelGateway"),
            ("composer", NodeType::SurfaceComposer, "SurfaceComposer"),
            ("evidence", NodeType::EvidenceIndex, "EvidenceIndex"),
            ("validator", NodeType::SurfaceValidator, "SurfaceValidator"),
            ("staged", NodeType::StagedExecutor, "StagedExecutor"),
            ("audit", NodeType::AuditLog, "AuditLog"),
            ("tools", NodeType::ToolRegistry, "ToolRegistry"),
            ("graph", NodeType::SystemGraph, "SystemGraph"),
            ("guardian", NodeType::Guardian, "Guardian"),
            ("wifi", NodeType::Specialist, "WiFi Specialist"),
            ("storage", NodeType::Specialist, "Storage Specialist"),
            ("network", NodeType::Specialist, "Network Specialist"),
            ("drivers", NodeType::Specialist, "Drivers Specialist"),
            ("graphics", NodeType::Specialist, "Graphics Specialist"),
            ("memory", NodeType::Specialist, "Memory Specialist"),
            ("power", NodeType::Specialist, "Power Specialist"),
            ("processes", NodeType::Specialist, "Processes Specialist"),
            ("security", NodeType::Specialist, "Security Specialist"),
            ("boot", NodeType::Specialist, "Boot Specialist"),
            ("packages", NodeType::Specialist, "Packages Specialist"),
        ];
        for (id, node_type, label) in definitions {
            if graph.get_node(&NodeId(id.to_string())).is_some() {
                continue;
            }
            let mut node = NodeMetadata::new(
                NodeId(id.to_string()),
                *node_type,
                ProvenanceSource::Declared {
                    package: "aios.core".into(),
                },
                TrustLevel::Trusted,
                t,
            );
            node.label = label.to_string();
            node.health = match node_type {
                NodeType::Guardian | NodeType::Specialist => HealthState::Unknown,
                _ => HealthState::Healthy,
            };
            node.attributes.insert("package".into(), "aios.core".into());
            let _ = graph.add_node(node);
        }
    }

    /// Declare the real control/data edges between control-plane components
    /// so the sidebar topology reflects actual ownership and dispatch paths.
    /// Runs after specialists are instantiated. Tolerant of partial graphs:
    /// missing endpoints and duplicates are skipped, never invented.
    pub(crate) fn ensure_control_plane_edges(&mut self) {
        let t = now();
        let mut graph = self.graph.write().expect("graph lock");
        let specialist_ids: Vec<NodeId> = graph
            .nodes()
            .iter()
            .filter(|(_, node)| node.node_type == NodeType::Specialist)
            .map(|(id, _)| id.clone())
            .collect();
        let guardian_ids: Vec<NodeId> = graph
            .nodes()
            .iter()
            .filter(|(_, node)| node.node_type == NodeType::Guardian)
            .map(|(id, _)| id.clone())
            .collect();
        let link = |graph: &mut SystemGraph, from: &str, to: &NodeId, edge_type: EdgeType| {
            let source = NodeId(from.to_string());
            if graph.get_node(&source).is_none() || graph.get_node(to).is_none() {
                return;
            }
            if graph.has_edge(&source, to, edge_type) {
                return;
            }
            let edge = EdgeMetadata {
                edge_id: EdgeId::new(),
                edge_type,
                source_node: source,
                target_node: to.clone(),
                provenance: EdgeProvenance::Declared {
                    declared_by: PrincipalId::system("coordinator"),
                    package: "aios.core".into(),
                },
                created_at: t,
                last_observed: t,
                expires_at: None,
                attributes: HashMap::new(),
            };
            let _ = graph.add_edge(edge);
        };
        link(
            &mut graph,
            "facade",
            &NodeId("coordinator".into()),
            EdgeType::Owns,
        );
        for target in [
            "planner",
            "verifier",
            "broker",
            "gateway",
            "staged",
            "audit",
            "tools",
            "composer",
            "graph",
        ] {
            link(
                &mut graph,
                "coordinator",
                &NodeId(target.into()),
                EdgeType::Controls,
            );
        }
        for specialist in &specialist_ids {
            link(&mut graph, "coordinator", specialist, EdgeType::Controls);
            link(
                &mut graph,
                specialist.0.as_str(),
                &NodeId("graph".into()),
                EdgeType::Owns,
            );
            link(
                &mut graph,
                "broker",
                specialist,
                EdgeType::CommunicatesWith,
            );
        }
        for guardian in &guardian_ids {
            link(
                &mut graph,
                "broker",
                guardian,
                EdgeType::CommunicatesWith,
            );
        }
        link(
            &mut graph,
            "gateway",
            &NodeId("composer".into()),
            EdgeType::CommunicatesWith,
        );
        link(
            &mut graph,
            "composer",
            &NodeId("evidence".into()),
            EdgeType::CommunicatesWith,
        );
        link(
            &mut graph,
            "evidence",
            &NodeId("validator".into()),
            EdgeType::CommunicatesWith,
        );
    }

    pub fn tools_help(&self) -> String {
        self.tools.help()
    }

    pub fn plan_and_review(
        &self,
        intent: &str,
    ) -> Result<(crate::planner::GeneratedPlan, crate::verifier::ReviewResult), AgentError> {
        let result = (|| {
            let plan = self.planner.plan(intent)?;
            for (index, step) in plan.steps.iter().enumerate() {
                if step.risk != "read-only" {
                    return Err(AgentError::Format(format!(
                        "M4 only permits read-only plan steps; step {} has risk {}",
                        index + 1,
                        step.risk
                    )));
                }
            }
            self.report(
                GraphPhase::Verifying,
                &["coordinator", "verifier", "gateway"],
            );
            let review = self.verifier.review(&plan)?;
            Ok((plan, review))
        })();
        match &result {
            Ok((plan, review)) => self.record_audit(
                "user",
                "plan",
                intent,
                &format!("ok ({} steps, {:?})", plan.steps.len(), review.verdict),
            ),
            Err(e) => self.record_audit("user", "plan", intent, &format!("error: {e}")),
        }
        result
    }

    /// Issue a broker-owned approval request covering a single reset action
    /// (human-interaction §1.4: the broker stores the approval, never the
    /// facade). The returned message id is what the user responds to. The
    /// scope binds exactly the given action, resource, operation, and tool.
    pub fn issue_reset_approval(
        &self,
        action_id: crate::protocol::ActionId,
        plan_hash: [u8; 32],
        resource: ResourceId,
        tool_id: String,
    ) -> Result<(uuid::Uuid, crate::protocol::PlanHash), String> {
        let plan_id = uuid::Uuid::new_v4();
        let request = crate::protocol::ApprovalRequest {
            envelope: crate::protocol::MessageEnvelope::new(
                crate::protocol::MessageType::ApprovalRequest,
                PrincipalId::user(),
                uuid::Uuid::new_v4(),
                DataClassification::Protected,
            ),
            plan_id,
            plan_hash,
            plan_summary: format!("reset device {resource} to known-good state"),
            affected_systems: vec![resource.clone()],
            expected_risks: vec!["driver reload".into()],
            rollback_state: None,
            // Risk-4 recovery approval window is 5 minutes (human-interaction
            // §3.3).
            expires_at: crate::protocol::now() + 300_000,
        };
        let scope = crate::protocol::ApprovalScope {
            actions: vec![crate::protocol::ApprovedAction {
                action_id,
                resource: resource.clone(),
                operation: Operation::Reset,
                tool_id: tool_id.clone().into(),
            }],
            resources: vec![resource.clone()],
            operations: vec![Operation::Reset],
        };
        let mut core = self.broker.core().lock().expect("broker lock");
        let request_id = core
            .issue_approval_request(request, scope)
            .map_err(|e| format!("{e:?}"))?;
        Ok((request_id, plan_hash))
    }

    /// Submit the user's decision through the broker-owned channel. Only the
    /// broker may record an approval; the facade only relays the yes/no
    /// (human-interaction §1).
    pub fn submit_approval(&self, approval_request_id: uuid::Uuid, approved: bool) -> String {
        let response = crate::protocol::UserResponse {
            envelope: crate::protocol::MessageEnvelope::new(
                crate::protocol::MessageType::UserResponse,
                PrincipalId::user(),
                uuid::Uuid::new_v4(),
                DataClassification::Protected,
            ),
            approval_request_id,
            decision: if approved {
                crate::protocol::UserDecision::Approved
            } else {
                crate::protocol::UserDecision::Rejected("user denied reset".into())
            },
        };
        let mut core = self.broker.core().lock().expect("broker lock");
        match core.submit_user_response(response) {
            Ok(()) => {
                self.record_audit(
                    "user",
                    "approval",
                    &format!("{approval_request_id}"),
                    "approved",
                );
                "approval recorded".to_string()
            }
            Err(e) => format!("approval failed: {e:?}"),
        }
    }

    pub fn scan(&self) -> String {
        let mut graph = crate::discovery::SysfsDiscovery::new()
            .scan()
            .map_err(|e| e.to_string());
        let summary = match graph {
            Ok(ref mut graph) => {
                let service_warning =
                    match crate::discovery::ServiceDiscovery::new().populate(graph, now()) {
                        Ok(_) => None,
                        Err(error) => {
                            self.record_audit(
                                "facade",
                                "service-discovery",
                                "systemd",
                                &format!("unavailable: {error}"),
                            );
                            Some(error.to_string())
                        }
                    };
                *self.graph.write().expect("graph lock") = graph.clone();
                let mut text = scan_summary(graph);
                if let Some(error) = service_warning {
                    text.push_str(&format!("\nservice discovery unavailable: {error}"));
                }
                self.last_scan_summary
                    .write()
                    .expect("scan lock")
                    .replace(text.clone());
                self.record_audit("facade", "scan", "system", "ok");
                text
            }
            Err(e) => {
                self.record_audit("facade", "scan", "system", &format!("error: {e}"));
                let text = format!("scan failed: {e}");
                self.last_scan_summary
                    .write()
                    .expect("scan lock")
                    .replace(text.clone());
                text
            }
        };
        summary
    }

    pub fn graph_summary(&self) -> String {
        let graph = self.graph.read().expect("graph lock");
        scan_summary(&graph)
    }

    pub fn state_panel(&self) -> String {
        crate::panel::render(&crate::panel::snapshot(self))
    }
}

fn scan_summary(graph: &SystemGraph) -> String {
    let devices = graph.get_nodes_by_type(NodeType::Device).len();
    let services = graph.get_nodes_by_type(NodeType::Service).len();
    let sensors = graph.get_nodes_by_type(NodeType::Sensor).len();
    let cpus = graph.get_nodes_by_type(NodeType::Cpu).len();
    let total = graph.nodes().len();
    format!(
        "scanned: {total} nodes ({devices} devices, {services} services, {sensors} sensors, {cpus} cpus)"
    )
}
/// Seed the enforcement-plane nodes (Guardian, capabilities, policies) in the
/// graph so the security specialist has a domain to own. Unlike hardware
/// nodes, these are not sysfs-discovered — they always exist as part of the
/// Aios enforcement plane (security-model.md). Idempotent: existing nodes are
/// left untouched.
pub(crate) fn seed_security_domain(graph: &mut SystemGraph) {
    let t = now();
    let seed = |graph: &mut SystemGraph, id: &str, node_type: NodeType, label: &str| {
        let node_id = crate::graph::NodeId(id.into());
        if graph.get_node(&node_id).is_some() {
            return;
        }
        let mut node = crate::graph::NodeMetadata::new(
            node_id,
            node_type,
            crate::graph::ProvenanceSource::Declared {
                package: "aios.enforcement".into(),
            },
            crate::graph::TrustLevel::Trusted,
            t,
        );
        node.label = label.into();
        node.health = HealthState::Healthy;
        let _ = graph.add_node(node);
    };
    seed(
        graph,
        "guardian:0",
        NodeType::Guardian,
        "Infrastructure Guardian",
    );
    seed(
        graph,
        "capability:session",
        NodeType::Capability,
        "session capability",
    );
    seed(graph, "policy:broker", NodeType::Policy, "broker policy");
}

pub(crate) fn seed_boot_domain(graph: &mut SystemGraph) {
    let t = now();
    let seed = |graph: &mut SystemGraph, id: &str, node_type: NodeType, label: &str| {
        let node_id = crate::graph::NodeId(id.into());
        if graph.get_node(&node_id).is_some() {
            return;
        }
        let mut node = crate::graph::NodeMetadata::new(
            node_id,
            node_type,
            crate::graph::ProvenanceSource::Declared {
                package: "aios.enforcement".into(),
            },
            crate::graph::TrustLevel::Trusted,
            t,
        );
        node.label = label.into();
        node.health = HealthState::Healthy;
        let _ = graph.add_node(node);
    };
    seed(
        graph,
        "bootimage:primary",
        NodeType::BootImage,
        "primary boot image",
    );
    seed(
        graph,
        "snapshot:pre-update",
        NodeType::Snapshot,
        "pre-update snapshot",
    );
    seed(graph, "watchdog:0", NodeType::Watchdog, "boot watchdog");
}
