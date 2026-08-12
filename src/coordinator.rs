use crate::action::FileActionStore;
use crate::audit::{AuditLog, audit_log_path};
use crate::broker::{Broker, BrokerClient};
use crate::capability::{
    Capability, CapabilityToken, Clearance, Operation, PrincipalId, ResourceId, ResourceState,
    ToolDefinition,
};
use crate::config::{AiosConfig, ConfigError, ModelConfig, ProviderConfig};
use crate::executor::StagedExecutor;
use crate::graph::{NodeType, SystemGraph};
use crate::http::HttpBackend;
use crate::local::LocalLlama;
use crate::wifi_driver::{MockDriverControl, WifiDriverResourceDriver};
use crate::model::{
    AgentRole, ConnectivityProbe, ConnectivityState, ModelEntry, ModelGateway, ModelId,
    ModelMessage, ModelRegistry, ModelRole, ModelTask, ProviderId, RoutingDecision, RoutingError,
};
use crate::planner::{
    AgentError, Planner, ToolCallRequest, parse_tool_calls, strip_tool_calls_json,
};
use crate::protocol::{DataClassification, HealthState, now};
use crate::tools::{ToolError, ToolRegistry, model_tool_instructions, resource_index};
use crate::verifier::Verifier;
use crate::wifi::WifiSpecialist;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, Ordering},
};

