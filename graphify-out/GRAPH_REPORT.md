# Graph Report - aios  (2026-08-21)

## Corpus Check
- 154 files · ~282,949 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 2786 nodes · 7629 edges · 115 communities (103 shown, 12 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 81 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `e62b55eb`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- discovery.rs
- Graph Components
- tools.rs
- Evidence Handling
- protocol.rs
- wifi_driver.rs
- NodeId
- tests.rs
- stub.rs
- composer.rs
- PrincipalId
- facade.rs
- Audit Logging
- harness.rs
- Model Catalog
- processes.rs
- broker.rs
- ResourceId
- power.rs
- drivers.rs
- memory.rs
- network.rs
- storage.rs
- graphics.rs
- HTTP Services
- Sidebar UI
- packages.rs
- boot.rs
- security.rs
- Coordinator
- LocalLlama
- Model Providers
- .boot_with_probe
- executor.rs
- Checkpoint Management
- Package Management
- Guardian
- wifi.rs
- Graph Activities
- StagedExecutor
- surface_harness.rs
- ToolRequest
- MockPlanner
- Provider Management
- .new
- planner.rs
- UI Elements
- config.rs
- Assets
- .chat_with_tools_outcome
- verifier.rs
- .grant_consent
- NodeType
- SurfaceDensity
- graphify-refresh.sh
- render.rs
- Project Grounding
- desktop-schema.json
- AiosConfig
- ModelId
- copilot-instructions.md
- definitions
- Linux Schema
- Widget Components
- Checkpoint
- install-graphify-hooks.sh
- schema.rs
- properties
- Schema Properties
- SurfaceWidget
- model.rs
- src/main.rs
- ProviderId
- graphify
- action.rs
- Schema References
- Schema References
- Project Documents
- Canvas Elements
- Capability Definitions
- Webview Types
- Webview Types
- Rendering Functions
- .fmt
- Capability Remote
- Capability Remote
- Surface Placement
- graphify
- Approval Queue
- Model Selection
- graph.rs
- Research Issues
- UI Specifications
- Settings Panel
- Chat Components
- Graph Visualization
- Aios-tauri
- End-to-End UI Tests
- Aios Logo
- Frontend App
- Aios Logo 128x128
- Aios Logo 128x128@2x
- Aios Logo 32x32

## God Nodes (most connected - your core abstractions)
1. `NodeId` - 154 edges
2. `SystemGraph` - 150 edges
3. `ResourceId` - 122 edges
4. `NodeMetadata` - 64 edges
5. `Coordinator` - 59 edges
6. `PrincipalId` - 58 edges
7. `PolicyBroker` - 51 edges
8. `ProviderId` - 41 edges
9. `stub_coordinator()` - 40 edges
10. `ToolRequest` - 39 edges

## Surprising Connections (you probably didn't know these)
- `build_graph_snapshot()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `handle_prompt()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `refresh_graph_snapshot()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `refresh_sidebar_status()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `sidebar_status_snapshot()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs

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

## Communities (115 total, 12 thin omitted)

### Community 0 - "discovery.rs"
Cohesion: 0.10
Nodes (51): device_firmware_attributes_create_nodes_and_edges(), devices_without_firmware_attributes_get_no_firmware_node(), discovered_nodes_go_stale_after_ttl(), DiscoveredService, discovery(), discovery_adds_dependency_edges(), DiscoveryError, DiscoveryEvent (+43 more)

### Community 1 - "Graph Components"
Cohesion: 0.07
Nodes (77): AppHandle, BackendStatus, EvidenceItem, GraphEdge, GraphNode, PromptResponse, Rectangle, RustConnection (+69 more)

### Community 2 - "tools.rs"
Cohesion: 0.07
Nodes (51): FnOnce, Sized, Dependencies, deps_renders_chain(), Diagnose, diagnose_reports_unhealthy_dependency(), edge(), find_nodes() (+43 more)

### Community 3 - "Evidence Handling"
Cohesion: 0.08
Nodes (66): cross_tool_reference_fails(), empty_results_give_empty_index(), empty_value_never_matches(), evidence_brief(), evidence_brief_quotes_keys_and_tools(), EvidenceEntry, EvidenceIndex, exact_copy_passes() (+58 more)

### Community 4 - "protocol.rs"
Cohesion: 0.07
Nodes (58): ApprovalId, CheckpointRef, MessageId, PlanId, RequestId, error_code_for(), ActionPlan, Approval (+50 more)

### Community 5 - "wifi_driver.rs"
Cohesion: 0.08
Nodes (33): checkpoint_captures_active_module(), driver(), DriverControl, fake_sysfs(), health_check_reflects_link_state(), LinuxDriverControl, live_control(), live_control_plans_mutations_without_executing() (+25 more)

### Community 6 - "NodeId"
Cohesion: 0.14
Nodes (18): Timestamp, EdgeMetadata, EdgeType, ImpactReport, NodeId, NodeMetadata, Display, HashMap (+10 more)

### Community 7 - "tests.rs"
Cohesion: 0.08
Nodes (60): tool_arguments(), boot_device(), boot_recovery_diagnose_reports_domain_invariants(), boot_recovery_observe_runs_through_broker(), boots_http_provider_and_status_shows_it(), broker_rolls_back_staged_request_when_health_fails(), broker_runs_staged_commit_through_booted_executor(), capability_token() (+52 more)

### Community 8 - "stub.rs"
Cohesion: 0.07
Nodes (52): ApprovalItem, app(), render_approval_item(), render_widget(), Element, String, Vec, spawn_message_task() (+44 more)

### Community 9 - "composer.rs"
Cohesion: 0.06
Nodes (47): compose_error_kind(), Coordinator, Option, Result, String, Surface, ToolResult, strip_think() (+39 more)

### Community 10 - "PrincipalId"
Cohesion: 0.06
Nodes (35): AuditEntryId, Fn, SpecialistCall, BrokerClient, LocalBroker, PolicyBroker, Arc, Box (+27 more)

### Community 11 - "facade.rs"
Cohesion: 0.07
Nodes (40): F, SplitWhitespace, audit_logs_chat_attempt(), audit_records_boot_and_commands(), bare_chat_goes_to_model(), consent_commands_roundtrip(), direct_model_query(), Facade (+32 more)

### Community 12 - "Audit Logging"
Cohesion: 0.10
Nodes (32): File, appends_across_sessions(), audit_log_path(), AuditEntry, AuditError, AuditLog, encode_field(), entries_are_forward_chained() (+24 more)

### Community 13 - "harness.rs"
Cohesion: 0.11
Nodes (35): all_steps_allow_when_capabilities_granted(), build_graph(), capabilities(), DeviceHistory, enforce_stops_campaign_at_first_denial(), harness_principal(), harness_tool_ids(), HarnessPlan (+27 more)

### Community 14 - "Model Catalog"
Cohesion: 0.10
Nodes (35): Arc<RecordingClient>, catalog_model_url_points_at_resolve_main(), CatalogModel, default_catalog(), hex(), HttpClient, HubError, model_with_sha256() (+27 more)

### Community 15 - "processes.rs"
Cohesion: 0.10
Nodes (38): cpu_cores(), cpu_sample(), cpu_stats(), CpuStats, diagnose_flags_missing_usage_evidence(), discovers_process_nodes(), exposes_only_read_only_tools(), format_process_row() (+30 more)

### Community 16 - "broker.rs"
Cohesion: 0.16
Nodes (33): Runtime, allows_with_valid_capability(), approval_channel_accepts_only_user_approval(), approval_channel_rejection_and_expiry_are_fail_closed(), approval_is_required_and_plan_hash_is_bound(), approval_request_for(), approval_scope_for(), audit_broken_denies_everything() (+25 more)

### Community 17 - "ResourceId"
Cohesion: 0.10
Nodes (33): tool(), Capability, GuardianClient, Operation, PrincipalType, ResourceId, RiskLevel, HashMap (+25 more)

### Community 18 - "power.rs"
Cohesion: 0.11
Nodes (31): diagnose_flags_missing_reading_evidence(), discovers_thermal_and_power_sensors(), exposes_only_read_only_tools(), health_counts_reading_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_power_sensor(), is_thermal_sensor() (+23 more)

### Community 19 - "drivers.rs"
Cohesion: 0.11
Nodes (30): diagnose_flags_missing_driver_attachment(), discovers_unclaimed_hardware_only(), DriversError, DriversHealth, DriversSpecialist, exposes_only_read_only_tools(), gpu_class_devices_are_not_claimed(), hardware_graph() (+22 more)

### Community 20 - "memory.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_capacity_evidence(), discovers_memory_nodes_and_ecc_sensors(), exposes_only_read_only_tools(), health_counts_capacity_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_memory_node(), memory_graph() (+21 more)

### Community 21 - "network.rs"
Cohesion: 0.11
Nodes (29): diagnose_flags_missing_connectivity(), discovers_wired_and_bluetooth_excludes_wireless(), exposes_only_read_only_tools(), health_counts_link_and_backing_evidence(), instantiates_with_owns_edges_skipping_wifi_owned(), is_bluetooth_controller(), is_wired_interface(), is_wireless() (+21 more)

### Community 22 - "storage.rs"
Cohesion: 0.11
Nodes (29): diagnose_flags_missing_capacity_and_backing(), discovers_block_devices_and_filesystems(), exposes_only_read_only_tools(), health_counts_capacity_and_backing_evidence(), instantiates_with_owns_edges_for_each_resource(), is_block_device(), is_filesystem(), node() (+21 more)

### Community 23 - "graphics.rs"
Cohesion: 0.11
Nodes (28): diagnose_flags_missing_gpu_state(), discovers_gpu_display_and_session(), exposes_only_read_only_tools(), graphics_graph(), GraphicsError, GraphicsHealth, GraphicsSpecialist, health_counts_state_evidence() (+20 more)

### Community 24 - "HTTP Services"
Cohesion: 0.13
Nodes (26): auth_header_sent_when_key_present(), backend(), empty_choices_is_error(), function_tool(), function_tool_no_args(), generate_hits_endpoint_and_parses(), health_check_against_live_server(), http_4xx_is_not_recoverable() (+18 more)

### Community 25 - "Sidebar UI"
Cohesion: 0.08
Nodes (37): computeActiveNodeIds(), EvidenceItem, FlightProgress, GRAPH_LAYER_Y, GraphEdge, GraphNode, graphReadout(), healthClass() (+29 more)

### Community 26 - "packages.rs"
Cohesion: 0.12
Nodes (27): diagnose_flags_missing_signature_evidence(), discovers_package_nodes(), exposes_only_read_only_tools(), health_counts_signature_evidence(), instantiates_with_owns_edges_for_each_resource(), is_package_node(), node(), not_found() (+19 more)

### Community 27 - "boot.rs"
Cohesion: 0.13
Nodes (26): boot_graph(), BootRecoveryError, BootRecoveryHealth, BootRecoverySpecialist, diagnose_flags_unhealthy_nodes(), discovers_boot_nodes(), exposes_only_read_only_tools(), health_counts_healthy_nodes() (+18 more)

### Community 28 - "security.rs"
Cohesion: 0.12
Nodes (26): diagnose_flags_missing_verified_evidence(), discovers_security_nodes(), exposes_only_read_only_tools(), health_counts_verified_evidence(), instantiates_with_owns_edges_for_each_resource(), is_security_node(), node(), not_found() (+18 more)

### Community 29 - "Coordinator"
Cohesion: 0.12
Nodes (22): ChatOutcome, classification_help(), config_dir_for(), Coordinator, expand_path(), ProviderCatalogue, providers_text(), resolve_local_model_path() (+14 more)

### Community 30 - "LocalLlama"
Cohesion: 0.10
Nodes (23): LlamaBackend, LlamaChatMessage, LlamaModel, LlamaToken, chat_messages(), chat_messages_maps_roles(), loads_and_generates_real_model(), LocalLlama (+15 more)

### Community 31 - "Model Providers"
Cohesion: 0.15
Nodes (16): CatalogProvider, Coordinator, DiscoveredModel, DiscoveredModel, Option, Result, String, Vec (+8 more)

### Community 32 - ".boot_with_probe"
Cohesion: 0.18
Nodes (11): BootError, Box, Display, Error, Formatter, Result, Self, seed_boot_domain() (+3 more)

### Community 33 - "executor.rs"
Cohesion: 0.22
Nodes (23): advance_to(), checkpoint_count(), checkpoint_verification_failure_enters_failed_and_retains_checkpoint(), failed_action_can_be_manually_recovered_from_retained_checkpoint(), fresh(), fresh_fault(), health_check_error_rolls_back(), health_check_failure_triggers_rollback() (+15 more)

### Community 34 - "Checkpoint Management"
Cohesion: 0.20
Nodes (16): CheckpointId, ActionRecord, file_store_round_trips_records(), FileActionStore, PersistenceError, ActionId, AsRef, CorrelationId (+8 more)

### Community 35 - "Package Management"
Cohesion: 0.07
Nodes (26): autoprefixer, author, dependencies, @tauri-apps/api, @tauri-apps/cli, description, devDependencies, autoprefixer (+18 more)

### Community 36 - "Guardian"
Cohesion: 0.17
Nodes (18): Guardian, guardian_allows_boot_config_with_fallback_image(), guardian_allows_read_only_operations(), guardian_allows_tested_firmware(), guardian_blocks_boot_config_without_fallback(), guardian_blocks_untested_driver_load(), guardian_blocks_untested_firmware(), InvariantCheck (+10 more)

### Community 37 - "wifi.rs"
Cohesion: 0.16
Nodes (19): diagnose_flags_missing_driver(), discovers_and_instantiates_from_seeded_graph(), exposes_bounded_tools_with_declared_risk(), health_reports_missing_dependencies_as_false(), health_sees_two_hop_driver_and_network_service(), observe_returns_device_state_metrics(), Display, Error (+11 more)

### Community 38 - "Graph Activities"
Cohesion: 0.10
Nodes (25): activityDetail(), activityLabel(), applyGraphActivity(), BackendStatus, currentWindow, DockEdge, GraphActivityEvent, LayoutMode (+17 more)

### Community 39 - "StagedExecutor"
Cohesion: 0.16
Nodes (17): HealthError, RollbackError, MockDriver, ResourceDriver, ActionId, Arc, Box, Mutex (+9 more)

### Community 40 - "surface_harness.rs"
Cohesion: 0.17
Nodes (24): ExitCode, boot(), Conversation, main(), Options, parse_args(), Probe, probe_conversation() (+16 more)

### Community 41 - "ToolRequest"
Cohesion: 0.24
Nodes (12): SpecialistHandler, BrokerError, build_request(), denied_result(), result_envelope(), Display, Formatter, Into (+4 more)

### Community 42 - "MockPlanner"
Cohesion: 0.20
Nodes (11): err_result(), MockPlanner, MockVerificationAgent, ok_result(), Capability, Option, Self, ToolResult (+3 more)

### Community 43 - "Provider Management"
Cohesion: 0.16
Nodes (24): addProvider(), assignRole(), assignRoleGroup(), autosizePrompt(), bindSidebar(), dockPanel(), loadProviderCatalog(), refreshGraph() (+16 more)

### Community 44 - ".new"
Cohesion: 0.19
Nodes (18): assigned_role_runs_on_assigned_provider_and_model(), ConnectivityState, failing_backend_surfaces_generation_error(), gateway_with(), internet_assignment_offline_fails(), ModelGateway, ModelRouter, personal_memory_needs_consent_for_assigned_provider() (+10 more)

### Community 45 - "planner.rs"
Cohesion: 0.09
Nodes (36): AgentError, empty_steps_still_parses(), extract_json(), extracts_json_from_prose(), format_plan(), garbage_becomes_freeform(), GeneratedPlan, missing_intent_falls_back() (+28 more)

### Community 46 - "UI Elements"
Cohesion: 0.18
Nodes (22): Child, Client, Drop, app_binary(), assert_surface(), ChildGuard, close_canvas(), dump_windows() (+14 more)

### Community 47 - "config.rs"
Cohesion: 0.14
Nodes (19): api_key_resolved_from_env(), default_ctx(), default_max_tokens(), default_threads(), dirs_home(), missing_config_is_default(), ModelConfig, parses_full_config() (+11 more)

### Community 48 - "Assets"
Cohesion: 0.10
Nodes (20): assets/128x128@2x.png, assets/128x128.png, assets/32x32.png, app, security, windows, build, beforeBuildCommand (+12 more)

### Community 49 - ".chat_with_tools_outcome"
Cohesion: 0.21
Nodes (14): Coordinator, operation_for_tool(), protocol_tool_result(), quote_value(), required_specialist_calls(), Option, Result, String (+6 more)

### Community 50 - "verifier.rs"
Cohesion: 0.09
Nodes (25): Coordinator, ActionId, PlanHash, Result, String, Uuid, scan_summary(), format_review() (+17 more)

### Community 51 - ".grant_consent"
Cohesion: 0.29
Nodes (4): Coordinator, Option, Result, String

### Community 52 - "NodeType"
Cohesion: 0.22
Nodes (19): process_health(), NodeType, count_health(), counts_health_and_never_hides_stale(), graph_with(), is_healthy(), PanelSnapshot, render() (+11 more)

### Community 53 - "SurfaceDensity"
Cohesion: 0.29
Nodes (6): LayoutMode, LayoutMode, Default, Self, SurfaceDensity, SurfaceLayout

### Community 54 - "graphify-refresh.sh"
Cohesion: 0.33
Nodes (5): GRAPHIFY_OPENAI_MODEL, OPENAI_API_KEY, OPENAI_BASE_URL, PATH, graphify-refresh.sh script

### Community 55 - "render.rs"
Cohesion: 0.22
Nodes (16): chart_renders_bars(), evidence_chips(), html_escape(), html_render_escapes_content(), placement_text(), priority_text(), region_html(), render_html() (+8 more)

### Community 56 - "Project Grounding"
Cohesion: 0.11
Nodes (18): Project Grounding Document, Session Notes Document, ADR-0001: Runs Above Linux, ADR-0002: Rust as Implementation Language, ADR-0003: Fail-Fast No Silent Fallbacks, ADR-0004: Two-Dimensional Authorization, ADR-0005: Freeze Triage, ADR-0006: Model Gateway (+10 more)

### Community 57 - "desktop-schema.json"
Cohesion: 0.40
Nodes (4): anyOf, description, $schema, title

### Community 58 - "AiosConfig"
Cohesion: 0.16
Nodes (17): AiosConfig, ConfigError, default_http_timeout_ms(), parse_capability(), parse_tier(), ProviderConfig, RoleAssignment, RolesConfig (+9 more)

### Community 59 - "ModelId"
Cohesion: 0.16
Nodes (12): GatewayError, GenerationError, ModelId, RegistryError, RoutingError, Display, Error, Formatter (+4 more)

### Community 61 - "definitions"
Cohesion: 0.12
Nodes (16): definitions, Identifier, Number, PermissionEntry, Target, Value, description, oneOf (+8 more)

### Community 62 - "Linux Schema"
Cohesion: 0.11
Nodes (17): anyOf, definitions, Number, PermissionEntry, Target, Value, description, anyOf (+9 more)

### Community 63 - "Widget Components"
Cohesion: 0.25
Nodes (16): ActionForm(), Chart(), ChartDataPoint, FormField, MetricCard(), ChartDataPoint, Element, FormField (+8 more)

### Community 64 - "Checkpoint"
Cohesion: 0.16
Nodes (11): Checkpoint, CheckpointError, CommitError, StageError, MockWifiDriver, ActionId, Arc, AtomicBool (+3 more)

### Community 66 - "schema.rs"
Cohesion: 0.18
Nodes (11): dangling_region_reference_deserializes_but_needs_validation(), DockEdge, example_surface(), missing_required_field_fails_to_deserialize(), RegionPriority, round_trips_a_valid_surface(), round_trips_every_widget_variant(), serializes_with_camelcase_fields_and_type_tags() (+3 more)

### Community 67 - "properties"
Cohesion: 0.14
Nodes (14): properties, default, description, type, type, default, description, type (+6 more)

### Community 68 - "Schema Properties"
Cohesion: 0.12
Nodes (17): properties, Identifier, default, description, type, description, oneOf, type (+9 more)

### Community 69 - "SurfaceWidget"
Cohesion: 0.20
Nodes (14): DockEdge, widget_title(), ChartPoint, Option, RegionPriority, StatusItem, String, Vec (+6 more)

### Community 70 - "model.rs"
Cohesion: 0.10
Nodes (28): AgentRole, combine(), DataPolicy, has_default_route(), has_default_route_v6(), http_reachable(), internet_model(), lan_model() (+20 more)

### Community 71 - "src/main.rs"
Cohesion: 0.20
Nodes (15): build_demo_plan(), build_graph(), describe(), kernel_module_request(), main(), register_principals(), register_tools(), Option (+7 more)

### Community 72 - "ProviderId"
Cohesion: 0.09
Nodes (17): AtomicU32, ConsentRecord, GatewayResponse, MockBackend, ModelRegistry, ProviderId, ProviderTier, ResourceRequirements (+9 more)

### Community 74 - "graphify"
Cohesion: 0.18
Nodes (10): command, enabled, type, mcp, graphify, plugin, $schema, /home/shane/.local/bin/graphify-mcp (+2 more)

### Community 75 - "action.rs"
Cohesion: 0.14
Nodes (14): ActionError, ActionState, ActionStore, can_transition(), CheckpointState, PendingTransition, RecoveryOutcome, ResetError (+6 more)

### Community 76 - "Schema References"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 77 - "Schema References"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 78 - "Project Documents"
Cohesion: 0.33
Nodes (11): Archive README, Implementation Roadmap, Message Protocol, Model Routing, Observability, Requirements, Security Model, Specialist Depth Plan (+3 more)

### Community 79 - "Canvas Elements"
Cohesion: 0.20
Nodes (9): canvas, core:default, core:event:allow-listen, sidebar, description, identifier, permissions, $schema (+1 more)

### Community 80 - "Capability Definitions"
Cohesion: 0.22
Nodes (10): description, required, type, Capability, description, required, type, Capability (+2 more)

### Community 81 - "Webview Types"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 82 - "Webview Types"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 83 - "Rendering Functions"
Cohesion: 0.39
Nodes (9): escapeHtml(), evidenceChips(), formatNumber(), gaugePercent(), renderCanvas(), renderChartBars(), renderSurface(), renderWidget() (+1 more)

### Community 85 - "Capability Remote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 86 - "Capability Remote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 87 - "Surface Placement"
Cohesion: 0.43
Nodes (7): ApprovalQueue(), ChatInput(), ChatMessages(), Element, Scope, Sidebar(), SidebarHeader()

### Community 89 - "graphify"
Cohesion: 0.25
Nodes (7): graphify, How to query it, Keeping it fresh (hooks), Repository Working Notes, Rules, Teaching the graph (work-memory loop), What's there

### Community 90 - "Approval Queue"
Cohesion: 0.33
Nodes (5): ApprovalItem, ApprovalQueue(), Element, String, Vec

### Community 91 - "Model Selection"
Cohesion: 0.40
Nodes (6): bindSelectCloser(), closeAllSelects(), loadModelsForBulk(), loadModelsForRole(), pickSelectOption(), syncCatalogSelection()

### Community 92 - "graph.rs"
Cohesion: 0.17
Nodes (23): add_edge_requires_both_endpoints(), add_node_rejects_duplicate(), dependencies_and_dependents_track_both_directions(), edge(), EdgeId, EdgeProvenance, impact_report_counts_related_components(), node() (+15 more)

### Community 93 - "Research Issues"
Cohesion: 0.50
Nodes (4): Research Issues Document, Generative Surface Roadmap, M8 UI Repair Plan, Legacy UI Specification

### Community 94 - "UI Specifications"
Cohesion: 0.67
Nodes (3): Canvas(), CanvasHeader(), Element

### Community 95 - "Settings Panel"
Cohesion: 0.50
Nodes (3): Element, Scope, SettingsPanel()

## Knowledge Gaps
- **195 isolated node(s):** `graphify`, `What's there`, `How to query it`, `Keeping it fresh (hooks)`, `Teaching the graph (work-memory loop)` (+190 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **12 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Coordinator` connect `Coordinator` to `Graph Components`, `NodeId`, `PrincipalId`, `Audit Logging`, `processes.rs`, `broker.rs`, `power.rs`, `drivers.rs`, `memory.rs`, `network.rs`, `storage.rs`, `graphics.rs`, `packages.rs`, `boot.rs`, `security.rs`, `.boot_with_probe`, `wifi.rs`, `.new`, `planner.rs`, `verifier.rs`, `AiosConfig`, `ProviderId`?**
  _High betweenness centrality (0.097) - this node is a cross-community bridge._
- **Why does `SystemGraph` connect `NodeId` to `discovery.rs`, `tools.rs`, `harness.rs`, `processes.rs`, `power.rs`, `drivers.rs`, `memory.rs`, `network.rs`, `storage.rs`, `graphics.rs`, `packages.rs`, `boot.rs`, `security.rs`, `Coordinator`, `.boot_with_probe`, `wifi.rs`, `verifier.rs`, `NodeType`, `src/main.rs`, `graph.rs`?**
  _High betweenness centrality (0.083) - this node is a cross-community bridge._
- **Why does `ResourceId` connect `ResourceId` to `protocol.rs`, `wifi_driver.rs`, `tests.rs`, `PrincipalId`, `harness.rs`, `processes.rs`, `broker.rs`, `power.rs`, `drivers.rs`, `memory.rs`, `network.rs`, `storage.rs`, `graphics.rs`, `packages.rs`, `boot.rs`, `security.rs`, `.boot_with_probe`, `executor.rs`, `Checkpoint Management`, `wifi.rs`, `StagedExecutor`, `ToolRequest`, `MockPlanner`, `.chat_with_tools_outcome`, `verifier.rs`, `Checkpoint`, `src/main.rs`, `.fmt`?**
  _High betweenness centrality (0.076) - this node is a cross-community bridge._
- **Are the 16 inferred relationships involving `NodeId` (e.g. with `.run_tool_as()` and `.ensure_control_plane_edges()`) actually correct?**
  _`NodeId` has 16 INFERRED edges - model-reasoned connections that need verification._
- **Are the 5 inferred relationships involving `ResourceId` (e.g. with `.run_tool_as()` and `.boot_with_probe()`) actually correct?**
  _`ResourceId` has 5 INFERRED edges - model-reasoned connections that need verification._
- **What connects `graphify`, `What's there`, `How to query it` to the rest of the system?**
  _195 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `discovery.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10031347962382445 - nodes in this community are weakly interconnected._