//! M7: Boot and recovery specialist. Owns the boot and recovery
//! domain: boot state, recovery images, snapshots, and recovery
//! paths. Per `docs/modules/boot-recovery.md`, v0.1 is read-only.
//! `observe_boot` and `diagnose_fault` are bounded read-only tools;
//! boot-level mutating operations (A/B image management, watchdogs)
//! are deferred to v0.2+ per ADR-0001.
//!
//! Unlike the hardware umbrellas, the boot domain is the trust
//! plane — boot images, snapshots, and watchdogs — which is seeded
//! by the coordinator rather than discovered from sysfs. The
//! specialist owns those nodes via `owns` edges. Invariants BOOT-001
//! (a known-good recovery image is available) and BOOT-002 (the
//! boot chain is not modified) are evaluated from graph evidence;
//! missing evidence is reported as unknown, never as healthy.
//! BOOT-002 belongs to the mutation pass.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata,
    NodeType, SystemGraph, TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "boot.specialist";

/// The boot and recovery domain: the umbrella specialist and the
/// boot/recovery resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootRecoverySpecialist {
    pub specialist: NodeId,
    /// Boot/recovery nodes (`bootimage:*`, `snapshot:*`,
    /// `watchdog:*`).
    pub boot_nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootRecoveryHealth {
    /// Boot/recovery nodes reporting Healthy (BOOT-001 evidence:
    /// recovery image available).
    pub healthy_nodes: usize,
    pub boot_nodes: usize,
    /// Boot/recovery nodes not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootRecoveryError {
    NoBootResources,
    Graph(String),
}

