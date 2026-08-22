use crate::action::FileActionStore;
use crate::audit::{AuditLog, audit_log_path};
use crate::boot::BootRecoverySpecialist;
use crate::broker::{Broker, BrokerClient};
use crate::capability::{
    Capability, CapabilityToken, Clearance, Operation, PrincipalId, ResourceId, ResourceState,
    ToolDefinition,
};
use crate::config::{AiosConfig, ConfigError, ModelConfig, ProviderConfig};
use crate::drivers::DriversSpecialist;
use crate::executor::StagedExecutor;
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType,
    ProvenanceSource, SystemGraph, TrustLevel,
};
use crate::progress::{GraphActivity, GraphPhase, ProgressSink};
use crate::graphics::GraphicsSpecialist;
use crate::http::HttpBackend;
use crate::local::LocalLlama;
use crate::memory::MemorySpecialist;
use crate::model::{
    AgentRole, ConnectivityProbe, ConnectivityState, ModelEntry, ModelGateway, ModelId,
    ModelMessage, ModelRegistry, ModelRole, ModelTask, ProviderId, RoutingDecision, RoutingError,
};
use crate::network::NetworkSpecialist;
use crate::packages::PackagesSpecialist;
use crate::planner::{
    AgentError, Planner, ToolCallRequest, parse_tool_calls, strip_tool_calls_json,
};
use crate::power::PowerSpecialist;
use crate::processes::ProcessesSpecialist;
use crate::protocol::{DataClassification, HealthState, now};
use crate::security::SecuritySpecialist;
use crate::storage::StorageSpecialist;
use crate::tools::{ToolError, ToolRegistry, model_tool_instructions, resource_index};
use crate::verifier::Verifier;
use crate::wifi::WifiSpecialist;
use crate::wifi_driver::{MockDriverControl, WifiDriverResourceDriver};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, Ordering},
};

static NEXT_TOOL_NONCE: AtomicU64 = AtomicU64::new(1);
use chat::tool_arguments;
use planning::{seed_security_domain, seed_boot_domain};
pub struct Coordinator {
    pub config: AiosConfig,
    pub config_dir: PathBuf,
    pub registry: Arc<RwLock<ModelRegistry>>,
    pub gateway: Arc<ModelGateway>,
    pub connectivity_probe: Box<dyn ConnectivityProbe>,
    pub graph: Arc<RwLock<SystemGraph>>,
    pub planner: Planner,
    pub verifier: Verifier,
    pub audit: Arc<AuditLog>,
    pub tools: ToolRegistry,
    pub broker: Broker,
    pub shell_max_tokens: u32,
    pub compose_max_tokens: u32,
    session_tokens: Vec<CapabilityToken>,
    session_principal: PrincipalId,
    last_scan_summary: RwLock<Option<String>>,
    local_model_path: Option<PathBuf>,
    wifi_specialist: Option<WifiSpecialist>,
    storage_specialist: Option<StorageSpecialist>,
    network_specialist: Option<NetworkSpecialist>,
    drivers_specialist: Option<DriversSpecialist>,
    graphics_specialist: Option<GraphicsSpecialist>,
    power_specialist: Option<PowerSpecialist>,
    memory_specialist: Option<MemorySpecialist>,
    processes_specialist: Option<ProcessesSpecialist>,
    security_specialist: Option<SecuritySpecialist>,
    boot_specialist: Option<BootRecoverySpecialist>,
    packages_specialist: Option<PackagesSpecialist>,
    progress: Option<ProgressSink>,
    /// Last /models discovery result per provider, including the failure
    /// reason when discovery did not succeed. The settings panel reads this
    /// to show per-provider errors in the model dropdowns.
    catalogue: RwLock<HashMap<String, ProviderCatalogue>>,
}

/// One provider's discovered model list (or the reason there is none).
#[derive(Clone)]
pub struct ProviderCatalogue {
    pub models: Vec<DiscoveredModel>,
    pub error: Option<String>,
}

impl ProviderCatalogue {
    fn from_result(result: Result<Vec<DiscoveredModel>, String>) -> Self {
        match result {
            Ok(models) => Self {
                models,
                error: None,
            },
            Err(error) => Self {
                models: Vec::new(),
                error: Some(error),
            },
        }
    }
}

