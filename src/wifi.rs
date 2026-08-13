use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "wifi.specialist";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiSpecialist {
    pub device: NodeId,
    pub specialist: NodeId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WifiHealth {
    pub device: HealthState,
    pub driver_present: bool,
    pub bus_present: bool,
    pub network_service_present: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WifiError {
    NoWirelessDevice,
    Graph(String),
}

impl std::fmt::Display for WifiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoWirelessDevice => f.write_str("no unambiguous Wi-Fi device was discovered"),
            Self::Graph(reason) => write!(f, "could not instantiate Wi-Fi specialist: {reason}"),
        }
    }
}

impl std::error::Error for WifiError {}

impl WifiSpecialist {
    /// Deterministically resolve the single wireless device the specialist
    /// owns (modules/wifi.md: "owns one discovered wireless device at a time").
    ///
    /// Preference order, all deterministic (no model):
    ///   1. A wireless *network interface* that is actually up
    ///      (attributes `operstate=up`, e.g. `wlp1s0`) — the active primary.
    ///   2. Otherwise a wireless PCI/USB controller.
    /// If more than one candidate remains at the same precedence level it is
    /// genuinely ambiguous → `NoWirelessDevice` (fail-closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<NodeId, WifiError> {
        let is_wireless = |node: &NodeMetadata| -> bool {
            let text = format!("{} {}", node.node_id, node.label).to_lowercase();
            let class = node
                .attributes
                .get("class")
                .map(|value| value.to_lowercase())
                .unwrap_or_default();
            let wireless_name = node.attributes.values().any(|value| {
                let value = value.to_lowercase();
                value.contains("wireless") || value.contains("wifi")
            });
            class.starts_with("0x028")
                || wireless_name
                || text.contains("wlan")
                || text.contains("wlp")
                || text.contains("wireless")
        };

        let wireless: Vec<NodeMetadata> = graph
            .nodes()
            .values()
            .filter(|node| node.node_type == NodeType::Device && is_wireless(node))
            .cloned()
            .collect();

        // Prefer a wireless network interface that is up.
        let active_interface: Vec<NodeId> = wireless
            .iter()
            .filter(|node| {
                (node.label.to_lowercase().contains("network interface")
                    || node.node_id.0.starts_with("device:net-"))
                    && node
                        .attributes
                        .get("operstate")
                        .map(|s| s.eq_ignore_ascii_case("up"))
                        .unwrap_or(false)
            })
            .map(|node| node.node_id.clone())
            .collect();

        if active_interface.len() == 1 {
            return Ok(active_interface[0].clone());
        }

        // Otherwise a wireless PCI/USB controller.
        let controllers: Vec<NodeId> = wireless
            .iter()
            .filter(|node| {
                node.node_id.0.starts_with("device:pci-")
                    || node.node_id.0.starts_with("device:usb-")
            })
            .map(|node| node.node_id.clone())
            .collect();

