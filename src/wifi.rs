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
    pub fn discover(graph: &SystemGraph) -> Result<NodeId, WifiError> {
        let mut devices = graph
            .nodes()
            .values()
            .filter(|node| node.node_type == NodeType::Device)
            .filter(|node| {
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
            })
            .map(|node| node.node_id.clone())
            .collect::<Vec<_>>();
        devices.sort_by(|a, b| a.0.cmp(&b.0));
        let pci_or_usb = devices
            .iter()
            .filter(|id| id.0.starts_with("device:pci-") || id.0.starts_with("device:usb-"))
            .cloned()
            .collect::<Vec<_>>();
        if !pci_or_usb.is_empty() {
            devices = pci_or_usb;
        }
        match devices.as_slice() {
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
                RiskLevel::Critical,
                Operation::Reset,
                &resource,
            ),
        ]
    }

    pub fn health(&self, graph: &SystemGraph) -> Result<WifiHealth, WifiError> {
        let device = graph
            .get_node(&self.device)
            .ok_or_else(|| WifiError::Graph(format!("device {} disappeared", self.device)))?;
        let dependencies = graph.get_dependencies(&self.device);
        let driver_present = dependencies.iter().any(|node| {
            node.node_type == NodeType::Driver || node.node_id.0.starts_with("driver:")
        });
        let bus_present = dependencies
            .iter()
            .any(|node| node.node_type == NodeType::Bus);
        let network_service_present = dependencies.iter().any(|node| {
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
        assert_eq!(tools[3].risk_level, RiskLevel::Critical);
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
}
