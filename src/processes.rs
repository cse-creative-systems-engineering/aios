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
//! (NodeType::Process) with `pid`, `comm`, `state`, `rss_kb`,
//! `cpu_utime_ticks`, `cpu_stime_ticks`, and `cmdline` attributes
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

fn cpu_sample() -> Option<(u64, u64)> {
    let contents = std::fs::read_to_string("/proc/stat").ok()?;
    let line = contents.lines().find(|line| line.starts_with("cpu "))?;
    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse().ok())
        .collect();
    let total = values.iter().sum();
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    Some((total, idle))
}

fn cpu_cores() -> Option<usize> {
    let contents = std::fs::read_to_string("/proc/stat").ok()?;
    Some(
        contents
            .lines()
            .filter(|line| line.starts_with("cpu") && !line.starts_with("cpu "))
            .count(),
    )
}

/// Parse the utime/stime tick counters from a `/proc/<pid>/stat` line. Fields
/// after the `)` that closes the comm field: state ppid pgrp session tty_nr
/// tpgid flags minflt cminflt majflt cmajflt utime stime.
fn parse_ticks(stat: &str) -> Option<(u64, u64)> {
    let open = stat.rfind(')')?;
    let rest: Vec<&str> = stat[open + 1..].split_whitespace().collect();
    if rest.len() < 13 {
        return None;
    }
    Some((rest[11].parse().ok()?, rest[12].parse().ok()?))
}

fn process_cpu_sample(pid: u32) -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_ticks(&stat)
}

/// A windowed view of system and per-process CPU ticks.
struct TickSnapshot {
    system_total: u64,
    system_idle: u64,
    cores: usize,
    processes: HashMap<String, (u32, u64)>,
}

fn tick_snapshot(resources: &[NodeId]) -> Option<TickSnapshot> {
    let (system_total, system_idle) = cpu_sample()?;
    let cores = cpu_cores().unwrap_or(1);
    let mut processes = HashMap::new();
    for id in resources {
        let Some(pid) = id.0.strip_prefix("process:").and_then(|s| s.parse().ok()) else {
            continue;
        };
        if let Some((utime, stime)) = process_cpu_sample(pid) {
            processes.insert(id.0.clone(), (pid, utime + stime));
        }
    }
    Some(TickSnapshot {
        system_total,
        system_idle,
        cores,
        processes,
    })
}

/// A windowed CPU sample: system utilization, core count, and per-process
/// CPU percentages keyed by node id as `(pid, cpu_percent)`.
struct CpuStats {
    utilization_percent: Option<f64>,
    cores: Option<usize>,
    per_process: HashMap<String, (u32, f64)>,
}

/// Sample system and per-process CPU ticks twice across a short window. A
/// process percent is its share of the windowed tick delta scaled to the
/// core count, so a process pegging one core reads ~100.
fn cpu_stats(resources: &[NodeId]) -> CpuStats {
    let first = tick_snapshot(resources);
    std::thread::sleep(std::time::Duration::from_millis(100));
    let second = tick_snapshot(resources);
    let (Some(first), Some(second)) = (first, second) else {
        return CpuStats {
            utilization_percent: None,
            cores: None,
            per_process: HashMap::new(),
        };
    };
    let Some(total_delta) = second.system_total.checked_sub(first.system_total) else {
        return CpuStats {
            utilization_percent: None,
            cores: None,
            per_process: HashMap::new(),
        };
    };
    let idle_delta = second.system_idle.saturating_sub(first.system_idle);
    let utilization_percent = if total_delta == 0 || idle_delta > total_delta {
        None
    } else {
        Some((total_delta - idle_delta) as f64 * 100.0 / total_delta as f64)
    };
    let cores = first.cores;
    let mut per_process = HashMap::new();
    if total_delta > 0 {
        for (node_id, (pid, start)) in first.processes {
            let Some((_, end)) = second.processes.get(&node_id) else {
                continue;
            };
            let delta = end.saturating_sub(start);
            let percent = delta as f64 * cores as f64 * 100.0 / total_delta as f64;
            per_process.insert(node_id, (pid, percent));
        }
    }
    CpuStats {
        utilization_percent,
        cores: Some(cores),
        per_process,
    }
}

