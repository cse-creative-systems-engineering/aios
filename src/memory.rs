//! M7: Memory specialist (umbrella). Owns the system's memory domain:
//! physical memory, swap, pressure, and ECC state. Per
//! `docs/modules/memory.md`, v0.1 is read-only: bounded Observe and Diagnose
//! tools only. Mutating operations (`stage_policy`, `request_reset`) are
//! deferred and will pass through the staged executor and Guardian.
//!
//! Discovery represents the memory domain as `memory:total` and
//! `memory:available` nodes (NodeType::Memory) with a `size_kb` attribute,
//! plus `memory:pressure` and `memory:vmstat` evidence nodes carrying the
//! full meminfo, PSI, and page counters (src/discovery.rs `discover_memory`).
//! The specialist owns those nodes via `owns` edges. Invariants MEMORY-001
//! (the memory subsystem is present and reports usable capacity) and
//! MEMORY-002 (ECC errors within threshold after a staged change) are
//! evaluated from graph evidence; missing evidence is reported as unknown,
//! never as healthy.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "memory.specialist";

/// The memory domain: the umbrella specialist and the resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemorySpecialist {
    pub specialist: NodeId,
    /// Memory nodes (`memory:total`, `memory:available`,
    /// `memory:pressure`, `memory:vmstat`, ...).
    pub memory_nodes: Vec<NodeId>,
    /// ECC sensors (`sensor:*` nodes reporting ECC/memory errors).
    pub ecc_sensors: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryHealth {
    /// Memory nodes reporting a `size_kb` attribute (MEMORY-001 evidence:
    /// present and reporting usable capacity).
    pub nodes_reporting_capacity: usize,
    pub memory_nodes: usize,
    pub ecc_sensors: usize,
    /// Memory nodes not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryError {
    NoMemoryResources,
    Graph(String),
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMemoryResources => {
                f.write_str("no memory resources were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate memory specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for MemoryError {}

/// Memory nodes are `NodeType::Memory` (discovery creates `memory:total` and
/// `memory:available`).
fn is_memory_node(node: &NodeMetadata) -> bool {
    node.node_type == NodeType::Memory
}

/// ECC sensors: `sensor:*` nodes whose id or label mentions memory/ECC.
fn is_ecc_sensor(node: &NodeMetadata) -> bool {
    if node.node_type != NodeType::Sensor {
        return false;
    }
    let text = format!("{} {}", node.node_id, node.label).to_lowercase();
    text.contains("ecc") || text.contains("memory") || text.contains("edac")
}

impl MemorySpecialist {
    /// Deterministically resolve the memory domain: every memory node and ECC
    /// sensor in the graph. Lists are sorted by id so the result is stable
    /// across boots. An empty domain is an error (fail closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, MemoryError> {
        let mut memory_nodes: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_memory_node(node))
            .map(|node| node.node_id.clone())
            .collect();
        let mut ecc_sensors: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_ecc_sensor(node))
            .map(|node| node.node_id.clone())
            .collect();
        memory_nodes.sort_by(|a, b| a.0.cmp(&b.0));
        ecc_sensors.sort_by(|a, b| a.0.cmp(&b.0));
        if memory_nodes.is_empty() && ecc_sensors.is_empty() {
            return Err(MemoryError::NoMemoryResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:memory:0".into()),
            memory_nodes,
            ecc_sensors,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, MemoryError> {
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
        node.label = "Memory specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(MemoryError::Graph)?;
        for resource in specialist
            .memory_nodes
            .iter()
            .chain(specialist.ecc_sensors.iter())
        {
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
    ) -> Result<(), MemoryError> {
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
            .map_err(MemoryError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("memory:domain".into());
        vec![
            tool(
                "observe_memory",
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

    /// Cross-layer health: MEMORY-001 evidence is a memory node reporting
    /// usable capacity (a `size_kb` attribute), plus memory and ECC-sensor
    /// counts for the domain.
    pub fn health(&self, graph: &SystemGraph) -> MemoryHealth {
        let nodes_reporting_capacity = self
            .memory_nodes
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .is_some_and(|node| node.attributes.contains_key("size_kb"))
            })
            .count();
        let degraded = self
            .memory_nodes
            .iter()
            .chain(self.ecc_sensors.iter())
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        MemoryHealth {
            nodes_reporting_capacity,
            memory_nodes: self.memory_nodes.len(),
            ecc_sensors: self.ecc_sensors.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `memory:`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self
                .memory_nodes
                .iter()
                .chain(self.ecc_sensors.iter())
                .cloned()
                .collect();
        }
        let mut matched: Vec<NodeId> = self
            .memory_nodes
            .iter()
            .chain(self.ecc_sensors.iter())
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: memory, swap, pressure, vmstat, and ECC state
    /// for the target resources (docs/modules/memory.md). Domain-wide when the
    /// target is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("memory_nodes".into(), health.memory_nodes.to_string());
        metrics.insert(
            "nodes_reporting_capacity".into(),
            health.nodes_reporting_capacity.to_string(),
        );
        metrics.insert("ecc_sensors".into(), health.ecc_sensors.to_string());
        metrics.insert("degraded".into(), health.degraded.to_string());

        let total = graph.get_node(&NodeId("memory:total".into()));
        if let Some(total) = &total {
            for (key, value) in &total.attributes {
                if let Some(rest) = key.strip_prefix("meminfo_") {
                    metrics.insert(format!("meminfo_{rest}"), value.clone());
                }
            }
        }
        let mem_kb = |node: &crate::graph::NodeMetadata, key: &str| -> Option<u64> {
            node.attributes.get(key)?.parse().ok()
        };
        if let Some(node) = &total {
            if let (Some(total_kb), Some(avail_kb)) = (
                mem_kb(node, "meminfo_memtotal"),
                mem_kb(node, "meminfo_memavailable"),
            ) {
                metrics.insert("total_kb".into(), total_kb.to_string());
                metrics.insert("available_kb".into(), avail_kb.to_string());
                metrics.insert("used_kb".into(), total_kb.saturating_sub(avail_kb).to_string());
            }
            if let Some(free_kb) = mem_kb(node, "meminfo_memfree") {
                metrics.insert("free_kb".into(), free_kb.to_string());
            }
            if let (Some(swap_total), Some(swap_free)) = (
                mem_kb(node, "meminfo_swaptotal"),
                mem_kb(node, "meminfo_swapfree"),
            ) {
                metrics.insert("swap_total_kb".into(), swap_total.to_string());
                metrics.insert("swap_free_kb".into(), swap_free.to_string());
                metrics.insert(
                    "swap_used_kb".into(),
                    swap_total.saturating_sub(swap_free).to_string(),
                );
            }
        }

        if let Some(pressure) = graph.get_node(&NodeId("memory:pressure".into())) {
            for (key, value) in &pressure.attributes {
                metrics.insert(format!("pressure_{key}"), value.clone());
            }
        }
        if let Some(vmstat) = graph.get_node(&NodeId("memory:vmstat".into())) {
            for (key, value) in &vmstat.attributes {
                metrics.insert(format!("vmstat_{key}"), value.clone());
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

    /// Bounded diagnose tool: compare observations with the memory invariants
    /// (MEMORY-001; MEMORY-002 belongs to the mutation pass). Missing evidence
    /// is reported as unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.nodes_reporting_capacity == 0 {
            findings.push(format!(
                "MEMORY-001: the memory domain is present but no node reports usable capacity ({} nodes)",
                health.memory_nodes
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} memory or ECC resources report non-healthy state",
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
        tool_id: format!("memory.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Memory specialist {name}"),
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

    fn memory_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut total = node("memory:total", NodeType::Memory);
        total.label = "total memory (16384000 kB)".into();
        total.attributes.insert("size_kb".into(), "16384000".into());
        graph.add_node(total).unwrap();
        let mut available = node("memory:available", NodeType::Memory);
        available.label = "available memory (8123456 kB)".into();
        available.attributes.insert("size_kb".into(), "8123456".into());
        graph.add_node(available).unwrap();
        let mut ecc = node("sensor:edac0-ecc", NodeType::Sensor);
        ecc.label = "edac memory ECC".into();
        graph.add_node(ecc).unwrap();
        graph
    }

    #[test]
    fn discovers_memory_nodes_and_ecc_sensors() {
        let graph = memory_graph();
        let specialist = MemorySpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.memory_nodes,
            vec![
                NodeId("memory:available".into()),
                NodeId("memory:total".into())
            ]
        );
        assert_eq!(
            specialist.ecc_sensors,
            vec![NodeId("sensor:edac0-ecc".into())]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = memory_graph();
        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(graph.get_edges(&specialist.specialist, EdgeType::Owns).len(), 3);
        for resource in specialist
            .memory_nodes
            .iter()
            .chain(specialist.ecc_sensors.iter())
        {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
    }

    #[test]
    fn fails_closed_without_memory_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            MemorySpecialist::discover(&graph),
            Err(MemoryError::NoMemoryResources)
        );
        assert!(MemorySpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = memory_graph();
        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["memory.observe_memory", "memory.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_capacity_evidence() {
        let mut graph = memory_graph();
        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.memory_nodes, 2);
        assert_eq!(health.nodes_reporting_capacity, 2);
        assert_eq!(health.ecc_sensors, 1);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = memory_graph();
        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("memory_nodes").unwrap(), "2");
        assert_eq!(metrics.get("nodes_reporting_capacity").unwrap(), "2");
        assert_eq!(metrics.get("ecc_sensors").unwrap(), "1");
        assert_eq!(metrics.get("degraded").unwrap(), "0");
    }

    #[test]
    fn observe_reports_meminfo_pressure_and_vmstat_metrics() {
        let mut graph = memory_graph();
        let total = NodeId("memory:total".into());
        let mut total_node = graph.get_node(&total).unwrap().clone();
        total_node.attributes.insert("meminfo_memtotal".into(), "16384000".into());
        total_node.attributes.insert("meminfo_memfree".into(), "10485760".into());
        total_node.attributes.insert("meminfo_memavailable".into(), "8123456".into());
        total_node.attributes.insert("meminfo_swaptotal".into(), "2097152".into());
        total_node.attributes.insert("meminfo_swapfree".into(), "1900544".into());
        graph.upsert_node(total_node);
        let mut pressure = node("memory:pressure", NodeType::Memory);
        pressure.attributes.insert("some_avg10".into(), "0.00".into());
        pressure.attributes.insert("full_total".into(), "0".into());
        graph.add_node(pressure).unwrap();
        let mut vmstat = node("memory:vmstat", NodeType::Memory);
        vmstat.attributes.insert("oom_kill".into(), "0".into());
        vmstat.attributes.insert("pswpin".into(), "12".into());
        graph.add_node(vmstat).unwrap();

        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("memory_nodes").unwrap(), "4");
        assert_eq!(metrics.get("total_kb").unwrap(), "16384000");
        assert_eq!(metrics.get("available_kb").unwrap(), "8123456");
        assert_eq!(metrics.get("used_kb").unwrap(), "8260544");
        assert_eq!(metrics.get("free_kb").unwrap(), "10485760");
        assert_eq!(metrics.get("swap_total_kb").unwrap(), "2097152");
        assert_eq!(metrics.get("swap_used_kb").unwrap(), "196608");
        assert_eq!(metrics.get("meminfo_memtotal").unwrap(), "16384000");
        assert_eq!(metrics.get("pressure_some_avg10").unwrap(), "0.00");
        assert_eq!(metrics.get("pressure_full_total").unwrap(), "0");
        assert_eq!(metrics.get("vmstat_oom_kill").unwrap(), "0");
        assert_eq!(metrics.get("vmstat_pswpin").unwrap(), "12");
    }

    #[test]
    fn observe_implements_the_memory_tool_claim() {
        let mut graph = memory_graph();
        let total = NodeId("memory:total".into());
        let mut total_node = graph.get_node(&total).unwrap().clone();
        total_node.attributes.insert("meminfo_memtotal".into(), "16384000".into());
        total_node.attributes.insert("meminfo_memfree".into(), "10485760".into());
        total_node.attributes.insert("meminfo_memavailable".into(), "8123456".into());
        total_node.attributes.insert("meminfo_cached".into(), "3145728".into());
        total_node.attributes.insert("meminfo_swaptotal".into(), "2097152".into());
        total_node.attributes.insert("meminfo_swapfree".into(), "1900544".into());
        graph.upsert_node(total_node);
        let mut pressure = node("memory:pressure", NodeType::Memory);
        pressure.attributes.insert("some_avg10".into(), "0.00".into());
        graph.add_node(pressure).unwrap();
        let mut vmstat = node("memory:vmstat", NodeType::Memory);
        vmstat.attributes.insert("pgfault".into(), "2345678".into());
        vmstat.attributes.insert("oom_kill".into(), "0".into());
        graph.add_node(vmstat).unwrap();

        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        let claim = crate::tools::MEMORY_TOOL_CLAIM;
        for (capability, metric) in [
            ("total", "total_kb"),
            ("available", "available_kb"),
            ("used", "used_kb"),
            ("free", "free_kb"),
            ("cached", "meminfo_cached"),
            ("swap", "swap_total_kb"),
            ("pressure", "pressure_some_avg10"),
            ("page fault", "vmstat_pgfault"),
            ("oom", "vmstat_oom_kill"),
        ] {
            assert!(
                claim.contains(capability),
                "tool claim must mention {capability}: {claim}"
            );
            assert!(
                metrics.contains_key(metric),
                "claim advertises {capability} but observe emits no {metric}"
            );
        }
    }

    #[test]
    fn diagnose_flags_missing_capacity_evidence() {
        let mut graph = memory_graph();
        // Remove the size_kb attribute from every memory node: MEMORY-001
        // evidence disappears.
        for id in [
            NodeId("memory:total".into()),
            NodeId("memory:available".into()),
        ] {
            let mut node = graph.get_node(&id).unwrap().clone();
            node.attributes.remove("size_kb");
            graph.upsert_node(node);
        }
        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("MEMORY-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = memory_graph();
        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("memory:"),
            vec![
                NodeId("memory:available".into()),
                NodeId("memory:total".into())
            ]
        );
        assert_eq!(specialist.resolve_target("all").len(), 3);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = memory_graph();
        let specialist = MemorySpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}
