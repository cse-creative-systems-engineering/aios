//! M8: System State panel. A read-only aggregator over the discovery graph,
//! the action store, the audit log, and the model route. Renders a terminal
//! overview with subsystem health, active operations, failed actions, and a
//! freshness-aware health summary: UNKNOWN and STALE are never presented as
//! healthy (implementation-roadmap M8 acceptance criterion #2).
//!
//! The panel only reads; every control that changes the system must go
//! through the broker/staging/rollback path (M8 acceptance criterion #6).

use crate::action::{ActionRecord, ActionStore, FileActionStore};
use crate::coordinator::Coordinator;
use crate::graph::{EdgeType, NodeType, SystemGraph};
use crate::protocol::HealthState;

/// One subsystem (node type) with its health roll-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubsystemState {
    pub node_type: NodeType,
    pub total: usize,
    /// Nodes not in a healthy state (Unhealthy, Degraded, Stale, Unknown).
    pub need_attention: usize,
    pub stale: usize,
    /// Count of `depends_on` edges touching nodes of this subsystem.
    pub dependencies: usize,
    /// Owning specialist(s) from the graph, if recorded.
    pub owners: Vec<String>,
}

/// A point-in-time read of everything the panel shows.
#[derive(Debug, Clone)]
pub struct PanelSnapshot {
    pub connectivity: String,
    pub route: String,
    pub total_nodes: usize,
    /// Health counts in a fixed display order.
    pub health_counts: Vec<(HealthState, usize)>,
    pub subsystems: Vec<SubsystemState>,
    /// Nodes needing attention: id and health.
    pub warnings: Vec<(String, HealthState)>,
    /// Actions that have not reached a terminal state.
    pub active_actions: Vec<ActionRecord>,
    /// Actions that failed; their retained checkpoints enable recovery.
    pub failed_actions: Vec<ActionRecord>,
    /// Newest first.
    pub recent_audits: Vec<crate::audit::AuditEntry>,
}

const HEALTH_ORDER: [HealthState; 5] = [
    HealthState::Healthy,
    HealthState::Degraded,
    HealthState::Unhealthy,
    HealthState::Unknown,
    HealthState::Stale,
];

fn is_healthy(state: &HealthState) -> bool {
    *state == HealthState::Healthy
}

fn subsystem_owners(graph: &SystemGraph, node_type: NodeType) -> Vec<String> {
    let mut owners: Vec<String> = graph
        .get_nodes_by_type(node_type)
        .iter()
        .filter_map(|node| {
            graph
                .get_owner(&node.node_id)
                .map(|owner| owner.label.clone())
        })
        .collect();
    owners.sort();
    owners.dedup();
    owners
}

fn count_health(graph: &SystemGraph, node_type: NodeType, state: &HealthState) -> usize {
    graph
        .get_nodes_by_type(node_type)
        .iter()
        .filter(|node| &node.health == state)
        .count()
}

pub fn snapshot(coordinator: &Coordinator) -> PanelSnapshot {
    let graph = coordinator.graph.read().expect("graph lock");
    let nodes: Vec<_> = graph.nodes().values().cloned().collect();
    let edges = graph.edges();

    let mut health_counts: Vec<(HealthState, usize)> = HEALTH_ORDER
        .into_iter()
        .map(|state| {
            let count = nodes.iter().filter(|node| node.health == state).count();
            (state, count)
        })
        .collect();
    health_counts.retain(|(_, count)| *count > 0);

    let mut subsystems: Vec<SubsystemState> = NodeType::all()
        .into_iter()
        .filter(|node_type| !graph.get_nodes_by_type(*node_type).is_empty())
        .map(|node_type| {
            let nodes = graph.get_nodes_by_type(node_type);
            let total = nodes.len();
            let need_attention = HEALTH_ORDER
                .into_iter()
                .filter(|state| !is_healthy(state))
                .map(|state| count_health(&graph, node_type, &state))
                .sum();
            SubsystemState {
                node_type,
                total,
                need_attention,
                stale: count_health(&graph, node_type, &HealthState::Stale),
                dependencies: edges
                    .iter()
                    .filter(|edge| edge.edge_type == EdgeType::DependsOn)
                    .filter(|edge| {
                        nodes.iter().any(|node| {
                            node.node_id == edge.source_node || node.node_id == edge.target_node
                        })
                    })
                    .count(),
                owners: subsystem_owners(&graph, node_type),
            }
        })
        .collect();
    subsystems.sort_by(|a, b| {
        let attention = b.need_attention.cmp(&a.need_attention);
        if attention != std::cmp::Ordering::Equal {
            attention
        } else {
            format!("{:?}", a.node_type).cmp(&format!("{:?}", b.node_type))
        }
    });

    let mut warnings: Vec<(String, HealthState)> = nodes
        .iter()
        .filter(|node| !is_healthy(&node.health))
        .map(|node| (node.node_id.0.clone(), node.health))
        .collect();
    warnings.sort_by(|a, b| a.0.cmp(&b.0));
    warnings.truncate(25);

    let route = coordinator
        .current_route()
        .map(|route| format!("{} / {}", route.provider, route.model))
        .unwrap_or_else(|error| format!("UNAVAILABLE ({error})"));

    let store = FileActionStore::new(coordinator.config_dir.join("actions"))
        .expect("action store for panel");
    let mut actions = store.load_all().unwrap_or_default();
    actions.sort_by_key(|record| record.created_at);
    let mut active_actions = Vec::new();
    let mut failed_actions = Vec::new();
    for record in actions {
        if record.state.is_terminal() {
            if record.state == crate::action::ActionState::Failed {
                failed_actions.push(record);
            }
        } else {
            active_actions.push(record);
        }
    }

    let mut entries = coordinator.audit.entries();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.timestamp));
    entries.truncate(8);

    PanelSnapshot {
        connectivity: format!("{:?}", coordinator.connectivity()),
        route,
        total_nodes: nodes.len(),
        health_counts,
        subsystems,
        warnings,
        active_actions,
        failed_actions,
        recent_audits: entries,
    }
}