pub struct ChatOutcome {
    pub answer: String,
    pub tool_results: Vec<crate::tools::ToolResult>,
}
impl Coordinator {
    pub fn boot() -> Result<Self, BootError> {
        let config = AiosConfig::load().map_err(BootError::Config)?;
        Self::boot_with(config)
    }

    pub fn boot_with(config: AiosConfig) -> Result<Self, BootError> {
        Self::boot_with_probe(
            config,
            Box::new(crate::model::LinuxConnectivityProbe::default()),
        )
    }

    pub fn boot_with_probe(
        config: AiosConfig,
        probe: Box<dyn ConnectivityProbe>,
    ) -> Result<Self, BootError> {
        let config_dir = config_dir_for(&config);
        let registry = Arc::new(RwLock::new(ModelRegistry::new()));
        let gateway = Arc::new(ModelGateway::new(registry.clone()));

        let mut local_model_path = None;

        for provider in &config.provider {
            let tier = provider.tier().map_err(BootError::Config)?;
            let capabilities = provider.capabilities().map_err(BootError::Config)?;
            let provider_id = ProviderId::new(&provider.id);
            let model_name = provider
                .model
                .clone()
                .unwrap_or_else(|| provider.id.clone());
            let model_id = ModelId::new(&model_name);

            match provider.kind.as_str() {
                "local" => {
                    let path = resolve_local_model_path(provider, &config, &config_dir);
                    match path {
                        Some(path) => {
                            let (n_ctx, n_threads) = model_params(config.model.as_ref());
                            let llama = LocalLlama::load(
                                provider_id.clone(),
                                model_id.clone(),
                                &path,
                                n_ctx,
                                n_threads,
                            )
                            .map_err(|e| BootError::Local {
                                path: path.display().to_string(),
                                reason: e.to_string(),
                            })?;
                            let entry = ModelEntry::new(
                                model_id.clone(),
                                provider_id.clone(),
                                tier,
                                capabilities,
                            );
                            registry
                                .write()
                                .expect("registry lock")
                                .register(entry)
                                .map_err(BootError::Registry)?;
                            gateway.register_backend(Arc::new(llama));
                            local_model_path = Some(path);
                        }
                        None => {
                            let mut entry = ModelEntry::new(
                                model_id.clone(),
                                provider_id.clone(),
                                tier,
                                capabilities,
                            );
                            entry.health.state = HealthState::Unhealthy;
                            registry
                                .write()
                                .expect("registry lock")
                                .register(entry)
                                .map_err(BootError::Registry)?;
                        }
                    }
                }
                "openai-compatible" => {
                    let endpoint = provider.endpoint.clone().ok_or_else(|| {
                        BootError::MissingField("endpoint".into(), provider.id.clone())
                    })?;
                    let api_key = provider.effective_api_key().map_err(BootError::Config)?;
                    let backend = HttpBackend::new(
                        provider_id.clone(),
                        model_name,
                        endpoint,
                        api_key,
                        tier,
                        provider.http_timeout_ms,
                    );
                    let entry = ModelEntry::new(model_id, provider_id.clone(), tier, capabilities);
                    registry
                        .write()
                        .expect("registry lock")
                        .register(entry)
                        .map_err(BootError::Registry)?;
                    gateway.register_backend(Arc::new(backend));
                }
                other => {
                    return Err(BootError::UnknownKind(other.to_string()));
                }
            }
        }

        let max_tokens = config.model.as_ref().map(|m| m.max_tokens).unwrap_or(1024);
        let shell_max_tokens = config
            .shell
            .as_ref()
            .map(|s| s.max_tokens)
            .unwrap_or(max_tokens);
        // The composer must emit a full JSON surface. Reasoning-heavy models
        // spend tokens on an untagged preamble first, so give the composition
        // call its own larger budget and keep the interactive budget as-is.
        // Surface pages are long (full HTML with inline CSS), and reasoning
        // models spend part of the budget thinking before writing; 4096 left
        // such models with nothing to emit.
        let compose_max_tokens = shell_max_tokens.max(8192);

        let audit_path = audit_log_path(&config_dir);

        let mut coordinator = Self {
            config_dir,
            registry,
            gateway: gateway.clone(),
            connectivity_probe: probe,
            graph: Arc::new(RwLock::new(SystemGraph::new())),
            planner: Planner::new(gateway.clone(), shell_max_tokens),
            verifier: Verifier::new(gateway.clone(), shell_max_tokens),
            audit: Arc::new(
                AuditLog::try_new(Some(audit_path))
                    .map_err(|error| BootError::Audit(error.to_string()))?,
            ),
            tools: ToolRegistry::new(),
            broker: Broker::new(),
            config,
            shell_max_tokens,
            compose_max_tokens,
            session_tokens: Vec::new(),
            session_principal: PrincipalId::agent("aios.core", "session"),
            last_scan_summary: RwLock::new(None),
            local_model_path,
            wifi_specialist: None,
            storage_specialist: None,
            network_specialist: None,
            drivers_specialist: None,
            graphics_specialist: None,
            memory_specialist: None,
            processes_specialist: None,
            power_specialist: None,
            security_specialist: None,
            boot_specialist: None,
            packages_specialist: None,
            progress: None,
            catalogue: RwLock::new(HashMap::new()),
        };
        coordinator.configure_read_only_broker();
        coordinator.ensure_control_plane_nodes();
        coordinator.refresh_connectivity();
        // Running Aios is consent to inspect this machine for the session.
        // Provider-level consent still remains explicit for other data classes,
        // and the user can revoke this session's machine-state sharing.
        for provider in &coordinator.config.provider {
            let _ = coordinator
                .gateway
                .router()
                .grant_consent(crate::model::ConsentRecord::new(
                    ProviderId::new(&provider.id),
                    vec![DataClassification::SystemConfig],
                ));
        }
        // Explicit role assignments from [roles] in config. Each is validated
        // against the registry here so a typo fails at boot, not mid-request
        // (ADR-0003: no silent fallback).
        if let Some(roles) = &coordinator.config.roles {
            if let Some(surface) = &roles.surface {
                coordinator.gateway.router().set_assignment(
                    "surface",
                    ProviderId::new(&surface.provider),
                    crate::model::ModelId::new(&surface.model),
                )
                .map_err(BootError::RoleAssignment)?;
            }
            if let Some(chat) = &roles.chat {
                coordinator.gateway.router().set_assignment(
                    "chat",
                    ProviderId::new(&chat.provider),
                    crate::model::ModelId::new(&chat.model),
                )
                .map_err(BootError::RoleAssignment)?;
            }
            if let Some(verification) = &roles.verification {
                coordinator.gateway.router().set_assignment(
                    "verification",
                    ProviderId::new(&verification.provider),
                    crate::model::ModelId::new(&verification.model),
                )
                .map_err(BootError::RoleAssignment)?;
            }
        }
        // Discovery is part of boot so the first model request has machine
        // state available. A discovery failure is retained as context below.
        let scan_result = coordinator.scan();
        if scan_result.starts_with("scan failed:") {
            return Err(BootError::Discovery(scan_result));
        }
        let wifi_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match WifiSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::wifi::WifiError::NoWirelessDevice) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.wifi_specialist = wifi_specialist;
        if let Some(specialist) = coordinator.wifi_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::wifi::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            // Register the specialist's bounded tools in the registry AND
            // spawn handlers so the Planner can route to them through the
            // broker (message-protocol §8.1). Read-only tools operate on the
            // live discovery graph.
            let device = specialist.device.clone();
            let specialist_id = specialist.specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let device = device.clone();
                let specialist_id = specialist_id.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let specialist = WifiSpecialist {
                            device: device.clone(),
                            specialist: specialist_id.clone(),
                        };
                        let graph = graph.read().expect("graph lock");
                        if handler_tool_id.ends_with("observe_device") {
                            specialist.observe(&graph)
                        } else if handler_tool_id.ends_with("diagnose_fault") {
                            specialist.diagnose(&graph)
                        } else {
                            crate::protocol::ToolResult {
                                envelope: crate::protocol::MessageEnvelope::new(
                                    crate::protocol::MessageType::ToolResult,
                                    request.envelope.origin,
                                    request.envelope.correlation_id,
                                    request.envelope.data_classification,
                                ),
                                request_id: request.request_id,
                                status: crate::protocol::ToolStatus::Failed,
                                data: None,
                                error: Some(crate::protocol::ToolError {
                                    code: crate::protocol::ToolErrorCode::OperationNotSupported,
                                    message: format!(
                                        "{handler_tool_id} not supported in read-only mode"
                                    ),
                                    recoverable: false,
                                }),
                                health_impact: None,
                            }
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            // Routing to the specialist requires the wifi device to be known
            // and available in the broker's own resource-state registry
            // (ADR-0005 P1-5 / capability-model §10.2).
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId(specialist.device.0.clone()),
                ResourceState::Available,
            );
            // Grant the session planner the wifi specialist's read-only tools
            // so it can route observe/diagnose through the broker to the owning
            // specialist (message-protocol §8.1, capability-model §3.3 per-resource).
            let wifi_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::wifi::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for wifi_capability in wifi_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, wifi_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Storage specialist (M7): the umbrella owns every discovered block
        // device and mounted filesystem. v0.1 is read-only (docs/modules/
        // storage.md) — observe_storage and diagnose_fault run through the
        // broker against the live graph exactly like the wifi read-only tools.
        let storage_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match StorageSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::storage::StorageError::NoStorageResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.storage_specialist = storage_specialist;
        if let Some(specialist) = coordinator.storage_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::storage::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let storage_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = storage_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_storage") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("storage:domain".into()),
                ResourceState::Available,
            );
            let storage_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::storage::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for storage_capability in storage_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, storage_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Network specialist (M7): the umbrella owns the network domain —
        // wired/LAN interfaces and bluetooth controllers. Wireless interfaces
        // stay owned by the Wi-Fi specialist (one-owner rule, architecture
        // §5); the umbrella never steals a claimed resource. v0.1 is
        // read-only (docs/modules/network.md) — observe_network and
        // diagnose_fault run through the broker against the live graph
        // exactly like the wifi and storage read-only tools.
        let network_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match NetworkSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::network::NetworkError::NoNetworkResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.network_specialist = network_specialist;
        if let Some(specialist) = coordinator.network_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::network::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let network_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = network_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_network") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("network:domain".into()),
                ResourceState::Available,
            );
            let network_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::network::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for network_capability in network_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, network_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Drivers and hardware specialist (M7): a peer of the domain
        // specialists (architecture §5). It owns the generic PCI/USB
        // inventory, firmware state, and loaded kernel modules that no domain
        // specialist owns; resources already claimed by another specialist
        // are skipped (one-owner rule). v0.1 is read-only
        // (docs/modules/drivers.md) — observe_device and diagnose_fault run
        // through the broker against the live graph exactly like the other
        // specialist read-only tools. stage_driver and request_reset belong
        // to the mutation pass (they will reuse the staged executor path).
        let drivers_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match DriversSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::drivers::DriversError::NoHardwareResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.drivers_specialist = drivers_specialist;
        if let Some(specialist) = coordinator.drivers_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::drivers::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let drivers_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = drivers_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_device") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("drivers:domain".into()),
                ResourceState::Available,
            );
            let drivers_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::drivers::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for drivers_capability in drivers_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, drivers_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Graphics specialist (M7): the umbrella for GPU, display, and session
        // resources (docs/modules/graphics.md). v0.1 is read-only and owns
        // only resources no peer has claimed first (one-owner rule). Both
        // observe_graphics and diagnose_fault run through the broker against
        // the live graph; the bounded diagnose reports GFX-001 evidence.
        let graphics_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match GraphicsSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::graphics::GraphicsError::NoGraphicsResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.graphics_specialist = graphics_specialist;
        if let Some(specialist) = coordinator.graphics_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::graphics::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let graphics_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = graphics_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_graphics") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("graphics:domain".into()),
                ResourceState::Available,
            );
            let graphics_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::graphics::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for graphics_capability in graphics_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, graphics_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Memory specialist (M7): the umbrella for the memory domain
        // (docs/modules/memory.md). v0.1 is read-only and owns only resources
        // no peer has claimed first (one-owner rule). Both observe_memory and
        // diagnose_fault run through the broker against the live graph; the
        // bounded diagnose reports MEMORY-001 evidence.
        let memory_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match MemorySpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::memory::MemoryError::NoMemoryResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.memory_specialist = memory_specialist;
        if let Some(specialist) = coordinator.memory_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::memory::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let memory_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = memory_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_memory") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("memory:domain".into()),
                ResourceState::Available,
            );
            let memory_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::memory::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for memory_capability in memory_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, memory_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Power and thermal specialist (M7): the umbrella for the power and
        // thermal domain (docs/modules/power-thermal.md). v0.1 is read-only
        // and owns only resources no peer has claimed first (one-owner rule).
        // Both observe_thermal and diagnose_fault run through the broker
        // against the live graph; the bounded diagnose reports THERMAL-001
        // evidence.
        let power_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match PowerSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::power::PowerError::NoPowerResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.power_specialist = power_specialist;
        if let Some(specialist) = coordinator.power_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::power::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let power_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = power_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_thermal") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("power:domain".into()),
                ResourceState::Available,
            );
            let power_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::power::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for power_capability in power_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, power_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Security and identity specialist (M7): the umbrella for the
        // security domain (docs/modules/security.md). Unlike the hardware
        // umbrellas, its domain is the enforcement plane — the Guardian,
        // capabilities, and policies — which always exists rather than being
        // sysfs-discovered. Boot seeds those nodes in the graph (mirroring
        // how discovery populates hardware nodes) so the specialist can own
        // them. v0.1 is read-only plus quarantine; observe_security and
        // diagnose_fault run through the broker against the live graph, and
        // the bounded diagnose reports SEC-001 evidence. quarantine (risk 4)
        // is deferred to the mutation pass.
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            seed_security_domain(&mut graph);
        }
        let security_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match SecuritySpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::security::SecurityError::NoSecurityResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.security_specialist = security_specialist;
        if let Some(specialist) = coordinator.security_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::security::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let security_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = security_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_security") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("security:domain".into()),
                ResourceState::Available,
            );
            let security_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::security::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for security_capability in security_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, security_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Processes and resources specialist (M7): the umbrella for
        // the process domain (docs/modules/processes.md). v0.1 is
        // read-only and owns only resources no peer has claimed
        // first (one-owner rule). Both observe_process and
        // diagnose_fault run through the broker against the live
        // graph; the bounded diagnose reports PROC-001 evidence.
        let processes_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match ProcessesSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::processes::ProcessesError::NoProcessResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.processes_specialist = processes_specialist;
        if let Some(specialist) = coordinator.processes_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal = PrincipalId::agent(
                crate::processes::PACKAGE_ID,
                specialist.specialist.0.clone(),
            );
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let processes_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = processes_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_process") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("processes:domain".into()),
                ResourceState::Available,
            );
            let processes_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::processes::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for processes_capability in processes_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, processes_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Boot and recovery specialist (M7): the umbrella for the
        // boot and recovery domain (docs/modules/boot-recovery.md).
        // Unlike hardware umbrellas, its domain is the trust plane —
        // boot images, snapshots, and watchdogs — which is seeded by
        // the coordinator rather than sysfs-discovered. v0.1 is
        // read-only; observe_boot and diagnose_fault run through the
        // broker against the live graph; the bounded diagnose reports
        // BOOT-001 evidence. Boot-level mutating operations (A/B
        // image management, watchdogs) are deferred to v0.2+ per
        // ADR-0001.
        {
            let mut graph = coordinator.graph.write().expect("graph lock");
            seed_boot_domain(&mut graph);
        }
        let boot_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match BootRecoverySpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::boot::BootRecoveryError::NoBootResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.boot_specialist = boot_specialist;
        if let Some(specialist) = coordinator.boot_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::boot::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let boot_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = boot_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_boot") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("boot:domain".into()),
                ResourceState::Available,
            );
            let boot_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::boot::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for boot_capability in boot_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, boot_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // Packages and updates specialist (M7): the umbrella for the
        // package domain (docs/modules/packages.md). v0.1 is read-only
        // with bounded Observe and Diagnose tools. Mutating operations
        // (stage_update, request_rollback) are deferred to the mutation
        // pass and will pass through the staged executor and Guardian.
        let packages_specialist = {
            let mut graph = coordinator.graph.write().expect("graph lock");
            match PackagesSpecialist::instantiate(&mut graph) {
                Ok(specialist) => Some(specialist),
                Err(crate::packages::PackagesError::NoPackageResources) => None,
                Err(error) => return Err(BootError::Discovery(error.to_string())),
            }
        };
        coordinator.packages_specialist = packages_specialist;
        if let Some(specialist) = coordinator.packages_specialist.as_ref() {
            let definitions = specialist.tool_definitions();
            let principal =
                PrincipalId::agent(crate::packages::PACKAGE_ID, specialist.specialist.0.clone());
            let capabilities = definitions
                .iter()
                .flat_map(|definition| definition.required_capabilities.clone())
                .collect();
            let packages_domain = specialist.clone();
            for definition in definitions {
                let tool_id = definition.tool_id.clone();
                coordinator.broker.register_tool(definition);
                let graph = coordinator.graph.clone();
                let domain = packages_domain.clone();
                let handler_tool_id = tool_id.clone();
                coordinator.broker.spawn_specialist(&tool_id, {
                    std::sync::Arc::new(move |request| {
                        let graph = graph.read().expect("graph lock");
                        let target = tool_arguments(&request.parameters);
                        if handler_tool_id.ends_with("observe_package") {
                            domain.observe(&graph, &target)
                        } else {
                            domain.diagnose(&graph, &target)
                        }
                    })
                });
            }
            coordinator
                .broker
                .register_principal(principal, capabilities, Clearance::max());
            coordinator.broker.set_resource_state(
                crate::capability::ResourceId("packages:domain".into()),
                ResourceState::Available,
            );
            let packages_capabilities: Vec<Capability> = coordinator
                .broker
                .client(crate::capability::PrincipalId::system("policy-broker"))
                .get_capabilities(&crate::capability::PrincipalId::agent(
                    crate::packages::PACKAGE_ID,
                    specialist.specialist.0.clone(),
                ))
                .into_iter()
                .filter(|c| matches!(c.operation, Operation::Observe | Operation::Diagnose))
                .collect();
            for packages_capability in packages_capabilities {
                coordinator
                    .broker
                    .grant_capability(&coordinator.session_principal, packages_capability);
            }
            coordinator.session_tokens = coordinator
                .broker
                .client(coordinator.session_principal.clone())
                .capability_tokens(&coordinator.session_principal);
        }
        // request_reset) run through the M5 action state machine instead of
        // failing with "no executor configured" (modules/wifi.md, M6). The
        // default control is a mock that records the intended modprobe
        // commands — real kernel changes are executed by the user on the
        // wired-connected machine (safety boundary). Setting
        // AIOS_LIVE_DRIVER_CONTROL switches to the live sysfs control, which
        // health-checks the real interface but still only plans mutations
        // unless execute is opted in on the control itself.
        let action_dir = coordinator.config_dir.join("actions");
        let store = match FileActionStore::new(&action_dir) {
            Ok(store) => store,
            Err(e) => {
                return Err(BootError::Discovery(format!(
                    "failed to init action store: {e:?}"
                )));
            }
        };
        let device_id = coordinator
            .wifi_specialist
            .as_ref()
            .map(|s| ResourceId(s.device.0.clone()))
            .unwrap_or_else(|| ResourceId("device:net-wlp1s0".into()));
        let control: Box<dyn crate::wifi_driver::DriverControl> =
            if std::env::var_os("AIOS_LIVE_DRIVER_CONTROL").is_some() {
                let interface = device_id
                    .0
                    .strip_prefix("device:net-")
                    .unwrap_or("wlan0")
                    .to_string();
                Box::new(crate::wifi_driver::LinuxDriverControl::new(interface))
            } else {
                Box::new(MockDriverControl::new())
            };
        let driver = WifiDriverResourceDriver::new(control, device_id);
        coordinator
            .broker
            .set_executor(StagedExecutor::new(Box::new(store), Box::new(driver)));
        coordinator.ensure_control_plane_edges();
        coordinator.record_audit("coordinator", "boot", "system", "ok");
        Ok(coordinator)
    }

    pub fn refresh_connectivity(&self) -> ConnectivityState {
        let state = self.connectivity_probe.probe();
        self.gateway.set_connectivity(state);
        state
    }

    /// Record an audit entry and fail closed if the audit log cannot be
    /// written (observability.md §1.7): the broker denies all further
    /// actions once the audit log is unavailable.
    pub(crate) fn record_audit(&self, actor: &str, action: &str, target: &str, outcome: &str) {
        if let Err(e) = self.audit.record(actor, action, target, outcome) {
            self.broker
                .core()
                .lock()
                .expect("broker lock")
                .set_audit_broken(true);
            eprintln!("aios: audit log failure — entering read-only mode: {e}");
        }
    }

    pub fn connectivity(&self) -> ConnectivityState {
        self.gateway.router().connectivity()
    }

    pub fn provider_entries(&self) -> Vec<crate::model::ModelEntry> {
        self.registry
            .read()
            .expect("registry lock")
            .iter()
            .cloned()
            .collect()
    }

    pub fn wifi_specialist(&self) -> Option<&WifiSpecialist> {
        self.wifi_specialist.as_ref()
    }

    pub fn storage_specialist(&self) -> Option<&StorageSpecialist> {
        self.storage_specialist.as_ref()
    }

    pub fn network_specialist(&self) -> Option<&NetworkSpecialist> {
        self.network_specialist.as_ref()
    }

    pub fn drivers_specialist(&self) -> Option<&DriversSpecialist> {
        self.drivers_specialist.as_ref()
    }

    pub fn graphics_specialist(&self) -> Option<&GraphicsSpecialist> {
        self.graphics_specialist.as_ref()
    }

    pub fn memory_specialist(&self) -> Option<&MemorySpecialist> {
        self.memory_specialist.as_ref()
    }

    pub fn processes_specialist(&self) -> Option<&ProcessesSpecialist> {
        self.processes_specialist.as_ref()
    }

    pub fn power_specialist(&self) -> Option<&PowerSpecialist> {
        self.power_specialist.as_ref()
    }

    pub fn security_specialist(&self) -> Option<&SecuritySpecialist> {
        self.security_specialist.as_ref()
    }

    pub fn boot_specialist(&self) -> Option<&BootRecoverySpecialist> {
        self.boot_specialist.as_ref()
    }

    pub fn packages_specialist(&self) -> Option<&PackagesSpecialist> {
        self.packages_specialist.as_ref()
    }

    fn persist_config(&self) -> Result<(), String> {
        let path = self.config.source_path();
        self.config
            .save_to(&path)
            .map_err(|e| format!("cannot persist config: {e}"))
    }

    /// Attach a sink that receives real activity events for the sidebar graph.
    pub fn set_progress_reporter(&mut self, sink: ProgressSink) {
        self.broker.set_progress_reporter(sink.clone());
        self.progress = Some(sink);
    }

    /// Emit a real activity event. No-ops until a reporter is attached.
    fn report(&self, phase: GraphPhase, active: &[&str]) {
        if let Some(sink) = &self.progress {
            sink.report(GraphActivity {
                phase,
                active_node_ids: active.iter().map(|s| s.to_string()).collect(),
                timestamp_ms: crate::progress::now_ms(),
            });
        }
    }

    pub fn local_model_path(&self) -> Option<&PathBuf> {
        self.local_model_path.as_ref()
    }
}

