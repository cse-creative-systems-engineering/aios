use crate::config::{AiosConfig, ConfigError, ModelConfig, ProviderConfig};
use crate::graph::{NodeType, SystemGraph};
use crate::http::HttpBackend;
use crate::local::LocalLlama;
use crate::model::{
    AgentRole, ConnectivityState, ConnectivityProbe, ModelEntry, ModelGateway, ModelId,
    ModelMessage, ModelRegistry, ModelRole, ModelTask, ProviderId, RoutingDecision, RoutingError,
};
use crate::planner::{AgentError, Planner};
use crate::protocol::{DataClassification, HealthState, now};
use crate::verifier::Verifier;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub struct Coordinator {
    pub config: AiosConfig,
    pub config_dir: PathBuf,
    pub registry: Arc<RwLock<ModelRegistry>>,
    pub gateway: Arc<ModelGateway>,
    pub connectivity_probe: Box<dyn ConnectivityProbe>,
    pub graph: Arc<RwLock<SystemGraph>>,
    pub planner: Planner,
    pub verifier: Verifier,
    pub shell_max_tokens: u32,
    last_scan_summary: RwLock<Option<String>>,
    local_model_path: Option<PathBuf>,
}

impl Coordinator {
    pub fn boot() -> Result<Self, BootError> {
        let config = AiosConfig::load().map_err(BootError::Config)?;
        Self::boot_with(config)
    }

    pub fn boot_with(config: AiosConfig) -> Result<Self, BootError> {
        Self::boot_with_probe(config, Box::new(crate::model::LinuxConnectivityProbe::default()))
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
            let model_name = provider.model.clone().unwrap_or_else(|| provider.id.clone());
            let model_id = ModelId::new(&model_name);

            match provider.kind.as_str() {
                "local" => {
                    let path = resolve_local_model_path(provider, &config, &config_dir);
                    match path {
                        Some(path) => {
                            let (n_ctx, n_threads) =
                                model_params(config.model.as_ref());
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
                    let endpoint = provider
                        .endpoint
                        .clone()
                        .ok_or_else(|| BootError::MissingField("endpoint".into(), provider.id.clone()))?;
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

        let max_tokens = config
            .model
            .as_ref()
            .map(|m| m.max_tokens)
            .unwrap_or(1024);
        let shell_max_tokens = config
            .shell
            .as_ref()
            .map(|s| s.max_tokens)
            .unwrap_or(max_tokens);

        let coordinator = Self {
            config_dir,
            registry,
            gateway: gateway.clone(),
            connectivity_probe: probe,
            graph: Arc::new(RwLock::new(SystemGraph::new())),
            planner: Planner::new(gateway.clone(), shell_max_tokens),
            verifier: Verifier::new(gateway.clone(), shell_max_tokens),
            config,
            shell_max_tokens,
            last_scan_summary: RwLock::new(None),
            local_model_path,
        };
        coordinator.refresh_connectivity();
        Ok(coordinator)
    }

    pub fn refresh_connectivity(&self) -> ConnectivityState {
        let state = self.connectivity_probe.probe();
        self.gateway.set_connectivity(state);
        state
    }

    pub fn connectivity(&self) -> ConnectivityState {
        self.gateway.router().connectivity()
    }

    pub fn provider_entries(&self) -> Vec<crate::model::ModelEntry> {
        self.registry.read().expect("registry lock").iter().cloned().collect()
    }

    pub fn current_route(&self) -> Result<RoutingDecision, RoutingError> {
        let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Public);
        self.gateway.router().route(&task, &[])
    }

    pub fn chat(&self, text: &str) -> Result<String, AgentError> {
        self.planner.explain(text, self.local_context())
    }

    pub fn local_context(&self) -> Option<String> {
        let summary = self.last_scan_summary.read().expect("scan lock").clone();
        if summary.is_none() {
            return None;
        }
        match self.current_route() {
            Ok(route) if route.provider == ProviderId::local() => summary,
            _ => None,
        }
    }

    pub fn plan_and_review(
        &self,
        intent: &str,
    ) -> Result<(crate::planner::GeneratedPlan, crate::verifier::ReviewResult), AgentError> {
        let plan = self.planner.plan(intent)?;
        let review = self.verifier.review(&plan)?;
        Ok((plan, review))
    }

    pub fn grant_consent(&self, provider: &str, class: DataClassification) -> Result<(), String> {
        let provider_id = ProviderId::new(provider);
        let record = crate::model::ConsentRecord::new(provider_id.clone(), vec![class]);
        self.gateway
            .router()
            .grant_consent(record)
            .map_err(|e| e.to_string())
    }

    pub fn revoke_consent(&self, provider: &str) {
        self.gateway.router().revoke_consent(&ProviderId::new(provider));
    }

    pub fn consent_for(&self, provider: &str) -> Option<crate::model::ConsentRecord> {
        self.gateway.router().consent_for(&ProviderId::new(provider))
    }

    pub fn scan(&self) -> String {
        let mut graph = crate::discovery::SysfsDiscovery::new().scan().map_err(|e| e.to_string());
        let summary = match graph {
            Ok(ref mut graph) => {
                let _ = crate::discovery::ServiceDiscovery::new()
                    .populate(graph, now())
                    .map_err(|e| e.to_string());
                *self.graph.write().expect("graph lock") = graph.clone();
                let text = scan_summary(graph);
                self.last_scan_summary.write().expect("scan lock").replace(text.clone());
                text
            }
            Err(e) => format!("scan failed: {e}"),
        };
        summary
    }

    pub fn graph_summary(&self) -> String {
        let graph = self.graph.read().expect("graph lock");
        scan_summary(&graph)
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
                write!(f, "provider '{provider}' is missing required field '{field}'")
            }
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
            if route.reduced_confidence { " reduced-confidence" } else { "" }
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

pub fn send_direct(
    coordinator: &Coordinator,
    text: &str,
) -> Result<String, AgentError> {
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
        Coordinator::boot_with_probe(stub_config(port), Box::new(FakeProbe(ConnectivityState::Internet)))
            .expect("boot")
    }

    fn handler(body: &str) -> String {
        if body.contains("steps: ") {
            testutil::openai_response(
                r#"{"verdict":"approve","concerns":[],"tests":["ping"]}"#,
            )
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
    fn plan_and_review_roundtrip() {
        let port = testutil::spawn_json_server(handler);
        let coordinator = stub_coordinator(port);
        let (plan, review) = coordinator.plan_and_review("fix my wifi").expect("plan");
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].risk, "read-only");
        assert_eq!(review.verdict, VerificationVerdict::Approve);
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
        assert!(Coordinator::boot_with_probe(
            config,
            Box::new(FakeProbe(ConnectivityState::Offline))
        )
        .is_err());
    }
}
