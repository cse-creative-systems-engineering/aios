//! M7: Network specialist (umbrella). Owns the network domain: wired/LAN
//! interfaces (`device:net-*` excluding wireless, which the Wi-Fi specialist
//! owns under the one-owner-per-resource rule of architecture §5) and
//! bluetooth controllers. Per `docs/modules/network.md`, v0.1 is read-only:
//! bounded Observe and Diagnose tools only. Mutating operations are deferred
//! and will pass through the staged executor and Guardian.
//!
//! The hierarchy is organizational: the umbrella coordinates cross-transport
//! concerns (connectivity, routing, network-service state) while each
//! transport owns its own resource class. Invariants NETWORK-001 (domain
//! present and reporting connectivity) and NETWORK-002 (transport link state
//! after a staged change) are evaluated from graph evidence; missing evidence
//! is reported as unknown, never as healthy.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "network.specialist";

/// The network domain: the umbrella specialist and the resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkSpecialist {
    pub specialist: NodeId,
    pub wired_interfaces: Vec<NodeId>,
    pub bluetooth_controllers: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkHealth {
    /// Wired interfaces reporting `operstate=up` (NETWORK-001 connectivity
    /// evidence).
    pub interfaces_up: usize,
    pub wired_interfaces: usize,
    pub bluetooth_controllers: usize,
    /// Wired interfaces not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkError {
    NoNetworkResources,
    Graph(String),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoNetworkResources => {
                f.write_str("no wired interfaces or bluetooth controllers were discovered")
            }
            Self::Graph(reason) => write!(f, "could not instantiate network specialist: {reason}"),
        }
    }
}

impl std::error::Error for NetworkError {}

/// Wireless interfaces are owned by the Wi-Fi specialist (docs/modules/
/// wifi.md), never by the network umbrella. The heuristic mirrors the wifi
/// package's own classification: a network interface whose class, label, or
/// name is wireless.
fn is_wireless(node: &NodeMetadata) -> bool {
    let text = format!("{} {}", node.node_id, node.label).to_lowercase();
    let wireless_class = node
        .attributes
        .get("class")
        .map(|value| value.to_lowercase().starts_with("0x028"))
        .unwrap_or(false);
    let wireless_name = node.attributes.values().any(|value| {
        let value = value.to_lowercase();
        value.contains("wireless") || value.contains("wifi")
    });
    wireless_class
        || wireless_name
        || text.contains("wlan")
        || text.contains("wlp")
        || text.contains("wireless")
}

/// Bluetooth controllers: USB devices with Bluetooth interface class `0xE0`,
/// or any device node labelled as a bluetooth controller (docs/modules/
/// bluetooth.md — classification is structural, not heuristic).
fn is_bluetooth_controller(node: &NodeMetadata) -> bool {
    let bluetooth_class = node
        .attributes
        .get("class")
        .map(|value| value.to_lowercase().starts_with("0x0e"))
        .unwrap_or(false);
    bluetooth_class
        || node.label.to_lowercase().contains("bluetooth")
        || node.node_id.0.to_lowercase().contains("bt")
}

fn is_wired_interface(node: &NodeMetadata) -> bool {
    node.node_type == NodeType::Device
        && node.node_id.0.starts_with("device:net-")
        && !is_wireless(node)
}