static NEXT_TOOL_NONCE: AtomicU64 = AtomicU64::new(1);

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
    session_tokens: Vec<CapabilityToken>,
    session_principal: PrincipalId,
    last_scan_summary: RwLock<Option<String>>,
    local_model_path: Option<PathBuf>,
    wifi_specialist: Option<WifiSpecialist>,
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

        let audit_path = audit_log_path(&config_dir);

        let mut coordinator = Self {
            config_dir,
            registry,
            gateway: gateway.clone(),
            connectivity_probe: probe,
            graph: Arc::new(RwLock::new(SystemGraph::new())),
            planner: Planner::new(gateway.clone(), shell_max_tokens),
            verifier: Verifier::new(gateway.clone(), shell_max_tokens),
            audit: Arc::new(AuditLog::new(Some(audit_path))),
            tools: ToolRegistry::new(),
            broker: Broker::new(),
            config,
            shell_max_tokens,
            session_tokens: Vec::new(),
            session_principal: PrincipalId::agent("aios.core", "session"),
            last_scan_summary: RwLock::new(None),
            local_model_path,
            wifi_specialist: None,
        };
        coordinator.configure_read_only_broker();
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
                        let specialist =
                            WifiSpecialist { device: device.clone(), specialist: specialist_id.clone() };
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
                                    message: format!("{handler_tool_id} not supported in read-only mode"),
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
        // Wire the staged executor so risk-2/risk-4 wifi tools (stage_driver,
        // request_reset) run through the M5 action state machine instead of
        // failing with "no executor configured" (modules/wifi.md, M6). The
        // v0.1 driver control is a mock that records the intended modprobe
        // commands — real kernel changes are executed by the user on the
        // wired-connected machine (safety boundary).
        let action_dir = coordinator.config_dir.join("actions");
        let store = match FileActionStore::new(&action_dir) {
            Ok(store) => store,
            Err(e) => {
                return Err(BootError::Discovery(
                    format!("failed to init action store: {e:?}"),
                ));
            }
        };
        let device_id = coordinator
            .wifi_specialist
            .as_ref()
            .map(|s| ResourceId(s.device.0.clone()))
            .unwrap_or_else(|| ResourceId("device:net-wlp1s0".into()));
        let driver = WifiDriverResourceDriver::new(
            Box::new(MockDriverControl::new()),
            device_id,
        );
        coordinator.broker.set_executor(StagedExecutor::new(
            Box::new(store),
            Box::new(driver),
        ));
        coordinator
            .record_audit("coordinator", "boot", "system", "ok");
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
            self.broker.core().lock().expect("broker lock").set_audit_broken(true);
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

    pub fn current_route(&self) -> Result<RoutingDecision, RoutingError> {
        let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Public);
        self.gateway.router().route(&task, &[])
    }

    pub fn chat(&self, text: &str) -> Result<String, AgentError> {
        let result = self.planner.explain(text, self.local_context());
        match &result {
            Ok(_) => self.record_audit("user", "chat", text, "ok"),
            Err(e) => self.record_audit("user", "chat", text, &format!("error: {e}")),
        }
        result
    }

    pub fn chat_with_tools(&self, messages: Vec<ModelMessage>) -> Result<String, AgentError> {
        const MAX_TOOL_TURNS: usize = 4;
        let mut messages = messages;
        if let Some(system) = messages
            .first_mut()
            .filter(|message| message.role == ModelRole::System)
        {
            system.content.push('\n');
            system.content.push_str(model_tool_instructions());
        }
        if let Some(context) = self.local_context() {
            let system = messages
                .first_mut()
                .ok_or_else(|| AgentError::Format("tool chat requires a system message".into()))?;
            if system.role != ModelRole::System {
                return Err(AgentError::Format(
                    "tool chat requires the first message to be system role".into(),
                ));
            }
            system.content.push_str("\n\nCurrent local system state:\n");
            system.content.push_str(&context);
        }
        let mut answer = self.planner.chat_with(messages.clone(), None)?;

        for turn in 0..MAX_TOOL_TURNS {
            let calls = parse_tool_calls(&answer);
            if calls.is_empty() {
                // Architecture §4: conversational answers are valid without a
                // tool call. Simple queries and greetings do not need to invoke
                // a model tool, and we do not force a tool for every trivial
                // read. If the Planner chose not to call a tool, its plain
                // answer is returned.
                return Ok(strip_tool_calls_json(&answer).trim().to_string());
            }

            messages.push(ModelMessage::new(ModelRole::Assistant, &answer));
            for call in calls {
                let result = self.run_tool_as("planner", &call);
                let content = match result {
                    Ok(result) => format!("tool {} result:\n{}", result.tool, result.text),
                    Err(error) => format!("tool {} error: {}", call.name, error),
                };
                messages.push(ModelMessage::new(ModelRole::User, content));
            }
            if turn + 1 == MAX_TOOL_TURNS {
                self.record_audit("planner", "tool_loop", "chat", "turn cap reached");
                return Err(AgentError::Format(
                    "tool-call turn cap reached before a grounded answer".into(),
                ));
            }
            answer = self.planner.chat_with(messages.clone(), None)?;
        }

        Ok(strip_tool_calls_json(&answer).trim().to_string())
    }

    pub fn local_context(&self) -> Option<String> {
        let summary = self.last_scan_summary.read().expect("scan lock").clone();
        let summary = summary?;
        let task = ModelTask::new(
            AgentRole::SpecialistReadOnly,
            DataClassification::SystemConfig,
        );
        if self.gateway.router().route(&task, &[]).is_err() {
            return None;
        }
        let mut context = summary;
        let graph = self.graph.read().expect("graph lock");
        let index = resource_index(&graph);
        if !index.is_empty() {
            context.push('\n');
            context.push_str(&index);
        }
        Some(context)
    }

    pub fn run_tool(&self, name: &str, args: &str) -> Result<crate::tools::ToolResult, ToolError> {
        let call = ToolCallRequest {
            name: name.to_string(),
            arguments: args.to_string(),
        };
        self.run_tool_as("user", &call)
    }

    fn run_tool_as(
        &self,
        actor: &str,
        call: &ToolCallRequest,
    ) -> Result<crate::tools::ToolResult, ToolError> {
        let operation =
            operation_for_tool(&call.name).ok_or_else(|| ToolError::Unknown(call.name.clone()))?;
        // Specialist tools route to the owning specialist's resource
        // (message-protocol §8.1); generic read-only tools route to the graph.
        let is_wifi_tool = matches!(
            call.name.as_str(),
            "wifi.observe_device" | "wifi.diagnose_fault"
                | "wifi.stage_driver"
                | "wifi.request_reset"
        );
        let resource = if is_wifi_tool {
            let device = self
                .wifi_specialist
                .as_ref()
                .ok_or_else(|| {
                    ToolError::Permission("no wi-fi specialist instantiated".into())
                })?
                .device
                .clone();
            ResourceId(device.0)
        } else {
            ResourceId("system:graph".into())
        };
        let principal = self.session_principal.clone();
        let client = self.broker.client(principal.clone());
        // Static session tokens issued at session start (capability-model §6.3).
        let token = self
            .session_tokens
            .iter()
            .find(|token| {
                token.capability.operation == operation && token.capability.resource == resource
            })
            .cloned()
            .ok_or_else(|| {
                ToolError::Permission(format!(
                    "no session token for {operation:?} on {resource}"
                ))
            })?;
        let parameters = tool_parameters(operation, &call.arguments);
        let mut request = crate::protocol::ToolRequest::new(
            principal,
            resource,
            operation,
            call.name.clone(),
            token,
            parameters,
            uuid::Uuid::new_v4(),
            DataClassification::SystemConfig,
            30,
        );
        request.nonce = NEXT_TOOL_NONCE.fetch_add(1, Ordering::Relaxed);
        let protocol_result = client
            .request_tool(request)
            .map_err(|e| ToolError::Permission(e.to_string()))?;
        let result = protocol_tool_result(&call.name, protocol_result);
        match &result {
            Ok(tool_result) => self.record_audit(
                actor,
                "tool",
                &format!("{} {}", call.name, call.arguments),
                &format!("ok ({} chars)", tool_result.text.len()),
            ),
            Err(e) => self.record_audit(
                actor,
                "tool",
                &format!("{} {}", call.name, call.arguments),
                &format!("error: {e}"),
            ),
        }
        result
    }

    fn configure_read_only_broker(&mut self) {
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
        self.broker.register_principal(
            principal.clone(),
            capabilities,
            Clearance::max(),
        );
        self.session_tokens = self
            .broker
            .client(principal.clone())
            .capability_tokens(&principal);
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

    pub fn grant_consent(&self, provider: &str, class: DataClassification) -> Result<(), String> {
        let provider_id = ProviderId::new(provider);
        let record = crate::model::ConsentRecord::new(provider_id.clone(), vec![class]);
        let result = self
            .gateway
            .router()
            .grant_consent(record)
            .map_err(|e| e.to_string());
        match &result {
            Ok(()) => self.record_audit(
                "user",
                "consent",
                &format!("{provider} {class:?}"),
                "granted",
            ),
            Err(e) => self.record_audit(
                "user",
                "consent",
                &format!("{provider} {class:?}"),
                &format!("error: {e}"),
            ),
        }
        result
    }

    pub fn revoke_consent(&self, provider: &str) {
        self.gateway
            .router()
            .revoke_consent(&ProviderId::new(provider));
        self.record_audit("user", "consent", provider, "revoked");
    }

    pub fn consent_for(&self, provider: &str) -> Option<crate::model::ConsentRecord> {
        self.gateway
            .router()
            .consent_for(&ProviderId::new(provider))
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
                self.record_audit("user", "approval", &format!("{approval_request_id}"), "approved");
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
                if let Err(error) = crate::discovery::ServiceDiscovery::new().populate(graph, now())
                {
                    return format!("scan failed: {error}");
                }
                *self.graph.write().expect("graph lock") = graph.clone();
                let text = scan_summary(graph);
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
        let graph = self.graph.read().expect("graph lock");
        let mut counts = std::collections::BTreeMap::new();
        for node in graph.nodes().values() {
            *counts.entry(format!("{:?}", node.health)).or_insert(0usize) += 1;
        }
        let route = self
            .current_route()
            .map(|route| format!("{} / {:?}", route.provider, route.model))
            .unwrap_or_else(|error| format!("UNAVAILABLE ({error})"));
        let mut lines = vec![
            "system state".to_string(),
            format!("connectivity: {:?}", self.connectivity()),
            format!("route: {route}"),
            format!("graph: {} nodes", graph.nodes().len()),
            "health:".to_string(),
        ];
        for (state, count) in counts {
            lines.push(format!("  {state}: {count}"));
        }
        lines.push(format!("audit entries: {}", self.audit.entries().len()));
        lines.join("\n")
    }

    pub fn local_model_path(&self) -> Option<&PathBuf> {
        self.local_model_path.as_ref()
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

fn operation_for_tool(name: &str) -> Option<Operation> {
    match name {
        "observe" => Some(Operation::Observe),
        "diagnose" => Some(Operation::Diagnose),
        "query" | "deps" | "impact" | "health" => Some(Operation::Query),
        "wifi.observe_device" => Some(Operation::Observe),
        "wifi.diagnose_fault" => Some(Operation::Diagnose),
        "wifi.stage_driver" => Some(Operation::Stage),
        "wifi.request_reset" => Some(Operation::Reset),
        _ => None,
    }
}

fn tool_parameters(operation: Operation, args: &str) -> crate::protocol::ToolParameters {
    match operation {
        Operation::Observe => crate::protocol::ToolParameters::Observe {
            fields: vec![args.into()],
        },
        Operation::Diagnose => crate::protocol::ToolParameters::Diagnose {
            symptom: args.into(),
        },
        Operation::Query => crate::protocol::ToolParameters::Query { query: args.into() },
        Operation::Stage => crate::protocol::ToolParameters::Stage {
            change: serde_json::json!({ "module": args.trim() }),
        },
        Operation::Reset => crate::protocol::ToolParameters::Reset {
            to_known_good: true,
        },
        _ => unreachable!("operation mapping is exhaustive"),
    }
}

fn tool_arguments(parameters: &crate::protocol::ToolParameters) -> String {
    match parameters {
        crate::protocol::ToolParameters::Observe { fields } => fields.join(" "),
        crate::protocol::ToolParameters::Diagnose { symptom } => symptom.clone(),
        crate::protocol::ToolParameters::Query { query } => query.clone(),
        _ => panic!("read-only specialist received a mutating parameter"),
    }
}

fn protocol_tool_result(
    name: &str,
    result: crate::protocol::ToolResult,
) -> Result<crate::tools::ToolResult, ToolError> {
    if result.status != crate::protocol::ToolStatus::Success {
        let message = result
            .error
            .map(|e| e.message)
            .unwrap_or_else(|| "broker denied tool".into());
        let message = message.strip_prefix("denied: ").unwrap_or(&message);
        if let Some(target) = message.strip_prefix("nothing matches: ") {
            return Err(ToolError::NotFound(target.into()));
        }
        return Err(ToolError::Permission(message.into()));
    }
    let text = match result.data {
        Some(crate::protocol::ToolData::QueryResult { data }) => data
            .get("text")
            .and_then(|value| value.as_str())
            .ok_or_else(|| ToolError::Usage(format!("tool {name} returned malformed data")))?
            .to_string(),
        Some(crate::protocol::ToolData::DeviceState { state, metrics }) => {
            let mut parts = vec![format!("state={state:?}")];
            let mut metrics: Vec<(String, String)> = metrics.into_iter().collect();
            metrics.sort();
            for (k, v) in metrics {
                parts.push(format!("{k}={v}"));
            }
            parts.join(" ")
        }
        Some(crate::protocol::ToolData::Diagnosis { findings, confidence }) => format!(
            "confidence={confidence} findings=[{}]",
            findings.join(" | ")
        ),
        _ => {
            return Err(ToolError::Usage(format!(
                "tool {name} returned no result data"
            )));
        }
    };
    Ok(crate::tools::ToolResult {
        tool: Box::leak(name.to_string().into_boxed_str()),
        text,
    })
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
    Registry(crate::model::RegistryError),
    Local { path: String, reason: String },
    UnknownKind(String),
    MissingField(String, String),
    Discovery(String),
}

impl std::fmt::Display for BootError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BootError::Config(e) => write!(f, "config: {e}"),
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
        Err(RoutingError::NoEligibleProvider) => {
            lines.push("route (public): no eligible provider".into())
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
    let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Public);
    let request = crate::model::GenerationRequest {
        task_id: task.task_id,
        messages: vec![
            ModelMessage::new(ModelRole::System, "You are Aios, a helpful assistant."),
            ModelMessage::new(ModelRole::User, text),
        ],
        max_tokens: coordinator.shell_max_tokens,
        temperature: 0.4,
        seed: None,
    };
    let response = coordinator
        .gateway
        .submit_with_fallback(&task, &request)
        .map_err(AgentError::from)?;
    Ok(crate::planner::strip_think(response.response.text.trim()).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderConfig;
    use crate::protocol::{DataClassification, HealthState, VerificationVerdict};
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
        Coordinator::boot_with_probe(
            stub_config(port),
            Box::new(FakeProbe(ConnectivityState::Internet)),
        )
        .expect("boot")
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
        let route = coordinator.current_route();
        assert!(matches!(
            route,
            Err(RoutingError::ProviderUnhealthy(p)) if p == ProviderId::local()
        ));
        let entries = coordinator.provider_entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].health.state, HealthState::Unhealthy);
    }

    #[test]
    fn bad_config_fails_boot() {
        let config = AiosConfig {
            model: None,
            shell: None,
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
}
