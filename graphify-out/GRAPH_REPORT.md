# Graph Report - aios  (2026-08-25)

## Corpus Check
- 189 files · ~315,008 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 3106 nodes · 7930 edges · 258 communities (113 shown, 145 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 90 edges (avg confidence: 0.78)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `04b9395e`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- tools.rs
- src-tauri/src/main.rs
- facade.rs
- NodeId
- protocol.rs
- tests.rs
- wifi_driver.rs
- sandbox.rs
- config.rs
- ResourceId
- composer.rs
- AuditLog
- harness.rs
- hub.rs
- processes.rs
- ActionRecord
- files.rs
- discovery.rs
- broker.rs
- ProviderId
- SysfsDiscovery
- http.rs
- Aios
- power.rs
- storage.rs
- drivers.rs
- memory.rs
- graphics.rs
- network.rs
- security.rs
- web.rs
- Coordinator
- boot.rs
- Result
- packages.rs
- sidebar.ts
- Checkpoint
- GenerationRequest
- evidence.rs
- PrincipalId
- .refresh_catalogue
- main.ts
- GenerativeWidget
- executor.rs
- package.json
- StagedExecutor
- Guardian
- Vec
- planner.rs
- ToolRequest
- AgentError
- wifi.rs
- .chat_with_tools_outcome
- MockPlanner
- model.rs
- action.rs
- tauri.conf.json
- NodeType
- verifier.rs
- Testing Strategy
- .boot_with_probe
- .new
- render
- Project Grounding Document
- definitions
- MockBackend
- widgets.rs
- Coordinator
- properties
- src/main.rs
- definitions
- stub_provider.rs
- String
- ModelId
- properties
- .fmt
- permissions
- permissions
- project.rs
- graphify
- Surface Harness
- default.json
- Capability
- webviews
- webviews
- progress.rs
- CapabilityRemote
- CapabilityRemote
- graphify
- sidebar.rs
- resolve_local_model_path
- .grant_consent
- ApprovalItem
- graphify-refresh.sh
- .fmt
- ui_file_artifact.rs
- pickSelectOption
- Q: Which surface generation path is Aios's intended architecture?
- ActionId
- Display
- Generative Surface Roadmap
- canvas.rs
- SettingsPanel
- ChatMessages
- loadProviderCatalog
- graphify.js
- dev.sh
- Filesystem Specialist
- copilot-instructions.md
- aios
- check-docs.sh
- install-docs-hook.sh
- install-graphify-hooks.sh
- new-grounding.sh
- ui-e2e.sh
- AgentError
- Aios Logo
- BootError
- AiosConfig
- Box
- DiscoveredModel
- Clearance
- ConnectivityProbe
- ConnectivityState
- GraphPhase
- PathBuf
- DockEdge
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
- ProgressSink
- Error
- ExitCode
- Facade
- Formatter
- From
- Aios Frontend
- GatewayError
- GenerationError
- GenerationRequest
- GenerationResponse
- Knowledge Graph
- SystemGraph
- HealthState
- ToolRegistry
- Item
- Iterator
- LayoutMode
- ModelBackend
- ModelEntry
- ModelGateway
- ModelId
- ModelRegistry
- Value
- Operation
- Path
- aios-frontend
- Planner
- AsRef
- ProviderId
- ProviderTier
- RegistryError
- CompositeDriver
- RiskLevel
- RoutingDecision
- RoutingError
- RwLock
- Self
- Default
- Sender
- Aios Logo (128x128)
- Aios Logo (128x128@2x)
- Aios Logo (32x32)
- Arc
- Coordinator
- Option
- PathBuf
- Result
- String
- Surface
- ToolResult
- Value
- Vec
- Capability
- ToolError
- ModelGateway
- RoutingDecision
- Surface
- ConnectivityProbe
- ConnectivityState
- ConnectivityProbe
- ConnectivityState
- RoutingDecision
- Surface
- Agent
- AtomicBool
- PlanHash
- ModelGateway
- RoutingDecision
- Surface
- RegionPriority
- String
- Surface
- Default
- Option
- RegionPriority
- Self
- StatusItem
- String
- Vec
- Arc
- Mutex
- Option
- Self
- String
- Surface
- Vec
- Display
- Error
- Formatter
- Option
- Result
- String
- Surface
- ToolResult
- Vec
- RoutingDecision
- Surface
- Duration
- Into
- Mutex
- ToolRequest
- ToolResult
- Send
- WidthClass
- Sync
- Timestamp
- Uuid

## God Nodes (most connected - your core abstractions)
1. `ResourceId` - 148 edges
2. `NodeId` - 139 edges
3. `SystemGraph` - 135 edges
4. `NodeMetadata` - 61 edges
5. `Coordinator` - 59 edges
6. `PrincipalId` - 59 edges
7. `PolicyBroker` - 51 edges
8. `ToolRequest` - 46 edges
9. `ProviderId` - 45 edges
10. `stub_coordinator()` - 42 edges

## Surprising Connections (you probably didn't know these)
- `harness_direct_broker()` --calls--> `Clearance`  [INFERRED]
  tests/harness_drive.rs → src/capability.rs
- `harness_direct_broker()` --calls--> `ResourceId`  [INFERRED]
  tests/harness_drive.rs → src/capability.rs
- `write_surface_trace()` --references--> `RoutingDecision`  [EXTRACTED]
  src-tauri/src/main.rs → src/model.rs
- `build_graph_snapshot()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs
- `handle_prompt()` --references--> `Facade`  [EXTRACTED]
  src-tauri/src/main.rs → src/facade.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
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

## Communities (258 total, 145 thin omitted)

### Community 0 - "tools.rs"
Cohesion: 0.07
Nodes (58): EdgeMetadata, EdgeType, FnOnce, NodeMetadata, NodeType, Sized, Dependencies, deps_renders_chain() (+50 more)

### Community 1 - "src-tauri/src/main.rs"
Cohesion: 0.07
Nodes (81): AppHandle, BackendStatus, EvidenceItem, GraphActivity, GraphEdge, GraphNode, ProgressReporter, PromptResponse (+73 more)

### Community 2 - "facade.rs"
Cohesion: 0.05
Nodes (53): F, SplitWhitespace, audit_logs_chat_attempt(), audit_records_boot_and_commands(), bare_chat_goes_to_model(), consent_commands_roundtrip(), direct_model_query(), Facade (+45 more)

### Community 3 - "NodeId"
Cohesion: 0.09
Nodes (40): add_edge_requires_both_endpoints(), add_node_rejects_duplicate(), dependencies_and_dependents_track_both_directions(), edge(), EdgeId, EdgeMetadata, EdgeProvenance, EdgeType (+32 more)

### Community 4 - "protocol.rs"
Cohesion: 0.08
Nodes (60): ApprovalId, AuditEntryId, CheckpointRef, MessageId, PlanHash, PlanId, RequestId, ActionPlan (+52 more)

### Community 5 - "tests.rs"
Cohesion: 0.07
Nodes (64): NodeId, tool_arguments(), boot_device(), boot_recovery_diagnose_reports_domain_invariants(), boot_recovery_observe_runs_through_broker(), boots_http_provider_and_status_shows_it(), broker_rolls_back_staged_request_when_health_fails(), broker_runs_staged_commit_through_booted_executor() (+56 more)

### Community 6 - "wifi_driver.rs"
Cohesion: 0.08
Nodes (33): checkpoint_captures_active_module(), driver(), DriverControl, fake_sysfs(), health_check_reflects_link_state(), LinuxDriverControl, live_control(), live_control_plans_mutations_without_executing() (+25 more)

### Community 7 - "sandbox.rs"
Cohesion: 0.06
Nodes (50): Child, Client, Command, Drop, Output, bubblewrap_argv_confines_and_binds_roots(), BubblewrapSandbox, bwrap_available() (+42 more)

### Community 8 - "config.rs"
Cohesion: 0.09
Nodes (35): AiosConfig, api_key_resolved_from_env(), ConfigError, default_ctx(), default_http_timeout_ms(), default_max_tokens(), default_threads(), dirs_home() (+27 more)

### Community 9 - "ResourceId"
Cohesion: 0.10
Nodes (33): tool(), Capability, capability_covers(), Operation, resource_covers(), ResourceId, RiskLevel, HashMap (+25 more)

### Community 10 - "composer.rs"
Cohesion: 0.07
Nodes (34): EvidenceIndex, Coordinator, Option, Result, String, ToolResult, aios_markers(), content_numbers() (+26 more)

### Community 11 - "AuditLog"
Cohesion: 0.10
Nodes (32): File, appends_across_sessions(), audit_log_path(), AuditEntry, AuditError, AuditLog, encode_field(), entries_are_forward_chained() (+24 more)

### Community 12 - "harness.rs"
Cohesion: 0.11
Nodes (35): all_steps_allow_when_capabilities_granted(), build_graph(), capabilities(), DeviceHistory, enforce_stops_campaign_at_first_denial(), harness_principal(), harness_tool_ids(), HarnessPlan (+27 more)

### Community 13 - "hub.rs"
Cohesion: 0.10
Nodes (35): Arc<RecordingClient>, catalog_model_url_points_at_resolve_main(), CatalogModel, default_catalog(), hex(), HttpClient, HubError, model_with_sha256() (+27 more)

### Community 14 - "processes.rs"
Cohesion: 0.10
Nodes (38): cpu_cores(), cpu_sample(), cpu_stats(), CpuStats, diagnose_flags_missing_usage_evidence(), discovers_process_nodes(), exposes_only_read_only_tools(), format_process_row() (+30 more)

### Community 15 - "ActionRecord"
Cohesion: 0.22
Nodes (15): CheckpointId, ActionRecord, file_store_round_trips_records(), FileActionStore, PersistenceError, ActionId, AsRef, CorrelationId (+7 more)

### Community 16 - "files.rs"
Cohesion: 0.07
Nodes (51): err_result(), exec_runs_echo(), ExecSpecialist, guardian_pattern_check(), guardian_patterns_deny_before_spawn(), indirect_rm_still_confined_by_sandbox(), request_with(), NodeId (+43 more)

### Community 17 - "discovery.rs"
Cohesion: 0.13
Nodes (34): device_firmware_attributes_create_nodes_and_edges(), devices_without_firmware_attributes_get_no_firmware_node(), discovered_nodes_go_stale_after_ttl(), DiscoveredService, discovery(), discovery_adds_dependency_edges(), DiscoveryEvent, DiscoveryOptions (+26 more)

### Community 18 - "broker.rs"
Cohesion: 0.16
Nodes (33): Runtime, allows_with_valid_capability(), approval_channel_accepts_only_user_approval(), approval_channel_rejection_and_expiry_are_fail_closed(), approval_is_required_and_plan_hash_is_bound(), approval_request_for(), approval_scope_for(), audit_broken_denies_everything() (+25 more)

### Community 19 - "ProviderId"
Cohesion: 0.12
Nodes (13): Duration, DataPolicy, ModelEntry, ModelProvenance, ModelRegistry, ProviderHealth, ProviderId, ResourceRequirements (+5 more)

### Community 20 - "SysfsDiscovery"
Cohesion: 0.23
Nodes (17): DiscoveryError, filesystem_usage(), parse_diskstat(), parse_meminfo(), parse_pressure(), parse_pressure_and_vmstat_key_their_fields(), parse_vmstat(), Display (+9 more)

### Community 21 - "http.rs"
Cohesion: 0.13
Nodes (27): Agent, auth_header_sent_when_key_present(), backend(), empty_choices_is_error(), function_tool(), function_tool_no_args(), generate_hits_endpoint_and_parses(), health_check_against_live_server() (+19 more)

### Community 22 - "Aios"
Cohesion: 0.06
Nodes (40): Archive README, Current State, Grounding Snapshot: Coordinator Modularization, Surface Harness, and Graphify, Open Work, Relevant Paths, Verification, Current State, Grounding Snapshot: Groundless Surfaces Validated Live, Provider Teardown Fix (+32 more)

### Community 23 - "power.rs"
Cohesion: 0.11
Nodes (31): diagnose_flags_missing_reading_evidence(), discovers_thermal_and_power_sensors(), exposes_only_read_only_tools(), health_counts_reading_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_power_sensor(), is_thermal_sensor() (+23 more)

### Community 24 - "storage.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_capacity_and_backing(), discovers_block_devices_and_filesystems(), exposes_only_read_only_tools(), health_counts_capacity_and_backing_evidence(), instantiates_with_owns_edges_for_each_resource(), is_block_device(), is_filesystem(), node() (+21 more)

### Community 25 - "drivers.rs"
Cohesion: 0.11
Nodes (30): diagnose_flags_missing_driver_attachment(), discovers_unclaimed_hardware_only(), DriversError, DriversHealth, DriversSpecialist, exposes_only_read_only_tools(), gpu_class_devices_are_not_claimed(), hardware_graph() (+22 more)

### Community 26 - "memory.rs"
Cohesion: 0.12
Nodes (29): diagnose_flags_missing_capacity_evidence(), discovers_memory_nodes_and_ecc_sensors(), exposes_only_read_only_tools(), health_counts_capacity_evidence(), instantiates_with_owns_edges_for_each_resource(), is_ecc_sensor(), is_memory_node(), memory_graph() (+21 more)

### Community 27 - "graphics.rs"
Cohesion: 0.11
Nodes (29): diagnose_flags_missing_gpu_state(), discovers_gpu_display_and_session(), exposes_only_read_only_tools(), graphics_graph(), GraphicsError, GraphicsHealth, GraphicsSpecialist, health_counts_state_evidence() (+21 more)

### Community 28 - "network.rs"
Cohesion: 0.12
Nodes (28): diagnose_flags_missing_connectivity(), discovers_wired_and_bluetooth_excludes_wireless(), exposes_only_read_only_tools(), health_counts_link_and_backing_evidence(), instantiates_with_owns_edges_skipping_wifi_owned(), is_bluetooth_controller(), is_wired_interface(), is_wireless() (+20 more)

### Community 29 - "security.rs"
Cohesion: 0.13
Nodes (26): diagnose_flags_missing_verified_evidence(), discovers_security_nodes(), exposes_only_read_only_tools(), health_counts_verified_evidence(), instantiates_with_owns_edges_for_each_resource(), is_security_node(), node(), not_found() (+18 more)

### Community 30 - "web.rs"
Cohesion: 0.12
Nodes (28): Arc, err_result(), fetch_rejects_non_http_scheme(), fetch_requires_capability(), fetch_strips_secret_lines(), fetch_url_returns_content_with_provenance(), Fetcher, is_allowed_url() (+20 more)

### Community 31 - "Coordinator"
Cohesion: 0.09
Nodes (27): AuditLog, BootRecoverySpecialist, Broker, CapabilityToken, DriversSpecialist, GraphicsSpecialist, GraphPhase, MemorySpecialist (+19 more)

### Community 32 - "boot.rs"
Cohesion: 0.12
Nodes (27): boot_graph(), BootRecoveryError, BootRecoveryHealth, BootRecoverySpecialist, diagnose_flags_unhealthy_nodes(), discovers_boot_nodes(), exposes_only_read_only_tools(), health_counts_healthy_nodes() (+19 more)

### Community 33 - "Result"
Cohesion: 0.22
Nodes (9): CheckpointError, HealthError, RollbackError, StageError, MockDriver, ActionId, Result, StagingError (+1 more)

### Community 34 - "packages.rs"
Cohesion: 0.13
Nodes (26): diagnose_flags_missing_signature_evidence(), discovers_package_nodes(), exposes_only_read_only_tools(), health_counts_signature_evidence(), instantiates_with_owns_edges_for_each_resource(), is_package_node(), node(), not_found() (+18 more)

### Community 35 - "sidebar.ts"
Cohesion: 0.08
Nodes (34): computeActiveNodeIds(), EvidenceItem, GRAPH_LAYER_Y, GraphEdge, GraphNode, graphReadout(), healthClass(), INSPECTOR (+26 more)

### Community 36 - "Checkpoint"
Cohesion: 0.21
Nodes (8): Checkpoint, CommitError, MockWifiDriver, ActionId, Arc, AtomicBool, Default, Result

### Community 37 - "GenerationRequest"
Cohesion: 0.11
Nodes (20): AsRef, LlamaBackend, LlamaChatMessage, LlamaModel, LlamaToken, chat_messages(), chat_messages_maps_roles(), loads_and_generates_real_model() (+12 more)

### Community 38 - "evidence.rs"
Cohesion: 0.14
Nodes (27): cross_tool_reference_fails(), empty_results_give_empty_index(), empty_value_never_matches(), evidence_brief(), evidence_brief_quotes_keys_and_tools(), EvidenceEntry, EvidenceIndex, exact_copy_passes() (+19 more)

### Community 39 - "PrincipalId"
Cohesion: 0.06
Nodes (33): Fn, SpecialistCall, BrokerClient, LocalBroker, PolicyBroker, Arc, Box, Default (+25 more)

### Community 40 - ".refresh_catalogue"
Cohesion: 0.15
Nodes (16): CatalogProvider, Coordinator, DiscoveredModel, DiscoveredModel, Option, Result, String, Vec (+8 more)

### Community 41 - "main.ts"
Cohesion: 0.09
Nodes (30): activityDetail(), activityLabel(), adoptSurfaceHtml(), applyGraphActivity(), BackendStatus, closeSurface(), currentWindow, DockEdge (+22 more)

### Community 42 - "GenerativeWidget"
Cohesion: 0.13
Nodes (27): ApprovalItem, app(), render_approval_item(), render_widget(), Element, String, Vec, spawn_message_task() (+19 more)

### Community 43 - "executor.rs"
Cohesion: 0.20
Nodes (25): advance_to(), checkpoint_count(), checkpoint_verification_failure_enters_failed_and_retains_checkpoint(), failed_action_can_be_manually_recovered_from_retained_checkpoint(), fresh(), fresh_fault(), health_check_error_rolls_back(), health_check_failure_triggers_rollback() (+17 more)

### Community 44 - "package.json"
Cohesion: 0.07
Nodes (28): autoprefixer, author, dependencies, @tauri-apps/api, @tauri-apps/cli, description, devDependencies, autoprefixer (+20 more)

### Community 45 - "StagedExecutor"
Cohesion: 0.20
Nodes (12): ActionState, can_transition(), RecoveryOutcome, TransitionError, Arc, Box, Mutex, Option (+4 more)

### Community 46 - "Guardian"
Cohesion: 0.16
Nodes (18): Guardian, guardian_allows_boot_config_with_fallback_image(), guardian_allows_read_only_operations(), guardian_allows_tested_firmware(), guardian_blocks_boot_config_without_fallback(), guardian_blocks_untested_driver_load(), guardian_blocks_untested_firmware(), InvariantCheck (+10 more)

### Community 47 - "Vec"
Cohesion: 0.15
Nodes (16): DataClassification, AgentRole, ConsentRecord, deregister_provider_drops_only_that_provider(), internet_model(), lan_model(), local_model(), ModelCapability (+8 more)

### Community 48 - "planner.rs"
Cohesion: 0.12
Nodes (24): empty_steps_still_parses(), extract_json(), extracts_json_from_prose(), format_plan(), garbage_becomes_freeform(), GeneratedPlan, missing_intent_falls_back(), multiple_calls_parsed_in_order() (+16 more)

### Community 49 - "ToolRequest"
Cohesion: 0.15
Nodes (17): SpecialistHandler, BrokerError, build_request(), denied_result(), error_code_for(), result_envelope(), Display, Formatter (+9 more)

### Community 50 - "AgentError"
Cohesion: 0.21
Nodes (12): AgentError, Planner, Arc, Display, Error, Formatter, From, Result (+4 more)

### Community 51 - "wifi.rs"
Cohesion: 0.17
Nodes (18): diagnose_flags_missing_driver(), discovers_and_instantiates_from_seeded_graph(), exposes_bounded_tools_with_declared_risk(), health_reports_missing_dependencies_as_false(), health_sees_two_hop_driver_and_network_service(), observe_returns_device_state_metrics(), Display, Error (+10 more)

### Community 52 - ".chat_with_tools_outcome"
Cohesion: 0.20
Nodes (17): Coordinator, extract_file_content(), extract_file_path(), extract_project_name(), extract_url(), operation_for_tool(), protocol_tool_result(), quote_value() (+9 more)

### Community 53 - "MockPlanner"
Cohesion: 0.20
Nodes (12): err_result(), MockPlanner, MockVerificationAgent, ok_result(), Capability, Option, Self, String (+4 more)

### Community 54 - "model.rs"
Cohesion: 0.11
Nodes (22): Default, Send, FakeProbe, combine(), ConnectivityProbe, ConnectivityState, FinishReason, GatewayResponse (+14 more)

### Community 55 - "action.rs"
Cohesion: 0.15
Nodes (11): ActionError, ActionStore, CheckpointState, PendingTransition, ResetError, Option, Send, String (+3 more)

### Community 56 - "tauri.conf.json"
Cohesion: 0.10
Nodes (20): assets/128x128@2x.png, assets/128x128.png, assets/32x32.png, app, security, windows, build, beforeBuildCommand (+12 more)

### Community 57 - "NodeType"
Cohesion: 0.22
Nodes (19): process_health(), NodeType, count_health(), counts_health_and_never_hides_stale(), graph_with(), is_healthy(), PanelSnapshot, render() (+11 more)

### Community 58 - "verifier.rs"
Cohesion: 0.19
Nodes (18): format_review(), garbage_becomes_freeform(), loose_review(), parse_review(), parses_approve(), parses_approve_with_conditions(), parses_reject(), review_formats_verdict() (+10 more)

### Community 59 - "Testing Strategy"
Cohesion: 0.38
Nodes (14): Action State Machine, Agent Packages, Architecture Vision, Capability Model, Bespoke Graph Snapshot, Coordinator Modularization Snapshot, Human Interaction, Testing Strategy (+6 more)

### Community 60 - ".boot_with_probe"
Cohesion: 0.18
Nodes (12): AiosConfig, Box, ConfigError, BootError, config_dir_for(), Display, Error, Formatter (+4 more)

### Community 61 - ".new"
Cohesion: 0.22
Nodes (20): assigned_role_runs_on_assigned_provider_and_model(), budget_retry_leaves_other_errors_untouched(), budget_retry_reissues_identical_prompt_with_doubled_tokens(), empty_content_errors_leave_the_provider_healthy(), failing_backend_surfaces_generation_error(), gateway_with(), internet_assignment_offline_fails(), ModelGateway (+12 more)

### Community 62 - "render"
Cohesion: 0.23
Nodes (19): addProvider(), assignRole(), assignRoleGroup(), autosizePrompt(), bindSidebar(), loadModelsForRole(), refreshGraph(), refreshSidebarStatus() (+11 more)

### Community 63 - "Project Grounding Document"
Cohesion: 0.11
Nodes (18): Project Grounding Document, Session Notes Document, ADR-0001: Runs Above Linux, ADR-0002: Rust as Implementation Language, ADR-0003: Fail-Fast No Silent Fallbacks, ADR-0004: Two-Dimensional Authorization, ADR-0005: Freeze Triage, ADR-0006: Model Gateway (+10 more)

### Community 64 - "definitions"
Cohesion: 0.11
Nodes (17): anyOf, definitions, Number, PermissionEntry, Target, Value, description, anyOf (+9 more)

### Community 65 - "MockBackend"
Cohesion: 0.16
Nodes (7): AtomicBool, AtomicU32, Mutex, MockBackend, ProviderTier, ReasoningControl, tier_allows()

### Community 66 - "widgets.rs"
Cohesion: 0.25
Nodes (16): ActionForm(), Chart(), ChartDataPoint, FormField, MetricCard(), ChartDataPoint, Element, FormField (+8 more)

### Community 67 - "Coordinator"
Cohesion: 0.18
Nodes (7): Coordinator, ActionId, PlanHash, Result, String, Uuid, scan_summary()

### Community 68 - "properties"
Cohesion: 0.12
Nodes (17): properties, Identifier, default, description, type, description, oneOf, type (+9 more)

### Community 69 - "src/main.rs"
Cohesion: 0.20
Nodes (15): build_demo_plan(), build_graph(), describe(), kernel_module_request(), main(), register_principals(), register_tools(), Option (+7 more)

### Community 70 - "definitions"
Cohesion: 0.11
Nodes (17): anyOf, definitions, Number, PermissionEntry, Target, Value, description, anyOf (+9 more)

### Community 71 - "stub_provider.rs"
Cohesion: 0.25
Nodes (12): escape_html(), fields_from_body(), main(), openai_response(), respond(), Option, String, Vec (+4 more)

### Community 72 - "String"
Cohesion: 0.19
Nodes (10): DiscoveredModel, ChatOutcome, classification_help(), ProviderCatalogue, providers_text(), String, ToolResult, Vec (+2 more)

### Community 73 - "ModelId"
Cohesion: 0.15
Nodes (14): Into, GatewayError, GenerationError, ModelId, ModelMessage, ModelRole, RegistryError, RoutingError (+6 more)

### Community 74 - "properties"
Cohesion: 0.12
Nodes (17): properties, Identifier, default, description, type, description, oneOf, type (+9 more)

### Community 76 - "permissions"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 77 - "permissions"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 78 - "project.rs"
Cohesion: 0.35
Nodes (10): HashMap, extract_port(), find_template(), ProjectTemplate, Option, String, Vec, scaffold_files() (+2 more)

### Community 79 - "graphify"
Cohesion: 0.18
Nodes (10): command, enabled, type, mcp, graphify, plugin, $schema, /home/shane/.local/bin/graphify-mcp (+2 more)

### Community 80 - "Surface Harness"
Cohesion: 0.20
Nodes (9): Canned conversations, Live run (real system data), Monitoring semantics, Output, Related, Stub run (no network, deterministic), Surface Harness, Usage (+1 more)

### Community 81 - "default.json"
Cohesion: 0.20
Nodes (9): canvas, core:default, core:event:allow-listen, sidebar, description, identifier, permissions, $schema (+1 more)

### Community 82 - "Capability"
Cohesion: 0.22
Nodes (10): description, required, type, Capability, description, required, type, Capability (+2 more)

### Community 83 - "webviews"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 84 - "webviews"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 85 - "progress.rs"
Cohesion: 0.25
Nodes (7): GraphActivity, GraphPhase, ProgressReporter, Send, String, Sync, Vec

### Community 86 - "CapabilityRemote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 87 - "CapabilityRemote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 88 - "graphify"
Cohesion: 0.25
Nodes (7): graphify, How to query it, Keeping it fresh (hooks), Repository Working Notes, Rules, Teaching the graph (work-memory loop), What's there

### Community 89 - "sidebar.rs"
Cohesion: 0.43
Nodes (7): ApprovalQueue(), ChatInput(), ChatMessages(), Element, Scope, Sidebar(), SidebarHeader()

### Community 90 - "resolve_local_model_path"
Cohesion: 0.28
Nodes (6): PathBuf, ProviderConfig, expand_path(), resolve_local_model_path(), Path, harness_direct_broker()

### Community 91 - ".grant_consent"
Cohesion: 0.29
Nodes (4): Coordinator, Option, Result, String

### Community 92 - "ApprovalItem"
Cohesion: 0.33
Nodes (5): ApprovalItem, ApprovalQueue(), Element, String, Vec

### Community 93 - "graphify-refresh.sh"
Cohesion: 0.33
Nodes (5): GRAPHIFY_OPENAI_MODEL, OPENAI_API_KEY, OPENAI_BASE_URL, PATH, graphify-refresh.sh script

### Community 95 - "ui_file_artifact.rs"
Cohesion: 0.53
Nodes (5): app_binary(), file_artifact_via_ui(), Duration, PathBuf, wait_for_port()

### Community 96 - "pickSelectOption"
Cohesion: 0.50
Nodes (5): bindSelectCloser(), closeAllSelects(), loadModelsForBulk(), pickSelectOption(), syncCatalogSelection()

### Community 97 - "Q: Which surface generation path is Aios's intended architecture?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Which surface generation path is Aios's intended architecture?, Source Nodes

### Community 99 - "Display"
Cohesion: 0.83
Nodes (4): Display, Graphics Specialist, GPU, Session

### Community 100 - "Generative Surface Roadmap"
Cohesion: 0.50
Nodes (4): Research Issues Document, Generative Surface Roadmap, M8 UI Repair Plan, Legacy UI Specification

### Community 101 - "canvas.rs"
Cohesion: 0.67
Nodes (3): Canvas(), CanvasHeader(), Element

### Community 102 - "SettingsPanel"
Cohesion: 0.50
Nodes (3): Element, Scope, SettingsPanel()

### Community 104 - "loadProviderCatalog"
Cohesion: 0.67
Nodes (3): loadProviderCatalog(), updateProviderCatalog(), updateRolesCatalog()

### Community 178 - "CompositeDriver"
Cohesion: 0.12
Nodes (14): ActionId, Checkpoint, CheckpointError, CommitError, FileDriver, HealthError, ResetError, ResourceDriver (+6 more)

## Knowledge Gaps
- **242 isolated node(s):** `Latest Snapshot`, `Sidebar Layout Design (2026-08-17)`, `Re-grounding Order`, `Snapshot Maintenance`, `Current State` (+237 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **145 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `ResourceId` connect `ResourceId` to `protocol.rs`, `tests.rs`, `wifi_driver.rs`, `harness.rs`, `processes.rs`, `ActionRecord`, `files.rs`, `broker.rs`, `power.rs`, `storage.rs`, `drivers.rs`, `memory.rs`, `graphics.rs`, `network.rs`, `security.rs`, `web.rs`, `boot.rs`, `Result`, `packages.rs`, `Checkpoint`, `PrincipalId`, `executor.rs`, `ToolRequest`, `.chat_with_tools_outcome`, `MockPlanner`, `Coordinator`, `src/main.rs`, `.fmt`, `resolve_local_model_path`?**
  _High betweenness centrality (0.069) - this node is a cross-community bridge._
- **Why does `SystemGraph` connect `NodeId` to `boot.rs`, `packages.rs`, `Coordinator`, `src/main.rs`, `harness.rs`, `network.rs`, `processes.rs`, `discovery.rs`, `wifi.rs`, `SysfsDiscovery`, `power.rs`, `storage.rs`, `drivers.rs`, `memory.rs`, `graphics.rs`, `.boot_with_probe`, `security.rs`, `NodeType`?**
  _High betweenness centrality (0.050) - this node is a cross-community bridge._
- **Why does `ModelId` connect `ModelId` to `GenerationRequest`, `hub.rs`, `Vec`, `ProviderId`, `http.rs`, `model.rs`, `.new`?**
  _High betweenness centrality (0.038) - this node is a cross-community bridge._
- **Are the 7 inferred relationships involving `ResourceId` (e.g. with `.run_tool_as()` and `exec_runs_echo()`) actually correct?**
  _`ResourceId` has 7 INFERRED edges - model-reasoned connections that need verification._
- **Are the 7 inferred relationships involving `NodeId` (e.g. with `.ensure_control_plane_edges()` and `.ensure_control_plane_nodes()`) actually correct?**
  _`NodeId` has 7 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Latest Snapshot`, `Sidebar Layout Design (2026-08-17)`, `Re-grounding Order` to the rest of the system?**
  _242 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `tools.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06526610644257703 - nodes in this community are weakly interconnected._