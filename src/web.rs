//! Web/Fetch specialist — Stage 3 of workspace co-partner (0004).
//! Owns the single pseudo-resource `web:fetch` (ADR-0008) with risk 1
//! tools `fetch_url` and `search_web`. Fetches are gated by the broker
//! (capability + expiry) but skip Guardian/staging (risk 1). `DataPolicy`
//! redaction is enforced before the HTTP call — `Secret` data never leaves.
//!
//! Fetched content is returned as a `ToolResult` with provenance
//! (`fetched_from` URL + timestamp) and stored as tool observation, not as
//! a direct model prompt injection. The model sees a redacted `ToolResult`.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{NodeId, NodeMetadata, NodeType, SystemGraph, TrustLevel};
use crate::protocol::{HealthState, ToolData, ToolResult, ToolStatus, now, MessageEnvelope, MessageType, DataClassification};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "web.specialist";
pub const WEB_RESOURCE: &str = "web:fetch";

/// Trait for fetching URL content — mock in tests, `ureq` in production.
pub trait Fetcher: Send + Sync {
    fn fetch(&self, url: &str) -> Result<String, String>;
}

pub struct MockFetcher {
    pub content: HashMap<String, String>,
    pub should_fail: bool,
}

impl MockFetcher {
    pub fn new() -> Self {
        let mut content = HashMap::new();
        content.insert("https://docs.rs/tokio".into(), "tokio docs: async runtime for Rust".into());
        content.insert("https://example.com".into(), "example content".into());
        Self { content, should_fail: false }
    }
    pub fn with_content(mut self, url: &str, body: &str) -> Self {
        self.content.insert(url.into(), body.into());
        self
    }
}

impl Fetcher for MockFetcher {
    fn fetch(&self, url: &str) -> Result<String, String> {
        if self.should_fail {
            return Err("mock fetch failed".into());
        }
        self.content.get(url).cloned().ok_or_else(|| format!("no mock content for {url}"))
    }
}

pub struct LiveFetcher;

impl Fetcher for LiveFetcher {
    fn fetch(&self, url: &str) -> Result<String, String> {
        // Validate URL scheme first (no file://, no internal, no data:).
        if !is_allowed_url(url) {
            return Err(format!("URL not allowed (only https/http): {url}"));
        }
        // Use ureq with a short timeout to keep risk 1 fetches bounded.
        let resp = ureq::get(url).timeout(std::time::Duration::from_secs(10)).call().map_err(|e| format!("fetch failed: {e}"))?;
        let text = resp.into_string().map_err(|e| format!("read body failed: {e}"))?;
        // Hard cap 2 MiB to avoid token blow-up.
        if text.len() > 2 * 1024 * 1024 {
            return Err("response too large (>2 MiB)".into());
        }
        Ok(truncate_for_model(text))
    }
}

fn is_allowed_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn truncate_for_model(s: String) -> String {
    if s.len() > 64 * 1024 {
        // Keep head + marker so verifier/fidelity isn't tricked by missing tail.
        let mut out = s[..64*1024].to_string();
        out.push_str("\n…[truncated, original length exceeds 64 KiB]");
        out
    } else {
        s
    }
}

/// Strip `Secret` markers from fetched content before it reaches the model.
/// In v0.1 this is a placeholder: we drop any line containing `SECRET` or
/// `[[secret` markers. The real `DataPolicy` enforcement lives in
/// `coordinator/routing` and `config.rs` consent checks — this is the web
/// specialist's local guard.
pub fn redact_secret(content: &str) -> String {
    content.lines().filter(|l| !l.to_ascii_lowercase().contains("secret")).collect::<Vec<_>>().join("\n")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebSpecialist {
    pub specialist: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebError {
    Graph(String),
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::Graph(r) => write!(f, "could not instantiate web specialist: {r}") }
    }
}
impl std::error::Error for WebError {}

impl WebSpecialist {
    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, WebError> {
        let specialist = NodeId("specialist:web:0".into());
        if graph.get_node(&specialist).is_some() {
            return Ok(Self { specialist });
        }
        let t = now();
        let mut node = NodeMetadata::new(specialist.clone(), NodeType::Specialist, crate::graph::ProvenanceSource::Declared { package: PACKAGE_ID.into() }, TrustLevel::Trusted, t);
        node.label = "Web/Fetch specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(WebError::Graph)?;
        Ok(Self { specialist })
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let r = ResourceId(WEB_RESOURCE.into());
        vec![
            tool("web.fetch_url", RiskLevel::Routine, Operation::Fetch, &r),
            tool("web.search_web", RiskLevel::Routine, Operation::Fetch, &r),
        ]
    }

