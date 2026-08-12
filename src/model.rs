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

pub trait ConnectivityProbe {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentRole {
    Planner,
    Verification,
    SpecialistReadOnly,
    SpecialistDiagnosis,
}

impl AgentRole {
    pub fn required_capabilities(&self) -> Vec<ModelCapability> {
        match self {
            AgentRole::Planner | AgentRole::Verification => {
                vec![ModelCapability::TextGeneration, ModelCapability::Reasoning]
            }
            AgentRole::SpecialistDiagnosis => {
                vec![ModelCapability::TextGeneration, ModelCapability::ToolUse]
            }
            AgentRole::SpecialistReadOnly => vec![ModelCapability::TextGeneration],
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModelTask {
    pub task_id: Uuid,
    pub role: AgentRole,
    pub data_classification: DataClassification,
    pub required_capabilities: Vec<ModelCapability>,
    pub safety_critical: bool,
}

impl ModelTask {
    pub fn new(role: AgentRole, data_classification: DataClassification) -> Self {
        Self {
            task_id: Uuid::new_v4(),
            role,
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
    index: HashMap<ModelId, usize>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, entry: ModelEntry) -> Result<(), RegistryError> {
        if self.index.contains_key(&entry.model_id) {
            return Err(RegistryError::DuplicateModel(entry.model_id.clone()));
        }
        let model_id = entry.model_id.clone();
        self.entries.push(entry);
        self.index.insert(model_id, self.entries.len() - 1);
        Ok(())
    }

    pub fn get(&self, model_id: &ModelId) -> Option<&ModelEntry> {
        self.index.get(model_id).map(|i| &self.entries[*i])
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

/// Cooldown (seconds) before an unhealthy provider may be re-probed and
/// returned to the routing pool (model-routing §3.5).
const HEALTH_RETRY_SECONDS: u64 = 30;

#[derive(Clone, Debug)]
pub struct Pin {
    pub provider: ProviderId,
    pub model: ModelId,
}

#[derive(Default)]
pub struct TaskPinner {
    pins: HashMap<Uuid, Pin>,
}

impl TaskPinner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pin(&mut self, task_id: Uuid, provider: ProviderId, model: ModelId) {
        self.pins.insert(task_id, Pin { provider, model });
    }

    pub fn get(&self, task_id: &Uuid) -> Option<&Pin> {
        self.pins.get(task_id)
    }

    pub fn unpin(&mut self, task_id: &Uuid) {
        self.pins.remove(task_id);
    }

    pub fn len(&self) -> usize {
        self.pins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pins.is_empty()
    }
}

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
    NoEligibleProvider,
    ProviderUnhealthy(ProviderId),
    DataClassificationBlocked(DataClassification),
    NoConsent(DataClassification),
    InsufficientResources,
}

impl std::fmt::Display for RoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RoutingError::NoEligibleProvider => write!(f, "no eligible provider"),
            RoutingError::ProviderUnhealthy(p) => write!(f, "provider unhealthy: {p}"),
            RoutingError::DataClassificationBlocked(c) => {
                write!(f, "data classification blocked: {c:?}")
            }
            RoutingError::NoConsent(c) => write!(f, "no consent for data class: {c:?}"),
            RoutingError::InsufficientResources => write!(f, "insufficient resources"),
        }
    }
}

impl std::error::Error for RoutingError {}

pub struct ModelRouter {
    registry: Arc<RwLock<ModelRegistry>>,
    consent: Arc<RwLock<HashMap<ProviderId, ConsentRecord>>>,
    connectivity: Arc<RwLock<ConnectivityState>>,
}

impl ModelRouter {
    pub fn new(registry: Arc<RwLock<ModelRegistry>>) -> Self {
        Self {
            registry,
            consent: Arc::new(RwLock::new(HashMap::new())),
            connectivity: Arc::new(RwLock::new(ConnectivityState::Offline)),
        }
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

    pub fn route(
        &self,
        task: &ModelTask,
        exclude: &[ProviderId],
    ) -> Result<RoutingDecision, RoutingError> {
        let state = self.connectivity();
        let consent = self.consent.read().expect("consent lock");
        let registry = self.registry.read().expect("registry lock");
        let tiers = tiers_for(state);

        let mut eligible: Vec<&ModelEntry> = Vec::new();
        let mut unhealthy: Vec<ProviderId> = Vec::new();
        let mut no_consent = false;
        let mut blocked = false;
        let mut no_capability = false;

        for entry in registry.iter() {
            if exclude.contains(&entry.provider) {
                continue;
            }
            if !tiers.contains(&entry.tier) {
                continue;
            }
            if entry.health.state == HealthState::Unhealthy {
                // model-routing §3.5: an unhealthy provider is re-checked
                // periodically and returns to the pool once its cooldown has
                // passed. It is re-probed via is_healthy() on selection.
                let cooldown_elapsed = entry
                    .health
                    .retry_after
                    .map(|t| now() >= t)
                    .unwrap_or(false);
                if !cooldown_elapsed {
                    unhealthy.push(entry.provider.clone());
                    continue;
                }
            }
            let consent_ok = consent
                .get(&entry.provider)
                .map(|c| c.is_active_for(task.data_classification))
                .unwrap_or(false);
            if !tier_allows(entry.tier, task.data_classification, consent_ok) {
                if needs_consent(entry.tier, task.data_classification) {
                    no_consent = true;
                } else {
                    blocked = true;
                }
                continue;
            }
            if !has_all_capabilities(&entry.capabilities, &task.required_capabilities) {
                no_capability = true;
                continue;
            }
            eligible.push(entry);
        }

        if eligible.is_empty() {
            return Err(diagnose_routing_error(
                unhealthy,
                no_consent,
                blocked,
                no_capability,
                task.data_classification,
            ));
        }

        let mut best = eligible[0];
        for entry in &eligible[1..] {
            if entry.tier.rank() > best.tier.rank() {
                best = entry;
            }
        }

        let reduced_confidence = state == ConnectivityState::Offline || eligible.len() == 1;
        Ok(RoutingDecision {
            provider: best.provider.clone(),
            model: best.model_id.clone(),
            connectivity_state: state,
            data_classification: task.data_classification,
            reduced_confidence,
        })
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

fn needs_consent(tier: ProviderTier, classification: DataClassification) -> bool {
    match tier {
        ProviderTier::Local => false,
        ProviderTier::Lan => !matches!(classification, DataClassification::Public),
        ProviderTier::Internet => matches!(
            classification,
            DataClassification::PersonalMemory | DataClassification::SystemConfig
        ),
    }
}

fn has_all_capabilities(model: &[ModelCapability], required: &[ModelCapability]) -> bool {
    required.iter().all(|cap| model.contains(cap))
}

fn diagnose_routing_error(
    unhealthy: Vec<ProviderId>,
    no_consent: bool,
    blocked: bool,
    no_capability: bool,
    classification: DataClassification,
) -> RoutingError {
    if let Some(provider) = unhealthy.first() {
        return RoutingError::ProviderUnhealthy(provider.clone());
    }
    if no_consent {
        return RoutingError::NoConsent(classification);
    }
    if blocked {
        return RoutingError::DataClassificationBlocked(classification);
    }
    if no_capability {
        return RoutingError::NoEligibleProvider;
    }
    RoutingError::NoEligibleProvider
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
    fn is_healthy(&self) -> bool;
    fn generate(&self, request: &GenerationRequest) -> Result<GenerationResponse, GenerationError>;
}

#[derive(Debug)]
pub enum GatewayError {
    Routing(RoutingError),
    ProviderUnavailable(ProviderId),
    Generation {
        provider: ProviderId,
        message: String,
    },
}

impl GatewayError {
    pub fn provider(&self) -> Option<&ProviderId> {
        match self {
            GatewayError::Routing(_) => None,
            GatewayError::ProviderUnavailable(p) => Some(p),
            GatewayError::Generation { provider, .. } => Some(provider),
        }
    }
}

impl std::fmt::Display for GatewayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GatewayError::Routing(e) => write!(f, "routing failed: {e}"),
            GatewayError::ProviderUnavailable(p) => write!(f, "provider unavailable: {p}"),
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
    pinner: Arc<RwLock<TaskPinner>>,
    backends: RwLock<HashMap<ProviderId, Arc<dyn ModelBackend>>>,
}

impl ModelGateway {
    pub fn new(registry: Arc<RwLock<ModelRegistry>>) -> Self {
        let router = ModelRouter::new(registry.clone());
        Self {
            registry,
            router,
            pinner: Arc::new(RwLock::new(TaskPinner::new())),
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

    pub fn submit(
        &self,
        task: &ModelTask,
        request: &GenerationRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        self.submit_inner(task, request, &[])
    }

    pub fn submit_with_fallback(
        &self,
        task: &ModelTask,
        request: &GenerationRequest,
    ) -> Result<GatewayResponse, GatewayError> {
        match self.submit_inner(task, request, &[]) {
            Ok(response) => Ok(response),
            Err(primary) => {
                let failed = match primary.provider() {
                    Some(provider) => provider.clone(),
                    None => return Err(primary),
                };
                self.registry
                    .write()
                    .expect("registry lock")
                    .mark_provider_unhealthy(&failed);
                let mut retry = task.clone();
                retry.task_id = Uuid::new_v4();
                self.submit_inner(&retry, request, &[failed])
            }
        }
    }

    fn submit_inner(
        &self,
        task: &ModelTask,
        request: &GenerationRequest,
        exclude: &[ProviderId],
    ) -> Result<GatewayResponse, GatewayError> {
        let pinned = self
            .pinner
            .read()
            .expect("pinner lock")
            .get(&task.task_id)
            .cloned();

        let (pin, decision) = match pinned {
            Some(pin) => {
                let decision = RoutingDecision {
                    provider: pin.provider.clone(),
                    model: pin.model.clone(),
                    connectivity_state: self.router.connectivity(),
                    data_classification: task.data_classification,
                    reduced_confidence: self.router.connectivity() == ConnectivityState::Offline,
                };
                (pin, decision)
            }
            None => {
                let decision = self
                    .router
                    .route(task, exclude)
                    .map_err(GatewayError::Routing)?;
                self.pinner.write().expect("pinner lock").pin(
                    task.task_id,
                    decision.provider.clone(),
                    decision.model.clone(),
                );
                let pin = Pin {
                    provider: decision.provider.clone(),
                    model: decision.model.clone(),
                };
                (pin, decision)
            }
        };

        match self.generate_on(&pin, request) {
            Ok(response) => Ok(GatewayResponse { decision, response }),
            Err(err) => {
                self.pinner.write().expect("pinner lock").unpin(&task.task_id);
                Err(err)
            }
        }
    }

    fn generate_on(
        &self,
        pin: &Pin,
        request: &GenerationRequest,
    ) -> Result<GenerationResponse, GatewayError> {
        let backend = self
            .backends
            .read()
            .expect("backends lock")
            .get(&pin.provider)
            .cloned()
            .ok_or_else(|| GatewayError::ProviderUnavailable(pin.provider.clone()))?;

        if !backend.is_healthy() {
            self.registry
                .write()
                .expect("registry lock")
                .mark_provider_unhealthy(&pin.provider);
            return Err(GatewayError::ProviderUnavailable(pin.provider.clone()));
        }

        let started = Instant::now();
        let result = backend.generate(request);
        let latency_ms = started.elapsed().as_millis() as u64;

        match result {
            Ok(response) => {
                self.registry
                    .write()
                    .expect("registry lock")
                    .record_success(&pin.provider, latency_ms);
                Ok(response)
            }
            Err(error) => {
                if error.recoverable {
                    self.registry
                        .write()
                        .expect("registry lock")
                        .mark_provider_unhealthy(&pin.provider);
                }
                Err(GatewayError::Generation {
                    provider: pin.provider.clone(),
                    message: error.message,
                })
            }
        }
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

    fn router(entries: Vec<ModelEntry>) -> ModelRouter {
        ModelRouter::new(registry_with(entries))
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
    fn offline_routes_local() {
        let r = router(vec![
            internet_model("qwen", &reasoning()),
            local_model("qwen-local", &reasoning()),
        ]);
        r.set_connectivity(ConnectivityState::Offline);
        let decision = r.route(&public_task(), &[]).expect("route");
        assert_eq!(decision.provider, ProviderId::local());
        assert_eq!(decision.model, ModelId::new("qwen-local"));
        assert!(decision.reduced_confidence);
    }

    #[test]
    fn offline_excludes_internet() {
        let r = router(vec![internet_model("qwen", &reasoning())]);
        r.set_connectivity(ConnectivityState::Offline);
        assert!(matches!(
            r.route(&public_task(), &[]),
            Err(RoutingError::NoEligibleProvider)
        ));
    }

    #[test]
    fn tier_priority_by_connectivity() {
        let entries = vec![
            local_model("local-a", &reasoning()),
            lan_model("lan-a", &reasoning()),
            internet_model("net-a", &reasoning()),
        ];

        let r = router(entries.clone());
        r.set_connectivity(ConnectivityState::Internet);
        let d = r.route(&public_task(), &[]).expect("route");
        assert_eq!(d.provider, ProviderId::new("openrouter"));

        let r = router(entries.clone());
        r.set_connectivity(ConnectivityState::LanOnly);
        let d = r.route(&public_task(), &[]).expect("route");
        assert_eq!(d.provider, ProviderId::new("lan-gpu-01"));

        let r = router(entries);
        r.set_connectivity(ConnectivityState::Offline);
        let d = r.route(&public_task(), &[]).expect("route");
        assert_eq!(d.provider, ProviderId::local());
    }

    #[test]
    fn registration_order_is_tiebreak() {
        let r = router(vec![
            internet_model("net-a", &reasoning()),
            internet_model("net-b", &reasoning()),
        ]);
        r.set_connectivity(ConnectivityState::Internet);
        let d = r.route(&public_task(), &[]).expect("route");
        assert_eq!(d.model, ModelId::new("net-a"));
    }

    #[test]
    fn protected_never_internet() {
        let entries = vec![
            internet_model("net-a", &reasoning()),
            local_model("local-a", &reasoning()),
        ];
        let r = router(entries);
        r.set_connectivity(ConnectivityState::Internet);
        let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Protected);
        let d = r.route(&task, &[]).expect("route");
        assert_eq!(d.provider, ProviderId::local());
    }

    #[test]
    fn protected_blocked_with_only_internet() {
        let r = router(vec![internet_model("net-a", &reasoning())]);
        r.set_connectivity(ConnectivityState::Internet);
        let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::Protected);
        assert!(matches!(
            r.route(&task, &[]),
            Err(RoutingError::DataClassificationBlocked(DataClassification::Protected))
        ));
    }

    #[test]
    fn personal_memory_needs_consent() {
        let entries = vec![internet_model("net-a", &reasoning())];
        let r = router(entries);
        r.set_connectivity(ConnectivityState::Internet);
        let task = ModelTask::new(
            AgentRole::SpecialistReadOnly,
            DataClassification::PersonalMemory,
        );
        assert!(matches!(
            r.route(&task, &[]),
            Err(RoutingError::NoConsent(DataClassification::PersonalMemory))
        ));

        let record = ConsentRecord::new(
            ProviderId::new("openrouter"),
            vec![DataClassification::PersonalMemory],
        );
        r.grant_consent(record).expect("grant");
        let d = r.route(&task, &[]).expect("route");
        assert_eq!(d.provider, ProviderId::new("openrouter"));
    }

    #[test]
    fn public_needs_no_consent() {
        let r = router(vec![internet_model("net-a", &reasoning())]);
        r.set_connectivity(ConnectivityState::Internet);
        let d = r.route(&public_task(), &[]).expect("route");
        assert_eq!(d.provider, ProviderId::new("openrouter"));
    }

    #[test]
    fn lan_requires_consent_for_non_public() {
        let entries = vec![lan_model("lan-a", &reasoning())];
        let r = router(entries);
        r.set_connectivity(ConnectivityState::LanOnly);
        let task = ModelTask::new(AgentRole::SpecialistReadOnly, DataClassification::SystemConfig);
        assert!(matches!(
            r.route(&task, &[]),
            Err(RoutingError::NoConsent(DataClassification::SystemConfig))
        ));
    }

    #[test]
    fn revoked_consent_blocks_routing() {
        let r = router(vec![internet_model("net-a", &reasoning())]);
        r.set_connectivity(ConnectivityState::Internet);
        let provider = ProviderId::new("openrouter");
        r.grant_consent(ConsentRecord::new(
            provider.clone(),
            vec![DataClassification::PersonalMemory],
        ))
        .expect("grant");
        r.revoke_consent(&provider);
        let task = ModelTask::new(
            AgentRole::SpecialistReadOnly,
            DataClassification::PersonalMemory,
        );
        assert!(matches!(r.route(&task, &[]), Err(RoutingError::NoConsent(_))));
    }

    #[test]
    fn unhealthy_provider_excluded() {
        let registry = registry_with(vec![
            internet_model("net-a", &reasoning()),
            lan_model("lan-a", &reasoning()),
        ]);
        registry
            .write()
            .expect("lock")
            .mark_provider_unhealthy(&ProviderId::new("openrouter"));
        let r = ModelRouter::new(registry);
        r.set_connectivity(ConnectivityState::Internet);
        let d = r.route(&public_task(), &[]).expect("route");
        assert_eq!(d.provider, ProviderId::new("lan-gpu-01"));
    }

    #[test]
    fn all_unhealthy_reports_provider() {
        let registry = registry_with(vec![internet_model("net-a", &reasoning())]);
        registry
            .write()
            .expect("lock")
            .mark_provider_unhealthy(&ProviderId::new("openrouter"));
        let r = ModelRouter::new(registry);
        r.set_connectivity(ConnectivityState::Internet);
        assert!(matches!(
            r.route(&public_task(), &[]),
            Err(RoutingError::ProviderUnhealthy(p)) if p == ProviderId::new("openrouter")
        ));
    }

    #[test]
    fn unhealthy_provider_returns_after_cooldown() {
        let registry = registry_with(vec![
            internet_model("net-a", &reasoning()),
            lan_model("lan-a", &reasoning()),
        ]);
        registry
            .write()
            .expect("lock")
            .mark_provider_unhealthy(&ProviderId::new("openrouter"));
        let r = ModelRouter::new(registry.clone());
        r.set_connectivity(ConnectivityState::Internet);

        // Immediately after the failure the provider is still in cooldown and
        // is excluded (model-routing §3.5: unhealthy providers excluded until
        // re-checked).
        let d = r.route(&public_task(), &[]).expect("route while cooling down");
        assert_eq!(d.provider, ProviderId::new("lan-gpu-01"));

        // Simulate the cooldown elapsing. The provider becomes eligible again
        // and, being the higher tier, is selected so it can be re-probed.
        registry
            .write()
            .expect("lock")
            .expire_cooldown(&ProviderId::new("openrouter"));
        let d = r.route(&public_task(), &[]).expect("route after cooldown");
        assert_eq!(d.provider, ProviderId::new("openrouter"));
    }

    #[test]
    fn capability_filter_applied() {
        let r = router(vec![internet_model("net-a", &text_only())]);
        r.set_connectivity(ConnectivityState::Internet);
        let task = ModelTask::new(AgentRole::Planner, DataClassification::Public);
        assert!(matches!(
            r.route(&task, &[]),
            Err(RoutingError::NoEligibleProvider)
        ));
    }

    #[test]
    fn diagnosis_role_requires_tool_use() {
        let r = router(vec![internet_model("net-a", &tool_use())]);
        r.set_connectivity(ConnectivityState::Internet);
        let task = ModelTask::new(AgentRole::SpecialistDiagnosis, DataClassification::Public);
        let d = r.route(&task, &[]).expect("route");
        assert_eq!(d.provider, ProviderId::new("openrouter"));
    }

    #[test]
    fn single_eligible_provider_reduces_confidence() {
        let r = router(vec![internet_model("net-a", &reasoning())]);
        r.set_connectivity(ConnectivityState::Internet);
        let d = r.route(&public_task(), &[]).expect("route");
        assert!(d.reduced_confidence);
    }

    #[test]
    fn fallback_excludes_failed_provider() {
        let r = router(vec![
            internet_model("net-a", &reasoning()),
            lan_model("lan-a", &reasoning()),
        ]);
        r.set_connectivity(ConnectivityState::Internet);
        let excluded = [ProviderId::new("openrouter")];
        let d = r.route(&public_task(), &excluded).expect("route");
        assert_eq!(d.provider, ProviderId::new("lan-gpu-01"));
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
    fn pinner_pin_get_unpin() {
        let mut pinner = TaskPinner::new();
        let id = Uuid::new_v4();
        pinner.pin(id, ProviderId::local(), ModelId::new("qwen-local"));
        let pin = pinner.get(&id).expect("pin");
        assert_eq!(pin.provider, ProviderId::local());
        pinner.unpin(&id);
        assert!(pinner.get(&id).is_none());
        assert!(pinner.is_empty());
    }

    struct MockBackend {
        provider: ProviderId,
        healthy: std::sync::atomic::AtomicBool,
        fail: bool,
        label: &'static str,
    }

    impl MockBackend {
        fn ok(provider: ProviderId, label: &'static str) -> Arc<Self> {
            Arc::new(Self {
                provider,
                healthy: std::sync::atomic::AtomicBool::new(true),
                fail: false,
                label,
            })
        }

        fn failing(provider: ProviderId, label: &'static str) -> Arc<Self> {
            Arc::new(Self {
                provider,
                healthy: std::sync::atomic::AtomicBool::new(true),
                fail: true,
                label,
            })
        }
    }

    impl ModelBackend for MockBackend {
        fn provider_id(&self) -> &ProviderId {
            &self.provider
        }

        fn is_healthy(&self) -> bool {
            self.healthy.load(std::sync::atomic::Ordering::Relaxed)
        }

        fn generate(
            &self,
            _request: &GenerationRequest,
        ) -> Result<GenerationResponse, GenerationError> {
            if self.fail {
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
        }
    }

    #[test]
    fn gateway_submit_routes_and_pins() {
        let registry = registry_with(vec![
            internet_model("net-a", &reasoning()),
            lan_model("lan-a", &reasoning()),
        ]);
        let gateway = ModelGateway::new(registry);
        gateway.set_connectivity(ConnectivityState::Internet);
        gateway.register_backend(MockBackend::ok(
            ProviderId::new("openrouter"),
            "openrouter",
        ));
        gateway.register_backend(MockBackend::ok(
            ProviderId::new("lan-gpu-01"),
            "lan",
        ));

        let task = public_task();
        let response = gateway.submit(&task, &request(&task)).expect("submit");
        assert_eq!(response.decision.provider, ProviderId::new("openrouter"));
        assert_eq!(response.response.text, "answer from openrouter");

        gateway.set_connectivity(ConnectivityState::LanOnly);
        let retried = gateway.submit(&task, &request(&task)).expect("submit");
        assert_eq!(retried.decision.provider, ProviderId::new("openrouter"));
    }

    #[test]
    fn gateway_fallback_uses_new_task_and_marks_unhealthy() {
        let registry = registry_with(vec![
            internet_model("net-a", &reasoning()),
            lan_model("lan-a", &reasoning()),
        ]);
        let gateway = ModelGateway::new(registry.clone());
        gateway.set_connectivity(ConnectivityState::Internet);
        gateway.register_backend(MockBackend::failing(
            ProviderId::new("openrouter"),
            "openrouter",
        ));
        gateway.register_backend(MockBackend::ok(ProviderId::new("lan-gpu-01"), "lan"));

        let task = public_task();
        let response = gateway
            .submit_with_fallback(&task, &request(&task))
            .expect("fallback");
        assert_eq!(response.decision.provider, ProviderId::new("lan-gpu-01"));
        assert_eq!(response.response.text, "answer from lan");
        assert_ne!(response.decision.model, ModelId::new("net-a"));

        let entry = registry
            .read()
            .expect("registry lock")
            .get(&ModelId::new("net-a"))
            .cloned()
            .expect("entry");
        assert_eq!(entry.health.state, HealthState::Unhealthy);
    }

    #[test]
    fn gateway_submit_without_fallback_clears_pin_on_failure() {
        let registry = registry_with(vec![internet_model("net-a", &reasoning())]);
        let gateway = ModelGateway::new(registry);
        gateway.set_connectivity(ConnectivityState::Internet);
        gateway.register_backend(MockBackend::failing(
            ProviderId::new("openrouter"),
            "openrouter",
        ));

        let task = public_task();
        assert!(matches!(
            gateway.submit(&task, &request(&task)),
            Err(GatewayError::Generation { .. })
        ));
    }
}