        let mut candidates = active_interface.clone();
        if candidates.is_empty() {
            candidates = controllers;
        }
        candidates.sort_by(|a, b| a.0.cmp(&b.0));
        match candidates.as_slice() {
            [device] => Ok(device.clone()),
            _ => Err(WifiError::NoWirelessDevice),
        }
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, WifiError> {
        let device = Self::discover(graph)?;
        let suffix = device.0.strip_prefix("device:").unwrap_or(&device.0);
        let specialist = NodeId(format!("specialist:wifi:{suffix}"));
        if graph.get_node(&specialist).is_some() {
            return Ok(Self { device, specialist });
        }
        let t = now();
        let mut node = NodeMetadata::new(
            specialist.clone(),
            NodeType::Specialist,
            crate::graph::ProvenanceSource::Declared {
                package: PACKAGE_ID.into(),
            },
            TrustLevel::Trusted,
            t,
        );
        node.label = "Wi-Fi specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(WifiError::Graph)?;
        graph
            .add_edge(EdgeMetadata {
                edge_id: EdgeId::new(),
                edge_type: EdgeType::Owns,
                source_node: specialist.clone(),
                target_node: device.clone(),
                provenance: EdgeProvenance::Declared {
                    declared_by: PrincipalId::system("coordinator"),
                    package: PACKAGE_ID.into(),
                },
                created_at: t,
                last_observed: t,
                expires_at: None,
                attributes: HashMap::new(),
            })
            .map_err(WifiError::Graph)?;
        Ok(Self { device, specialist })
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId(self.device.0.clone());
        vec![
            tool(
                "observe_device",
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
            tool(
                "stage_driver",
                RiskLevel::Staged,
                Operation::Stage,
                &resource,
            ),
            tool(
                "request_reset",
                RiskLevel::Recovery,
                Operation::Reset,
                &resource,
            ),
        ]
    }

    pub fn health(&self, graph: &SystemGraph) -> Result<WifiHealth, WifiError> {
        let device = graph
            .get_node(&self.device)
            .ok_or_else(|| WifiError::Graph(format!("device {} disappeared", self.device)))?;
        // Walk the device's dependency neighborhood (the interface depends on
        // the physical PCI/USB device, which depends on the driver, bus, and
        // firmware). Direct deps alone would miss the 2-hop driver/firmware.
        let neighborhood: Vec<crate::graph::NodeMetadata> = graph
            .get_subgraph(&self.device, 3)
            .map(|s| s.nodes)
            .unwrap_or_else(|| graph.get_dependencies(&self.device));
        let driver_present = neighborhood.iter().any(|node| {
            node.node_type == NodeType::Driver || node.node_id.0.starts_with("driver:")
        });
        let bus_present = neighborhood
            .iter()
            .any(|node| node.node_type == NodeType::Bus);
        let network_service_present = neighborhood.iter().any(|node| {
            node.node_type == NodeType::Service
                && (node.label.to_lowercase().contains("network")
                    || node.node_id.0.contains("networkd"))
        });
        Ok(WifiHealth {
            device: device.health,
            driver_present,
            bus_present,
            network_service_present,
        })
    }

    /// Bounded observe tool: read device, driver, bus, and network-service state.
    /// Returns a structured `ToolData::DeviceState`. (REQ-FUNC-003 / message-protocol §8.1)
    pub fn observe(&self, graph: &SystemGraph) -> crate::protocol::ToolResult {
        let device = graph.get_node(&self.device);
        let mut metrics = HashMap::new();
        match self.health(graph) {
            Ok(health) => {
                metrics.insert("driver".into(), health.driver_present.to_string());
                metrics.insert("bus".into(), health.bus_present.to_string());
                metrics.insert(
                    "network_service".into(),
                    health.network_service_present.to_string(),
                );
                metrics.insert("device_health".into(), format!("{:?}", health.device));
            }
            Err(e) => {
                metrics.insert("error".into(), e.to_string());
            }
        }
        if let Some(node) = &device {
            metrics.insert(
                "state".into(),
                graph
                    .get_owner(&self.device)
                    .map(|_| "available".to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            );
            let _ = node;
        }
        crate::protocol::ToolResult {
            envelope: crate::protocol::MessageEnvelope::new(
                crate::protocol::MessageType::ToolResult,
                PrincipalId::system(PACKAGE_ID),
                uuid::Uuid::new_v4(),
                crate::protocol::DataClassification::SystemConfig,
            ),
            request_id: uuid::Uuid::new_v4(),
            status: crate::protocol::ToolStatus::Success,
            data: Some(crate::protocol::ToolData::DeviceState {
                state: crate::capability::ResourceState::Available,
                metrics,
            }),
            error: None,
            health_impact: None,
        }
    }

    /// Bounded diagnose tool: compare observations with the Wi-Fi invariants
    /// (DRIVER-001, NETWORK-002). (modules/wifi.md)
    pub fn diagnose(&self, graph: &SystemGraph) -> crate::protocol::ToolResult {
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        match self.health(graph) {
            Ok(health) => {
                if !health.driver_present {
                    findings.push("DRIVER-001: no active driver present".into());
                    confidence = 0.8;
                }
                if !health.network_service_present {
                    findings.push("NETWORK-002: network service not detected".into());
                    confidence = 0.8;
                }
                if health.device == HealthState::Unhealthy {
                    findings.push("device reports unhealthy".into());
                    confidence = 0.7;
                }
                if findings.is_empty() {
                    findings.push("no invariant violation found".into());
                    confidence = 0.9;
                }
            }
            Err(e) => {
                findings.push(format!("graph error: {e}"));
                confidence = 0.3;
            }
        }
        crate::protocol::ToolResult {
            envelope: crate::protocol::MessageEnvelope::new(
                crate::protocol::MessageType::ToolResult,
                PrincipalId::system(PACKAGE_ID),
                uuid::Uuid::new_v4(),
                crate::protocol::DataClassification::SystemConfig,
            ),
            request_id: uuid::Uuid::new_v4(),
            status: crate::protocol::ToolStatus::Success,
            data: Some(crate::protocol::ToolData::Diagnosis { findings, confidence }),
            error: None,
            health_impact: None,
        }
    }
}

fn tool(
    name: &'static str,
    risk_level: RiskLevel,
    operation: Operation,
    resource: &ResourceId,
) -> crate::capability::ToolDefinition {
    crate::capability::ToolDefinition {
        tool_id: format!("wifi.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Wi-Fi specialist {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{EdgeType, NodeMetadata, NodeType, ProvenanceSource};

    fn wifi_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut wifi = NodeMetadata::new(
            NodeId("device:net-wlp1s0".into()),
            NodeType::Device,
            ProvenanceSource::Discovered {
                via: "sysfs".into(),
            },
            TrustLevel::Trusted,
            1,
        );
        wifi.label = "network interface wlp1s0".into();
        wifi.attributes.insert("operstate".into(), "up".into());
        graph.add_node(wifi).unwrap();
        graph
    }

    #[test]
    fn discovers_and_instantiates_from_seeded_graph() {
        let mut graph = wifi_graph();
        let specialist = WifiSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(specialist.device.0, "device:net-wlp1s0");
        assert_eq!(
            graph.get_owner(&specialist.device).unwrap().node_id,
            specialist.specialist
        );
        assert_eq!(
            graph
                .get_edges(&specialist.specialist, EdgeType::Owns)
                .len(),
            1
        );
    }

    #[test]
    fn exposes_bounded_tools_with_declared_risk() {
        let mut graph = wifi_graph();
        let specialist = WifiSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "wifi.observe_device",
                "wifi.diagnose_fault",
                "wifi.stage_driver",
                "wifi.request_reset"
            ]
        );
        assert_eq!(tools[2].risk_level, RiskLevel::Staged);
        assert_eq!(tools[3].risk_level, RiskLevel::Recovery);
    }

    #[test]
    fn health_reports_missing_dependencies_as_false() {
        let mut graph = wifi_graph();
        let specialist = WifiSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph).unwrap();
        assert!(!health.driver_present);
        assert!(!health.bus_present);
        assert!(!health.network_service_present);
    }