fn model_params(config: Option<&ModelConfig>) -> (u32, i32) {
    match config {
        Some(model) => (model.n_ctx, model.n_threads),
        None => (4096, 4),
    }
}

fn resolve_local_model_path(
    provider: &ProviderConfig,
    config: &AiosConfig,
    config_dir: &Path,
) -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = std::iter::empty()
        .chain(provider.model.as_ref().map(PathBuf::from))
        .chain(config.model.as_ref().and_then(|m| m.path.clone()))
        .map(|p| expand_path(&p, config_dir))
        .collect();

    if let Some(path) = candidates.into_iter().find(|p| p.is_file()) {
        return Some(path);
    }

    let models_dir = config_dir.join("models");
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&models_dir)
        .map(|dir| {
            dir.flatten()
                .filter(|entry| entry.path().extension().is_some_and(|e| e == "gguf"))
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default();
    entries.sort();
    entries.first().cloned()
}

fn expand_path(path: &Path, base: &Path) -> PathBuf {
    let as_str = path.to_string_lossy();
    if let Some(rest) = as_str.strip_prefix("~/") {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(rest);
    }
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
fn config_dir_for(_config: &AiosConfig) -> PathBuf {
    let path = std::env::var("AIOS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| AiosConfig::default_path());
    match path.parent() {
        Some(dir) => dir.to_path_buf(),
        None => path.clone(),
    }
}

#[derive(Debug)]
pub enum BootError {
    Config(ConfigError),
    Audit(String),
    Registry(crate::model::RegistryError),
    Local { path: String, reason: String },
    UnknownKind(String),
    MissingField(String, String),
    Discovery(String),
    RoleAssignment(crate::model::RoutingError),
}

impl std::fmt::Display for BootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootError::Config(e) => write!(f, "config: {e}"),
            BootError::Audit(e) => write!(f, "audit: {e}"),
            BootError::Registry(e) => write!(f, "registry: {e}"),
            BootError::Local { path, reason } => {
                write!(f, "cannot load local model from {path}: {reason}")
            }
            BootError::UnknownKind(kind) => write!(f, "unknown provider kind: {kind}"),
            BootError::MissingField(field, provider) => {
                write!(
                    f,
                    "provider '{provider}' is missing required field '{field}'"
                )
            }
            BootError::Discovery(reason) => write!(f, "discovery: {reason}"),
            BootError::RoleAssignment(e) => write!(f, "role assignment: {e}"),
        }
    }
}

