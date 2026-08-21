# Graph Report - aios  (2026-08-21)

## Corpus Check
- 155 files · ~284,368 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 2845 nodes · 7702 edges · 145 communities (116 shown, 29 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 104 edges (avg confidence: 0.79)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `1fbf2430`
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
- Checkpoint
- discovery.rs
- power.rs
- action.rs
- memory.rs
- graphics.rs
- storage.rs
- drivers.rs
- http.rs
- sidebar.ts
- network.rs
- packages.rs
- PolicyBroker
- boot.rs
- planner.rs
- model.rs
- Coordinator
- composer.rs
- wifi_driver.rs
- LocalLlama
- .refresh_catalogue
- evidence.rs
- stub.rs
- LinuxDriverControl
- GenerativeWidget
- ModelEntry
- executor.rs
- package.json
- Guardian
- wifi.rs
- main.ts
- MockPlanner
- DataClassification
- validator.rs
- PrincipalId
- surface_harness.rs
- render
- .new
- ui_e2e.rs
- config.rs
- tauri.conf.json
- NodeType
- verifier.rs
- ToolRequest
- StagedExecutor
- SurfaceComposeError
- .chat_with_tools_outcome
- render.rs
- Project Grounding Document
- ConfigError
- RoutingError
- AgentError
- definitions
- definitions
- widgets.rs
- Coordinator
- schema.rs
- properties
- properties
- SurfaceWidget
- AiosConfig
- .boot_with_probe
- src/main.rs
- ProviderId
- Testing Strategy
- String
- ValidationError
- Aios
- permissions
- permissions
- graphify
- default.json
- Capability
- webviews
- webviews
- escapeHtml
- .compose_surface_with_meta
- CapabilityRemote
- CapabilityRemote
- graphify
- sidebar.rs
- SurfaceDensity
- coordinator/mod.rs
- ApprovalItem
- pickSelectOption
- graphify-refresh.sh
- .fmt
- Display
- Generative Surface Roadmap
- canvas.rs
- SettingsPanel
- ChatMessages
- graphify.js
- .fmt
- .keys
- Filesystem Specialist
- copilot-instructions.md
- aios
- install-graphify-hooks.sh
- ui-e2e.sh
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
- `refresh_sidebar_status()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `sidebar_status_snapshot()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `build_graph_snapshot()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `refresh_graph_snapshot()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `handle_prompt()` --references--> `Facade`  [EXTRACTED]
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

## Communities (145 total, 29 thin omitted)

### Community 0 - "src-tauri/src/main.rs"
Cohesion: 0.07
Nodes (77): AppHandle, BackendStatus, EvidenceItem, GraphEdge, GraphNode, PromptResponse, Rectangle, RustConnection (+69 more)

### Community 1 - "tools.rs"
Cohesion: 0.07
Nodes (52): FnOnce, Sized, Dependencies, deps_renders_chain(), Diagnose, diagnose_reports_unhealthy_dependency(), edge(), find_nodes() (+44 more)

### Community 2 - "NodeId"
Cohesion: 0.09
Nodes (40): Timestamp, Timestamp, add_edge_requires_both_endpoints(), add_node_rejects_duplicate(), dependencies_and_dependents_track_both_directions(), edge(), EdgeId, EdgeMetadata (+32 more)

### Community 3 - "protocol.rs"
Cohesion: 0.08
Nodes (56): ApprovalId, AuditEntryId, CheckpointRef, MessageId, PlanId, RequestId, ActionPlan, Approval (+48 more)

### Community 4 - "tests.rs"
Cohesion: 0.08
Nodes (59): tool_arguments(), boot_device(), boot_recovery_diagnose_reports_domain_invariants(), boot_recovery_observe_runs_through_broker(), boots_http_provider_and_status_shows_it(), broker_rolls_back_staged_request_when_health_fails(), broker_runs_staged_commit_through_booted_executor(), capability_token() (+51 more)

### Community 5 - "security.rs"
Cohesion: 0.12
Nodes (27): diagnose_flags_missing_verified_evidence(), discovers_security_nodes(), exposes_only_read_only_tools(), health_counts_verified_evidence(), instantiates_with_owns_edges_for_each_resource(), is_security_node(), node(), not_found() (+19 more)

### Community 6 - "facade.rs"
Cohesion: 0.07
Nodes (39): F, SplitWhitespace, audit_logs_chat_attempt(), audit_records_boot_and_commands(), bare_chat_goes_to_model(), consent_commands_roundtrip(), direct_model_query(), Facade (+31 more)

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
Cohesion: 0.09
Nodes (34): CorrelationId, tool(), Capability, GuardianClient, Operation, PrincipalType, ResourceId, RiskLevel (+26 more)

### Community 11 - "processes.rs"
Cohesion: 0.10
Nodes (37): cpu_cores(), cpu_sample(), cpu_stats(), CpuStats, diagnose_flags_missing_usage_evidence(), discovers_process_nodes(), exposes_only_read_only_tools(), format_process_row() (+29 more)

### Community 12 - "broker.rs"
Cohesion: 0.16
Nodes (33): Runtime, allows_with_valid_capability(), approval_channel_accepts_only_user_approval(), approval_channel_rejection_and_expiry_are_fail_closed(), approval_is_required_and_plan_hash_is_bound(), approval_request_for(), approval_scope_for(), audit_broken_denies_everything() (+25 more)

### Community 13 - "SysfsDiscovery"
Cohesion: 0.23
Nodes (17): DiscoveryError, filesystem_usage(), parse_diskstat(), parse_meminfo(), parse_pressure(), parse_pressure_and_vmstat_key_their_fields(), parse_vmstat(), Display (+9 more)

### Community 14 - "Checkpoint"
Cohesion: 0.13
Nodes (17): Checkpoint, CheckpointError, CommitError, HealthError, RollbackError, StageError, MockDriver, ActionId (+9 more)

### Community 15 - "discovery.rs"
Cohesion: 0.13
Nodes (34): device_firmware_attributes_create_nodes_and_edges(), devices_without_firmware_attributes_get_no_firmware_node(), discovered_nodes_go_stale_after_ttl(), DiscoveredService, discovery(), discovery_adds_dependency_edges(), DiscoveryEvent, DiscoveryOptions (+26 more)

### Community 16 - "power.rs"
Cohesion: 0.11
Nodes (31): diagnose_flags_missing_reading_evidence(), discovers_thermal_and_power_sensors(), exposes_only_read_only_tools(), health_counts_reading_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_power_sensor(), is_thermal_sensor() (+23 more)

### Community 17 - "action.rs"
Cohesion: 0.12
Nodes (25): CheckpointId, ActionError, ActionRecord, ActionStore, CheckpointState, file_store_round_trips_records(), FileActionStore, PendingTransition (+17 more)

### Community 18 - "memory.rs"
Cohesion: 0.12
Nodes (30): diagnose_flags_missing_capacity_evidence(), discovers_memory_nodes_and_ecc_sensors(), exposes_only_read_only_tools(), health_counts_capacity_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_memory_node(), memory_graph() (+22 more)

### Community 19 - "graphics.rs"
Cohesion: 0.11
Nodes (29): diagnose_flags_missing_gpu_state(), discovers_gpu_display_and_session(), exposes_only_read_only_tools(), graphics_graph(), GraphicsError, GraphicsHealth, GraphicsSpecialist, health_counts_state_evidence() (+21 more)

### Community 20 - "storage.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_capacity_and_backing(), discovers_block_devices_and_filesystems(), exposes_only_read_only_tools(), health_counts_capacity_and_backing_evidence(), instantiates_with_owns_edges_for_each_resource(), is_block_device(), is_filesystem(), node() (+21 more)

### Community 21 - "drivers.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_driver_attachment(), discovers_unclaimed_hardware_only(), DriversError, DriversHealth, DriversSpecialist, exposes_only_read_only_tools(), gpu_class_devices_are_not_claimed(), hardware_graph() (+21 more)

### Community 22 - "http.rs"
Cohesion: 0.13
Nodes (26): auth_header_sent_when_key_present(), backend(), empty_choices_is_error(), function_tool(), function_tool_no_args(), generate_hits_endpoint_and_parses(), health_check_against_live_server(), http_4xx_is_not_recoverable() (+18 more)

### Community 23 - "sidebar.ts"
Cohesion: 0.08
Nodes (37): computeActiveNodeIds(), EvidenceItem, FlightProgress, GRAPH_LAYER_Y, GraphEdge, GraphNode, graphReadout(), healthClass() (+29 more)

### Community 24 - "network.rs"
Cohesion: 0.12
Nodes (28): diagnose_flags_missing_connectivity(), discovers_wired_and_bluetooth_excludes_wireless(), exposes_only_read_only_tools(), health_counts_link_and_backing_evidence(), instantiates_with_owns_edges_skipping_wifi_owned(), is_bluetooth_controller(), is_wired_interface(), is_wireless() (+20 more)

### Community 25 - "packages.rs"
Cohesion: 0.12
Nodes (27): diagnose_flags_missing_signature_evidence(), discovers_package_nodes(), exposes_only_read_only_tools(), health_counts_signature_evidence(), instantiates_with_owns_edges_for_each_resource(), is_package_node(), node(), not_found() (+19 more)

### Community 26 - "PolicyBroker"
Cohesion: 0.08
Nodes (22): Fn, SpecialistCall, BrokerClient, LocalBroker, PolicyBroker, Arc, Box, Default (+14 more)

### Community 27 - "boot.rs"
Cohesion: 0.13
Nodes (26): boot_graph(), BootRecoveryError, BootRecoveryHealth, BootRecoverySpecialist, diagnose_flags_unhealthy_nodes(), discovers_boot_nodes(), exposes_only_read_only_tools(), health_counts_healthy_nodes() (+18 more)

### Community 28 - "planner.rs"
Cohesion: 0.12
Nodes (25): empty_steps_still_parses(), extract_json(), extracts_json_from_prose(), format_plan(), garbage_becomes_freeform(), GeneratedPlan, missing_intent_falls_back(), multiple_calls_parsed_in_order() (+17 more)

### Community 29 - "model.rs"
Cohesion: 0.10
Nodes (21): StubProbe, FakeProbe, combine(), ConnectivityProbe, ConnectivityState, FinishReason, GatewayResponse, GenerationResponse (+13 more)

### Community 30 - "Coordinator"
Cohesion: 0.11
Nodes (16): ChatOutcome, classification_help(), Coordinator, ProviderCatalogue, providers_text(), Arc, DiscoveredModel, Option (+8 more)

### Community 31 - "composer.rs"
Cohesion: 0.11
Nodes (23): aios_markers(), content_numbers(), coverage_gap_reported_for_requested_domain_without_evidence(), coverage_gaps(), fidelity_rejects_changed_value(), fidelity_rejects_unknown_field(), instructions_describe_closed_vocabulary(), instructions_forbid_derived_values() (+15 more)

### Community 32 - "wifi_driver.rs"
Cohesion: 0.13
Nodes (25): checkpoint_captures_active_module(), driver(), DriverControl, fake_sysfs(), health_check_reflects_link_state(), live_control(), live_control_plans_mutations_without_executing(), live_control_reads_module_and_version_from_sysfs() (+17 more)

### Community 33 - "LocalLlama"
Cohesion: 0.11
Nodes (20): LlamaBackend, LlamaChatMessage, LlamaModel, LlamaToken, chat_messages(), chat_messages_maps_roles(), loads_and_generates_real_model(), LocalLlama (+12 more)

### Community 34 - ".refresh_catalogue"
Cohesion: 0.15
Nodes (16): CatalogProvider, Coordinator, DiscoveredModel, DiscoveredModel, Option, Result, String, Vec (+8 more)

### Community 35 - "evidence.rs"
Cohesion: 0.16
Nodes (26): cross_tool_reference_fails(), empty_results_give_empty_index(), empty_value_never_matches(), evidence_brief(), evidence_brief_quotes_keys_and_tools(), EvidenceEntry, EvidenceIndex, exact_copy_passes() (+18 more)

### Community 36 - "stub.rs"
Cohesion: 0.14
Nodes (25): all_themed_surfaces_validate_against_health_evidence(), compose_body(), health_evidence(), health_theme_binds_real_numbers_from_evidence(), number_after(), number_before(), openai_response(), respond() (+17 more)

### Community 37 - "LinuxDriverControl"
Cohesion: 0.16
Nodes (8): LinuxDriverControl, MockDriverControl, Default, Option, PathBuf, Result, String, Vec

### Community 38 - "GenerativeWidget"
Cohesion: 0.13
Nodes (27): ApprovalItem, app(), render_approval_item(), render_widget(), Element, String, Vec, spawn_message_task() (+19 more)

### Community 39 - "ModelEntry"
Cohesion: 0.11
Nodes (17): AtomicU32, DataPolicy, lan_model(), MockBackend, ModelEntry, ModelId, ModelProvenance, ProviderHealth (+9 more)

### Community 40 - "executor.rs"
Cohesion: 0.20
Nodes (25): advance_to(), checkpoint_count(), checkpoint_verification_failure_enters_failed_and_retains_checkpoint(), failed_action_can_be_manually_recovered_from_retained_checkpoint(), fresh(), fresh_fault(), health_check_error_rolls_back(), health_check_failure_triggers_rollback() (+17 more)

### Community 41 - "package.json"
Cohesion: 0.07
Nodes (26): autoprefixer, author, dependencies, @tauri-apps/api, @tauri-apps/cli, description, devDependencies, autoprefixer (+18 more)

### Community 42 - "Guardian"
Cohesion: 0.17
Nodes (18): Guardian, guardian_allows_boot_config_with_fallback_image(), guardian_allows_read_only_operations(), guardian_allows_tested_firmware(), guardian_blocks_boot_config_without_fallback(), guardian_blocks_untested_driver_load(), guardian_blocks_untested_firmware(), InvariantCheck (+10 more)

### Community 43 - "wifi.rs"
Cohesion: 0.16
Nodes (19): diagnose_flags_missing_driver(), discovers_and_instantiates_from_seeded_graph(), exposes_bounded_tools_with_declared_risk(), health_reports_missing_dependencies_as_false(), health_sees_two_hop_driver_and_network_service(), observe_returns_device_state_metrics(), Display, Error (+11 more)

### Community 44 - "main.ts"
Cohesion: 0.10
Nodes (25): activityDetail(), activityLabel(), applyGraphActivity(), BackendStatus, currentWindow, DockEdge, GraphActivityEvent, LayoutMode (+17 more)

### Community 45 - "MockPlanner"
Cohesion: 0.18
Nodes (14): error_code_for(), err_result(), MockPlanner, MockVerificationAgent, ok_result(), Capability, Option, Self (+6 more)

### Community 46 - "DataClassification"
Cohesion: 0.14
Nodes (16): Coordinator, Option, Result, String, AgentRole, ConsentRecord, internet_model(), ModelCapability (+8 more)

### Community 47 - "validator.rs"
Cohesion: 0.23
Nodes (25): base_surface(), chart_point_missing_is_diagnostic_not_error(), compact_request_requires_compact_density(), compact_status_list_has_a_hard_row_budget(), cross_tool_evidence_does_not_satisfy_value(), dangling_region_reference_rejected(), diagnostics(), fabricated_gauge_value_rejected() (+17 more)

### Community 48 - "PrincipalId"
Cohesion: 0.14
Nodes (12): Capability, Vec, CapabilityToken, PrincipalId, Provenance, Into, PackageId, Self (+4 more)

### Community 49 - "surface_harness.rs"
Cohesion: 0.19
Nodes (23): ExitCode, boot(), Conversation, main(), Options, parse_args(), Probe, probe_conversation() (+15 more)

### Community 50 - "render"
Cohesion: 0.16
Nodes (24): addProvider(), assignRole(), assignRoleGroup(), autosizePrompt(), bindSidebar(), dockPanel(), loadProviderCatalog(), refreshGraph() (+16 more)

### Community 51 - ".new"
Cohesion: 0.28
Nodes (15): assigned_role_runs_on_assigned_provider_and_model(), failing_backend_surfaces_generation_error(), gateway_with(), internet_assignment_offline_fails(), ModelGateway, personal_memory_needs_consent_for_assigned_provider(), recoverable_generation_errors_are_retried_once(), registry_with() (+7 more)

### Community 52 - "ui_e2e.rs"
Cohesion: 0.18
Nodes (22): Child, Client, Drop, app_binary(), assert_surface(), ChildGuard, close_canvas(), dump_windows() (+14 more)

### Community 53 - "config.rs"
Cohesion: 0.14
Nodes (17): api_key_resolved_from_env(), default_ctx(), default_http_timeout_ms(), default_max_tokens(), default_threads(), dirs_home(), missing_config_is_default(), parses_full_config() (+9 more)

### Community 54 - "tauri.conf.json"
Cohesion: 0.10
Nodes (20): assets/128x128@2x.png, assets/128x128.png, assets/32x32.png, app, security, windows, build, beforeBuildCommand (+12 more)

### Community 55 - "NodeType"
Cohesion: 0.22
Nodes (19): process_health(), NodeType, count_health(), counts_health_and_never_hides_stale(), graph_with(), is_healthy(), PanelSnapshot, render() (+11 more)

### Community 56 - "verifier.rs"
Cohesion: 0.19
Nodes (18): format_review(), garbage_becomes_freeform(), loose_review(), parse_review(), parses_approve(), parses_approve_with_conditions(), parses_reject(), review_formats_verdict() (+10 more)

### Community 57 - "ToolRequest"
Cohesion: 0.20
Nodes (13): SpecialistHandler, BrokerError, build_request(), denied_result(), result_envelope(), Display, Formatter, Into (+5 more)

### Community 58 - "StagedExecutor"
Cohesion: 0.20
Nodes (12): ActionState, can_transition(), RecoveryOutcome, TransitionError, Arc, Box, Mutex, Option (+4 more)

### Community 59 - "SurfaceComposeError"
Cohesion: 0.15
Nodes (17): compose_error_kind(), compose_surface(), compose_surface_with_meta(), compose_unconstrained_html(), parse_surface(), parse_surface_accepts_fenced_json(), parse_surface_keeps_verbose_error_on_schema_rejection(), parse_surface_rejects_prose_without_json() (+9 more)

### Community 60 - ".chat_with_tools_outcome"
Cohesion: 0.24
Nodes (12): Coordinator, operation_for_tool(), protocol_tool_result(), quote_value(), required_specialist_calls(), Option, Result, String (+4 more)

### Community 61 - "render.rs"
Cohesion: 0.22
Nodes (16): chart_renders_bars(), evidence_chips(), html_escape(), html_render_escapes_content(), placement_text(), priority_text(), region_html(), render_html() (+8 more)

### Community 62 - "Project Grounding Document"
Cohesion: 0.11
Nodes (18): Project Grounding Document, Session Notes Document, ADR-0001: Runs Above Linux, ADR-0002: Rust as Implementation Language, ADR-0003: Fail-Fast No Silent Fallbacks, ADR-0004: Two-Dimensional Authorization, ADR-0005: Freeze Triage, ADR-0006: Model Gateway (+10 more)

### Community 63 - "ConfigError"
Cohesion: 0.24
Nodes (8): ConfigError, parse_capability(), parse_tier(), Display, Error, Formatter, Result, Self

### Community 64 - "RoutingError"
Cohesion: 0.20
Nodes (9): GatewayError, GenerationError, registry_rejects_duplicate_model(), RegistryError, RoutingError, Display, Error, Formatter (+1 more)

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

### Community 70 - "schema.rs"
Cohesion: 0.17
Nodes (11): dangling_region_reference_deserializes_but_needs_validation(), DockEdge, example_surface(), missing_required_field_fails_to_deserialize(), RegionPriority, round_trips_a_valid_surface(), round_trips_every_widget_variant(), serializes_with_camelcase_fields_and_type_tags() (+3 more)

### Community 71 - "properties"
Cohesion: 0.12
Nodes (17): properties, Identifier, default, description, type, description, oneOf, type (+9 more)

### Community 72 - "properties"
Cohesion: 0.12
Nodes (17): properties, Identifier, default, description, type, description, oneOf, type (+9 more)

### Community 73 - "SurfaceWidget"
Cohesion: 0.20
Nodes (14): DockEdge, widget_title(), ChartPoint, Option, RegionPriority, StatusItem, String, Vec (+6 more)

### Community 74 - "AiosConfig"
Cohesion: 0.26
Nodes (11): AiosConfig, ModelConfig, ProviderConfig, RoleAssignment, RolesConfig, Default, HashMap, Option (+3 more)

### Community 75 - ".boot_with_probe"
Cohesion: 0.21
Nodes (9): BootError, Box, Display, Error, Formatter, Result, Self, seed_boot_domain() (+1 more)

### Community 76 - "src/main.rs"
Cohesion: 0.22
Nodes (14): build_demo_plan(), build_graph(), describe(), kernel_module_request(), main(), register_principals(), register_tools(), Option (+6 more)

### Community 77 - "ProviderId"
Cohesion: 0.22
Nodes (4): ModelRegistry, ProviderId, Item, Iterator

### Community 78 - "Testing Strategy"
Cohesion: 0.38
Nodes (14): Action State Machine, Agent Packages, Architecture Vision, Capability Model, Bespoke Graph Snapshot, Coordinator Modularization Snapshot, Human Interaction, Testing Strategy (+6 more)

### Community 79 - "String"
Cohesion: 0.26
Nodes (6): local_model(), ModelMessage, ModelRole, Into, Self, String

### Community 80 - "ValidationError"
Cohesion: 0.27
Nodes (13): err(), evidence_check(), layout_check(), requested_top_count(), Display, Error, Formatter, Option (+5 more)

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

### Community 89 - "escapeHtml"
Cohesion: 0.39
Nodes (9): escapeHtml(), evidenceChips(), formatNumber(), gaugePercent(), renderCanvas(), renderChartBars(), renderSurface(), renderWidget() (+1 more)

### Community 90 - ".compose_surface_with_meta"
Cohesion: 0.39
Nodes (6): Coordinator, Option, Result, String, Surface, ToolResult

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

### Community 95 - "SurfaceDensity"
Cohesion: 0.29
Nodes (6): LayoutMode, LayoutMode, Default, Self, SurfaceDensity, SurfaceLayout

### Community 96 - "coordinator/mod.rs"
Cohesion: 0.52
Nodes (6): config_dir_for(), expand_path(), resolve_local_model_path(), HashMap, Path, PathBuf

### Community 97 - "ApprovalItem"
Cohesion: 0.33
Nodes (5): ApprovalItem, ApprovalQueue(), Element, String, Vec

### Community 98 - "pickSelectOption"
Cohesion: 0.40
Nodes (6): bindSelectCloser(), closeAllSelects(), loadModelsForBulk(), loadModelsForRole(), pickSelectOption(), syncCatalogSelection()

### Community 99 - "graphify-refresh.sh"
Cohesion: 0.33
Nodes (5): GRAPHIFY_OPENAI_MODEL, OPENAI_API_KEY, OPENAI_BASE_URL, PATH, graphify-refresh.sh script

### Community 101 - "Display"
Cohesion: 0.83
Nodes (4): Display, Graphics Specialist, GPU, Session

### Community 102 - "Generative Surface Roadmap"
Cohesion: 0.50
Nodes (4): Research Issues Document, Generative Surface Roadmap, M8 UI Repair Plan, Legacy UI Specification

### Community 103 - "canvas.rs"
Cohesion: 0.67
Nodes (3): Canvas(), CanvasHeader(), Element

### Community 104 - "SettingsPanel"
Cohesion: 0.50
Nodes (3): Element, Scope, SettingsPanel()

## Knowledge Gaps
- **228 isolated node(s):** `The problem I'm trying to solve`, `The rule everything else follows`, `What actually happens when you ask for something`, `Where the project actually is`, `What I need help with` (+223 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **29 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ResourceId` connect `ResourceId` to `protocol.rs`, `tests.rs`, `security.rs`, `harness.rs`, `processes.rs`, `broker.rs`, `Checkpoint`, `power.rs`, `action.rs`, `memory.rs`, `graphics.rs`, `storage.rs`, `drivers.rs`, `network.rs`, `packages.rs`, `PolicyBroker`, `boot.rs`, `wifi_driver.rs`, `executor.rs`, `wifi.rs`, `MockPlanner`, `PrincipalId`, `ToolRequest`, `.chat_with_tools_outcome`, `Coordinator`, `.boot_with_probe`, `src/main.rs`, `.fmt`?**
  _High betweenness centrality (0.065) - this node is a cross-community bridge._
- **Why does `Coordinator` connect `Coordinator` to `src-tauri/src/main.rs`, `NodeId`, `security.rs`, `AuditLog`, `processes.rs`, `broker.rs`, `power.rs`, `memory.rs`, `graphics.rs`, `storage.rs`, `drivers.rs`, `network.rs`, `packages.rs`, `boot.rs`, `model.rs`, `wifi.rs`, `PrincipalId`, `.new`, `verifier.rs`, `AgentError`, `AiosConfig`, `.boot_with_probe`, `ProviderId`, `coordinator/mod.rs`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **Why does `SystemGraph` connect `NodeId` to `tools.rs`, `security.rs`, `harness.rs`, `processes.rs`, `SysfsDiscovery`, `discovery.rs`, `power.rs`, `memory.rs`, `graphics.rs`, `storage.rs`, `drivers.rs`, `network.rs`, `packages.rs`, `boot.rs`, `Coordinator`, `wifi.rs`, `NodeType`, `Coordinator`, `.boot_with_probe`, `src/main.rs`?**
  _High betweenness centrality (0.056) - this node is a cross-community bridge._
- **Are the 16 inferred relationships involving `NodeId` (e.g. with `.run_tool_as()` and `.ensure_control_plane_edges()`) actually correct?**
  _`NodeId` has 16 INFERRED edges - model-reasoned connections that need verification._
- **Are the 5 inferred relationships involving `ResourceId` (e.g. with `.run_tool_as()` and `.boot_with_probe()`) actually correct?**
  _`ResourceId` has 5 INFERRED edges - model-reasoned connections that need verification._
- **What connects `The problem I'm trying to solve`, `The rule everything else follows`, `What actually happens when you ask for something` to the rest of the system?**
  _228 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `src-tauri/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07196627521830774 - nodes in this community are weakly interconnected._