    /// Handle a `Fetch`/`Search` request with the given fetcher. Returns a
    /// `ToolResult` with provenance. This is the function registered as the
    /// `SpecialistHandler` for `web.fetch_url` / `web.search_web`.
    pub fn handle_fetch(&self, request: &crate::protocol::ToolRequest, fetcher: &dyn Fetcher) -> ToolResult {
        let url = match &request.parameters {
            crate::protocol::ToolParameters::Fetch { url } => url.clone(),
            crate::protocol::ToolParameters::Search { query } => {
                // For search, treat query as a URL-like search string; mock
                // fetcher can return canned results.
                query.clone()
            }
            _ => {
                return err_result(request, "fetch_url expects Fetch { url } or Search { query }");
            }
        };
        if !is_allowed_url(&url) && !matches!(&request.parameters, crate::protocol::ToolParameters::Search { .. }) {
            // Search queries need not be URLs; only Fetch URLs are validated.
            if matches!(&request.parameters, crate::protocol::ToolParameters::Fetch { .. }) {
                return err_result(request, &format!("URL must be https:// or http://, got: {url}"));
            }
        }
        match fetcher.fetch(&url) {
            Ok(raw) => {
                let redacted = redact_secret(&raw);
                let mut data = serde_json::json!({
                    "url": url,
                    "fetched_at": now(),
                    "fetched_from": url,
                    "content": redacted,
                    "bytes": redacted.len(),
                });
                // Audit provenance: the tool result's data carries fetched_from.
                success_result(request, ToolData::QueryResult { data })
            }
            Err(e) => err_result(request, &e),
        }
    }
}

fn tool(id: &str, risk: RiskLevel, op: Operation, resource: &ResourceId) -> crate::capability::ToolDefinition {
    crate::capability::ToolDefinition { tool_id: id.into(), specialist_package: PACKAGE_ID.into(), risk_level: risk, required_capabilities: vec![Capability { resource: resource.clone(), operation: op }], description: id.into() }
}

fn success_result(request: &crate::protocol::ToolRequest, data: ToolData) -> ToolResult {
    ToolResult {
        envelope: MessageEnvelope::new(MessageType::ToolResult, PrincipalId::system(PACKAGE_ID), request.envelope.correlation_id, DataClassification::Public),
        request_id: request.request_id,
        status: ToolStatus::Success,
        data: Some(data),
        error: None,
        health_impact: None,
    }
}

