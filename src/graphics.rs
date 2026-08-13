//! M7: Graphics specialist (umbrella). Owns the graphics and session domain:
//! GPUs (`Device` nodes with display-controller class or a GPU label),
//! displays and the display service, and user/desktop sessions. Per
//! `docs/modules/graphics.md`, v0.1 is read-only: bounded Observe and
//! Diagnose tools only. Mutating operations (display configuration, GPU
//! reset) are deferred and will pass through the staged executor and
//! Guardian.
//!
//! The children form a stack (session runs on a display, which renders on a
//! GPU — architecture §6); ownership is still per-resource with exactly one
//! owner. This specialist owns the *hardware* (GPU, display, session) and is
//! separate from the Aios UI itself (docs/ui.md).

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "graphics.specialist";

/// The graphics domain: the umbrella specialist and the resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphicsSpecialist {
    pub specialist: NodeId,
    pub gpus: Vec<NodeId>,
    pub displays: Vec<NodeId>,
    pub sessions: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphicsHealth {
    /// GPUs reporting Healthy (GFX-001 evidence: present and reporting
    /// state).
    pub gpus_with_state: usize,
    pub gpus: usize,
    pub displays: usize,
    pub sessions: usize,
    /// Domain resources not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphicsError {
    NoGraphicsResources,
    Graph(String),
}

