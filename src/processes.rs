//! M7: Processes and resources specialist (umbrella). Owns the system's
//! process and resource domain: running processes, namespaces, and their
//! resource usage (CPU, memory). Per `docs/modules/processes.md`, v0.1 is
//! read-only: bounded Observe and Diagnose tools only. Mutating operations
//! (stopping a process, adjusting a resource limit) are deferred and will
//! pass through the staged executor and Guardian. Resource budget enforcement
//! is advisory in v0.1 (REQ-PERF-002) and deferred to v0.2+ with process
//! isolation.
//!
//! Discovery represents the process domain as `process:<pid>` nodes
//! (NodeType::Process) with `pid`, `comm`, `state`, and `rss_kb` attributes
//! (src/discovery.rs `discover_processes`). The specialist owns those nodes
//! via `owns` edges. Invariants PROC-001 (processes are present and report
//! resource usage) is evaluated from graph evidence; missing evidence is
//! reported as unknown, never as healthy. PROC-002 (resource usage within
//! budgets) belongs to the mutation pass.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "processes.specialist";

/// The process domain: the umbrella specialist and the resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessesSpecialist {
    pub specialist: NodeId,
    /// Process nodes (`process:<pid>`, ...).
    pub process_nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessesHealth {
    /// Process nodes reporting a `rss_kb` attribute (PROC-001 evidence:
    /// present and reporting resource usage).
    pub nodes_with_usage: usize,
    pub process_nodes: usize,
    /// Process nodes not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessesError {
    NoProcessResources,
    Graph(String),
}

impl std::fmt::Display for ProcessesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoProcessResources => {
                f.write_str("no process resources were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate processes specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for ProcessesError {}

/// Process nodes are `NodeType::Process` (discovery creates `process:<pid>`).
fn is_process_node(node: &NodeMetadata) -> bool {
    node.node_type == NodeType::Process
}

impl ProcessesSpecialist {
    /// Deterministically resolve the process domain: every process node in the
    /// graph. The list is sorted by id so the result is stable across boots.
    /// An empty domain is an error (fail closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, ProcessesError> {
        let mut process_nodes: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_process_node(node))
            .map(|node| node.node_id.clone())
            .collect();
        process_nodes.sort_by(|a, b| a.0.cmp(&b.0));
        if process_nodes.is_empty() {
            return Err(ProcessesError::NoProcessResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:processes:0".into()),
            process_nodes,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, ProcessesError> {
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
        node.label = "Processes specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(ProcessesError::Graph)?;
        for resource in specialist.process_nodes.iter() {
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
    ) -> Result<(), ProcessesError> {
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
            .map_err(ProcessesError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("processes:domain".into());
        vec![
            tool(
                "observe_process",
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

    /// Cross-layer health: PROC-001 evidence is a process node reporting
    /// resource usage (a `rss_kb` attribute), plus the process count for the
    /// domain.
    pub fn health(&self, graph: &SystemGraph) -> ProcessesHealth {
        let nodes_with_usage = self
            .process_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .is_some_and(|node| node.attributes.contains_key("rss_kb"))
            })
            .count();
        let degraded = self
            .process_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        ProcessesHealth {
            nodes_with_usage,
            process_nodes: self.process_nodes.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `process:`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self.process_nodes.clone();
        }
        let mut matched: Vec<NodeId> = self
            .process_nodes
            .iter()
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: process, namespace, and resource-usage state for
    /// the target resources (docs/modules/processes.md). Domain-wide when the
    /// target is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("process_nodes".into(), health.process_nodes.to_string());
        metrics.insert(
            "nodes_with_usage".into(),
            health.nodes_with_usage.to_string(),
        );
        metrics.insert("degraded".into(), health.degraded.to_string());
        metrics.insert(
            "resources".into(),
            resources.iter().map(|id| id.0.clone()).collect::<Vec<_>>().join(","),
        );
        for id in resources.iter().take(8) {
            if let Some(node) = graph.get_node(id) {
                metrics.insert(format!("state:{id}"), format!("{:?}", node.health));
                if let Some(comm) = node.attributes.get("comm") {
                    metrics.insert(format!("comm:{id}"), comm.clone());
                }
                if let Some(rss) = node.attributes.get("rss_kb") {
                    metrics.insert(format!("rss_kb:{id}"), rss.clone());
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

    /// Bounded diagnose tool: compare observations with the process invariants
    /// (PROC-001; PROC-002 belongs to the mutation pass). Missing evidence is
    /// reported as unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.nodes_with_usage < health.process_nodes {
            findings.push(format!(
                "PROC-001: {} of {} process nodes lack resource-usage evidence (present but not confirmed reporting)",
                health.process_nodes - health.nodes_with_usage,
                health.process_nodes
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} process resources report non-healthy state",
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
        tool_id: format!("processes.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Processes specialist {name}"),
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

    fn process_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut aios = node("process:100", NodeType::Process);
        aios.label = "process 100 (aios)".into();
        aios.attributes.insert("pid".into(), "100".into());
        aios.attributes.insert("comm".into(), "aios".into());
        aios.attributes.insert("rss_kb".into(), "123456".into());
        graph.add_node(aios).unwrap();
        let mut systemd = node("process:1", NodeType::Process);
        systemd.label = "process 1 (systemd)".into();
        systemd.attributes.insert("pid".into(), "1".into());
        systemd.attributes.insert("comm".into(), "systemd".into());
        systemd.attributes.insert("rss_kb".into(), "8192".into());
        graph.add_node(systemd).unwrap();
        graph
    }

    #[test]
    fn discovers_process_nodes() {
        let graph = process_graph();
        let specialist = ProcessesSpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.process_nodes,
            vec![NodeId("process:1".into()), NodeId("process:100".into())]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = process_graph();
        let specialist = ProcessesSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(graph.get_edges(&specialist.specialist, EdgeType::Owns).len(), 2);
        for resource in specialist.process_nodes.iter() {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
    }

    #[test]
    fn fails_closed_without_process_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            ProcessesSpecialist::discover(&graph),
            Err(ProcessesError::NoProcessResources)
        );
        assert!(ProcessesSpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = process_graph();
        let specialist = ProcessesSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["processes.observe_process", "processes.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_usage_evidence() {
        let mut graph = process_graph();
        let specialist = ProcessesSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.process_nodes, 2);
        assert_eq!(health.nodes_with_usage, 2);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = process_graph();
        let specialist = ProcessesSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("process_nodes").unwrap(), "2");
        assert_eq!(metrics.get("nodes_with_usage").unwrap(), "2");
        assert_eq!(metrics.get("degraded").unwrap(), "0");
    }

    #[test]
    fn diagnose_flags_missing_usage_evidence() {
        let mut graph = process_graph();
        // Remove the rss_kb attribute from one process node: PROC-001
        // evidence disappears.
        let process = NodeId("process:1".into());
        let mut node = graph.get_node(&process).unwrap().clone();
        node.attributes.remove("rss_kb");
        graph.upsert_node(node);
        let specialist = ProcessesSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("PROC-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = process_graph();
        let specialist = ProcessesSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("process:"),
            vec![NodeId("process:1".into()), NodeId("process:100".into())]
        );
        assert_eq!(specialist.resolve_target("all").len(), 2);
        assert_eq!(
            specialist.resolve_target("process:"),
            vec![NodeId("process:1".into()), NodeId("process:100".into())]
        );
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = process_graph();
        let specialist = ProcessesSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}