fn format_process_row(node: &NodeMetadata, cpu_percent: f64) -> String {
    let get = |key: &str| node.attributes.get(key).cloned().unwrap_or_default();
    format!(
        "pid={} comm={} cpu_percent={cpu_percent:.1} rss_kb={} state={} cmdline={}",
        get("pid"),
        get("comm"),
        get("rss_kb"),
        get("state"),
        get("cmdline")
    )
}

/// The domain processes specialist `observe` reports at most this many rows
/// when the target covers the whole domain.
const TOP_PROCESSES: usize = 10;

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
            Self::NoProcessResources => f.write_str("no process resources were discovered"),
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
    /// target is empty or `all`, reported as the top processes by CPU.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let stats = cpu_stats(&resources);
        let mut metrics = HashMap::new();
        metrics.insert("process_nodes".into(), health.process_nodes.to_string());
        metrics.insert(
            "processes_reporting_rss".into(),
            health.nodes_with_usage.to_string(),
        );
        metrics.insert("degraded".into(), health.degraded.to_string());
        if let Some(cores) = stats.cores {
            metrics.insert("cpu_cores".into(), cores.to_string());
        }
        if let Some(utilization) = stats.utilization_percent {
            metrics.insert(
                "cpu_utilization_percent".into(),
                format!("{utilization:.1}"),
            );
        }
        let is_domain = target.trim().is_empty() || target.trim() == "all";
        if is_domain {
            let mut rows: Vec<(String, f64)> = Vec::new();
            for id in resources.iter() {
                let Some((_, percent)) = stats.per_process.get(&id.0) else {
                    continue;
                };
                let Some(node) = graph.get_node(id) else {
                    continue;
                };
                rows.push((format_process_row(&node, *percent), *percent));
            }
            rows.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (i, (row, _)) in rows.iter().take(TOP_PROCESSES).enumerate() {
                metrics.insert(format!("top_cpu_{i}"), row.clone());
            }
        } else {
            for id in resources.iter() {
                let Some((_, percent)) = stats.per_process.get(&id.0) else {
                    continue;
                };
                let Some(node) = graph.get_node(id) else {
                    continue;
                };
                metrics.insert(
                    id.0.clone(),
                    format_process_row(&node, *percent),
                );
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
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            },
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

fn ok(
    data: crate::protocol::ToolData,
    error: Option<crate::protocol::ToolError>,
) -> crate::protocol::ToolResult {
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
            ProvenanceSource::Discovered { via: "test".into() },
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
        assert_eq!(
            graph
                .get_edges(&specialist.specialist, EdgeType::Owns)
                .len(),
            2
        );
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
        assert_eq!(metrics.get("processes_reporting_rss").unwrap(), "2");
        assert_eq!(metrics.get("degraded").unwrap(), "0");
    }

    #[test]
    fn parse_ticks_reads_utime_and_stime() {
        let stat = "123 (aios worker) S 1 2 3 4 5 6 7 8 9 10 111 222 333 444";
        assert_eq!(parse_ticks(stat), Some((111, 222)));
        let short = "123 (aios) S 1 2 3 4 5 6 7";
        assert_eq!(parse_ticks(short), None);
        assert_eq!(parse_ticks("no closing paren"), None);
    }

    #[test]
    fn format_process_row_orders_fields_with_cpu_percent() {
        let mut node = node("process:100", NodeType::Process);
        node.attributes.insert("pid".into(), "100".into());
        node.attributes.insert("comm".into(), "aios".into());
        node.attributes.insert("rss_kb".into(), "123456".into());
        node.attributes.insert("state".into(), "S".into());
        node.attributes.insert("cmdline".into(), "aios serve".into());
        let row = format_process_row(&node, 42.5);
        assert_eq!(
            row,
            "pid=100 comm=aios cpu_percent=42.5 rss_kb=123456 state=S cmdline=aios serve"
        );
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
        assert!(result.error.unwrap().message.contains("nothing matches"));
    }
}
