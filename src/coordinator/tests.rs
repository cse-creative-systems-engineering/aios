    use super::*;
    use super::chat::tool_arguments;
    use crate::broker::build_request;
    use crate::capability::{RiskLevel, ToolDefinition};
    use crate::config::ProviderConfig;
    use crate::graph::NodeId;
    use crate::guardian::Guardian;
    use crate::mocks::MockWifiDriver;
    use crate::protocol::{
        DataClassification, HealthState, ToolData, ToolErrorCode, ToolParameters, ToolStatus,
        VerificationVerdict,
    };
    use crate::testutil;

    struct FakeProbe(ConnectivityState);

    impl ConnectivityProbe for FakeProbe {
        fn probe(&self) -> ConnectivityState {
            self.0
        }
    }

    fn stub_config(port: u16) -> AiosConfig {
        AiosConfig {
            model: None,
            shell: None,
            roles: None,
            provider: vec![ProviderConfig {
                id: "stub".into(),
                kind: "openai-compatible".into(),
                tier: "internet".into(),
                model: Some("stub-model".into()),
                endpoint: Some(format!("http://127.0.0.1:{port}")),
                api_key: None,
                api_key_env: None,
                capabilities: None,
                http_timeout_ms: 5000,
            }],
        }
    }

    fn stub_coordinator(port: u16) -> Coordinator {
        let coordinator = Coordinator::boot_with_probe(
            stub_config(port),
            Box::new(FakeProbe(ConnectivityState::Internet)),
        )
        .expect("boot");
        // The router no longer tier-ranks: tests assign the stub provider to
        // the roles they exercise, exactly like the settings panel would.
        let provider = ProviderId::new("stub");
        let model = ModelId::new("stub-model");
        coordinator
            .gateway
            .router()
            .set_assignment("chat", provider.clone(), model.clone())
            .expect("chat assignment");
        coordinator
            .gateway
            .router()
            .set_assignment("surface", provider.clone(), model.clone())
            .expect("surface assignment");
        coordinator
            .gateway
            .router()
            .set_assignment("verification", provider, model)
            .expect("verification assignment");
        coordinator
    }

    fn handler(body: &str) -> String {
        if body.contains("steps: ") {
            testutil::openai_response(r#"{"verdict":"approve","concerns":[],"tests":["ping"]}"#)
        } else if body.contains("fix my wifi") {
            testutil::openai_response(
                r#"{"intent":"fix my wifi","steps":[{"description":"check link","tool":"iw dev","resource":"wifi0","risk":"read-only"}]}"#,
            )
        } else {
            testutil::openai_response("hello from stub")
        }
    }

    #[test]
    fn boots_http_provider_and_status_shows_it() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let text = status_text(&coordinator);
        assert!(text.contains("connectivity: Internet"), "{text}");
        assert!(text.contains("stub"), "{text}");
        assert!(text.contains("stub-model"), "{text}");
        let route = coordinator.current_route().expect("route");
        assert_eq!(route.provider, ProviderId::new("stub"));
    }

    #[test]
    fn chat_returns_provider_text() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let answer = coordinator.chat("hello").expect("chat");
        assert_eq!(answer, "hello from stub");
    }

    #[test]
    fn surface_relay_returns_generated_html() {
        let handler = move |request: &str| {
            // Structural guard: the generation call must never be offered tools.
            assert!(
                !request.contains("\"tools\"") && !request.contains("tool_calls"),
                "surface relay advertised tools"
            );
            assert!(request.contains("generative UI designer"));
            assert!(request.contains("Available fields"));
            testutil::openai_response("<section data-tauri-drag-region><span data-aios=\"used_percent\">42%</span></section>")
        };
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let evidence = vec![crate::tools::ToolResult {
            tool: "disk",
            text: "used_percent=42".into(),
        }];
        let (html, _route) = coordinator
            .compose_unconstrained_html("how is the disk", &evidence, None)
            .expect("html");
        assert!(html.contains("data-aios=\"used_percent\""), "{html}");
    }

    #[test]
    fn surface_relay_passes_previous_design_for_edits() {
        let handler = |request: &str| {
            assert!(request.contains("Previous generated design:"), "{request}");
            testutil::openai_response("<p>revised</p>")
        };
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let evidence = vec![crate::tools::ToolResult {
            tool: "disk",
            text: "used_percent=42".into(),
        }];
        let (html, _) = coordinator
            .compose_unconstrained_html("how is the disk", &evidence, Some("<p>old</p>"))
            .expect("html");
        assert_eq!(html, "<p>revised</p>");
    }

    #[test]
    fn surface_relay_rejects_empty_model_reply() {
        let handler = |_: &str| testutil::openai_response("");
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let error = coordinator
            .compose_unconstrained_html("how is the disk", &[], None)
            .expect_err("must fail");
        assert!(
            matches!(error, crate::surface::SurfaceComposeError::EmptyResponse),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn system_context_is_consent_gated() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let context = coordinator.local_context().expect("implicit consent");
        assert!(context.contains("scanned:"), "{context}");
        assert!(context.contains("devices:"), "{context}");
        coordinator.revoke_consent("stub");
        assert!(coordinator.local_context().is_none());
    }

    #[test]
    fn chat_tool_loop_executes_and_returns_final_answer() {
        let port = testutil::spawn_json_server(|body| {
            if body.contains("tool health result") {
                testutil::openai_response("machine looks healthy")
            } else {
                testutil::openai_response(r#"{"tool_calls":[{"tool":"health","args":""}]}"#)
            }
        });
        let coordinator = stub_coordinator(port);
        let answer = coordinator
            .chat_with_tools(vec![
                ModelMessage::new(ModelRole::System, "You are Aios."),
                ModelMessage::new(ModelRole::User, "check machine"),
            ])
            .expect("chat");
        assert_eq!(answer, "machine looks healthy");
        assert!(
            coordinator
                .audit
                .filter("planner")
                .iter()
                .any(|entry| entry.action == "tool")
        );
    }

    #[test]
    fn plan_and_review_roundtrip() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let (plan, review) = coordinator.plan_and_review("fix my wifi").expect("plan");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].risk, "read-only");
        assert_eq!(review.verdict, VerificationVerdict::Approve);
    }

    #[test]
    fn mutating_plan_is_rejected_before_verifier_in_m4() {
        let port = testutil::spawn_json_server(|body| {
            if body.contains("mutate the wifi") {
                testutil::openai_response(
                    r#"{"intent":"mutate the wifi","steps":[{"description":"stage driver","tool":"stage_driver","resource":"wifi0","risk":"staged"}]}"#,
                )
            } else {
                testutil::openai_response("unexpected")
            }
        });
        let coordinator = stub_coordinator(port);
        let error = coordinator
            .plan_and_review("mutate the wifi")
            .expect_err("M4 must reject mutating plans");
        assert!(error.to_string().contains("only permits read-only"));
    }

    #[test]
    fn consent_grant_and_revoke() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        coordinator
            .grant_consent("stub", DataClassification::SystemConfig)
            .expect("grant");
        let record = coordinator.consent_for("stub").expect("record");
        assert!(record.is_active_for(DataClassification::SystemConfig));
        coordinator.revoke_consent("stub");
        let record = coordinator.consent_for("stub").expect("record");
        assert!(!record.is_active_for(DataClassification::SystemConfig));
    }

    #[test]
    fn missing_local_model_marks_provider_unhealthy() {
        let config = AiosConfig {
            model: None,
            shell: None,
            roles: None,
            provider: vec![ProviderConfig {
                id: "local".into(),
                kind: "local".into(),
                tier: "local".into(),
                model: Some("definitely-missing.gguf".into()),
                endpoint: None,
                api_key: None,
                api_key_env: None,
                capabilities: None,
                http_timeout_ms: 5000,
            }],
        };
        let coordinator =
            Coordinator::boot_with_probe(config, Box::new(FakeProbe(ConnectivityState::Offline)))
                .expect("boot");
        // A missing local model file marks the provider unhealthy at boot;
        // routing itself no longer probes health, the gateway does per submit.
        let entries = coordinator.provider_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].health.state, HealthState::Unhealthy);
    }

    #[test]
    fn group_assignment_applies_to_every_specialist() {
        // persist_config writes to AIOS_CONFIG; point it at a scratch file so
        // the test never touches a real config. Safe here: no other test in
        // this binary reads AIOS_CONFIG (they all pass config directly).
        let config_path = std::env::temp_dir().join(format!(
            "aios-test-group-assignment-{}.toml",
            std::process::id()
        ));
        unsafe {
            std::env::set_var("AIOS_CONFIG", &config_path);
        }

        let port = testutil::spawn_json_server(|body| {
            if body.is_empty() {
                // GET {endpoint}/models
                serde_json::json!({
                    "data": [{ "id": "stub-model" }, { "id": "other-model" }]
                })
                .to_string()
            } else {
                testutil::openai_response("hello from stub")
            }
        });
        let mut coordinator = stub_coordinator(port);
        // Discovery needs a credential; setting one also warms the catalogue.
        coordinator
            .set_provider_credential("stub", "sk-test".into())
            .expect("credential");

        let assigned = coordinator
            .set_role_group_assignment("specialists", "stub", "other-model")
            .expect("group assignment");
        assert_eq!(assigned.len(), 11);
        assert!(assigned.contains(&"specialist:wifi".to_string()));

        let route = coordinator
            .role_route("specialist:packages")
            .expect("valid role")
            .expect("assigned");
        assert_eq!(route.provider, ProviderId::new("stub"));
        assert_eq!(route.model, ModelId::new("other-model"));

        let roles = coordinator.config.roles.as_ref().expect("roles");
        assert_eq!(roles.specialists.len(), 11);

        assert!(coordinator
            .set_role_group_assignment("nope", "stub", "stub-model")
            .is_err());
        assert!(coordinator
            .set_role_group_assignment("all", "stub", "missing-model")
            .is_err());

        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn removed_provider_leaves_no_ghost_and_can_be_added_again() {
        // persist_config writes to AIOS_CONFIG; point it at a scratch file so
        // the test never touches a real config. Safe here: no other test in
        // this binary reads AIOS_CONFIG (they all pass config directly).
        let config_path = std::env::temp_dir().join(format!(
            "aios-test-remove-readd-{}.toml",
            std::process::id()
        ));
        unsafe {
            std::env::set_var("AIOS_CONFIG", &config_path);
        }

        let mut coordinator = stub_coordinator(9);
        coordinator.remove_provider("stub").expect("remove");
        assert!(coordinator.config.provider("stub").is_none());
        assert!(coordinator
            .provider_entries()
            .iter()
            .all(|entry| entry.provider.to_string() != "stub"));

        // The registry must not keep a ghost entry, or the re-add fails with
        // "model already registered" while key updates fail with "not
        // configured" — exactly the broken state users cannot escape.
        coordinator
            .add_provider(
                "stub".into(),
                "openai-compatible".into(),
                "internet".into(),
                Some("http://127.0.0.1:9".into()),
                Some("stub-model".into()),
                None,
                None,
            )
            .expect("re-add after remove");
        assert!(coordinator.config.provider("stub").is_some());
        assert_eq!(coordinator.provider_entries().len(), 1);

        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn bad_config_fails_boot() {        let config = AiosConfig {
            model: None,
            shell: None,
            roles: None,
            provider: vec![ProviderConfig {
                id: "x".into(),
                kind: "openai-compatible".into(),
                tier: "space".into(),
                model: Some("m".into()),
                endpoint: Some("https://x.example/v1".into()),
                api_key: None,
                api_key_env: None,
                capabilities: None,
                http_timeout_ms: 5000,
            }],
        };
        assert!(matches!(
            Coordinator::boot_with_probe(config, Box::new(FakeProbe(ConnectivityState::Offline))),
            Err(BootError::Config(_))
        ));
    }

    #[test]
    fn missing_key_env_fails_boot() {
        let config = AiosConfig {
            model: None,
            shell: None,
            roles: None,
            provider: vec![ProviderConfig {
                id: "deepseek".into(),
                kind: "openai-compatible".into(),
                tier: "internet".into(),
                model: Some("deepseek-chat".into()),
                endpoint: Some("https://api.deepseek.com/v1".into()),
                api_key: None,
                api_key_env: Some("AIOS_DEFINITELY_MISSING_KEY".into()),
                capabilities: None,
                http_timeout_ms: 5000,
            }],
        };
        assert!(
            Coordinator::boot_with_probe(config, Box::new(FakeProbe(ConnectivityState::Offline)))
                .is_err()
        );
    }

    #[test]
    fn panel_renders_live_snapshot() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let text = coordinator.state_panel();
        assert!(text.contains("== aios system state =="), "{text}");
        assert!(text.contains("connectivity:"), "{text}");
        assert!(text.contains("route:"), "{text}");
        assert!(text.contains("graph:"), "{text}");
        assert!(text.contains("recovery:"), "{text}");
        assert!(text.contains("subsystems:"), "{text}");
    }

    // M7: the storage specialist must be reachable through the broker like the
    // wifi read-only tools. This helper seeds storage nodes into the graph
    // when the machine discovered none, instantiates the specialist, and
    // registers its tools + session grants exactly like boot does.
    fn wire_storage(coordinator: &mut Coordinator) {
        if coordinator.storage_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            let has_storage = graph.nodes().values().any(|node| {
                node.node_type == NodeType::Filesystem || node.label.starts_with("block device ")
            });
            if !has_storage {
                let mut nvme = crate::graph::NodeMetadata::new(
                    NodeId("device:nvme0n1".into()),
                    NodeType::Device,
                    crate::graph::ProvenanceSource::Discovered {
                        via: "sysfs".into(),
                    },
                    crate::graph::TrustLevel::Trusted,
                    crate::protocol::now(),
                );
                nvme.label = "block device nvme0n1".into();
                nvme.attributes
                    .insert("size_bytes".into(), "500107862016".into());
                graph.add_node(nvme).unwrap();
            }
            crate::storage::StorageSpecialist::instantiate(&mut graph).unwrap();
        }
        let specialist = coordinator.storage_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_storage") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator.broker.set_resource_state(
            ResourceId("storage:domain".into()),
            ResourceState::Available,
        );
        let principal =
            PrincipalId::agent(crate::storage::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("storage:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("storage:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("storage:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("storage:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn storage_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_storage(&mut coordinator);
        let call = ToolCallRequest {
            name: "storage.observe_storage".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("storage observe through broker");
        assert!(result.text.contains("block_devices"), "{}", result.text);
    }

    #[test]
    fn storage_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_storage(&mut coordinator);
        let call = ToolCallRequest {
            name: "storage.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("storage diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // M7: the network umbrella must be reachable through the broker like the
    // wifi and storage read-only tools. Boot wires it when the machine has
    // wired interfaces or bluetooth controllers; this helper seeds wired and
    // bluetooth nodes otherwise, then mirrors the boot wiring.
    fn wire_network(coordinator: &mut Coordinator) {
        if coordinator.network_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            let has_network = graph
                .nodes()
                .values()
                .any(|node| node.node_id.0.starts_with("device:net-"));
            if !has_network {
                let mut eth = crate::graph::NodeMetadata::new(
                    NodeId("device:net-enx00deadbeef".into()),
                    NodeType::Device,
                    crate::graph::ProvenanceSource::Discovered {
                        via: "sysfs".into(),
                    },
                    crate::graph::TrustLevel::Trusted,
                    crate::protocol::now(),
                );
                eth.label = "network interface enx00deadbeef".into();
                eth.attributes.insert("operstate".into(), "up".into());
                graph.add_node(eth).unwrap();
            }
            crate::network::NetworkSpecialist::instantiate(&mut graph).unwrap();
        }
        let specialist = coordinator.network_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_network") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator.broker.set_resource_state(
            ResourceId("network:domain".into()),
            ResourceState::Available,
        );
        let principal =
            PrincipalId::agent(crate::network::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("network:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("network:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("network:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("network:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn network_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_network(&mut coordinator);
        let call = ToolCallRequest {
            name: "network.observe_network".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("network observe through broker");
        assert!(result.text.contains("wired_interfaces"), "{}", result.text);
    }

    #[test]
    fn network_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_network(&mut coordinator);
        let call = ToolCallRequest {
            name: "network.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("network diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // M7: the drivers and hardware specialist must be reachable through the
    // broker like the other read-only specialist tools. Boot wires it when
    // the machine has unclaimed hardware; this helper seeds a PCI device,
    // firmware, and driver node otherwise, then mirrors the boot wiring.
    fn wire_drivers(coordinator: &mut Coordinator) {
        if coordinator.drivers_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            let has_hardware = graph.nodes().values().any(|node| {
                node.node_type == NodeType::Driver
                    || node.node_type == NodeType::Firmware
                    || node.node_id.0.starts_with("device:pci-")
            });
            if !has_hardware {
                let mut gpu = crate::graph::NodeMetadata::new(
                    NodeId("device:pci-0000:01:00.0".into()),
                    NodeType::Device,
                    crate::graph::ProvenanceSource::Discovered {
                        via: "sysfs".into(),
                    },
                    crate::graph::TrustLevel::Trusted,
                    crate::protocol::now(),
                );
                gpu.label = "VGA compatible controller".into();
                graph.add_node(gpu).unwrap();
            }
            crate::drivers::DriversSpecialist::instantiate(&mut graph).unwrap();
        }
        let specialist = coordinator.drivers_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_device") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator.broker.set_resource_state(
            ResourceId("drivers:domain".into()),
            ResourceState::Available,
        );
        let principal =
            PrincipalId::agent(crate::drivers::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("drivers:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("drivers:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("drivers:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("drivers:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn drivers_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_drivers(&mut coordinator);
        let call = ToolCallRequest {
            name: "drivers.observe_device".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("drivers observe through broker");
        assert!(result.text.contains("devices"), "{}", result.text);
    }

    #[test]
    fn drivers_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_drivers(&mut coordinator);
        let call = ToolCallRequest {
            name: "drivers.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("drivers diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // M7: the graphics specialist must be reachable through the broker like
    // the other read-only specialist tools. Boot wires it when the machine
    // has GPU, display, or session resources; this helper seeds a GPU
    // (structural PCI class 0x03 node, exactly what discovery reports)
    // otherwise, then mirrors the boot wiring.
    fn wire_graphics(coordinator: &mut Coordinator) {
        if coordinator.graphics_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match GraphicsSpecialist::instantiate(&mut graph) {
                Ok(_) => {}
                Err(crate::graphics::GraphicsError::NoGraphicsResources) => {
                    let mut gpu = crate::graph::NodeMetadata::new(
                        NodeId("device:pci-0000:00:02.0".into()),
                        NodeType::Device,
                        crate::graph::ProvenanceSource::Discovered {
                            via: "sysfs".into(),
                        },
                        crate::graph::TrustLevel::Trusted,
                        crate::protocol::now(),
                    );
                    gpu.label = "PCI device 0000:00:02.0".into();
                    gpu.attributes.insert("class".into(), "0x030000".into());
                    gpu.health = HealthState::Healthy;
                    graph.add_node(gpu).unwrap();
                    GraphicsSpecialist::instantiate(&mut graph).unwrap();
                }
                Err(error) => panic!("graphics instantiation failed: {error}"),
            }
        }
        let specialist = coordinator.graphics_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_graphics") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator.broker.set_resource_state(
            ResourceId("graphics:domain".into()),
            ResourceState::Available,
        );
        let principal =
            PrincipalId::agent(crate::graphics::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("graphics:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("graphics:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("graphics:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("graphics:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn graphics_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_graphics(&mut coordinator);
        let call = ToolCallRequest {
            name: "graphics.observe_graphics".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("graphics observe through broker");
        assert!(result.text.contains("gpus"), "{}", result.text);
    }

    #[test]
    fn graphics_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_graphics(&mut coordinator);
        let call = ToolCallRequest {
            name: "graphics.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("graphics diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // M7: the memory specialist must be reachable through the broker like the
    // other read-only specialist tools. Boot wires it when the machine has
    // memory resources; this helper seeds memory nodes (exactly what
    // discovery reports) otherwise, then mirrors the boot wiring.
    fn wire_memory(coordinator: &mut Coordinator) {
        if coordinator.memory_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match MemorySpecialist::instantiate(&mut graph) {
                Ok(_) => {}
                Err(crate::memory::MemoryError::NoMemoryResources) => {
                    let mut total = crate::graph::NodeMetadata::new(
                        NodeId("memory:total".into()),
                        NodeType::Memory,
                        crate::graph::ProvenanceSource::Discovered {
                            via: "sysfs".into(),
                        },
                        crate::graph::TrustLevel::Trusted,
                        crate::protocol::now(),
                    );
                    total.label = "total memory (16384000 kB)".into();
                    total.attributes.insert("size_kb".into(), "16384000".into());
                    total.health = HealthState::Healthy;
                    graph.add_node(total).unwrap();
                    MemorySpecialist::instantiate(&mut graph).unwrap();
                }
                Err(error) => panic!("memory instantiation failed: {error}"),
            }
        }
        let specialist = coordinator.memory_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_memory") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator
            .broker
            .set_resource_state(ResourceId("memory:domain".into()), ResourceState::Available);
        let principal =
            PrincipalId::agent(crate::memory::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("memory:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("memory:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("memory:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("memory:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn memory_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_memory(&mut coordinator);
        let call = ToolCallRequest {
            name: "memory.observe_memory".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("memory observe through broker");
        assert!(result.text.contains("memory_nodes"), "{}", result.text);
    }

    #[test]
    fn memory_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_memory(&mut coordinator);
        let call = ToolCallRequest {
            name: "memory.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("memory diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // M7: the processes specialist must be reachable through the broker
    // like the other read-only specialist tools. Boot wires it when the
    // machine has process resources; this helper seeds process nodes
    // (exactly what discovery reports) otherwise, then mirrors the boot
    // wiring.
    fn wire_processes(coordinator: &mut Coordinator) {
        if coordinator.processes_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match ProcessesSpecialist::instantiate(&mut graph) {
                Ok(_) => {}
                Err(crate::processes::ProcessesError::NoProcessResources) => {
                    let mut init = crate::graph::NodeMetadata::new(
                        NodeId("process:1".into()),
                        NodeType::Process,
                        crate::graph::ProvenanceSource::Discovered { via: "proc".into() },
                        crate::graph::TrustLevel::Trusted,
                        crate::protocol::now(),
                    );
                    init.label = "process 1 (init)".into();
                    init.attributes.insert("pid".into(), "1".into());
                    init.attributes.insert("comm".into(), "init".into());
                    init.attributes.insert("rss_kb".into(), "1024".into());
                    init.health = HealthState::Healthy;
                    graph.add_node(init).unwrap();
                    ProcessesSpecialist::instantiate(&mut graph).unwrap();
                }
                Err(error) => panic!("processes instantiation failed: {error}"),
            }
        }
        let specialist = coordinator.processes_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_process") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator.broker.set_resource_state(
            ResourceId("processes:domain".into()),
            ResourceState::Available,
        );
        let principal = PrincipalId::agent(
            crate::processes::PACKAGE_ID,
            specialist.specialist.0.clone(),
        );
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("processes:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("processes:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("processes:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("processes:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn processes_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_processes(&mut coordinator);
        let call = ToolCallRequest {
            name: "processes.observe_process".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("processes observe through broker");
        assert!(result.text.contains("process_nodes"), "{}", result.text);
    }

    #[test]
    fn processes_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_processes(&mut coordinator);
        let call = ToolCallRequest {
            name: "processes.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("processes diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // M7: the power and thermal specialist must be reachable through the
    // broker like the other read-only specialist tools. Boot wires it when
    // the machine has power/thermal resources; this helper seeds thermal and
    // power sensor nodes (exactly what discovery reports) otherwise, then
    // mirrors the boot wiring.
    fn wire_power(coordinator: &mut Coordinator) {
        if coordinator.power_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match PowerSpecialist::instantiate(&mut graph) {
                Ok(_) => {}
                Err(crate::power::PowerError::NoPowerResources) => {
                    let mut temp = crate::graph::NodeMetadata::new(
                        NodeId("sensor:hwmon0-temp1".into()),
                        NodeType::Sensor,
                        crate::graph::ProvenanceSource::Discovered {
                            via: "sysfs".into(),
                        },
                        crate::graph::TrustLevel::Trusted,
                        crate::protocol::now(),
                    );
                    temp.label = "coretemp temp1".into();
                    temp.attributes.insert("value".into(), "52000".into());
                    temp.attributes
                        .insert("unit".into(), "millidegree_c".into());
                    temp.health = HealthState::Healthy;
                    graph.add_node(temp).unwrap();
                    PowerSpecialist::instantiate(&mut graph).unwrap();
                }
                Err(error) => panic!("power instantiation failed: {error}"),
            }
        }
        let specialist = coordinator.power_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_thermal") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator
            .broker
            .set_resource_state(ResourceId("power:domain".into()), ResourceState::Available);
        let principal =
            PrincipalId::agent(crate::power::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("power:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("power:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("power:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("power:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn power_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_power(&mut coordinator);
        let call = ToolCallRequest {
            name: "power.observe_thermal".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("power observe through broker");
        assert!(result.text.contains("thermal_sensors"), "{}", result.text);
    }

    #[test]
    fn power_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_power(&mut coordinator);
        let call = ToolCallRequest {
            name: "power.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("power diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // M7: the security and identity specialist must be reachable through the
    // broker like the other read-only specialist tools. Boot seeds the
    // enforcement-plane nodes (guardian/capability/policy) and wires the
    // specialist; this helper mirrors that wiring so the tests pass without a
    // full boot.
    fn wire_security(coordinator: &mut Coordinator) {
        if coordinator.security_specialist.is_some() {
            return;
        }
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            seed_security_domain(&mut graph);
            match SecuritySpecialist::instantiate(&mut graph) {
                Ok(_) => {}
                Err(error) => panic!("security instantiation failed: {error}"),
            }
        }
        let specialist = coordinator.security_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_security") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator.broker.set_resource_state(
            ResourceId("security:domain".into()),
            ResourceState::Available,
        );
        let principal =
            PrincipalId::agent(crate::security::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("security:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("security:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("security:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("security:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn security_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_security(&mut coordinator);
        let call = ToolCallRequest {
            name: "security.observe_security".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("security observe through broker");
        assert!(result.text.contains("security_nodes"), "{}", result.text);
    }

    #[test]
    fn security_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_security(&mut coordinator);
        let call = ToolCallRequest {
            name: "security.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("security diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // Boot and recovery specialist (M7): the umbrella for the
    // boot and recovery domain (docs/modules/boot-recovery.md).
    // Unlike hardware umbrellas, its domain is the trust plane —
    // boot images, snapshots, and watchdogs — which is seeded by
    // the coordinator rather than sysfs-discovered. v0.1 is
    // read-only; observe_boot and diagnose_fault run through the
    // broker against the live graph; the bounded diagnose reports
    // BOOT-001 evidence.
    fn wire_boot_recovery(coordinator: &mut Coordinator) {
        if coordinator.boot_specialist.is_some() {
            return;
        }
        let boot_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            seed_boot_domain(&mut graph);
            match BootRecoverySpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(error) => panic!("boot recovery instantiation failed: {error}"),
            }
        };
        coordinator.boot_specialist = boot_specialist;
        let specialist = coordinator.boot_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_boot") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator
            .broker
            .set_resource_state(ResourceId("boot:domain".into()), ResourceState::Available);
        let principal =
            PrincipalId::agent(crate::boot::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("boot:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("boot:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("boot:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("boot:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn boot_recovery_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_boot_recovery(&mut coordinator);
        let call = ToolCallRequest {
            name: "boot.observe_boot".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("boot observe through broker");
        assert!(result.text.contains("boot_nodes"), "{}", result.text);
    }

    #[test]
    fn boot_recovery_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_boot_recovery(&mut coordinator);
        let call = ToolCallRequest {
            name: "boot.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("boot diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // Packages and updates specialist (M7): the umbrella for the
    // package domain (docs/modules/packages.md). v0.1 is read-only
    // with bounded Observe and Diagnose tools. Mutating operations
    // (stage_update, request_rollback) are deferred to the mutation
    // pass.
    fn wire_packages(coordinator: &mut Coordinator) {
        if coordinator.packages_specialist.is_some() {
            return;
        }
        let packages_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match PackagesSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::packages::PackagesError::NoPackageResources) => {
                    let mut pkg = crate::graph::NodeMetadata::new(
                        NodeId("package:linux-kernel".into()),
                        NodeType::Package,
                        crate::graph::ProvenanceSource::Discovered { via: "dpkg".into() },
                        crate::graph::TrustLevel::Trusted,
                        crate::protocol::now(),
                    );
                    pkg.label = "linux-kernel".into();
                    pkg.attributes.insert("version".into(), "6.1.0".into());
                    pkg.attributes
                        .insert("signature".into(), "sha256:abc123".into());
                    pkg.attributes.insert("state".into(), "installed".into());
                    graph.add_node(pkg).unwrap();
                    PackagesSpecialist::instantiate(&mut graph).ok()
                }
                Err(error) => panic!("packages instantiation failed: {error}"),
            }
        };
        coordinator.packages_specialist = packages_specialist;
        let specialist = coordinator.packages_specialist.as_ref().unwrap().clone();
        for definition in specialist.tool_definitions() {
            let tool_id = definition.tool_id.clone();
            coordinator.broker.register_tool(definition);
            let graph = coordinator.graph.clone();
            let domain = specialist.clone();
            coordinator.broker.spawn_specialist(&tool_id.clone(), {
                std::sync::Arc::new(move |request| {
                    let graph = graph.read().expect("graph lock");
                    let target = tool_arguments(&request.parameters);
                    if tool_id.ends_with("observe_package") {
                        domain.observe(&graph, &target)
                    } else {
                        domain.diagnose(&graph, &target)
                    }
                })
            });
        }
        coordinator.broker.set_resource_state(
            ResourceId("packages:domain".into()),
            ResourceState::Available,
        );
        let principal =
            PrincipalId::agent(crate::packages::PACKAGE_ID, specialist.specialist.0.clone());
        coordinator.broker.register_principal(
            principal.clone(),
            vec![
                Capability {
                    resource: ResourceId("packages:domain".into()),
                    operation: Operation::Observe,
                },
                Capability {
                    resource: ResourceId("packages:domain".into()),
                    operation: Operation::Diagnose,
                },
            ],
            Clearance::max(),
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("packages:domain".into()),
                operation: Operation::Observe,
            },
        );
        coordinator.broker.grant_capability(
            &coordinator.session_principal,
            Capability {
                resource: ResourceId("packages:domain".into()),
                operation: Operation::Diagnose,
            },
        );
        coordinator.session_tokens = coordinator
            .broker
            .client(coordinator.session_principal.clone())
            .capability_tokens(&coordinator.session_principal);
    }

    #[test]
    fn packages_observe_runs_through_broker() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_packages(&mut coordinator);
        let call = ToolCallRequest {
            name: "packages.observe_package".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("packages observe through broker");
        assert!(result.text.contains("package_nodes"), "{}", result.text);
    }

    #[test]
    fn packages_diagnose_reports_domain_invariants() {
        let port = testutil::spawn_json_server(handler);
        let mut coordinator = stub_coordinator(port);
        wire_packages(&mut coordinator);
        let call = ToolCallRequest {
            name: "packages.diagnose_fault".into(),
            arguments: "all".into(),
        };
        let result = coordinator
            .run_tool_as("planner", &call)
            .expect("packages diagnose through broker");
        assert!(result.text.contains("findings"), "{}", result.text);
    }

    // The executor wired at boot (modules/wifi.md, M6) must run mutating
    // wifi tools (stage_driver, request_reset) through the action state
    // machine. When a device was discovered, boot already registered the tool
    // and the specialist principal (with the right capabilities); otherwise
    // they are registered here so the tests pass on machines without a Wi-Fi
    // controller. Either way the resource owner is wired explicitly.
    fn wire_specialist(
        coordinator: &Coordinator,
        device: &ResourceId,
        tool_id: &str,
        operation: Operation,
        risk: RiskLevel,
        clearance: Clearance,
    ) -> PrincipalId {
        coordinator.broker.set_guardian(Guardian::new());
        if let Some(specialist) = coordinator.wifi_specialist.as_ref() {
            let principal =
                PrincipalId::agent(crate::wifi::PACKAGE_ID, specialist.specialist.0.clone());
            coordinator
                .broker
                .set_resource_owner(device.clone(), principal.clone());
            return principal;
        }
        let principal = PrincipalId::agent("wifi.specialist", "wifi0-instance-001");
        let capability = Capability {
            resource: device.clone(),
            operation,
        };
        coordinator.broker.register_tool(ToolDefinition {
            tool_id: tool_id.to_string(),
            specialist_package: "wifi.specialist".into(),
            risk_level: risk,
            required_capabilities: vec![capability.clone()],
            description: format!("{tool_id} on {device}"),
        });
        coordinator
            .broker
            .register_principal(principal.clone(), vec![capability], clearance);
        coordinator
            .broker
            .set_resource_state(device.clone(), ResourceState::Available);
        coordinator
            .broker
            .set_resource_owner(device.clone(), principal.clone());
        principal
    }

    fn boot_device(coordinator: &Coordinator) -> ResourceId {
        coordinator
            .wifi_specialist
            .as_ref()
            .map(|s| ResourceId(s.device.0.clone()))
            .unwrap_or_else(|| ResourceId("device:net-wlp1s0".into()))
    }

    fn capability_token(
        coordinator: &Coordinator,
        principal: &PrincipalId,
        operation: Operation,
    ) -> CapabilityToken {
        coordinator
            .broker
            .client(principal.clone())
            .capability_tokens(principal)
            .into_iter()
            .find(|token| token.capability.operation == operation)
            .expect("capability token issued")
    }

    fn stage_request(
        principal: &PrincipalId,
        device: &ResourceId,
        token: &CapabilityToken,
        nonce: u64,
    ) -> crate::protocol::ToolRequest {
        build_request(
            principal.clone(),
            device.clone(),
            Operation::Stage,
            "wifi.stage_driver",
            token,
            ToolParameters::Stage {
                change: serde_json::json!({ "module": "iwlwifi-next" }),
            },
            uuid::Uuid::new_v4(),
            nonce,
        )
    }

    fn reset_request(
        principal: &PrincipalId,
        device: &ResourceId,
        token: &CapabilityToken,
        action_id: uuid::Uuid,
        plan_hash: [u8; 32],
        nonce: u64,
    ) -> crate::protocol::ToolRequest {
        let mut request = build_request(
            principal.clone(),
            device.clone(),
            Operation::Reset,
            "wifi.request_reset",
            token,
            ToolParameters::Reset {
                to_known_good: true,
            },
            uuid::Uuid::new_v4(),
            nonce,
        );
        request.action_id = Some(action_id);
        request.plan_hash = Some(plan_hash);
        request
    }

    #[test]
    fn broker_runs_staged_commit_through_booted_executor() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let device = boot_device(&coordinator);
        let principal = wire_specialist(
            &coordinator,
            &device,
            "wifi.stage_driver",
            Operation::Stage,
            RiskLevel::Staged,
            Clearance(RiskLevel::Staged),
        );
        let token = capability_token(&coordinator, &principal, Operation::Stage);
        let request = stage_request(&principal, &device, &token, 9001);
        let result = coordinator
            .broker
            .client(principal)
            .request_tool(request)
            .expect("broker response");
        assert_eq!(result.status, ToolStatus::Success);
        assert!(matches!(
            result.data,
            Some(ToolData::CommitResult {
                committed: true,
                health_verified: true
            })
        ));
    }

    #[test]
    fn broker_rolls_back_staged_request_when_health_fails() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let device = boot_device(&coordinator);
        let principal = wire_specialist(
            &coordinator,
            &device,
            "wifi.stage_driver",
            Operation::Stage,
            RiskLevel::Staged,
            Clearance(RiskLevel::Staged),
        );
        let dir = tempfile::tempdir().expect("action store directory");
        let store = crate::action::FileActionStore::new(dir.path()).expect("action store");
        let driver = MockWifiDriver::new();
        driver
            .health_ok
            .store(false, std::sync::atomic::Ordering::Relaxed);
        coordinator
            .broker
            .set_executor(crate::executor::StagedExecutor::new(
                Box::new(store),
                Box::new(driver),
            ));
        let token = capability_token(&coordinator, &principal, Operation::Stage);
        let request = stage_request(&principal, &device, &token, 9002);
        let result = coordinator
            .broker
            .client(principal)
            .request_tool(request)
            .expect("broker response");
        assert_eq!(result.status, ToolStatus::RolledBack);
        assert_eq!(
            result.error.expect("rollback explains health failure").code,
            ToolErrorCode::HealthCheckFailed
        );
    }

    // M6 acceptance criterion #5: a driver reset is risk 4 and must be denied
    // unless a broker-owned approval covering the exact action is present.
    // This is the same guarantee the broker-level test makes, reached through
    // the boot-wired coordinator.
    #[test]
    fn reset_denied_without_broker_owned_approval() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let device = boot_device(&coordinator);
        let principal = wire_specialist(
            &coordinator,
            &device,
            "wifi.request_reset",
            Operation::Reset,
            RiskLevel::Recovery,
            Clearance(RiskLevel::Recovery),
        );
        let token = capability_token(&coordinator, &principal, Operation::Reset);
        let request = reset_request(
            &principal,
            &device,
            &token,
            uuid::Uuid::new_v4(),
            [7; 32],
            9101,
        );
        let result = coordinator
            .broker
            .client(principal)
            .request_tool(request)
            .expect("broker response");
        assert_eq!(result.status, ToolStatus::Denied);
        let message = result.error.expect("denial carries a reason").message;
        assert!(message.contains("approval"), "denied: {message}");
    }

    // End-to-end through the facade's own approval channel: issue_reset_approval
    // then submit_approval record the broker-owned approval (human-interaction
    // §1.4), and the risk-4 reset commits through the executor.
    #[test]
    fn reset_commits_through_facade_approval_channel() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let device = boot_device(&coordinator);
        let principal = wire_specialist(
            &coordinator,
            &device,
            "wifi.request_reset",
            Operation::Reset,
            RiskLevel::Recovery,
            Clearance(RiskLevel::Recovery),
        );
        let action_id = uuid::Uuid::new_v4();
        let plan_hash = [9; 32];
        let (request_id, _) = coordinator
            .issue_reset_approval(
                action_id,
                plan_hash,
                device.clone(),
                "wifi.request_reset".into(),
            )
            .expect("approval request issued");
        coordinator.submit_approval(request_id, true);
        let token = capability_token(&coordinator, &principal, Operation::Reset);
        let request = reset_request(&principal, &device, &token, action_id, plan_hash, 9102);
        let result = coordinator
            .broker
            .client(principal)
            .request_tool(request)
            .expect("broker response");
        assert_eq!(result.status, ToolStatus::Success);
        assert!(matches!(
            result.data,
            Some(ToolData::CommitResult {
                committed: true,
                health_verified: true
            })
        ));
    }
