# Graph Report - aios  (2026-08-21)

## Corpus Check
- 151 files · ~276,084 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 2815 nodes · 7270 edges · 234 communities (105 shown, 129 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 89 edges (avg confidence: 0.78)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `8ccc9fa3`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- src-tauri/src/main.rs
- tools.rs
- NodeId
- protocol.rs
- tests.rs
- security.rs
- facade.rs
- AuditLog
- harness.rs
- hub.rs
- ResourceId
- processes.rs
- broker.rs
- SysfsDiscovery
- executor.rs
- graph.rs
- power.rs
- BackendStatus
- memory.rs
- graphics.rs
- storage.rs
- drivers.rs
- http.rs
- sidebar.ts
- network.rs
- packages.rs
- PrincipalId
- boot.rs
- planner.rs
- .new
- Coordinator
- composer.rs
- wifi_driver.rs
- ModelId
- Result
- evidence.rs
- Arc
- String
- GenerativeWidget
- ModelEntry
- String
- package.json
- Guardian
- wifi.rs
- main.ts
- MockPlanner
- .grant_consent
- String
- discovery.rs
- ExitCode
- render
- model.rs
- ui_e2e.rs
- config.rs
- tauri.conf.json
- panel.rs
- verifier.rs
- ToolRequest
- Q: Which surface generation path is Aios's intended architecture?
- From
- .chat_with_tools_outcome
- RegionPriority
- Project Grounding Document
- .fmt
- RoutingError
- AgentError
- definitions
- definitions
- widgets.rs
- Coordinator
- StagedExecutor
- properties
- properties
- DockEdge
- EvidenceItem
- .boot_with_probe
- src/main.rs
- ProviderId
- Testing Strategy
- ModelTask
- Display
- Aios
- permissions
- permissions
- graphify
- default.json
- Capability
- webviews
- webviews
- GraphEdge
- Surface
- CapabilityRemote
- CapabilityRemote
- graphify
- sidebar.rs
- LayoutMode
- GraphNode
- ApprovalItem
- pickSelectOption
- graphify-refresh.sh
- ActionRecord
- action.rs
- Generative Surface Roadmap
- canvas.rs
- SettingsPanel
- ChatMessages
- graphify.js
- SidebarRoute
- Arc
- Filesystem Specialist
- copilot-instructions.md
- aios
- install-graphify-hooks.sh
- ui-e2e.sh
- stub_provider.rs
- Aios Logo
- Documentation
- Graphics Children Stack
- Memory Specialist
- Network Specialist
- Packages and Updates Specialist
- Power and Thermal Specialist
- Processes and Resources Specialist
- Security and Identity Specialist
- Session Specialist
- Storage Specialist
- Wi-Fi Specialist
- Wired/LAN Specialist
- Aios Frontend
- Knowledge Graph
- aios-frontend
- Aios Logo (128x128)
- Aios Logo (128x128@2x)
- Aios Logo (32x32)
- Coordinator
- Option
- PathBuf
- Result
- String
- Surface
- ToolResult
- Value
- Vec
- ToolResult
- Coordinator
- Checkpoint
- Coordinator
- Surface
- ToolResult
- Item
- Iterator
- Surface
- .fmt
- String
- Surface
- Default
- Option
- RegionPriority
- Self
- StatusItem
- String
- Vec
- Mutex
- Option
- Self
- String
- Surface
- Vec
- Error
- Formatter
- Option
- Result
- Surface
- ToolResult
- Vec
- coordinator/mod.rs
- ModelGateway
- Mutex
- Sender
- Surface
- ToolResult
- Box
- Default
- Send
- Sync
- Timestamp
- SystemGraphSnapshot
- Duration
- Path
- PathBuf
- WidthClass
- PromptResponse
- ConnectivityProbe
- ConnectivityState
- AtomicBool
- Default
- Display
- Duration
- Error
- Formatter
- HashMap
- Into
- Item
- Iterator
- RwLock
- Self
- Send
- Sync
- Timestamp
- RoutingDecision
- Uuid
- Arc
- Box
- DiscoveredModel
- HashMap
- Path
- PathBuf
- ProgressSink
- RwLock
- ToolRegistry
- Agent
- Value
- RoutingDecision

## God Nodes (most connected - your core abstractions)
1. `NodeId` - 148 edges
2. `SystemGraph` - 135 edges
3. `ResourceId` - 108 edges
4. `NodeMetadata` - 61 edges
5. `Coordinator` - 59 edges
6. `PrincipalId` - 53 edges
7. `PolicyBroker` - 51 edges
8. `stub_coordinator()` - 41 edges
9. `ProviderId` - 40 edges
10. `ToolRequest` - 37 edges

## Surprising Connections (you probably didn't know these)
- `write_surface_trace()` --references--> `RoutingDecision`  [EXTRACTED]
  src-tauri/src/main.rs → src/model.rs
- `boots_http_provider_and_status_shows_it()` --calls--> `status_text()`  [INFERRED]
  src/coordinator/tests.rs → src/coordinator/mod.rs
- `compose_unconstrained_html()` --calls--> `strip_think()`  [INFERRED]
  src/surface/composer.rs → src/planner.rs
- `edge()` --calls--> `t()`  [INFERRED]
  src/tools.rs → src/graph.rs
- `node()` --calls--> `t()`  [INFERRED]
  src/tools.rs → src/graph.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Document Dependencies** — docs_implementation_roadmap, docs_message_protocol, docs_model_routing, docs_observability, docs_requirements, docs_security_model, docs_system_graph, docs_specialist_depth_plan, docs_surface_harness, docs_ui [INFERRED 0.75]
- **ADR-0001 and Project Grounding** — docs/decisions/0001-v01-runs-above-linux.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **ADR-0002 and Project Grounding** — docs/decisions/0002-rust-as-implementation-language.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **ADR-0003 and Project Grounding** — docs/decisions/0003-fail-fast-no-silent-fallbacks.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **ADR-0004 and Project Grounding** — docs/decisions/0004-two-dimensional-authorization.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **ADR-0005 and Project Grounding** — docs/decisions/0005-freeze-triage.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **ADR-0006 and Project Grounding** — docs/decisions/0006-model-gateway.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **ADR-0007 and Project Grounding** — docs/decisions/0007-groundless-generation-model.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Graphics Stack** — gpu, display, session [INFERRED 0.80]
- **Grounding Snapshot and Project Grounding** — docs/grounding/project_grounding_2026-08-17_17-19-56.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Grounding Snapshot and Project Grounding** — docs/grounding/project_grounding_2026-08-17_18-05-00.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Legacy UI Specification and Generative Surface Roadmap** — docs/archive/superseded/ui-v0.1-legacy.md, docs/archive/superseded/generative-surface-roadmap-2026-08-16.md [INFERRED 0.80]
- **M8 UI Repair Plan and Generative Surface Roadmap** — docs/archive/superseded/m8-ui-repair-plan.md, docs/archive/superseded/generative-surface-roadmap-2026-08-16.md [INFERRED 0.80]
- **Milestone and Project Grounding** — docs/milestones/0001-generative-surface-desktop-foundation.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Milestone and Project Grounding** — docs/milestones/0002-multi-surface-lifecycle-plan.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Milestone and Project Grounding** — docs/milestones/0003-sidebar-administration-panel.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Module and Project Grounding** — docs/modules/block-disk.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Module and Project Grounding** — docs/modules/bluetooth.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Module and Project Grounding** — docs/modules/boot-recovery.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Module and Project Grounding** — docs/modules/display.md, docs/archive/context/project_grounding_2026-08-14_10-22-59.md [INFERRED 0.80]
- **Project Grounding and Session Notes** — docs/archive/context/project_grounding_2026-08-14_10-22-59.md, docs/archive/context/session-notes-2026-08-12.md [INFERRED 0.80]
- **Research Issues and Generative Surface Roadmap** — docs/archive/research/aios_issues-2026-08-16.md, docs/archive/superseded/generative-surface-roadmap-2026-08-16.md [INFERRED 0.80]
- **Capability Enforcement System** — docs/capability-model.md, docs/action-state-machine.md, docs/human-interaction.md, src/coordinator [INFERRED]
- **Surface Lifecycle Management** — docs/grounding/project_grounding_2026-08-21_17-21-55.md, src/surface, docs_modules_files_data_md, docs_modules_drivers_md [INFERRED]

## Communities (234 total, 129 thin omitted)

### Community 0 - "src-tauri/src/main.rs"
Cohesion: 0.09
Nodes (71): AppHandle, Facade, GraphActivity, ProgressReporter, Rectangle, RustConnection, Sender, GraphActivity (+63 more)

### Community 1 - "tools.rs"
Cohesion: 0.07
Nodes (54): Default, EdgeMetadata, EdgeType, FnOnce, NodeId, NodeMetadata, NodeType, Sized (+46 more)

### Community 2 - "NodeId"
Cohesion: 0.11
Nodes (22): print_hardware_report(), Timestamp, EdgeMetadata, EdgeType, ImpactReport, NodeId, NodeMetadata, Capability (+14 more)

### Community 3 - "protocol.rs"
Cohesion: 0.08
Nodes (61): ApprovalId, AuditEntryId, CheckpointRef, MessageId, PlanId, RequestId, ActionPlan, Approval (+53 more)

### Community 4 - "tests.rs"
Cohesion: 0.06
Nodes (74): CapabilityToken, Clearance, Operation, PrincipalId, ResourceId, RiskLevel, tool_arguments(), Coordinator (+66 more)

### Community 5 - "security.rs"
Cohesion: 0.13
Nodes (26): diagnose_flags_missing_verified_evidence(), discovers_security_nodes(), exposes_only_read_only_tools(), health_counts_verified_evidence(), instantiates_with_owns_edges_for_each_resource(), is_security_node(), node(), not_found() (+18 more)

### Community 6 - "facade.rs"
Cohesion: 0.07
Nodes (42): BootError, F, SplitWhitespace, Coordinator, audit_logs_chat_attempt(), audit_records_boot_and_commands(), bare_chat_goes_to_model(), consent_commands_roundtrip() (+34 more)

### Community 7 - "AuditLog"
Cohesion: 0.10
Nodes (32): File, appends_across_sessions(), audit_log_path(), AuditEntry, AuditError, AuditLog, encode_field(), entries_are_forward_chained() (+24 more)

### Community 8 - "harness.rs"
Cohesion: 0.11
Nodes (35): all_steps_allow_when_capabilities_granted(), build_graph(), capabilities(), DeviceHistory, enforce_stops_campaign_at_first_denial(), harness_principal(), harness_tool_ids(), HarnessPlan (+27 more)

### Community 9 - "hub.rs"
Cohesion: 0.10
Nodes (35): Arc<RecordingClient>, catalog_model_url_points_at_resolve_main(), CatalogModel, default_catalog(), hex(), HttpClient, HubError, model_with_sha256() (+27 more)

### Community 10 - "ResourceId"
Cohesion: 0.10
Nodes (32): tool(), Capability, GuardianClient, Operation, PrincipalType, ResourceId, RiskLevel, HashMap (+24 more)

### Community 11 - "processes.rs"
Cohesion: 0.10
Nodes (38): cpu_cores(), cpu_sample(), cpu_stats(), CpuStats, diagnose_flags_missing_usage_evidence(), discovers_process_nodes(), exposes_only_read_only_tools(), format_process_row() (+30 more)

### Community 12 - "broker.rs"
Cohesion: 0.16
Nodes (33): Runtime, allows_with_valid_capability(), approval_channel_accepts_only_user_approval(), approval_channel_rejection_and_expiry_are_fail_closed(), approval_is_required_and_plan_hash_is_bound(), approval_request_for(), approval_scope_for(), audit_broken_denies_everything() (+25 more)

### Community 13 - "SysfsDiscovery"
Cohesion: 0.23
Nodes (17): DiscoveryError, filesystem_usage(), parse_diskstat(), parse_meminfo(), parse_pressure(), parse_pressure_and_vmstat_key_their_fields(), parse_vmstat(), Display (+9 more)

### Community 14 - "executor.rs"
Cohesion: 0.20
Nodes (24): advance_to(), checkpoint_count(), checkpoint_verification_failure_enters_failed_and_retains_checkpoint(), failed_action_can_be_manually_recovered_from_retained_checkpoint(), fresh(), fresh_fault(), health_check_error_rolls_back(), health_check_failure_triggers_rollback() (+16 more)

### Community 15 - "graph.rs"
Cohesion: 0.17
Nodes (23): add_edge_requires_both_endpoints(), add_node_rejects_duplicate(), dependencies_and_dependents_track_both_directions(), edge(), EdgeId, EdgeProvenance, impact_report_counts_related_components(), node() (+15 more)

### Community 16 - "power.rs"
Cohesion: 0.12
Nodes (30): diagnose_flags_missing_reading_evidence(), discovers_thermal_and_power_sensors(), exposes_only_read_only_tools(), health_counts_reading_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_power_sensor(), is_thermal_sensor() (+22 more)

### Community 18 - "memory.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_capacity_evidence(), discovers_memory_nodes_and_ecc_sensors(), exposes_only_read_only_tools(), health_counts_capacity_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_memory_node(), memory_graph() (+21 more)

### Community 19 - "graphics.rs"
Cohesion: 0.12
Nodes (28): diagnose_flags_missing_gpu_state(), discovers_gpu_display_and_session(), exposes_only_read_only_tools(), graphics_graph(), GraphicsError, GraphicsHealth, GraphicsSpecialist, health_counts_state_evidence() (+20 more)

### Community 20 - "storage.rs"
Cohesion: 0.12
Nodes (28): diagnose_flags_missing_capacity_and_backing(), discovers_block_devices_and_filesystems(), exposes_only_read_only_tools(), health_counts_capacity_and_backing_evidence(), instantiates_with_owns_edges_for_each_resource(), is_block_device(), is_filesystem(), node() (+20 more)

### Community 21 - "drivers.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_driver_attachment(), discovers_unclaimed_hardware_only(), DriversError, DriversHealth, DriversSpecialist, exposes_only_read_only_tools(), gpu_class_devices_are_not_claimed(), hardware_graph() (+21 more)

### Community 22 - "http.rs"
Cohesion: 0.11
Nodes (33): Agent, GenerationError, GenerationRequest, GenerationResponse, ModelBackend, ModelId, ProviderId, ProviderTier (+25 more)

### Community 23 - "sidebar.ts"
Cohesion: 0.09
Nodes (33): computeActiveNodeIds(), EvidenceItem, GRAPH_LAYER_Y, GraphEdge, GraphNode, graphReadout(), healthClass(), INSPECTOR (+25 more)

### Community 24 - "network.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_connectivity(), discovers_wired_and_bluetooth_excludes_wireless(), exposes_only_read_only_tools(), health_counts_link_and_backing_evidence(), instantiates_with_owns_edges_skipping_wifi_owned(), is_bluetooth_controller(), is_wired_interface(), is_wireless() (+21 more)

### Community 25 - "packages.rs"
Cohesion: 0.13
Nodes (26): diagnose_flags_missing_signature_evidence(), discovers_package_nodes(), exposes_only_read_only_tools(), health_counts_signature_evidence(), instantiates_with_owns_edges_for_each_resource(), is_package_node(), node(), not_found() (+18 more)

### Community 26 - "PrincipalId"
Cohesion: 0.07
Nodes (26): Fn, PolicyBroker, Box, Capability, Default, HashSet, Option, ProgressSink (+18 more)

### Community 27 - "boot.rs"
Cohesion: 0.12
Nodes (27): boot_graph(), BootRecoveryError, BootRecoveryHealth, BootRecoverySpecialist, diagnose_flags_unhealthy_nodes(), discovers_boot_nodes(), exposes_only_read_only_tools(), health_counts_healthy_nodes() (+19 more)

### Community 28 - "planner.rs"
Cohesion: 0.12
Nodes (25): empty_steps_still_parses(), extract_json(), extracts_json_from_prose(), format_plan(), garbage_becomes_freeform(), GeneratedPlan, missing_intent_falls_back(), multiple_calls_parsed_in_order() (+17 more)

### Community 29 - ".new"
Cohesion: 0.20
Nodes (19): HashMap, RwLock, Send, assigned_role_runs_on_assigned_provider_and_model(), failing_backend_surfaces_generation_error(), gateway_with(), internet_assignment_offline_fails(), ModelBackend (+11 more)

### Community 30 - "Coordinator"
Cohesion: 0.09
Nodes (24): Arc, AuditLog, BootRecoverySpecialist, Broker, DriversSpecialist, GraphicsSpecialist, GraphPhase, MemorySpecialist (+16 more)

### Community 31 - "composer.rs"
Cohesion: 0.07
Nodes (36): EvidenceIndex, From, GatewayError, RoutingDecision, Option, Result, RoutingDecision, String (+28 more)

### Community 32 - "wifi_driver.rs"
Cohesion: 0.08
Nodes (35): ResourceDriver, Send, checkpoint_captures_active_module(), driver(), DriverControl, fake_sysfs(), health_check_reflects_link_state(), LinuxDriverControl (+27 more)

### Community 33 - "ModelId"
Cohesion: 0.11
Nodes (21): LlamaBackend, LlamaChatMessage, LlamaModel, LlamaToken, chat_messages(), chat_messages_maps_roles(), loads_and_generates_real_model(), LocalLlama (+13 more)

### Community 34 - "Result"
Cohesion: 0.30
Nodes (8): assignable_roles(), Coordinator, RoleDescriptor, Option, Result, String, Vec, validate_role_id()

### Community 35 - "evidence.rs"
Cohesion: 0.14
Nodes (27): cross_tool_reference_fails(), empty_results_give_empty_index(), empty_value_never_matches(), evidence_brief(), evidence_brief_quotes_keys_and_tools(), EvidenceEntry, EvidenceIndex, exact_copy_passes() (+19 more)

### Community 37 - "String"
Cohesion: 0.15
Nodes (13): AgentError, ConnectivityState, DiscoveredModel, ModelEntry, ChatOutcome, classification_help(), ProviderCatalogue, providers_text() (+5 more)

### Community 38 - "GenerativeWidget"
Cohesion: 0.13
Nodes (27): ApprovalItem, app(), render_approval_item(), render_widget(), Element, String, Vec, spawn_message_task() (+19 more)

### Community 39 - "ModelEntry"
Cohesion: 0.11
Nodes (15): AtomicBool, AtomicU32, HealthState, DataPolicy, lan_model(), MockBackend, ModelEntry, ModelProvenance (+7 more)

### Community 40 - "String"
Cohesion: 0.31
Nodes (6): Into, Self, GenerationError, ModelMessage, ModelRole, String

### Community 41 - "package.json"
Cohesion: 0.07
Nodes (26): autoprefixer, author, dependencies, @tauri-apps/api, @tauri-apps/cli, description, devDependencies, autoprefixer (+18 more)

### Community 42 - "Guardian"
Cohesion: 0.17
Nodes (18): Guardian, guardian_allows_boot_config_with_fallback_image(), guardian_allows_read_only_operations(), guardian_allows_tested_firmware(), guardian_blocks_boot_config_without_fallback(), guardian_blocks_untested_driver_load(), guardian_blocks_untested_firmware(), InvariantCheck (+10 more)

### Community 43 - "wifi.rs"
Cohesion: 0.17
Nodes (18): diagnose_flags_missing_driver(), discovers_and_instantiates_from_seeded_graph(), exposes_bounded_tools_with_declared_risk(), health_reports_missing_dependencies_as_false(), health_sees_two_hop_driver_and_network_service(), observe_returns_device_state_metrics(), Display, Error (+10 more)

### Community 44 - "main.ts"
Cohesion: 0.10
Nodes (28): activityDetail(), activityLabel(), applyGraphActivity(), BackendStatus, currentWindow, DockEdge, GraphActivityEvent, loadProviderCatalog() (+20 more)

### Community 45 - "MockPlanner"
Cohesion: 0.19
Nodes (12): err_result(), MockPlanner, MockVerificationAgent, ok_result(), Capability, Option, Self, String (+4 more)

### Community 46 - ".grant_consent"
Cohesion: 0.29
Nodes (4): Coordinator, Option, Result, String

### Community 48 - "discovery.rs"
Cohesion: 0.13
Nodes (34): device_firmware_attributes_create_nodes_and_edges(), devices_without_firmware_attributes_get_no_firmware_node(), discovered_nodes_go_stale_after_ttl(), DiscoveredService, discovery(), discovery_adds_dependency_edges(), DiscoveryEvent, DiscoveryOptions (+26 more)

### Community 50 - "render"
Cohesion: 0.22
Nodes (20): addProvider(), assignRole(), assignRoleGroup(), autosizePrompt(), bindSidebar(), dockPanel(), escapeHtml(), loadModelsForRole() (+12 more)

### Community 51 - "model.rs"
Cohesion: 0.14
Nodes (15): combine(), ConnectivityProbe, ConnectivityState, FinishReason, GatewayResponse, GenerationResponse, has_default_route(), has_default_route_v6() (+7 more)

### Community 52 - "ui_e2e.rs"
Cohesion: 0.20
Nodes (19): Child, Client, Drop, Duration, app_binary(), assert_surface(), ChildGuard, dump_windows() (+11 more)

### Community 53 - "config.rs"
Cohesion: 0.09
Nodes (35): AiosConfig, api_key_resolved_from_env(), ConfigError, default_ctx(), default_http_timeout_ms(), default_max_tokens(), default_threads(), dirs_home() (+27 more)

### Community 54 - "tauri.conf.json"
Cohesion: 0.10
Nodes (20): assets/128x128@2x.png, assets/128x128.png, assets/32x32.png, app, security, windows, build, beforeBuildCommand (+12 more)

### Community 55 - "panel.rs"
Cohesion: 0.26
Nodes (16): count_health(), counts_health_and_never_hides_stale(), graph_with(), is_healthy(), PanelSnapshot, render(), render_lists_failed_actions_with_recovery_hint(), render_shows_stale_and_unknown_explicitly() (+8 more)

### Community 56 - "verifier.rs"
Cohesion: 0.19
Nodes (18): format_review(), garbage_becomes_freeform(), loose_review(), parse_review(), parses_approve(), parses_approve_with_conditions(), parses_reject(), review_formats_verdict() (+10 more)

### Community 57 - "ToolRequest"
Cohesion: 0.14
Nodes (20): SpecialistCall, SpecialistHandler, BrokerClient, BrokerError, build_request(), denied_result(), error_code_for(), LocalBroker (+12 more)

### Community 58 - "Q: Which surface generation path is Aios's intended architecture?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Which surface generation path is Aios's intended architecture?, Source Nodes

### Community 60 - ".chat_with_tools_outcome"
Cohesion: 0.24
Nodes (12): Coordinator, operation_for_tool(), protocol_tool_result(), quote_value(), required_specialist_calls(), Option, Result, String (+4 more)

### Community 62 - "Project Grounding Document"
Cohesion: 0.11
Nodes (18): Project Grounding Document, Session Notes Document, ADR-0001: Runs Above Linux, ADR-0002: Rust as Implementation Language, ADR-0003: Fail-Fast No Silent Fallbacks, ADR-0004: Two-Dimensional Authorization, ADR-0005: Freeze Triage, ADR-0006: Model Gateway (+10 more)

### Community 64 - "RoutingError"
Cohesion: 0.21
Nodes (10): Display, Graphics Specialist, Error, Formatter, GPU, Session, GatewayError, RegistryError (+2 more)

### Community 65 - "AgentError"
Cohesion: 0.21
Nodes (12): AgentError, Planner, Arc, Display, Error, Formatter, From, Result (+4 more)

### Community 66 - "definitions"
Cohesion: 0.11
Nodes (17): anyOf, definitions, Number, PermissionEntry, Target, Value, description, anyOf (+9 more)

### Community 67 - "definitions"
Cohesion: 0.11
Nodes (17): anyOf, definitions, Number, PermissionEntry, Target, Value, description, anyOf (+9 more)

### Community 68 - "widgets.rs"
Cohesion: 0.25
Nodes (16): ActionForm(), Chart(), ChartDataPoint, FormField, MetricCard(), ChartDataPoint, Element, FormField (+8 more)

### Community 69 - "Coordinator"
Cohesion: 0.18
Nodes (7): Coordinator, ActionId, PlanHash, Result, String, Uuid, scan_summary()

### Community 70 - "StagedExecutor"
Cohesion: 0.23
Nodes (10): CheckpointError, StageError, MockDriver, ActionId, Arc, Mutex, Result, StagedExecutor (+2 more)

### Community 71 - "properties"
Cohesion: 0.12
Nodes (17): properties, Identifier, default, description, type, description, oneOf, type (+9 more)

### Community 72 - "properties"
Cohesion: 0.12
Nodes (17): properties, Identifier, default, description, type, description, oneOf, type (+9 more)

### Community 75 - ".boot_with_probe"
Cohesion: 0.14
Nodes (13): Box, ConfigError, ConnectivityProbe, RegistryError, RoutingError, BootError, Display, Error (+5 more)

### Community 76 - "src/main.rs"
Cohesion: 0.20
Nodes (15): build_demo_plan(), build_graph(), describe(), kernel_module_request(), main(), register_principals(), register_tools(), Option (+7 more)

### Community 77 - "ProviderId"
Cohesion: 0.18
Nodes (4): Item, Iterator, ModelRegistry, ProviderId

### Community 78 - "Testing Strategy"
Cohesion: 0.38
Nodes (14): Action State Machine, Agent Packages, Architecture Vision, Capability Model, Bespoke Graph Snapshot, Coordinator Modularization Snapshot, Human Interaction, Testing Strategy (+6 more)

### Community 79 - "ModelTask"
Cohesion: 0.17
Nodes (15): DataClassification, AgentRole, ConsentRecord, deregister_provider_drops_only_that_provider(), internet_model(), local_model(), ModelCapability, ModelTask (+7 more)

### Community 81 - "Aios"
Cohesion: 0.08
Nodes (33): Archive README, Current State, Grounding Snapshot: Coordinator Modularization, Surface Harness, and Graphify, Open Work, Relevant Paths, Verification, Implementation Roadmap, Message Protocol (+25 more)

### Community 82 - "permissions"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 83 - "permissions"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 84 - "graphify"
Cohesion: 0.18
Nodes (10): command, enabled, type, mcp, graphify, plugin, $schema, /home/shane/.local/bin/graphify-mcp (+2 more)

### Community 85 - "default.json"
Cohesion: 0.20
Nodes (9): canvas, core:default, core:event:allow-listen, sidebar, description, identifier, permissions, $schema (+1 more)

### Community 86 - "Capability"
Cohesion: 0.22
Nodes (10): description, required, type, Capability, description, required, type, Capability (+2 more)

### Community 87 - "webviews"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 88 - "webviews"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 91 - "CapabilityRemote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 92 - "CapabilityRemote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 93 - "graphify"
Cohesion: 0.25
Nodes (7): graphify, How to query it, Keeping it fresh (hooks), Repository Working Notes, Rules, Teaching the graph (work-memory loop), What's there

### Community 94 - "sidebar.rs"
Cohesion: 0.43
Nodes (7): ApprovalQueue(), ChatInput(), ChatMessages(), Element, Scope, Sidebar(), SidebarHeader()

### Community 97 - "ApprovalItem"
Cohesion: 0.33
Nodes (5): ApprovalItem, ApprovalQueue(), Element, String, Vec

### Community 98 - "pickSelectOption"
Cohesion: 0.50
Nodes (5): bindSelectCloser(), closeAllSelects(), loadModelsForBulk(), pickSelectOption(), syncCatalogSelection()

### Community 99 - "graphify-refresh.sh"
Cohesion: 0.33
Nodes (5): GRAPHIFY_OPENAI_MODEL, OPENAI_API_KEY, OPENAI_BASE_URL, PATH, graphify-refresh.sh script

### Community 100 - "ActionRecord"
Cohesion: 0.20
Nodes (16): CheckpointId, ActionRecord, file_store_round_trips_records(), FileActionStore, PersistenceError, ActionId, AsRef, CorrelationId (+8 more)

### Community 101 - "action.rs"
Cohesion: 0.13
Nodes (17): ActionError, ActionState, ActionStore, can_transition(), CheckpointState, PendingTransition, RecoveryOutcome, Send (+9 more)

### Community 102 - "Generative Surface Roadmap"
Cohesion: 0.50
Nodes (4): Research Issues Document, Generative Surface Roadmap, M8 UI Repair Plan, Legacy UI Specification

### Community 103 - "canvas.rs"
Cohesion: 0.67
Nodes (3): Canvas(), CanvasHeader(), Element

### Community 104 - "SettingsPanel"
Cohesion: 0.50
Nodes (3): Element, Scope, SettingsPanel()

### Community 115 - "stub_provider.rs"
Cohesion: 0.25
Nodes (12): escape_html(), fields_from_body(), main(), openai_response(), respond(), Option, String, Vec (+4 more)

### Community 156 - "Checkpoint"
Cohesion: 0.16
Nodes (11): Checkpoint, CommitError, HealthError, ResetError, RollbackError, MockWifiDriver, ActionId, Arc (+3 more)

### Community 186 - "coordinator/mod.rs"
Cohesion: 0.29
Nodes (9): AiosConfig, Path, PathBuf, ProviderConfig, config_dir_for(), expand_path(), resolve_local_model_path(), CatalogProvider (+1 more)

## Knowledge Gaps
- **220 isolated node(s):** `GraphEdge`, `GraphNode`, `MessageState`, `RolePanelState`, `SidebarRoute` (+215 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **129 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `NodeId` connect `NodeId` to `tests.rs`, `security.rs`, `harness.rs`, `processes.rs`, `SysfsDiscovery`, `graph.rs`, `power.rs`, `memory.rs`, `graphics.rs`, `storage.rs`, `drivers.rs`, `network.rs`, `packages.rs`, `boot.rs`, `wifi.rs`, `discovery.rs`, `panel.rs`, `.chat_with_tools_outcome`, `Coordinator`, `src/main.rs`?**
  _High betweenness centrality (0.059) - this node is a cross-community bridge._
- **Why does `SystemGraph` connect `NodeId` to `security.rs`, `harness.rs`, `processes.rs`, `SysfsDiscovery`, `graph.rs`, `power.rs`, `memory.rs`, `graphics.rs`, `storage.rs`, `drivers.rs`, `network.rs`, `packages.rs`, `boot.rs`, `wifi.rs`, `discovery.rs`, `panel.rs`, `Coordinator`, `.boot_with_probe`, `src/main.rs`?**
  _High betweenness centrality (0.044) - this node is a cross-community bridge._
- **Why does `ResourceId` connect `ResourceId` to `protocol.rs`, `security.rs`, `harness.rs`, `processes.rs`, `broker.rs`, `executor.rs`, `power.rs`, `memory.rs`, `graphics.rs`, `storage.rs`, `drivers.rs`, `network.rs`, `packages.rs`, `PrincipalId`, `boot.rs`, `Checkpoint`, `wifi_driver.rs`, `MockPlanner`, `ToolRequest`, `.chat_with_tools_outcome`, `.fmt`, `Coordinator`, `StagedExecutor`, `.boot_with_probe`, `src/main.rs`, `ActionRecord`?**
  _High betweenness centrality (0.039) - this node is a cross-community bridge._
- **Are the 16 inferred relationships involving `NodeId` (e.g. with `.run_tool_as()` and `.ensure_control_plane_edges()`) actually correct?**
  _`NodeId` has 16 INFERRED edges - model-reasoned connections that need verification._
- **Are the 5 inferred relationships involving `ResourceId` (e.g. with `.run_tool_as()` and `.boot_with_probe()`) actually correct?**
  _`ResourceId` has 5 INFERRED edges - model-reasoned connections that need verification._
- **What connects `GraphEdge`, `GraphNode`, `MessageState` to the rest of the system?**
  _220 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `src-tauri/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.08540540540540541 - nodes in this community are weakly interconnected._