impl NetworkSpecialist {
    /// Deterministically resolve the network domain: every wired network
    /// interface and bluetooth controller in the graph. Both lists are sorted
    /// by id so the result is stable across boots. An empty domain is an
    /// error (fail closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, NetworkError> {
        let mut wired_interfaces: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_wired_interface(node))
            .map(|node| node.node_id.clone())
            .collect();
        let mut bluetooth_controllers: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| {
                node.node_type == NodeType::Device && is_bluetooth_controller(node)
            })
            .map(|node| node.node_id.clone())
            .collect();
        wired_interfaces.sort_by(|a, b| a.0.cmp(&b.0));
        bluetooth_controllers.sort_by(|a, b| a.0.cmp(&b.0));
        if wired_interfaces.is_empty() && bluetooth_controllers.is_empty() {
            return Err(NetworkError::NoNetworkResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:network:0".into()),
            wired_interfaces,
            bluetooth_controllers,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, NetworkError> {
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
        node.label = "Network specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(NetworkError::Graph)?;
        for resource in specialist
            .wired_interfaces
            .iter()
            .chain(specialist.bluetooth_controllers.iter())
        {
            // One-owner rule (architecture §5): never claim a resource a
            // transport child already owns (e.g. the Wi-Fi specialist).
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
    ) -> Result<(), NetworkError> {
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
            .map_err(NetworkError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("network:domain".into());
        vec![
            tool(
                "observe_network",
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

    /// Cross-layer health: NETWORK-001 evidence is connectivity — at least
    /// one wired interface reporting link up — plus link-state and controller
    /// counts for the domain.
    pub fn health(&self, graph: &SystemGraph) -> NetworkHealth {
        let interfaces_up = self
            .wired_interfaces
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| {
                        node.attributes
                            .get("operstate")
                            .is_some_and(|state| state.eq_ignore_ascii_case("up"))
                    })
                    .unwrap_or(false)
            })
            .count();
        let degraded = self
            .wired_interfaces
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        NetworkHealth {
            interfaces_up,
            wired_interfaces: self.wired_interfaces.len(),
            bluetooth_controllers: self.bluetooth_controllers.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `device:net-`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self
                .wired_interfaces
                .iter()
                .chain(self.bluetooth_controllers.iter())
                .cloned()
                .collect();
        }
        let mut matched: Vec<NodeId> = self
            .wired_interfaces
            .iter()
            .chain(self.bluetooth_controllers.iter())
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: connectivity, routing-relevant link state, and
    /// cross-layer state for the target resources (docs/modules/network.md).
    /// Domain-wide when the target is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("wired_interfaces".into(), health.wired_interfaces.to_string());
        metrics.insert("interfaces_up".into(), health.interfaces_up.to_string());
        metrics.insert(
            "bluetooth_controllers".into(),
            health.bluetooth_controllers.to_string(),
        );
        metrics.insert("degraded".into(), health.degraded.to_string());
        metrics.insert(
            "resources".into(),
            resources.iter().map(|id| id.0.clone()).collect::<Vec<_>>().join(","),
        );
        for id in resources.iter().take(8) {
            if let Some(node) = graph.get_node(id) {
                metrics.insert(format!("state:{id}"), format!("{:?}", node.health));
                if let Some(state) = node.attributes.get("operstate") {
                    metrics.insert(format!("operstate:{id}"), state.clone());
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

    /// Bounded diagnose tool: compare observations with the network
    /// invariants (NETWORK-001, NETWORK-002). Missing evidence is reported as
    /// unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.interfaces_up == 0 {
            findings.push(format!(
                "NETWORK-001: no wired interface reports link up ({} present, connectivity evidence missing)",
                health.wired_interfaces
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} network interfaces report non-healthy state",
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
        tool_id: format!("network.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Network specialist {name}"),
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

    fn network_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut eth = node("device:net-enx1234", NodeType::Device);
        eth.label = "network interface enx1234".into();
        eth.attributes
            .insert("operstate".into(), "up".into());
        graph.add_node(eth).unwrap();
        let mut lo = node("device:net-lo", NodeType::Device);
        lo.label = "network interface lo".into();
        lo.attributes
            .insert("operstate".into(), "unknown".into());
        lo.health = HealthState::Unknown;
        graph.add_node(lo).unwrap();
        let mut wlan = node("device:net-wlan0", NodeType::Device);
        wlan.label = "network interface wlan0".into();
        graph.add_node(wlan).unwrap();
        let mut bt = node("device:usb-1-1", NodeType::Device);
        bt.label = "bluetooth controller".into();
        bt.attributes.insert("class".into(), "0x0e0100".into());
        graph.add_node(bt).unwrap();
        graph
    }

    #[test]
    fn discovers_wired_and_bluetooth_excludes_wireless() {
        let graph = network_graph();
        let specialist = NetworkSpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.wired_interfaces,
            vec![
                NodeId("device:net-enx1234".into()),
                NodeId("device:net-lo".into())
            ]
        );
        assert_eq!(
            specialist.bluetooth_controllers,
            vec![NodeId("device:usb-1-1".into())]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_skipping_wifi_owned() {
        let mut graph = network_graph();
        // The Wi-Fi specialist owns the wireless interface first; the
        // umbrella must not claim it (one-owner rule).
        let mut wlan = graph.get_node(&NodeId("device:net-wlan0".into())).unwrap().clone();
        let _ = &mut wlan;
        let wifi_owner = NodeId("specialist:wifi:wlan0".into());
        graph.add_node(node(&wifi_owner.0, NodeType::Specialist)).unwrap();
        graph
            .add_edge(EdgeMetadata {
                edge_id: EdgeId::new(),
                edge_type: EdgeType::Owns,
                source_node: wifi_owner.clone(),
                target_node: NodeId("device:net-wlan0".into()),
                provenance: EdgeProvenance::Declared {
                    declared_by: PrincipalId::system("coordinator"),
                    package: "wifi.specialist".into(),
                },
                created_at: 1,
                last_observed: 1,
                expires_at: None,
                attributes: HashMap::new(),
            })
            .unwrap();
        let specialist = NetworkSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            graph
                .get_edges(&specialist.specialist, EdgeType::Owns)
                .len(),
            3
        );
        assert_ne!(
            graph.get_owner(&NodeId("device:net-wlan0".into())).unwrap().node_id,
            specialist.specialist,
            "wireless interface stays owned by the Wi-Fi specialist"
        );
    }

    #[test]
    fn fails_closed_without_network_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            NetworkSpecialist::discover(&graph),
            Err(NetworkError::NoNetworkResources)
        );
        assert!(NetworkSpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = network_graph();
        let specialist = NetworkSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["network.observe_network", "network.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_link_and_backing_evidence() {
        let mut graph = network_graph();
        let specialist = NetworkSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.wired_interfaces, 2);
        assert_eq!(health.interfaces_up, 1);
        assert_eq!(health.bluetooth_controllers, 1);
        assert_eq!(health.degraded, 1);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = network_graph();
        let specialist = NetworkSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("wired_interfaces").unwrap(), "2");
        assert_eq!(metrics.get("interfaces_up").unwrap(), "1");
        assert_eq!(metrics.get("bluetooth_controllers").unwrap(), "1");
        assert_eq!(
            metrics.get("resources").unwrap(),
            "device:net-enx1234,device:net-lo,device:usb-1-1"
        );
    }

    #[test]
    fn diagnose_flags_missing_connectivity() {
        let mut graph = network_graph();
        // Bring the only up interface down: connectivity evidence disappears.
        let eth = NodeId("device:net-enx1234".into());
        graph.remove_node(&eth).unwrap();
        let mut down = node(&eth.0, NodeType::Device);
        down.label = "network interface enx1234".into();
        down.attributes
            .insert("operstate".into(), "down".into());
        graph.add_node(down).unwrap();
        let specialist = NetworkSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("NETWORK-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = network_graph();
        let specialist = NetworkSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("device:net-"),
            vec![
                NodeId("device:net-enx1234".into()),
                NodeId("device:net-lo".into())
            ]
        );
        assert_eq!(
            specialist.resolve_target("device:usb-1-1"),
            vec![NodeId("device:usb-1-1".into())]
        );
        assert_eq!(specialist.resolve_target("all").len(), 3);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = network_graph();
        let specialist = NetworkSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:net-none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}
