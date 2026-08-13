//! M7: Security and identity specialist (umbrella). Owns the security
//! domain: identity, credentials, trust boundaries, and anomaly response.
//! Per `docs/modules/security.md`, v0.1 is read-only plus quarantine.
//! `observe_security` and `diagnose_fault` are bounded read-only tools;
//! `quarantine` (risk 4, the bounded containment response) is deferred to the
//! mutation pass and will pass through the staged executor and Guardian.
//!
//! Unlike the hardware umbrellas, the security domain is the enforcement
//! plane — the Guardian's invariants, the broker's capabilities, and the
//! audit log — not sysfs-discovered hardware. The specialist owns a
//! `security:domain` resource and reports on that state. Secrets never leave
//! the local trust boundary (security-model §5): credentials and keys are
//! never sent to models, never recorded in logs, and never accepted as agent
//! input. Invariants SEC-001 (identity and trust boundaries are present and
//! verified) and SEC-002 (no secret leaves the local trust boundary) are
//! evaluated from evidence; missing evidence is reported as unknown, never as
//! healthy.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "security.specialist";

/// The security domain: the umbrella specialist and the enforcement-plane
/// resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecuritySpecialist {
    pub specialist: NodeId,
    /// Enforcement-plane nodes the specialist owns (`guardian:*`,
    /// `capability:*`, `policy:*`).
    pub security_nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityHealth {
    /// Enforcement-plane nodes reporting Healthy (SEC-001 evidence: identity
    /// and trust boundaries present and verified).
    pub healthy_nodes: usize,
    pub security_nodes: usize,
    /// Enforcement-plane nodes not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityError {
    NoSecurityResources,
    Graph(String),
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSecurityResources => {
                f.write_str("no security resources were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate security specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for SecurityError {}

/// Enforcement-plane nodes: the Guardian, capabilities, and policies that
/// make up the security domain.
fn is_security_node(node: &NodeMetadata) -> bool {
    matches!(
        node.node_type,
        NodeType::Guardian | NodeType::Capability | NodeType::Policy
    )
}

impl SecuritySpecialist {
    /// Deterministically resolve the security domain: every enforcement-plane
    /// node in the graph. Lists are sorted by id so the result is stable
    /// across boots. An empty domain is an error (fail closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, SecurityError> {
        let mut security_nodes: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_security_node(node))
            .map(|node| node.node_id.clone())
            .collect();
        security_nodes.sort_by(|a, b| a.0.cmp(&b.0));
        if security_nodes.is_empty() {
            return Err(SecurityError::NoSecurityResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:security:0".into()),
            security_nodes,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, SecurityError> {
        let specialist = Self::discover(graph)?;
        if graph.get_node(&specialist.specialist).is_some() {
            return Ok(specialist);
        }
        let t = now();
        let mut node = NodeMetadata::new(
            specialist.specialist.clone(),
            NodeType::Specialist,
            crate::graph::ProvenanceSource::Declared {
                package: PACKAGE_ID.into(),
            },
            TrustLevel::Trusted,
            t,
        );
        node.label = "Security and identity specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(SecurityError::Graph)?;
        for resource in specialist.security_nodes.iter() {
            // One-owner rule (architecture §5): if another specialist somehow
            // owns the resource first, the umbrella does not take it over.
            if graph.get_owner(resource).is_none() {
                specialist.add_ownership(graph, resource, t)?;
            }
        }
        Ok(specialist)
    }

    fn add_ownership(
        &self,
        graph: &mut SystemGraph,
        resource: &NodeId,
        t: crate::protocol::Timestamp,
    ) -> Result<(), SecurityError> {
        graph
            .add_edge(EdgeMetadata {
                edge_id: EdgeId::new(),
                edge_type: EdgeType::Owns,
                source_node: self.specialist.clone(),
                target_node: resource.clone(),
                provenance: EdgeProvenance::Declared {
                    declared_by: PrincipalId::system("coordinator"),
                    package: PACKAGE_ID.into(),
                },
                created_at: t,
                last_observed: t,
                expires_at: None,
                attributes: HashMap::new(),
            })
            .map_err(SecurityError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("security:domain".into());
        vec![
            tool(
                "observe_security",
                RiskLevel::ReadOnly,
                Operation::Observe,
                &resource,
            ),
            tool(
                "diagnose_fault",
                RiskLevel::ReadOnly,
                Operation::Diagnose,
                &resource,
            ),
        ]
    }

    /// Cross-layer health: SEC-001 evidence is an enforcement-plane node
    /// reporting Healthy, plus the count of security nodes in the domain.
    pub fn health(&self, graph: &SystemGraph) -> SecurityHealth {
        let healthy_nodes = self
            .security_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health == HealthState::Healthy)
                    .unwrap_or(false)
            })
            .count();
        let degraded = self
            .security_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        SecurityHealth {
            healthy_nodes,
            security_nodes: self.security_nodes.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `guardian:`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self.security_nodes.clone();
        }
        let mut matched: Vec<NodeId> = self
            .security_nodes
            .iter()
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: identity, trust, and security state for the
    /// target resources (docs/modules/security.md). Domain-wide when the
    /// target is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("security_nodes".into(), health.security_nodes.to_string());
        metrics.insert("healthy_nodes".into(), health.healthy_nodes.to_string());
        metrics.insert("degraded".into(), health.degraded.to_string());
        metrics.insert(
            "resources".into(),
            resources.iter().map(|id| id.0.clone()).collect::<Vec<_>>().join(","),
        );
        for id in resources.iter().take(8) {
            if let Some(node) = graph.get_node(id) {
                metrics.insert(format!("state:{id}"), format!("{:?}", node.health));
            }
        }
        ok(
            crate::protocol::ToolData::DeviceState {
                state: crate::capability::ResourceState::Available,
                metrics,
            },
            None,
        )
    }

    /// Bounded diagnose tool: compare observations with the security
    /// invariants (SEC-001; SEC-002 belongs to the mutation pass). Missing
    /// evidence is reported as unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.healthy_nodes < health.security_nodes {
            findings.push(format!(
                "SEC-001: {} of {} security resources are not verified healthy (identity or trust boundary degraded)",
                health.security_nodes - health.healthy_nodes,
                health.security_nodes
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} security resources report non-healthy state",
                health.degraded
            ));
            confidence = 0.7;
        }
        if findings.is_empty() {
            findings.push("no invariant violation found".into());
            confidence = 0.9;
        }
        ok(
            crate::protocol::ToolData::Diagnosis { findings, confidence },
            None,
        )
    }
}