impl std::fmt::Display for BootRecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBootResources => {
                f.write_str("no boot/recovery resources were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate boot/recovery specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for BootRecoveryError {}

/// Boot/recovery nodes: BootImage, Snapshot, and Watchdog nodes.
fn is_boot_node(node: &NodeMetadata) -> bool {
    matches!(
        node.node_type,
        NodeType::BootImage | NodeType::Snapshot | NodeType::Watchdog
    )
}

impl BootRecoverySpecialist {
    /// Deterministically resolve the boot/recovery domain: every
    /// boot/recovery node in the graph. The list is sorted by id so
    /// the result is stable across boots. An empty domain is an error
    /// (fail closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, BootRecoveryError> {
        let mut boot_nodes: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_boot_node(node))
            .map(|node| node.node_id.clone())
            .collect();
        boot_nodes.sort_by(|a, b| a.0.cmp(&b.0));
        if boot_nodes.is_empty() {
            return Err(BootRecoveryError::NoBootResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:boot:0".into()),
            boot_nodes,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, BootRecoveryError> {
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
        node.label = "Boot and recovery specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(BootRecoveryError::Graph)?;
        for resource in specialist.boot_nodes.iter() {
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
    ) -> Result<(), BootRecoveryError> {
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
            .map_err(BootRecoveryError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("boot:domain".into());
        vec![
            tool(
                "observe_boot",
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

    /// Cross-layer health: BOOT-001 evidence is a boot/recovery node
    /// reporting Healthy, plus the boot node count for the domain.
    pub fn health(&self, graph: &SystemGraph) -> BootRecoveryHealth {
        let healthy_nodes = self
            .boot_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health == HealthState::Healthy)
                    .unwrap_or(false)
            })
            .count();
        let degraded = self
            .boot_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        BootRecoveryHealth {
            healthy_nodes,
            boot_nodes: self.boot_nodes.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `bootimage:`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self.boot_nodes.clone();
        }
        let mut matched: Vec<NodeId> = self
            .boot_nodes
            .iter()
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: boot state and recovery-image
    /// availability for the target resources
    /// (docs/modules/boot-recovery.md). Domain-wide when the
    /// target is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("boot_nodes".into(), health.boot_nodes.to_string());
        metrics.insert(
            "healthy_nodes".into(),
            health.healthy_nodes.to_string(),
        );
        metrics.insert("degraded".into(), health.degraded.to_string());
        metrics.insert(
            "resources".into(),
            resources.iter().map(|id| id.0.clone()).collect::<Vec<_>>().join(","),
        );
        for id in resources.iter().take(8) {
            if let Some(node) = graph.get_node(id) {
                metrics.insert(format!("state:{id}"), format!("{:?}", node.health));
                if let Some(label) = node.attributes.get("label") {
                    metrics.insert(format!("label:{id}"), label.clone());
                }
                if let Some(kind) = node.attributes.get("kind") {
                    metrics.insert(format!("kind:{id}"), kind.clone());
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

    /// Bounded diagnose tool: compare observations with the boot
    /// invariants (BOOT-001; BOOT-002 belongs to the mutation
    /// pass). Missing evidence is reported as unknown findings,
    /// never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.healthy_nodes < health.boot_nodes {
            findings.push(format!(
                "BOOT-001: {} of {} boot/recovery nodes are not healthy (recovery image may be unavailable)",
                health.boot_nodes - health.healthy_nodes,
                health.boot_nodes
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} boot/recovery resources report non-healthy state",
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
        tool_id: format!("boot.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Boot and recovery specialist {name}"),
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

    fn boot_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut boot_img = node("bootimage:primary", NodeType::BootImage);
        boot_img.label = "primary boot image".into();
        boot_img.attributes.insert("kind".into(), "A".into());
        graph.add_node(boot_img).unwrap();
        let mut snapshot = node("snapshot:pre-update", NodeType::Snapshot);
        snapshot.label = "pre-update snapshot".into();
        snapshot.attributes.insert("kind".into(), "rollback".into());
        graph.add_node(snapshot).unwrap();
        let mut watchdog = node("watchdog:0", NodeType::Watchdog);
        watchdog.label = "boot watchdog".into();
        watchdog.attributes.insert("kind".into(), "reboot".into());
        graph.add_node(watchdog).unwrap();
        graph
    }

    #[test]
    fn discovers_boot_nodes() {
        let graph = boot_graph();
        let specialist = BootRecoverySpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.boot_nodes,
            vec![
                NodeId("bootimage:primary".into()),
                NodeId("snapshot:pre-update".into()),
                NodeId("watchdog:0".into()),
            ]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = boot_graph();
        let specialist = BootRecoverySpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(graph.get_edges(&specialist.specialist, EdgeType::Owns).len(), 3);
        for resource in specialist.boot_nodes.iter() {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
    }

    #[test]
    fn fails_closed_without_boot_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            BootRecoverySpecialist::discover(&graph),
            Err(BootRecoveryError::NoBootResources)
        );
        assert!(BootRecoverySpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = boot_graph();
        let specialist = BootRecoverySpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["boot.observe_boot", "boot.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_healthy_nodes() {
        let mut graph = boot_graph();
        let specialist = BootRecoverySpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.boot_nodes, 3);
        assert_eq!(health.healthy_nodes, 3);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = boot_graph();
        let specialist = BootRecoverySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("boot_nodes").unwrap(), "3");
        assert_eq!(metrics.get("healthy_nodes").unwrap(), "3");
        assert_eq!(metrics.get("degraded").unwrap(), "0");
    }

    #[test]
    fn diagnose_flags_unhealthy_nodes() {
        let mut graph = boot_graph();
        // Mark one boot node as unhealthy: BOOT-001 evidence weakens.
        let boot_node = NodeId("bootimage:primary".into());
        let mut node = graph.get_node(&boot_node).unwrap().clone();
        node.health = HealthState::Degraded;
        graph.upsert_node(node);
        let specialist = BootRecoverySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("BOOT-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = boot_graph();
        let specialist = BootRecoverySpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("bootimage:"),
            vec![NodeId("bootimage:primary".into())]
        );
        assert_eq!(specialist.resolve_target("all").len(), 3);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = boot_graph();
        let specialist = BootRecoverySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}