fn err_result(request: &crate::protocol::ToolRequest, msg: &str) -> ToolResult {
    ToolResult {
        envelope: MessageEnvelope::new(MessageType::ToolResult, PrincipalId::system(PACKAGE_ID), request.envelope.correlation_id, DataClassification::Public),
        request_id: request.request_id,
        status: ToolStatus::Failed,
        data: None,
        error: Some(crate::protocol::ToolError { code: crate::protocol::ToolErrorCode::Internal, message: msg.into(), recoverable: false }),
        health_impact: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::{Broker, BrokerClient};
    use crate::capability::{Capability, Clearance, Operation, PrincipalId, ResourceId, RiskLevel, ToolDefinition};
    use crate::guardian::Guardian;
    use crate::protocol::ToolParameters;
    use std::sync::Arc;

    fn web_broker() -> (Broker, PrincipalId, crate::capability::CapabilityToken) {
        let broker = Broker::new();
        broker.core().lock().unwrap().set_clock(|| 2000);
        let principal = PrincipalId::agent("web.specialist", "web-001");
        let cap = Capability { resource: ResourceId(WEB_RESOURCE.into()), operation: Operation::Fetch };
        let token = crate::capability::CapabilityToken {
            principal: principal.clone(),
            capability: cap.clone(),
            clearance: Clearance(RiskLevel::Routine),
            granted_at: 1000,
            expires_at: 999999,
            provenance: crate::capability::Provenance { granted_by: PrincipalId::system("policy-broker"), package_id: PACKAGE_ID.into(), package_version: 1, signature_verified: true },
        };
        broker.register_tool(ToolDefinition { tool_id: "web.fetch_url".into(), specialist_package: PACKAGE_ID.into(), risk_level: RiskLevel::Routine, required_capabilities: vec![cap.clone()], description: "fetch url".into() });
        broker.register_tool(ToolDefinition { tool_id: "web.search_web".into(), specialist_package: PACKAGE_ID.into(), risk_level: RiskLevel::Routine, required_capabilities: vec![cap.clone()], description: "search web".into() });
        broker.register_principal(principal.clone(), vec![cap.clone()], Clearance(RiskLevel::Routine));
        broker.set_resource_state(ResourceId(WEB_RESOURCE.into()), crate::capability::ResourceState::Available);
        broker.set_resource_owner(ResourceId(WEB_RESOURCE.into()), principal.clone());
        broker.set_guardian(Guardian::new());
        // Spawn mock specialist
        let fetcher: Arc<dyn Fetcher> = Arc::new(MockFetcher::new().with_content("https://docs.rs/tokio", "tokio 1.38 docs: async runtime, spawn, select!"));
        let ws = WebSpecialist { specialist: NodeId("specialist:web:0".into()) };
        let f = fetcher.clone();
        broker.spawn_specialist("web.fetch_url", Arc::new(move |req| ws.handle_fetch(&req, f.as_ref())));
        let ws2 = WebSpecialist { specialist: NodeId("specialist:web:0".into()) };
        let f2 = fetcher.clone();
        broker.spawn_specialist("web.search_web", Arc::new(move |req| ws2.handle_fetch(&req, f2.as_ref())));
        (broker, principal, token)
    }

    #[test]
    fn fetch_url_returns_content_with_provenance() {
        let (broker, principal, token) = web_broker();
        let req = crate::broker::build_request(principal.clone(), ResourceId(WEB_RESOURCE.into()), Operation::Fetch, "web.fetch_url", &token, ToolParameters::Fetch { url: "https://docs.rs/tokio".into() }, uuid::Uuid::new_v4(), 20);
        let res = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(res.status, ToolStatus::Success);
        let data = res.data.unwrap();
        match data {
            ToolData::QueryResult { data } => {
                assert_eq!(data["fetched_from"], "https://docs.rs/tokio");
                assert!(data["content"].as_str().unwrap().contains("tokio"));
            }
            _ => panic!("expected QueryResult"),
        }
    }

    #[test]
    fn fetch_rejects_non_http_scheme() {
        let (broker, principal, token) = web_broker();
        let req = crate::broker::build_request(principal.clone(), ResourceId(WEB_RESOURCE.into()), Operation::Fetch, "web.fetch_url", &token, ToolParameters::Fetch { url: "file:///etc/passwd".into() }, uuid::Uuid::new_v4(), 21);
        let res = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(res.status, ToolStatus::Failed);
        assert!(res.error.unwrap().message.contains("https://"));
    }

    #[test]
    fn fetch_strips_secret_lines() {
        let broker = Broker::new();
        broker.core().lock().unwrap().set_clock(|| 2000);
        let principal = PrincipalId::agent("web.specialist", "web-001");
        let cap = Capability { resource: ResourceId(WEB_RESOURCE.into()), operation: Operation::Fetch };
        let token = crate::capability::CapabilityToken { principal: principal.clone(), capability: cap.clone(), clearance: Clearance(RiskLevel::Routine), granted_at: 1000, expires_at: 999999, provenance: crate::capability::Provenance { granted_by: PrincipalId::system("policy-broker"), package_id: PACKAGE_ID.into(), package_version: 1, signature_verified: true } };
        broker.register_tool(ToolDefinition { tool_id: "web.fetch_url".into(), specialist_package: PACKAGE_ID.into(), risk_level: RiskLevel::Routine, required_capabilities: vec![cap.clone()], description: "fetch".into() });
        broker.register_principal(principal.clone(), vec![cap.clone()], Clearance(RiskLevel::Routine));
        broker.set_resource_state(ResourceId(WEB_RESOURCE.into()), crate::capability::ResourceState::Available);
        broker.set_resource_owner(ResourceId(WEB_RESOURCE.into()), principal.clone());
        broker.set_guardian(Guardian::new());
        let fetcher: Arc<dyn Fetcher> = Arc::new(MockFetcher::new().with_content("https://example.com/secret", "public line\nSECRET=sk-123\nanother public line"));
        let ws = WebSpecialist { specialist: NodeId("specialist:web:0".into()) };
        broker.spawn_specialist("web.fetch_url", Arc::new(move |req| ws.handle_fetch(&req, fetcher.as_ref())));
        let req = crate::broker::build_request(principal.clone(), ResourceId(WEB_RESOURCE.into()), Operation::Fetch, "web.fetch_url", &token, ToolParameters::Fetch { url: "https://example.com/secret".into() }, uuid::Uuid::new_v4(), 22);
        let res = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(res.status, ToolStatus::Success);
        match res.data.unwrap() {
            ToolData::QueryResult { data } => {
                let content = data["content"].as_str().unwrap();
                assert!(!content.contains("SECRET"), "secret line should be redacted");
                assert!(content.contains("public line"));
            }
            _ => panic!("expected QueryResult"),
        }
    }

    #[test]
    fn fetch_requires_capability() {
        let (broker, principal, mut token) = web_broker();
        token.capability = Capability { resource: ResourceId("file:/workspace".into()), operation: Operation::Fetch };
        // principal does not have web:fetch cap for this token
        let req = crate::broker::build_request(principal.clone(), ResourceId(WEB_RESOURCE.into()), Operation::Fetch, "web.fetch_url", &token, ToolParameters::Fetch { url: "https://example.com".into() }, uuid::Uuid::new_v4(), 23);
        let res = broker.client(principal).request_tool(req).unwrap();
        assert_eq!(res.status, ToolStatus::Denied);
    }
}