    // Build the real 2-hop dependency shape seen in live discovery:
    // interface (device:net-wlp1s0) depends_on PCI device
    // (device:pci-...), which depends_on the driver, bus, and network
    // service. Direct 1-hop deps miss the driver/bus; health() must walk
    // the subgraph to see them (modules/wifi.md invariants DRIVER-001,
    // NETWORK-002).
    fn seeded_two_hop_graph() -> SystemGraph {
        let mut graph = wifi_graph();
        let mut pci = NodeMetadata::new(
            NodeId("device:pci-0000:01:00.0".into()),
            NodeType::Device,
            ProvenanceSource::Discovered {
                via: "sysfs".into(),
            },
            TrustLevel::Trusted,
            1,
        );
        pci.label = "wireless PCI device".into();
        let driver = NodeMetadata::new(
            NodeId("driver:mt7921e_git".into()),
            NodeType::Driver,
            ProvenanceSource::Discovered {
                via: "sysfs".into(),
            },
            TrustLevel::Trusted,
            1,
        );
        let bus = NodeMetadata::new(
            NodeId("bus:pci0000:01".into()),
            NodeType::Bus,
            ProvenanceSource::Discovered {
                via: "sysfs".into(),
            },
            TrustLevel::Trusted,
            1,
        );
        let service = NodeMetadata::new(
            NodeId("service:systemd-networkd".into()),
            NodeType::Service,
            ProvenanceSource::Discovered {
                via: "systemd".into(),
            },
            TrustLevel::Trusted,
            1,
        );
        for node in [pci, driver, bus, service] {
            graph.add_node(node).unwrap();
        }
        graph
            .add_edge(EdgeMetadata {
                edge_id: EdgeId::new(),
                edge_type: EdgeType::DependsOn,
                source_node: NodeId("device:net-wlp1s0".into()),
                target_node: NodeId("device:pci-0000:01:00.0".into()),
                provenance: EdgeProvenance::Observed {
                    observed_by: PrincipalId::system("mock-discovery"),
                    event_type: crate::protocol::EventType::DeviceAdded,
                },
                created_at: crate::protocol::now(),
                last_observed: crate::protocol::now(),
                expires_at: None,
                attributes: Default::default(),
            })
            .unwrap();
        for target in [
            "driver:mt7921e_git",
            "bus:pci0000:01",
            "service:systemd-networkd",
        ] {
            graph
                .add_edge(EdgeMetadata {
                    edge_id: EdgeId::new(),
                    edge_type: EdgeType::DependsOn,
                    source_node: NodeId("device:pci-0000:01:00.0".into()),
                    target_node: NodeId(target.into()),
                    provenance: EdgeProvenance::Observed {
                        observed_by: PrincipalId::system("mock-discovery"),
                        event_type: crate::protocol::EventType::DeviceAdded,
                    },
                    created_at: crate::protocol::now(),
                    last_observed: crate::protocol::now(),
                    expires_at: None,
                    attributes: Default::default(),
                })
                .unwrap();
        }
        graph
    }