pub fn render(snapshot: &PanelSnapshot) -> String {
    let mut lines = vec!["== aios system state ==".to_string()];
    lines.push(format!("connectivity: {}", snapshot.connectivity));
    lines.push(format!("route: {}", snapshot.route));
    lines.push(format!("graph: {} nodes", snapshot.total_nodes));
    let health: Vec<String> = snapshot
        .health_counts
        .iter()
        .map(|(state, count)| format!("{state:?}: {count}"))
        .collect();
    lines.push(format!("health: {}", health.join(", ")));

    if snapshot.subsystems.is_empty() {
        lines.push("subsystems: (none discovered)".into());
    } else {
        lines.push("subsystems:".into());
        for subsystem in &snapshot.subsystems {
            let owners = if subsystem.owners.is_empty() {
                "no owner".to_string()
            } else {
                subsystem.owners.join(", ")
            };
            let attention = if subsystem.need_attention == 0 {
                String::new()
            } else {
                format!(", {} need attention", subsystem.need_attention)
            };
            let stale = if subsystem.stale == 0 {
                String::new()
            } else {
                format!(", {} stale", subsystem.stale)
            };
            let dependencies = if subsystem.dependencies == 0 {
                String::new()
            } else {
                format!(", {} dependencies", subsystem.dependencies)
            };
            lines.push(format!(
                "  {:?}: {}{}{}{} (owner: {})",
                subsystem.node_type,
                subsystem.total,
                attention,
                stale,
                dependencies,
                owners
            ));
        }
    }

    if snapshot.warnings.is_empty() {
        lines.push("warnings: none".into());
    } else {
        lines.push("warnings:".into());
        for (id, state) in &snapshot.warnings {
            lines.push(format!("  {id}: {state:?}"));
        }
    }

    if snapshot.active_actions.is_empty() {
        lines.push("active operations: none".into());
    } else {
        lines.push("active operations:".into());
        for record in &snapshot.active_actions {
            lines.push(format!(
                "  {} {:?} on {} ({:?})",
                record.action_id, record.operation, record.resource, record.state
            ));
        }
    }

    if snapshot.failed_actions.is_empty() {
        lines.push("recovery: no failed actions".into());
    } else {
        lines.push("recovery:".into());
        for record in &snapshot.failed_actions {
            let checkpoint = if record.checkpoint_id.is_some() {
                "checkpoint retained"
            } else {
                "no checkpoint"
            };
            lines.push(format!(
                "  {} {:?} on {} (failed, {})",
                record.action_id, record.operation, record.resource, checkpoint
            ));
        }
    }

    if snapshot.recent_audits.is_empty() {
        lines.push("recent audit: none".into());
    } else {
        lines.push("recent audit:".into());
        for entry in &snapshot.recent_audits {
            lines.push(format!(
                "  {} {} {} -> {}",
                entry.timestamp, entry.actor, entry.action, entry.outcome
            ));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{NodeId, NodeMetadata, ProvenanceSource, TrustLevel};

    fn graph_with(nodes: &[(&str, NodeType, HealthState)]) -> SystemGraph {
        let mut graph = SystemGraph::new();
        for (id, node_type, health) in nodes {
            let mut node = NodeMetadata::new(
                NodeId(id.to_string()),
                *node_type,
                ProvenanceSource::Discovered {
                    via: "test".into(),
                },
                TrustLevel::Trusted,
                1,
            );
            node.health = *health;
            graph.add_node(node).unwrap();
        }
        graph
    }

    #[test]
    fn subsystem_dependency_counts_cover_depends_on_edges() {
        let mut graph = graph_with(&[
            ("device:wifi0", NodeType::Device, HealthState::Healthy),
            ("driver:iwlwifi", NodeType::Driver, HealthState::Healthy),
            ("bus:pci0", NodeType::Bus, HealthState::Healthy),
        ]);
        graph
            .add_edge(crate::graph::EdgeMetadata {
                edge_id: crate::graph::EdgeId::new(),
                edge_type: EdgeType::DependsOn,
                source_node: NodeId("device:wifi0".into()),
                target_node: NodeId("driver:iwlwifi".into()),
                provenance: crate::graph::EdgeProvenance::Observed {
                    observed_by: crate::capability::PrincipalId::agent(
                        "test.specialist",
                        "test-instance",
                    ),
                    event_type: crate::protocol::EventType::DeviceAdded,
                },
                created_at: 1,
                last_observed: 1,
                expires_at: None,
                attributes: std::collections::HashMap::new(),
            })
            .unwrap();
        let edges = graph.edges();
        let deps = |node_type: NodeType| {
            let nodes = graph.get_nodes_by_type(node_type);
            edges
                .iter()
                .filter(|edge| edge.edge_type == EdgeType::DependsOn)
                .filter(|edge| {
                    nodes.iter().any(|node| {
                        node.node_id == edge.source_node || node.node_id == edge.target_node
                    })
                })
                .count()
        };
        assert_eq!(deps(NodeType::Device), 1);
        assert_eq!(deps(NodeType::Driver), 1);
        assert_eq!(deps(NodeType::Bus), 0);
    }

    #[test]
    fn counts_health_and_never_hides_stale() {
        let graph = graph_with(&[
            ("device:a", NodeType::Device, HealthState::Healthy),
            ("device:b", NodeType::Device, HealthState::Unhealthy),
            ("device:c", NodeType::Device, HealthState::Stale),
            ("service:d", NodeType::Service, HealthState::Unknown),
        ]);
        let counts = HEALTH_ORDER
            .into_iter()
            .map(|state| count_health(&graph, NodeType::Device, &state))
            .collect::<Vec<_>>();
        assert_eq!(counts, vec![1, 0, 1, 0, 1]);
        assert!(HEALTH_ORDER.contains(&HealthState::Stale));
    }

    #[test]
    fn subsystems_roll_up_attention() {
        let graph = graph_with(&[
            ("device:a", NodeType::Device, HealthState::Healthy),
            ("device:b", NodeType::Device, HealthState::Unhealthy),
            ("service:c", NodeType::Service, HealthState::Stale),
        ]);
        let device = NodeType::all()
            .into_iter()
            .find(|node_type| *node_type == NodeType::Device)
            .unwrap();
        let subsystem = SubsystemState {
            node_type: device,
            total: graph.get_nodes_by_type(NodeType::Device).len(),
            need_attention: HEALTH_ORDER
                .into_iter()
                .filter(|state| !is_healthy(state))
                .map(|state| count_health(&graph, NodeType::Device, &state))
                .sum(),
            stale: count_health(&graph, NodeType::Device, &HealthState::Stale),
            dependencies: 0,
            owners: Vec::new(),
        };
        assert_eq!(subsystem.total, 2);
        assert_eq!(subsystem.need_attention, 1);
        assert_eq!(subsystem.stale, 0);
    }

    #[test]
    fn render_shows_stale_and_unknown_explicitly() {
        let snapshot = PanelSnapshot {
            connectivity: "Internet".into(),
            route: "openai / stub".into(),
            total_nodes: 2,
            health_counts: vec![
                (HealthState::Healthy, 1),
                (HealthState::Stale, 1),
                (HealthState::Unknown, 0),
            ],
            subsystems: Vec::new(),
            warnings: vec![("device:b".into(), HealthState::Stale)],
            active_actions: Vec::new(),
            failed_actions: Vec::new(),
            recent_audits: Vec::new(),
        };
        let text = render(&snapshot);
        assert!(text.contains("Stale: 1"), "{text}");
        assert!(text.contains("warnings:"), "{text}");
        assert!(text.contains("device:b: Stale"), "{text}");
        assert!(!text.contains("health: none"), "{text}");
    }

    #[test]
    fn render_lists_failed_actions_with_recovery_hint() {
        let snapshot = PanelSnapshot {
            connectivity: "Offline".into(),
            route: "UNAVAILABLE (no eligible provider)".into(),
            total_nodes: 0,
            health_counts: Vec::new(),
            subsystems: Vec::new(),
            warnings: Vec::new(),
            active_actions: Vec::new(),
            failed_actions: Vec::new(),
            recent_audits: Vec::new(),
        };
        let text = render(&snapshot);
        assert!(text.contains("recovery: no failed actions"), "{text}");
        assert!(text.contains("UNAVAILABLE"), "{text}");
    }
}