impl std::fmt::Display for GraphicsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGraphicsResources => {
                f.write_str("no GPUs, displays, or sessions were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate graphics specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for GraphicsError {}

/// GPUs are `Device` nodes classified structurally (docs/modules/gpu.md):
/// PCI display-controller class `0x03` (VGA, 3D) or a self-identifying GPU.
fn is_gpu(node: &NodeMetadata) -> bool {
    let gpu_class = node
        .attributes
        .get("class")
        .map(|value| value.to_lowercase().starts_with("0x03"))
        .unwrap_or(false);
    let text = format!("{} {}", node.node_id, node.label).to_lowercase();
    gpu_class || text.contains("vga") || text.contains("gpu") || text.contains("3d controller")
}

/// Displays: the display service and monitor state (docs/modules/display.md).
fn is_display(node: &NodeMetadata) -> bool {
    let text = format!("{} {}", node.node_id, node.label).to_lowercase();
    text.contains("display") || text.contains("drm")
}

/// Sessions: user/desktop sessions (docs/modules/session.md).
fn is_session(node: &NodeMetadata) -> bool {
    let text = format!("{} {}", node.node_id, node.label).to_lowercase();
    text.contains("session")
}

impl GraphicsSpecialist {
    /// Deterministically resolve the graphics domain: every GPU, display, and
    /// session in the graph. Lists are sorted by id so the result is stable
    /// across boots. An empty domain is an error (fail closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, GraphicsError> {
        let mut gpus: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| node.node_type == NodeType::Device && is_gpu(node))
            .map(|node| node.node_id.clone())
            .collect();
        let mut displays: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| node.node_type == NodeType::Service && is_display(node))
            .map(|node| node.node_id.clone())
            .collect();
        let mut sessions: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| node.node_type == NodeType::Service && is_session(node))
            .map(|node| node.node_id.clone())
            .collect();
        gpus.sort_by(|a, b| a.0.cmp(&b.0));
        displays.sort_by(|a, b| a.0.cmp(&b.0));
        sessions.sort_by(|a, b| a.0.cmp(&b.0));
        if gpus.is_empty() && displays.is_empty() && sessions.is_empty() {
            return Err(GraphicsError::NoGraphicsResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:graphics:0".into()),
            gpus,
            displays,
            sessions,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, GraphicsError> {
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
        node.label = "Graphics specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(GraphicsError::Graph)?;
        for resource in specialist
            .gpus
            .iter()
            .chain(specialist.displays.iter())
            .chain(specialist.sessions.iter())
        {
            // One-owner rule (architecture §5): the drivers peer never claims
            // GPU-class devices, but if another specialist somehow owns the
            // resource first, the umbrella does not take it over.
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
    ) -> Result<(), GraphicsError> {
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
            .map_err(GraphicsError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("graphics:domain".into());
        vec![
            tool(
                "observe_graphics",
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

    /// Cross-layer health: GFX-001 evidence is a GPU reporting state (not
    /// Unknown), plus GPU, display, and session counts for the domain.
    pub fn health(&self, graph: &SystemGraph) -> GraphicsHealth {
        let gpus_with_state = self
            .gpus
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Unknown)
                    .unwrap_or(false)
            })
            .count();
        let degraded = self
            .gpus
            .iter()
            .chain(self.displays.iter())
            .chain(self.sessions.iter())
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        GraphicsHealth {
            gpus_with_state,
            gpus: self.gpus.len(),
            displays: self.displays.len(),
            sessions: self.sessions.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `device:pci-`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self
                .gpus
                .iter()
                .chain(self.displays.iter())
                .chain(self.sessions.iter())
                .cloned()
                .collect();
        }
        let mut matched: Vec<NodeId> = self
            .gpus
            .iter()
            .chain(self.displays.iter())
            .chain(self.sessions.iter())
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: GPU, display, and session state for the target
    /// resources (docs/modules/graphics.md). Domain-wide when the target is
    /// empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("gpus".into(), health.gpus.to_string());
        metrics.insert("gpus_with_state".into(), health.gpus_with_state.to_string());
        metrics.insert("displays".into(), health.displays.to_string());
        metrics.insert("sessions".into(), health.sessions.to_string());
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

    /// Bounded diagnose tool: compare observations with the graphics
    /// invariants (GFX-001; GFX-002 belongs to the mutation pass). Missing
    /// evidence is reported as unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.gpus_with_state < health.gpus {
            findings.push(format!(
                "GFX-001: {} of {} GPUs report no state evidence",
                health.gpus - health.gpus_with_state,
                health.gpus
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} graphics or session resources report non-healthy state",
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
        tool_id: format!("graphics.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Graphics specialist {name}"),
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

    fn graphics_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut gpu = node("device:pci-0000:00:02.0", NodeType::Device);
        gpu.label = "PCI device 0000:00:02.0".into();
        gpu.attributes.insert("class".into(), "0x030000".into());
        graph.add_node(gpu).unwrap();
        let mut sata = node("device:pci-0000:01:00.0", NodeType::Device);
        sata.label = "SATA controller".into();
        sata.attributes.insert("class".into(), "0x010601".into());
        graph.add_node(sata).unwrap();
        let mut display = node("service:display-manager", NodeType::Service);
        display.label = "display manager (active)".into();
        graph.add_node(display).unwrap();
        let mut session = node("service:systemd-user-sessions", NodeType::Service);
        session.label = "Permit User Sessions (active)".into();
        graph.add_node(session).unwrap();
        graph
    }

    #[test]
    fn discovers_gpu_display_and_session() {
        let graph = graphics_graph();
        let specialist = GraphicsSpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.gpus,
            vec![NodeId("device:pci-0000:00:02.0".into())]
        );
        assert_eq!(
            specialist.displays,
            vec![NodeId("service:display-manager".into())]
        );
        assert_eq!(
            specialist.sessions,
            vec![NodeId("service:systemd-user-sessions".into())]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = graphics_graph();
        let specialist = GraphicsSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(graph.get_edges(&specialist.specialist, EdgeType::Owns).len(), 3);
        for resource in specialist
            .gpus
            .iter()
            .chain(specialist.displays.iter())
            .chain(specialist.sessions.iter())
        {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
    }

    #[test]
    fn fails_closed_without_graphics_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            GraphicsSpecialist::discover(&graph),
            Err(GraphicsError::NoGraphicsResources)
        );
        assert!(GraphicsSpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = graphics_graph();
        let specialist = GraphicsSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["graphics.observe_graphics", "graphics.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_state_evidence() {
        let mut graph = graphics_graph();
        let specialist = GraphicsSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.gpus, 1);
        assert_eq!(health.gpus_with_state, 1);
        assert_eq!(health.displays, 1);
        assert_eq!(health.sessions, 1);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = graphics_graph();
        let specialist = GraphicsSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("gpus").unwrap(), "1");
        assert_eq!(metrics.get("gpus_with_state").unwrap(), "1");
        assert_eq!(metrics.get("displays").unwrap(), "1");
        assert_eq!(metrics.get("sessions").unwrap(), "1");
    }

    #[test]
    fn diagnose_flags_missing_gpu_state() {
        let mut graph = graphics_graph();
        let gpu = NodeId("device:pci-0000:00:02.0".into());
        graph.update_health(&gpu, HealthState::Unknown);
        let specialist = GraphicsSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("GFX-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = graphics_graph();
        let specialist = GraphicsSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("service:"),
            vec![
                NodeId("service:display-manager".into()),
                NodeId("service:systemd-user-sessions".into())
            ]
        );
        assert_eq!(specialist.resolve_target("all").len(), 3);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = graphics_graph();
        let specialist = GraphicsSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}