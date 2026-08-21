use crate::protocol::{DataClassification, Duration, HealthState, Timestamp, now};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectivityState {
    Offline,
    LanOnly,
    Internet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderTier {
    Local,
    Lan,
    Internet,
}

impl ProviderTier {
    pub fn rank(self) -> u8 {
        match self {
            ProviderTier::Local => 0,
            ProviderTier::Lan => 1,
            ProviderTier::Internet => 2,
        }
    }
}

pub fn tiers_for(state: ConnectivityState) -> &'static [ProviderTier] {
    match state {
        ConnectivityState::Offline => &[ProviderTier::Local],
        ConnectivityState::LanOnly => &[ProviderTier::Local, ProviderTier::Lan],
        ConnectivityState::Internet => &[
            ProviderTier::Local,
            ProviderTier::Lan,
            ProviderTier::Internet,
        ],
    }
}

pub fn combine(has_default_route: bool, has_internet: bool) -> ConnectivityState {
    match (has_default_route, has_internet) {
        (false, _) => ConnectivityState::Offline,
        (true, true) => ConnectivityState::Internet,
        (true, false) => ConnectivityState::LanOnly,
    }
}

pub fn has_default_route(proc_net_route: &str) -> bool {
    proc_net_route.lines().skip(1).any(|line| {
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next(), fields.next()) {
            (Some(_iface), Some(destination), Some(gateway)) => {
                destination == "00000000" && gateway != "00000000"
            }
            _ => false,
        }
    })
}

pub fn has_default_route_v6(proc_net_ipv6_route: &str) -> bool {
    const ZERO: &str = "00000000000000000000000000000000";
    proc_net_ipv6_route.lines().any(|line| {
        let mut fields = line.split_whitespace();
        match (fields.next(), fields.next(), fields.next(), fields.next()) {
            (Some(destination), Some(_prefix), Some(_source), Some(next_hop)) => {
                destination == ZERO && next_hop != ZERO && !line.contains('*')
            }
            _ => false,
        }
    })
}

pub trait ConnectivityProbe: Send + Sync {
    fn probe(&self) -> ConnectivityState;
}

pub struct LinuxConnectivityProbe {
    pub route_file_v4: String,
    pub route_file_v6: String,
    pub probe_url: String,
    pub http_timeout_ms: u64,
}

impl Default for LinuxConnectivityProbe {
    fn default() -> Self {
        Self {
            route_file_v4: "/proc/net/route".into(),
            route_file_v6: "/proc/net/ipv6_route".into(),
            probe_url: "https://connectivitycheck.gstatic.com/generate_204".into(),
            http_timeout_ms: 2000,
        }
    }
}

impl ConnectivityProbe for LinuxConnectivityProbe {
    fn probe(&self) -> ConnectivityState {
        let v4 = std::fs::read_to_string(&self.route_file_v4).unwrap_or_default();
        let v6 = std::fs::read_to_string(&self.route_file_v6).unwrap_or_default();
        let has_route = has_default_route(&v4) || has_default_route_v6(&v6);
        let online = has_route && http_reachable(&self.probe_url, self.http_timeout_ms);
        combine(has_route, online)
    }
}