fn tool(
    name: &'static str,
    risk_level: RiskLevel,
    operation: Operation,
    resource: &ResourceId,
) -> crate::capability::ToolDefinition {
    crate::capability::ToolDefinition {
        tool_id: format!("security.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Security and identity specialist {name}"),
    }
}

fn ok(data: crate::protocol::ToolData, error: Option<crate::protocol::ToolError>) -> crate::protocol::ToolResult {
    crate::protocol::ToolResult {
        envelope: crate::protocol::MessageEnvelope::new(
            crate::protocol::MessageType::ToolResult,
            PrincipalId::system(PACKAGE_ID),
            uuid::Uuid::new_v4(),
            crate::protocol::DataClassification::SystemConfig,
        ),
        request_id: uuid::Uuid::new_v4(),
        status: if error.is_some() {
            crate::protocol::ToolStatus::Failed
        } else {
            crate::protocol::ToolStatus::Success
        },
        data: Some(data),
        error,
        health_impact: None,
    }
}

fn not_found(target: &str) -> crate::protocol::ToolResult {
    ok(
        crate::protocol::ToolData::QueryResult {
            data: serde_json::json!({"text": format!("nothing matches: {target}")}),
        },
        Some(crate::protocol::ToolError {
            code: crate::protocol::ToolErrorCode::Internal,
            message: format!("nothing matches: {target}"),
            recoverable: false,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{ProvenanceSource, TrustLevel};

    fn node(id: &str, node_type: NodeType) -> NodeMetadata {
        let mut node = NodeMetadata::new(
            NodeId(id.into()),
            node_type,
            ProvenanceSource::Discovered {
                via: "test".into(),
            },
            TrustLevel::Trusted,
            1,
        );
        node.health = HealthState::Healthy;
        node
    }

    fn security_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut guardian = node("guardian:0", NodeType::Guardian);
        guardian.label = "Infrastructure Guardian".into();
        graph.add_node(guardian).unwrap();
        let mut capability = node("capability:session", NodeType::Capability);
        capability.label = "session capability".into();
        graph.add_node(capability).unwrap();
        let mut policy = node("policy:broker", NodeType::Policy);
        policy.label = "broker policy".into();
        graph.add_node(policy).unwrap();
        graph
    }

    #[test]
    fn discovers_security_nodes() {
        let graph = security_graph();
        let specialist = SecuritySpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.security_nodes,
            vec![
                NodeId("capability:session".into()),
                NodeId("guardian:0".into()),
                NodeId("policy:broker".into())
            ]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = security_graph();
        let specialist = SecuritySpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            graph.get_edges(&specialist.specialist, EdgeType::Owns).len(),
            3
        );
        for resource in specialist.security_nodes.iter() {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
    }

    #[test]
    fn fails_closed_without_security_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            SecuritySpecialist::discover(&graph),
            Err(SecurityError::NoSecurityResources)
        );
        assert!(SecuritySpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = security_graph();
        let specialist = SecuritySpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["security.observe_security", "security.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_verified_evidence() {
        let mut graph = security_graph();
        let specialist = SecuritySpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.security_nodes, 3);
        assert_eq!(health.healthy_nodes, 3);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = security_graph();
        let specialist = SecuritySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("security_nodes").unwrap(), "3");
        assert_eq!(metrics.get("healthy_nodes").unwrap(), "3");
    }

    #[test]
    fn diagnose_flags_missing_verified_evidence() {
        let mut graph = security_graph();
        // Mark one security node degraded: SEC-001 evidence disappears.
        let guardian = NodeId("guardian:0".into());
        graph.update_health(&guardian, HealthState::Degraded);
        let specialist = SecuritySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("SEC-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = security_graph();
        let specialist = SecuritySpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("guardian:"),
            vec![NodeId("guardian:0".into())]
        );
        assert_eq!(specialist.resolve_target("all").len(), 3);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = security_graph();
        let specialist = SecuritySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}
