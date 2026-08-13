//! M7: Packages and updates specialist. Owns the package and update
//! domain: installed packages, their versions and signatures, and
//! update state. Per `docs/modules/packages.md`, v0.1 is read-only
//! with bounded Observe and Diagnose tools. Mutating operations
//! (stage_update, request_rollback) are deferred to the mutation
//! pass and will pass through the staged executor and Guardian.
//!
//! Discovery represents package resources as `package:<name>` nodes
//! (NodeType::Package) with `version`, `signature`, and `state`
//! attributes (src/discovery.rs `discover_packages`). The specialist
//! owns those nodes via `owns` edges. Invariants PKG-001 (packages
//! are present, signed, and versioned) and PKG-002 (an update does
//! not silently broaden an agent's capabilities) are evaluated from
//! graph evidence; missing evidence is reported as unknown, never as
//! healthy. PKG-002 belongs to the mutation pass.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType,
    SystemGraph, TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "packages.specialist";

/// The package domain: the umbrella specialist and the package
/// resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackagesSpecialist {
    pub specialist: NodeId,
    /// Package nodes (`package:<name>`, ...).
    pub package_nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackagesHealth {
    /// Package nodes reporting a `signature` attribute (PKG-001
    /// evidence: present and signed).
    pub nodes_signed: usize,
    pub package_nodes: usize,
    /// Package nodes not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackagesError {
    NoPackageResources,
    Graph(String),
}

impl std::fmt::Display for PackagesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPackageResources => {
                f.write_str("no package resources were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate packages specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for PackagesError {}

/// Package nodes are `NodeType::Package` (discovery creates
/// `package:<name>`).
fn is_package_node(node: &NodeMetadata) -> bool {
    node.node_type == NodeType::Package
}

impl PackagesSpecialist {
    /// Deterministically resolve the package domain: every package
    /// node in the graph. The list is sorted by id so the result is
    /// stable across boots. An empty domain is an error (fail closed,
    /// read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, PackagesError> {
        let mut package_nodes: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_package_node(node))
            .map(|node| node.node_id.clone())
            .collect();
        package_nodes.sort_by(|a, b| a.0.cmp(&b.0));
        if package_nodes.is_empty() {
            return Err(PackagesError::NoPackageResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:packages:0".into()),
            package_nodes,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, PackagesError> {
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
        node.label = "Packages and updates specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(PackagesError::Graph)?;
        for resource in specialist.package_nodes.iter() {
            // One-owner rule (architecture §5): if another specialist
            // somehow owns the resource first, the umbrella does not
            // take it over.
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
    ) -> Result<(), PackagesError> {
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
            .map_err(PackagesError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("packages:domain".into());
        vec![
            tool(
                "observe_package",
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

    /// Cross-layer health: PKG-001 evidence is a package node
    /// reporting a signature (present and signed), plus the package
    /// count for the domain.
    pub fn health(&self, graph: &SystemGraph) -> PackagesHealth {
        let nodes_signed = self
            .package_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .is_some_and(|node| node.attributes.contains_key("signature"))
            })
            .count();
        let degraded = self
            .package_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        PackagesHealth {
            nodes_signed,
            package_nodes: self.package_nodes.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `package:`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self.package_nodes.clone();
        }
        let mut matched: Vec<NodeId> = self
            .package_nodes
            .iter()
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: package, version, and signature state for
    /// the target resources (docs/modules/packages.md). Domain-wide when the
    /// target is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("package_nodes".into(), health.package_nodes.to_string());
        metrics.insert(
            "nodes_signed".into(),
            health.nodes_signed.to_string(),
        );
        metrics.insert("degraded".into(), health.degraded.to_string());
        metrics.insert(
            "resources".into(),
            resources.iter().map(|id| id.0.clone()).collect::<Vec<_>>().join(","),
        );
        for id in resources.iter().take(8) {
            if let Some(node) = graph.get_node(id) {
                metrics.insert(format!("state:{id}"), format!("{:?}", node.health));
                if let Some(version) = node.attributes.get("version") {
                    metrics.insert(format!("version:{id}"), version.clone());
                }
                if let Some(signature) = node.attributes.get("signature") {
                    metrics.insert(format!("signature:{id}"), signature.clone());
                }
                if let Some(state) = node.attributes.get("state") {
                    metrics.insert(format!("state_attr:{id}"), state.clone());
                }
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

    /// Bounded diagnose tool: compare observations with the package
    /// invariants (PKG-001; PKG-002 belongs to the mutation pass).
    /// Missing evidence is reported as unknown findings, never as
    /// healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.nodes_signed < health.package_nodes {
            findings.push(format!(
                "PKG-001: {} of {} package nodes lack signature evidence (present but not confirmed signed)",
                health.package_nodes - health.nodes_signed,
                health.package_nodes
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} package resources report non-healthy state",
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
        tool_id: format!("packages.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Packages specialist {name}"),
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

    fn package_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut kernel = node("package:linux-kernel", NodeType::Package);
        kernel.label = "linux-kernel".into();
        kernel.attributes.insert("version".into(), "6.1.0".into());
        kernel.attributes.insert("signature".into(), "sha256:abc123".into());
        kernel.attributes.insert("state".into(), "installed".into());
        graph.add_node(kernel).unwrap();
        let mut openssh = node("package:openssh", NodeType::Package);
        openssh.label = "openssh".into();
        openssh.attributes.insert("version".into(), "9.3p1".into());
        openssh.attributes.insert("signature".into(), "sha256:def456".into());
        openssh.attributes.insert("state".into(), "installed".into());
        graph.add_node(openssh).unwrap();
        graph
    }

    #[test]
    fn discovers_package_nodes() {
        let graph = package_graph();
        let specialist = PackagesSpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.package_nodes,
            vec![NodeId("package:linux-kernel".into()), NodeId("package:openssh".into())]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = package_graph();
        let specialist = PackagesSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(graph.get_edges(&specialist.specialist, EdgeType::Owns).len(), 2);
        for resource in specialist.package_nodes.iter() {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
    }

    #[test]
    fn fails_closed_without_package_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            PackagesSpecialist::discover(&graph),
            Err(PackagesError::NoPackageResources)
        );
        assert!(PackagesSpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = package_graph();
        let specialist = PackagesSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["packages.observe_package", "packages.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_signature_evidence() {
        let mut graph = package_graph();
        let specialist = PackagesSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.package_nodes, 2);
        assert_eq!(health.nodes_signed, 2);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = package_graph();
        let specialist = PackagesSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("package_nodes").unwrap(), "2");
        assert_eq!(metrics.get("nodes_signed").unwrap(), "2");
        assert_eq!(metrics.get("degraded").unwrap(), "0");
    }

    #[test]
    fn diagnose_flags_missing_signature_evidence() {
        let mut graph = package_graph();
        // Remove the signature attribute from one package node: PKG-001
        // evidence disappears.
        let package = NodeId("package:linux-kernel".into());
        let mut node = graph.get_node(&package).unwrap().clone();
        node.attributes.remove("signature");
        graph.upsert_node(node);
        let specialist = PackagesSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("PKG-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = package_graph();
        let specialist = PackagesSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("package:"),
            vec![NodeId("package:linux-kernel".into()), NodeId("package:openssh".into())]
        );
        assert_eq!(specialist.resolve_target("all").len(), 2);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = package_graph();
        let specialist = PackagesSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}