impl std::error::Error for BootError {}
pub fn status_text(coordinator: &Coordinator) -> String {
    let connectivity = coordinator.refresh_connectivity();
    let mut lines = vec![format!("connectivity: {connectivity:?}")];

    match coordinator.current_route() {
        Ok(route) => lines.push(format!(
            "route (public): {} ({:?}){}",
            route.provider,
            route.model,
            if route.reduced_confidence {
                " reduced-confidence"
            } else {
                ""
            }
        )),
        Err(RoutingError::NoAssignmentForRole(_)) => {
            lines.push("route (public): no model assigned".into())
        }
        Err(e) => lines.push(format!("route (public): {e}")),
    }

    match coordinator.local_model_path() {
        Some(path) => lines.push(format!("local model: {}", path.display())),
        None => lines.push(
            "local model: not provisioned (put a .gguf in ~/.aios/models/ or set [model] path)"
                .into(),
        ),
    }

    lines.push("providers:".into());
    for entry in coordinator.provider_entries() {
        let latency = entry
            .health
            .latency_ms
            .map(|ms| format!(" {ms}ms"))
            .unwrap_or_default();
        lines.push(format!(
            "  {:<10} {:>9} {:<24} {:?}{}",
            entry.provider,
            format!("{:?}", entry.tier),
            entry.model_id,
            entry.health.state,
            latency,
        ));
    }

    if let Some(summary) = coordinator.graph_summary().strip_prefix("scanned: ") {
        lines.push(format!("graph: {summary}"));
    } else {
        lines.push("graph: no scan yet (use 'scan')".into());
    }
    lines.join("\n")
}