fn http_reachable(url: &str, timeout_ms: u64) -> bool {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .build();
    agent.get(url).call().is_ok()
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn local() -> Self {
        Self("local".into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModelId(String);

impl ModelId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelCapability {
    TextGeneration,
    ToolUse,
    CodeGeneration,
    Reasoning,
    Multimodal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentRole {
    Planner,
    Verification,
    SurfaceComposition,
    SpecialistReadOnly,
    SpecialistDiagnosis,
}

impl AgentRole {
    pub fn required_capabilities(&self) -> Vec<ModelCapability> {
        match self {
            AgentRole::Planner | AgentRole::Verification => {
                vec![ModelCapability::TextGeneration, ModelCapability::Reasoning]
            }
            AgentRole::SurfaceComposition => vec![ModelCapability::TextGeneration],
            AgentRole::SpecialistDiagnosis => {
                vec![ModelCapability::TextGeneration, ModelCapability::ToolUse]
            }
            AgentRole::SpecialistReadOnly => vec![ModelCapability::TextGeneration],
        }
    }

    /// The default assignment role id for this agent role. Specialist work
    /// overrides this with a per-domain id ("specialist:wifi", ...) via
    /// `ModelTask::with_role_id`.
    pub fn id(&self) -> &'static str {
        match self {
            AgentRole::Planner => "chat",
            AgentRole::Verification => "verification",
            AgentRole::SurfaceComposition => "surface",
            AgentRole::SpecialistReadOnly | AgentRole::SpecialistDiagnosis => "specialist",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModelTask {
    pub task_id: Uuid,
    pub role: AgentRole,
    /// Assignment role id override. When set, the gateway looks up the
    /// assignment under this id instead of the agent role's default.
    pub role_id: Option<String>,
    pub data_classification: DataClassification,
    pub required_capabilities: Vec<ModelCapability>,
    pub safety_critical: bool,
}

impl ModelTask {
    pub fn new(role: AgentRole, data_classification: DataClassification) -> Self {
        Self {
            task_id: Uuid::new_v4(),
            role,
            role_id: None,
            data_classification,
            required_capabilities: role.required_capabilities(),
            safety_critical: false,
        }
    }

    pub fn with_capabilities(
        role: AgentRole,
        data_classification: DataClassification,
        capabilities: Vec<ModelCapability>,
    ) -> Self {
        let mut task = Self::new(role, data_classification);
        task.required_capabilities = capabilities;
        task
    }

    /// Override the assignment role id, e.g. "specialist:wifi".
    pub fn with_role_id(mut self, role_id: impl Into<String>) -> Self {
        self.role_id = Some(role_id.into());
        self
    }

    /// The assignment role id this task resolves against.
    pub fn assignment_role(&self) -> &str {
        self.role_id.as_deref().unwrap_or(self.role.id())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelProvenance {
    pub source: String,
    pub hash: [u8; 32],
    pub signature_verified: bool,
    pub license: String,
    pub training_data_policy: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResourceRequirements {
    pub min_cpu_cores: u32,
    pub min_memory_mb: u32,
    pub min_gpu_memory_mb: Option<u32>,
    pub storage_gb: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProviderHealth {
    pub state: HealthState,
    pub last_checked: Timestamp,
    pub latency_ms: Option<u32>,
    pub error_rate: f64,
    /// Earliest time (unix seconds) at which an unhealthy provider may be
    /// re-probed and returned to the routing pool (model-routing §3.5).
    pub retry_after: Option<Timestamp>,
}

impl ProviderHealth {
    pub fn healthy() -> Self {
        Self {
            state: HealthState::Healthy,
            last_checked: now(),
            latency_ms: None,
            error_rate: 0.0,
            retry_after: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataPolicy {
    pub retains_data: bool,
    pub trains_on_data: bool,
    pub retention_period: Option<Duration>,
    pub policy_version: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelEntry {
    pub model_id: ModelId,
    pub provider: ProviderId,
    pub tier: ProviderTier,
    pub capabilities: Vec<ModelCapability>,
    pub provenance: ModelProvenance,
    pub resource_requirements: ResourceRequirements,
    pub health: ProviderHealth,
    pub data_policy: Option<DataPolicy>,
}

impl ModelEntry {
    pub fn new(
        model_id: ModelId,
        provider: ProviderId,
        tier: ProviderTier,
        capabilities: Vec<ModelCapability>,
    ) -> Self {
        Self {
            model_id,
            provider,
            tier,
            capabilities,
            provenance: ModelProvenance {
                source: "declared".into(),
                hash: [0; 32],
                signature_verified: false,
                license: "unknown".into(),
                training_data_policy: None,
            },
            resource_requirements: ResourceRequirements {
                min_cpu_cores: 1,
                min_memory_mb: 256,
                min_gpu_memory_mb: None,
                storage_gb: 0.0,
            },
            health: ProviderHealth::healthy(),
            data_policy: None,
        }
    }

    pub fn with_provenance(mut self, provenance: ModelProvenance) -> Self {
        self.provenance = provenance;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsentRecord {
    pub consent_id: Uuid,
    pub provider_id: ProviderId,
    pub policy_version: String,
    pub data_scope: Vec<DataClassification>,
    pub granted_at: Timestamp,
    pub revoked_at: Option<Timestamp>,
    pub revocable: bool,
}

impl ConsentRecord {
    pub fn new(provider_id: ProviderId, data_scope: Vec<DataClassification>) -> Self {
        Self {
            consent_id: Uuid::new_v4(),
            provider_id,
            policy_version: "v1".into(),
            data_scope,
            granted_at: now(),
            revoked_at: None,
            revocable: true,
        }
    }

    pub fn is_active_for(&self, classification: DataClassification) -> bool {
        self.revoked_at.is_none() && self.data_scope.contains(&classification)
    }
}

#[derive(Clone, Debug)]
pub enum RegistryError {
    DuplicateModel(ModelId),
    UnknownProvider(ProviderId),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::DuplicateModel(id) => write!(f, "model already registered: {id}"),
            RegistryError::UnknownProvider(provider) => {
                write!(f, "provider not in registry: {provider}")
            }
        }
    }
}

impl std::error::Error for RegistryError {}

#[derive(Default)]
pub struct ModelRegistry {
    entries: Vec<ModelEntry>,
    /// Keyed by (provider, model): the same model id may legitimately exist
    /// on two providers (e.g. surface and chat roles pointing at one
    /// OpenRouter model through separate provider entries).
    index: HashMap<(ProviderId, ModelId), usize>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entry: ModelEntry) -> Result<(), RegistryError> {
        let key = (entry.provider.clone(), entry.model_id.clone());
        if self.index.contains_key(&key) {
            return Err(RegistryError::DuplicateModel(entry.model_id.clone()));
        }
        self.entries.push(entry);
        self.index.insert(key, self.entries.len() - 1);
        Ok(())
    }

    pub fn get(&self, model_id: &ModelId) -> Option<&ModelEntry> {
        self.entries.iter().find(|e| &e.model_id == model_id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ModelEntry> {
        self.entries.iter()
    }

    pub fn models_for_provider(&self, provider: &ProviderId) -> impl Iterator<Item = &ModelEntry> {
        self.entries.iter().filter(move |e| e.provider == *provider)
    }

    pub fn has_provider(&self, provider: &ProviderId) -> bool {
        self.entries.iter().any(|e| e.provider == *provider)
    }

    pub fn set_health(&mut self, provider: &ProviderId, state: HealthState) {
        for entry in &mut self.entries {
            if entry.provider == *provider {
                entry.health.state = state;
                entry.health.last_checked = now();
            }
        }
    }

    pub fn mark_provider_unhealthy(&mut self, provider: &ProviderId) {
        for entry in &mut self.entries {
            if entry.provider == *provider {
                entry.health.state = HealthState::Unhealthy;
                entry.health.last_checked = now();
                entry.health.error_rate += 1.0;
                // Cooldown before the provider may be re-probed (model-routing
                // §3.5: unhealthy provider is re-checked periodically).
                entry.health.retry_after = Some(now() + HEALTH_RETRY_SECONDS);
            }
        }
    }

    pub fn mark_provider_healthy(&mut self, provider: &ProviderId) {
        for entry in &mut self.entries {
            if entry.provider == *provider {
                entry.health.state = HealthState::Healthy;
                entry.health.last_checked = now();
                entry.health.error_rate *= 0.5;
                entry.health.retry_after = None;
            }
        }
    }

    /// Expire the cooldown for a provider so it becomes eligible for re-probe
    /// (used by tests to simulate the cooldown elapsing).
    pub fn expire_cooldown(&mut self, provider: &ProviderId) {
        for entry in &mut self.entries {
            if entry.provider == *provider {
                entry.health.retry_after = Some(now() - 1);
            }
        }
    }

    pub fn record_success(&mut self, provider: &ProviderId, latency_ms: u64) {
        for entry in &mut self.entries {
            if entry.provider == *provider {
                entry.health.state = HealthState::Healthy;
                entry.health.last_checked = now();
                entry.health.latency_ms = Some(latency_ms.min(u32::MAX as u64) as u32);
                entry.health.error_rate *= 0.5;
                entry.health.retry_after = None;
            }
        }
    }
}

/// Cooldown (seconds) before an unhealthy provider may be re-probed
/// (model-routing §3.5).
const HEALTH_RETRY_SECONDS: u64 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingDecision {
    pub provider: ProviderId,
    pub model: ModelId,
    pub connectivity_state: ConnectivityState,
    pub data_classification: DataClassification,
    pub reduced_confidence: bool,
}

#[derive(Debug)]
pub enum RoutingError {
    NoAssignmentForRole(String),
    ProviderUnhealthy(ProviderId),
    NoConsent(DataClassification),
    UnknownAssignment(ProviderId, ModelId),
}

impl std::fmt::Display for RoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoutingError::NoAssignmentForRole(role) => write!(
                f,
                "no model assigned for role '{role}'. Open Settings and assign a provider and model."
            ),
            RoutingError::ProviderUnhealthy(p) => write!(f, "provider unhealthy: {p}"),
            RoutingError::NoConsent(c) => write!(f, "no consent for data class: {c:?}"),
            RoutingError::UnknownAssignment(p, m) => {
                write!(f, "role assignment names unknown model {m} on provider {p}")
            }
        }
    }
}

impl std::error::Error for RoutingError {}

pub struct ModelRouter {
    registry: Arc<RwLock<ModelRegistry>>,
    consent: Arc<RwLock<HashMap<ProviderId, ConsentRecord>>>,
    connectivity: Arc<RwLock<ConnectivityState>>,
    /// Explicit per-role assignments set from the settings panel. There is
    /// no fallback routing: an unassigned role fails loudly at submit time.
    /// Keys are role ids ("chat", "verification", "surface",
    /// "specialist:wifi", ...).
    assignments: Arc<RwLock<HashMap<String, (ProviderId, ModelId)>>>,
}

impl ModelRouter {
    pub fn new(registry: Arc<RwLock<ModelRegistry>>) -> Self {
        Self {
            registry,
            consent: Arc::new(RwLock::new(HashMap::new())),
            connectivity: Arc::new(RwLock::new(ConnectivityState::Offline)),
            assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store the assignment for one role. Pair validation against the live
    /// discovery catalogue happens in the coordinator before this is called;
    /// the router is the source of truth for what is assigned, not what
    /// exists.
    pub fn set_assignment(
        &self,
        role: &str,
        provider: ProviderId,
        model: ModelId,
    ) -> Result<(), RoutingError> {
        self.assignments
            .write()
            .expect("assignments lock")
            .insert(role.to_string(), (provider, model));
        Ok(())
    }

    pub fn assignment(&self, role: &str) -> Option<(ProviderId, ModelId)> {
        self.assignments
            .read()
            .expect("assignments lock")
            .get(role)
            .cloned()
    }

    pub fn clear_assignment(&self, role: &str) {
        self.assignments
            .write()
            .expect("assignments lock")
            .remove(role);
    }

    pub fn connectivity_handle(&self) -> Arc<RwLock<ConnectivityState>> {
        self.connectivity.clone()
    }

    pub fn set_connectivity(&self, state: ConnectivityState) {
        *self.connectivity.write().expect("connectivity lock") = state;
    }

    pub fn connectivity(&self) -> ConnectivityState {
        *self.connectivity.read().expect("connectivity lock")
    }

    pub fn grant_consent(&self, record: ConsentRecord) -> Result<(), RegistryError> {
        let registry = self.registry.read().expect("registry lock");
        if !registry.has_provider(&record.provider_id) {
            return Err(RegistryError::UnknownProvider(record.provider_id.clone()));
        }
        drop(registry);
        self.consent
            .write()
            .expect("consent lock")
            .insert(record.provider_id.clone(), record);
        Ok(())
    }

    pub fn revoke_consent(&self, provider: &ProviderId) {
        if let Some(record) = self.consent.write().expect("consent lock").get_mut(provider) {
            record.revoked_at = Some(now());
        }
    }

    pub fn consent_for(&self, provider: &ProviderId) -> Option<ConsentRecord> {
        self.consent
            .read()
            .expect("consent lock")
            .get(provider)
            .cloned()
    }
}

fn tier_allows(tier: ProviderTier, classification: DataClassification, consent_ok: bool) -> bool {
    match tier {
        ProviderTier::Local => true,
        ProviderTier::Lan => matches!(classification, DataClassification::Public) || consent_ok,
        ProviderTier::Internet => match classification {
            DataClassification::Public => true,
            DataClassification::Protected => false,
            _ => consent_ok,
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelRole {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelMessage {
    pub role: ModelRole,
    pub content: String,
}

impl ModelMessage {
    pub fn new(role: ModelRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GenerationRequest {
    pub task_id: Uuid,
    pub messages: Vec<ModelMessage>,
    pub max_tokens: u32,
    pub temperature: f32,
    pub seed: Option<u64>,
    /// The model the request should run on. Set by the gateway from the
    /// role assignment; backends fall back to their registered default when
    /// absent.
    pub model: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishReason {
    Stop,
    Length,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenerationResponse {
    pub text: String,
    pub tokens_used: u32,
    pub finish_reason: FinishReason,
    pub latency_ms: u64,
}

#[derive(Debug)]
pub struct GenerationError {
    pub message: String,
    pub recoverable: bool,
}

impl GenerationError {
    pub fn new(message: impl Into<String>, recoverable: bool) -> Self {
        Self {
            message: message.into(),
            recoverable,
        }
    }
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for GenerationError {}

pub trait ModelBackend: Send + Sync {
    fn provider_id(&self) -> &ProviderId;
    fn tier(&self) -> ProviderTier;
    fn is_healthy(&self) -> bool;
    fn generate(&self, request: &GenerationRequest) -> Result<GenerationResponse, GenerationError>;
}

#[derive(Debug)]
pub enum GatewayError {
    Routing(RoutingError),
    ProviderUnavailable(ProviderId),
    ProviderOffline(ProviderId),
    Generation {
        provider: ProviderId,
        message: String,
    },
}

impl GatewayError {
    pub fn provider(&self) -> Option<&ProviderId> {
        match self {
            GatewayError::Routing(_) => None,
            GatewayError::ProviderUnavailable(p) | GatewayError::ProviderOffline(p) => Some(p),
            GatewayError::Generation { provider, .. } => Some(provider),
        }
    }
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatewayError::Routing(e) => write!(f, "{e}"),
            GatewayError::ProviderUnavailable(p) => write!(f, "provider unavailable: {p}"),
            GatewayError::ProviderOffline(p) => write!(
                f,
                "provider {p} needs internet but the system is offline"
            ),
            GatewayError::Generation { provider, message } => {
                write!(f, "generation failed on {provider}: {message}")
            }
        }
    }
}

impl std::error::Error for GatewayError {}

#[derive(Clone, Debug)]
pub struct GatewayResponse {
    pub decision: RoutingDecision,
    pub response: GenerationResponse,
}

pub struct ModelGateway {
    registry: Arc<RwLock<ModelRegistry>>,
    router: ModelRouter,
    backends: RwLock<HashMap<ProviderId, Arc<dyn ModelBackend>>>,
}

impl ModelGateway {
    pub fn new(registry: Arc<RwLock<ModelRegistry>>) -> Self {
        let router = ModelRouter::new(registry.clone());
        Self {
            registry,
            router,
            backends: RwLock::new(HashMap::new()),
        }
    }

    pub fn router(&self) -> &ModelRouter {
        &self.router
    }

    pub fn set_connectivity(&self, state: ConnectivityState) {
        self.router.set_connectivity(state);
    }

    pub fn register_backend(&self, backend: Arc<dyn ModelBackend>) {
        let provider = backend.provider_id().clone();
        self.backends
            .write()
            .expect("backends lock")
            .insert(provider.clone(), backend);
        self.registry
            .write()
            .expect("registry lock")
            .mark_provider_healthy(&provider);
    }

    /// Run one request on the role's assigned provider/model. There is no
    /// fallback routing: an unassigned role fails loudly, and a failing
    /// assignment surfaces its error to the caller.
    pub fn submit(
        &self,
        task: &ModelTask,
        request: &GenerationRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        let (provider, model) = self
            .router
            .assignment(task.assignment_role())
            .ok_or_else(|| {
                GatewayError::Routing(RoutingError::NoAssignmentForRole(
                    task.assignment_role().to_string(),
                ))
            })?;

        let backend = self
            .backends
            .read()
            .expect("backends lock")
            .get(&provider)
            .cloned()
            .ok_or_else(|| GatewayError::ProviderUnavailable(provider.clone()))?;

        if !backend.is_healthy() {
            self.registry
                .write()
                .expect("registry lock")
                .mark_provider_unhealthy(&provider);
            return Err(GatewayError::ProviderUnavailable(provider.clone()));
        }

        // Policy gates that used to live in tier ranking: an internet
        // provider is unusable offline, and consent still applies to the
        // assigned provider's tier.
        let tier = backend.tier();
        if tier == ProviderTier::Internet
            && self.router.connectivity() == ConnectivityState::Offline
        {
            return Err(GatewayError::ProviderOffline(provider.clone()));
        }
        let classification = task.data_classification;
        let consent_ok = self
            .router
            .consent_for(&provider)
            .map(|c| c.is_active_for(classification))
            .unwrap_or(false);
        if !tier_allows(tier, classification, consent_ok) {
            return Err(GatewayError::Routing(RoutingError::NoConsent(
                classification,
            )));
        }

        let mut request = request.clone();
        request.model = Some(model.to_string());

        let started = Instant::now();
        let mut result = backend.generate(&request);
        // Recoverable failures (upstream provider blips, transport drops) get
        // one immediate retry: they are usually transient and a single bad
        // response should not fail the task or cool the provider down.
        if matches!(&result, Err(error) if error.recoverable) {
            std::thread::sleep(std::time::Duration::from_millis(250));
            result = backend.generate(&request);
        }
        let latency_ms = started.elapsed().as_millis() as u64;

        let response = match result {
            Ok(response) => {
                self.registry
                    .write()
                    .expect("registry lock")
                    .record_success(&provider, latency_ms);
                response
            }
            Err(error) => {
                if error.recoverable {
                    self.registry
                        .write()
                        .expect("registry lock")
                        .mark_provider_unhealthy(&provider);
                }
                return Err(GatewayError::Generation {
                    provider: provider.clone(),
                    message: error.message,
                });
            }
        };

        let decision = RoutingDecision {
            provider,
            model,
            connectivity_state: self.router.connectivity(),
            data_classification: task.data_classification,
            reduced_confidence: self.router.connectivity() == ConnectivityState::Offline,
        };
        Ok(GatewayResponse { decision, response })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_model(id: &str, capabilities: &[ModelCapability]) -> ModelEntry {
        ModelEntry::new(
            ModelId::new(id),
            ProviderId::local(),
            ProviderTier::Local,
            capabilities.to_vec(),
        )
    }

    fn lan_model(id: &str, capabilities: &[ModelCapability]) -> ModelEntry {
        ModelEntry::new(
            ModelId::new(id),
            ProviderId::new("lan-gpu-01"),
            ProviderTier::Lan,
            capabilities.to_vec(),
        )
    }

    fn internet_model(id: &str, capabilities: &[ModelCapability]) -> ModelEntry {
        ModelEntry::new(
            ModelId::new(id),
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            capabilities.to_vec(),
        )
    }

    fn text_only() -> Vec<ModelCapability> {
        vec![ModelCapability::TextGeneration]
    }

    fn reasoning() -> Vec<ModelCapability> {
        vec![ModelCapability::TextGeneration, ModelCapability::Reasoning]
    }

    fn tool_use() -> Vec<ModelCapability> {
        vec![ModelCapability::TextGeneration, ModelCapability::ToolUse]
    }

    fn registry_with(entries: Vec<ModelEntry>) -> Arc<RwLock<ModelRegistry>> {
        let mut registry = ModelRegistry::new();
        for entry in entries {
            registry.register(entry).expect("register");
        }
        Arc::new(RwLock::new(registry))
    }

    fn public_task() -> ModelTask {
        ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Public)
    }

    #[test]
    fn default_route_v4_parsed() {
        let sample = "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n\
                      wlan0\t00000000\t0102A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n";
        assert!(has_default_route(sample));
        let no_route = "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n\
                        wlan0\t0102A8C0\t00000000\t0001\t0\t0\t0\t00FFFFFF\t0\t0\t0\n";
        assert!(!has_default_route(no_route));
        assert!(!has_default_route(""));
    }

    #[test]
    fn default_route_v6_parsed() {
        let sample = "00000000000000000000000000000000 00 00000000000000000000000000000000 00 \
                      fe800000000000000000000000000000 00 00000000 00000002 00000000 00 wlan0\n";
        assert!(has_default_route_v6(sample));
        assert!(!has_default_route_v6(""));
    }

    #[test]
    fn combine_states_table() {
        assert_eq!(combine(false, false), ConnectivityState::Offline);
        assert_eq!(combine(false, true), ConnectivityState::Offline);
        assert_eq!(combine(true, false), ConnectivityState::LanOnly);
        assert_eq!(combine(true, true), ConnectivityState::Internet);
    }

    #[test]
    fn registry_rejects_duplicate_model() {
        let mut registry = ModelRegistry::new();
        registry
            .register(local_model("qwen-local", &text_only()))
            .expect("first");
        assert!(matches!(
            registry.register(local_model("qwen-local", &text_only())),
            Err(RegistryError::DuplicateModel(_))
        ));
    }

    #[test]
    fn same_model_on_two_providers_registers() {
        // The surface and chat roles may point at one OpenRouter model
        // through separate provider entries; the registry must key on the
        // (provider, model) pair, not the model id alone.
        let mut surface = internet_model("openrouter/stealth/ox-alpha", &reasoning());
        surface.provider = ProviderId::new("surface-openrouter-openai");
        let mut chat = internet_model("openrouter/stealth/ox-alpha", &reasoning());
        chat.provider = ProviderId::new("chat-openrouter");
        let mut registry = ModelRegistry::new();
        registry.register(surface).expect("surface entry");
        registry.register(chat).expect("chat entry");
        assert_eq!(registry.len(), 2);
    }

    struct MockBackend {
        provider: ProviderId,
        tier: ProviderTier,
        healthy: std::sync::atomic::AtomicBool,
        fail: bool,
        /// Number of further calls that fail before succeeding (transient
        /// failure simulation for the gateway retry path).
        remaining_failures: std::sync::atomic::AtomicU32,
        label: &'static str,
        last_model: std::sync::Mutex<Option<String>>,
    }

    impl MockBackend {
        fn ok(provider: ProviderId, tier: ProviderTier, label: &'static str) -> Arc<Self> {
            Arc::new(Self {
                provider,
                tier,
                healthy: std::sync::atomic::AtomicBool::new(true),
                fail: false,
                remaining_failures: std::sync::atomic::AtomicU32::new(0),
                label,
                last_model: std::sync::Mutex::new(None),
            })
        }

        fn failing(provider: ProviderId, tier: ProviderTier, label: &'static str) -> Arc<Self> {
            Arc::new(Self {
                provider,
                tier,
                healthy: std::sync::atomic::AtomicBool::new(true),
                fail: true,
                remaining_failures: std::sync::atomic::AtomicU32::new(0),
                label,
                last_model: std::sync::Mutex::new(None),
            })
        }

        fn flaky(
            provider: ProviderId,
            tier: ProviderTier,
            label: &'static str,
            failures: u32,
        ) -> Arc<Self> {
            Arc::new(Self {
                provider,
                tier,
                healthy: std::sync::atomic::AtomicBool::new(true),
                fail: false,
                remaining_failures: std::sync::atomic::AtomicU32::new(failures),
                label,
                last_model: std::sync::Mutex::new(None),
            })
        }

        fn sent_model(&self) -> Option<String> {
            self.last_model
                .lock()
                .expect("model lock")
                .clone()
        }
    }

    impl ModelBackend for MockBackend {
        fn provider_id(&self) -> &ProviderId {
            &self.provider
        }

        fn tier(&self) -> ProviderTier {
            self.tier
        }

        fn is_healthy(&self) -> bool {
            self.healthy.load(std::sync::atomic::Ordering::Relaxed)
        }

        fn generate(
            &self,
            request: &GenerationRequest,
        ) -> Result<GenerationResponse, GenerationError> {
            *self.last_model.lock().expect("model lock") = request.model.clone();
            let outstanding = self
                .remaining_failures
                .load(std::sync::atomic::Ordering::SeqCst);
            if self.fail || outstanding > 0 {
                self.remaining_failures
                    .store(outstanding.saturating_sub(1), std::sync::atomic::Ordering::SeqCst);
                return Err(GenerationError::new("boom", true));
            }
            Ok(GenerationResponse {
                text: format!("answer from {}", self.label),
                tokens_used: 10,
                finish_reason: FinishReason::Stop,
                latency_ms: 1,
            })
        }
    }

    fn request(task: &ModelTask) -> GenerationRequest {
        GenerationRequest {
            task_id: task.task_id,
            messages: vec![ModelMessage::new(ModelRole::User, "hello")],
            max_tokens: 32,
            temperature: 0.7,
            seed: None,
            model: None,
        }
    }

    fn gateway_with(backends: Vec<Arc<MockBackend>>) -> ModelGateway {
        let registry = registry_with(vec![]);
        let gateway = ModelGateway::new(registry.clone());
        for backend in &backends {
            // Consent grants require the provider to exist in the registry,
            // mirroring how boot seeds one entry per provider.
            let entry = ModelEntry::new(
                ModelId::new(backend.label),
                backend.provider.clone(),
                backend.tier,
                vec![ModelCapability::TextGeneration, ModelCapability::Reasoning],
            );
            registry
                .write()
                .expect("registry lock")
                .register(entry)
                .expect("seed entry");
            gateway.register_backend(backend.clone());
        }
        gateway.set_connectivity(ConnectivityState::Internet);
        gateway
    }

    #[test]
    fn unassigned_role_fails_loudly() {
        let gateway = gateway_with(vec![MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        )]);
        let task = public_task();
        assert!(matches!(
            gateway.submit(&task, &request(&task)),
            Err(GatewayError::Routing(RoutingError::NoAssignmentForRole(role)))
              if role == "specialist"
        ));
    }

    #[test]
    fn assigned_role_runs_on_assigned_provider_and_model() {
        let openrouter = MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        );
        let gateway = gateway_with(vec![openrouter]);
        gateway
            .router()
            .set_assignment(
                "chat",
                ProviderId::new("openrouter"),
                ModelId::new("openai/gpt-5.6-luna-pro"),
            )
            .expect("assignment");

        let task = ModelTask::new(AgentRole::Planner, DataClassification::Public);
        let response = gateway.submit(&task, &request(&task)).expect("submit");
        assert_eq!(response.decision.provider, ProviderId::new("openrouter"));
        assert_eq!(
            response.decision.model,
            ModelId::new("openai/gpt-5.6-luna-pro")
        );
        assert_eq!(response.response.text, "answer from openrouter");
    }

    #[test]
    fn request_carries_assigned_model_to_backend() {
        let openrouter = MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        );
        let gateway = gateway_with(vec![openrouter.clone()]);
        gateway
            .router()
            .set_assignment(
                "surface",
                ProviderId::new("openrouter"),
                ModelId::new("openrouter/stealth/ox-alpha"),
            )
            .expect("assignment");

        let task = ModelTask::new(AgentRole::SurfaceComposition, DataClassification::Public);
        gateway.submit(&task, &request(&task)).expect("submit");
        assert_eq!(
            openrouter.sent_model().as_deref(),
            Some("openrouter/stealth/ox-alpha")
        );
    }

    #[test]
    fn role_id_override_selects_specialist_assignment() {
        let wifi = MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "wifi-backend",
        );
        let gateway = gateway_with(vec![wifi]);
        gateway
            .router()
            .set_assignment(
                "specialist:wifi",
                ProviderId::new("openrouter"),
                ModelId::new("qwen/wifi-72b"),
            )
            .expect("assignment");

        let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Public)
            .with_role_id("specialist:wifi");
        let response = gateway.submit(&task, &request(&task)).expect("submit");
        assert_eq!(response.decision.model, ModelId::new("qwen/wifi-72b"));
    }

    #[test]
    fn internet_assignment_offline_fails() {
        let openrouter = MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        );
        let gateway = gateway_with(vec![openrouter]);
        gateway.set_connectivity(ConnectivityState::Offline);
        gateway
            .router()
            .set_assignment(
                "chat",
                ProviderId::new("openrouter"),
                ModelId::new("net-a"),
            )
            .expect("assignment");
        let task = ModelTask::new(AgentRole::Planner, DataClassification::Public);
        assert!(matches!(
            gateway.submit(&task, &request(&task)),
            Err(GatewayError::ProviderOffline(_))
        ));
    }

    #[test]
    fn personal_memory_needs_consent_for_assigned_provider() {
        let openrouter = MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        );
        let gateway = gateway_with(vec![openrouter]);
        gateway
            .router()
            .set_assignment(
                "chat",
                ProviderId::new("openrouter"),
                ModelId::new("net-a"),
            )
            .expect("assignment");
        let task = ModelTask::new(AgentRole::Planner, DataClassification::PersonalMemory);

        assert!(matches!(
            gateway.submit(&task, &request(&task)),
            Err(GatewayError::Routing(RoutingError::NoConsent(
                DataClassification::PersonalMemory
            )))
        ));

        gateway
            .router()
            .grant_consent(ConsentRecord::new(
                ProviderId::new("openrouter"),
                vec![DataClassification::PersonalMemory],
            ))
            .expect("grant");
        assert!(gateway.submit(&task, &request(&task)).is_ok());
    }

    #[test]
    fn revoked_consent_blocks_submission() {
        let openrouter = MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        );
        let gateway = gateway_with(vec![openrouter]);
        let provider = ProviderId::new("openrouter");
        gateway
            .router()
            .set_assignment("chat", provider.clone(), ModelId::new("net-a"))
            .expect("assignment");
        gateway
            .router()
            .grant_consent(ConsentRecord::new(
                provider.clone(),
                vec![DataClassification::PersonalMemory],
            ))
            .expect("grant");
        gateway.router().revoke_consent(&provider);

        let task = ModelTask::new(AgentRole::Planner, DataClassification::PersonalMemory);
        assert!(matches!(
            gateway.submit(&task, &request(&task)),
            Err(GatewayError::Routing(RoutingError::NoConsent(_)))
        ));
    }

    #[test]
    fn unhealthy_backend_reports_unavailable() {
        let openrouter = MockBackend::ok(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        );
        openrouter
            .healthy
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let gateway = gateway_with(vec![openrouter]);
        gateway
            .router()
            .set_assignment(
                "chat",
                ProviderId::new("openrouter"),
                ModelId::new("net-a"),
            )
            .expect("assignment");
        let task = ModelTask::new(AgentRole::Planner, DataClassification::Public);
        assert!(matches!(
            gateway.submit(&task, &request(&task)),
            Err(GatewayError::ProviderUnavailable(_))
        ));
    }

    #[test]
    fn failing_backend_surfaces_generation_error() {
        let openrouter = MockBackend::failing(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
        );
        let gateway = gateway_with(vec![openrouter]);
        gateway
            .router()
            .set_assignment(
                "chat",
                ProviderId::new("openrouter"),
                ModelId::new("net-a"),
            )
            .expect("assignment");
        let task = ModelTask::new(AgentRole::Planner, DataClassification::Public);
        assert!(matches!(
            gateway.submit(&task, &request(&task)),
            Err(GatewayError::Generation { .. })
        ));
    }

    #[test]
    fn recoverable_generation_errors_are_retried_once() {
        let openrouter = MockBackend::flaky(
            ProviderId::new("openrouter"),
            ProviderTier::Internet,
            "openrouter",
            1,
        );
        let gateway = gateway_with(vec![openrouter.clone()]);
        gateway
            .router()
            .set_assignment(
                "chat",
                ProviderId::new("openrouter"),
                ModelId::new("net-a"),
            )
            .expect("assignment");
        let task = ModelTask::new(AgentRole::Planner, DataClassification::Public);
        let response = gateway
            .submit(&task, &request(&task))
            .expect("retry should succeed");
        assert_eq!(response.response.text, "answer from openrouter");
        // The transient failure must not cool the provider down.
        assert!(openrouter.is_healthy());
        assert!(
            gateway.submit(&task, &request(&task)).is_ok(),
            "provider stays usable after a recovered blip"
        );
    }
}