    // Regression test for the M6 health() subgraph walk: the interface is
    // only 1 hop from the PCI device, and the driver/bus/network service are
    // a further hop beyond it. health() must report them present rather than
    // "no active driver / network service not detected" (wifi.rs bug fixed
    // this milestone).
    #[test]
    fn health_sees_two_hop_driver_and_network_service() {
        let mut graph = seeded_two_hop_graph();
        let specialist = WifiSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph).unwrap();
        assert!(
            health.driver_present,
            "2-hop driver must be reported present; got {health:?}"
        );
        assert!(
            health.bus_present,
            "2-hop bus must be reported present; got {health:?}"
        );
        assert!(
            health.network_service_present,
            "2-hop network service must be reported present; got {health:?}"
        );
    }

    #[test]
    fn observe_returns_device_state_metrics() {
        let mut graph = wifi_graph();
        let specialist = WifiSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph);
        assert_eq!(result.status, crate::protocol::ToolStatus::Success);
        match result.data {
            Some(crate::protocol::ToolData::DeviceState { metrics, .. }) => {
                assert!(metrics.contains_key("driver"), "{metrics:?}");
                assert!(metrics.contains_key("bus"), "{metrics:?}");
            }
            other => panic!("expected DeviceState, got {other:?}"),
        }
    }

    #[test]
    fn diagnose_flags_missing_driver() {
        let mut graph = wifi_graph();
        let specialist = WifiSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph);
        assert_eq!(result.status, crate::protocol::ToolStatus::Success);
        match result.data {
            Some(crate::protocol::ToolData::Diagnosis { findings, confidence }) => {
                assert!(
                    findings.iter().any(|f| f.contains("DRIVER-001")),
                    "{findings:?}"
                );
                assert!(confidence > 0.0, "{confidence}");
            }
            other => panic!("expected Diagnosis, got {other:?}"),
        }
    }
}