pub fn providers_text(coordinator: &Coordinator) -> String {
    let mut lines = Vec::new();
    for entry in coordinator.provider_entries() {
        let kind = coordinator
            .config
            .provider(&entry.provider.to_string())
            .map(|p| p.kind.as_str())
            .unwrap_or("declared");
        let consent = coordinator
            .gateway
            .router()
            .consent_for(&entry.provider)
            .map(|c| format!(" consent={:?}", c.data_scope))
            .unwrap_or_default();
        lines.push(format!(
            "{} kind={kind} tier={:?} model={} health={:?}{}",
            entry.provider, entry.tier, entry.model_id, entry.health.state, consent
        ));
    }
    if lines.is_empty() {
        "no providers configured".into()
    } else {
        lines.join("\n")
    }
}

pub fn classification_help() -> String {
    "consent <provider> <class> <on|off>  class in: public, personal-memory, system-config, protected"
        .into()
}

pub fn send_direct(coordinator: &Coordinator, text: &str) -> Result<String, AgentError> {
    let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Public)
        .with_role_id("chat");
    let request = crate::model::GenerationRequest {
        task_id: task.task_id,
        messages: vec![
            ModelMessage::new(ModelRole::System, "You are Aios, a helpful assistant."),
            ModelMessage::new(ModelRole::User, text),
        ],
        max_tokens: coordinator.shell_max_tokens,
        temperature: 0.4,
        seed: None,
        model: None,
        // Chat keeps thinking enabled — it earns its tokens here. Only the
        // budget retry adapts, for models that think longer than the
        // configured allowance.
        reasoning_disabled: false,
    };
    let response = crate::model::submit_with_budget_retry(&coordinator.gateway, &task, request)
        .map_err(AgentError::from)?;
    Ok(crate::planner::strip_think(response.response.text.trim()).to_string())
}

mod routing;
mod providers;
mod chat;
mod planning;
mod consent;
mod surface;
#[cfg(test)]
mod tests;

pub use providers::*;
pub use routing::*;

