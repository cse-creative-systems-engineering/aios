//! M7: Drivers and hardware specialist. A peer of the domain specialists
//! (architecture §5): owns the generic PCI/USB inventory, firmware state, and
//! loaded kernel modules that no domain specialist owns. Domain-specific
//! devices stay with their domain (Wi-Fi interfaces with Network children,
//! block devices with Storage), and only unclaimed resources get an `owns`
//! edge from this specialist (one-owner rule). Per `docs/modules/drivers.md`,
//! this first pass is read-only: bounded Observe and Diagnose tools only.
//! `stage_driver` and `request_reset` are deferred to the mutation pass that
//! follows the established staged-executor path.
//!
//! Invariants DRIVER-001 (the active driver is present, loadable, and
//! attached to the discovered device) and DEVICE-002 (device state after a
//! staged change, deferred with mutations) are evaluated from graph evidence;
//! missing evidence is reported as unknown, never as healthy.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "drivers.specialist";

/// The drivers and hardware domain: the peer specialist and the resources it
/// owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriversSpecialist {
    pub specialist: NodeId,
    /// Unclaimed PCI/USB device controllers (hardware inventory).
    pub devices: Vec<NodeId>,
    /// Firmware state (`firmware:*` nodes).
    pub firmware: Vec<NodeId>,
    /// Loaded kernel modules (`driver:*` nodes).
    pub drivers: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DriversHealth {
    /// Owned devices with an attached driver node in the graph (DRIVER-001
    /// evidence: present and wired to the device).
    pub devices_with_driver: usize,
    pub devices: usize,
    pub drivers: usize,
    pub firmware: usize,
    /// Owned devices not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DriversError {
    NoHardwareResources,
    Graph(String),
}

impl std::fmt::Display for DriversError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoHardwareResources => {
                f.write_str("no unclaimed devices, firmware, or drivers were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate drivers specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for DriversError {}

/// Domain-claimed resources are excluded: block devices belong to the
/// storage umbrella, wireless devices belong to the wi-fi specialist
/// (network transport children), and anything with an owner already is
/// skipped by `instantiate`. Classification is structural: the device is a
/// PCI or USB hardware controller without a storage or wireless marker.
fn is_unclaimed_hardware(node: &NodeMetadata) -> bool {
    node.node_type == NodeType::Device
        && (node.node_id.0.starts_with("device:pci-")
            || node.node_id.0.starts_with("device:usb-"))
        && !node.label.starts_with("block device ")
        && !is_wireless(node)
        && !is_gpu(node)
}

/// GPUs belong to the Graphics domain (docs/modules/graphics.md), never to
/// the drivers peer. Structural classification: PCI class `0x03` (display
/// controller: VGA, 3D) or a device that calls itself a GPU/VGA.
fn is_gpu(node: &NodeMetadata) -> bool {
    let gpu_class = node
        .attributes
        .get("class")
        .map(|value| value.to_lowercase().starts_with("0x03"))
        .unwrap_or(false);
    let text = format!("{} {}", node.node_id, node.label).to_lowercase();
    gpu_class || text.contains("vga") || text.contains("gpu") || text.contains("3d controller")
}

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

impl DriversSpecialist {
    /// Deterministically resolve the drivers domain: the unclaimed PCI/USB
    /// device inventory, firmware nodes, and driver/module nodes in the
    /// graph. Lists are sorted by id so the result is stable across boots.
    /// An empty domain is an error (fail closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, DriversError> {
        let mut devices: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_unclaimed_hardware(node))
            .map(|node| node.node_id.clone())
            .collect();
        let mut firmware: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| node.node_type == NodeType::Firmware)
            .map(|node| node.node_id.clone())
            .collect();
        let mut drivers: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| node.node_type == NodeType::Driver)
            .map(|node| node.node_id.clone())
            .collect();
        devices.sort_by(|a, b| a.0.cmp(&b.0));
        firmware.sort_by(|a, b| a.0.cmp(&b.0));
        drivers.sort_by(|a, b| a.0.cmp(&b.0));
        if devices.is_empty() && firmware.is_empty() && drivers.is_empty() {
            return Err(DriversError::NoHardwareResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:drivers:0".into()),
            devices,
            firmware,
            drivers,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, DriversError> {
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
        node.label = "Drivers specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(DriversError::Graph)?;
        for resource in specialist
            .devices
            .iter()
            .chain(specialist.firmware.iter())
            .chain(specialist.drivers.iter())
        {
            // One-owner rule (architecture §5): a domain specialist may have
            // claimed this resource first (e.g. a GPU handed to Graphics
            // later); the peer specialist never takes it over.
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
    ) -> Result<(), DriversError> {
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
            .map_err(DriversError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("drivers:domain".into());
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
        ]
    }

    /// Cross-layer health: DRIVER-001 evidence is a driver node attached to
    /// each owned device (the device's `depends_on` edge to a `driver:*`
    /// node), plus device, firmware, and module counts for the domain.
    pub fn health(&self, graph: &SystemGraph) -> DriversHealth {
        let devices_with_driver = self
            .devices
            .iter()
            .filter(|id| {
                graph
                    .get_edges(id, EdgeType::DependsOn)
                    .iter()
                    .any(|edge| edge.target_node.0.starts_with("driver:"))
            })
            .count();
        let degraded = self
            .devices
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        DriversHealth {
            devices_with_driver,
            devices: self.devices.len(),
            drivers: self.drivers.len(),
            firmware: self.firmware.len(),
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
                .devices
                .iter()
                .chain(self.firmware.iter())
                .chain(self.drivers.iter())
                .cloned()
                .collect();
        }
        let mut matched: Vec<NodeId> = self
            .devices
            .iter()
            .chain(self.firmware.iter())
            .chain(self.drivers.iter())
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: device, driver, firmware, and module state for
    /// the target resources (docs/modules/drivers.md). Domain-wide when the
    /// target is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("devices".into(), health.devices.to_string());
        metrics.insert(
            "devices_with_driver".into(),
            health.devices_with_driver.to_string(),
        );
        metrics.insert("drivers".into(), health.drivers.to_string());
        metrics.insert("firmware".into(), health.firmware.to_string());
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

    /// Bounded diagnose tool: compare observations with the driver invariants
    /// (DRIVER-001; DEVICE-002 belongs to the mutation pass). Missing evidence
    /// is reported as unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.devices_with_driver < health.devices {
            findings.push(format!(
                "DRIVER-001: {} of {} devices have no attached driver evidence",
                health.devices - health.devices_with_driver,
                health.devices
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} devices report non-healthy state",
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
        tool_id: format!("drivers.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Drivers specialist {name}"),
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

    fn hardware_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut sata = node("device:pci-0000:01:00.0", NodeType::Device);
        sata.label = "SATA controller".into();
        sata.attributes.insert("class".into(), "0x010601".into());
        graph.add_node(sata).unwrap();
        let mut gpu = node("device:pci-0000:00:02.0", NodeType::Device);
        gpu.label = "PCI device 0000:00:02.0".into();
        gpu.attributes.insert("class".into(), "0x030000".into());
        graph.add_node(gpu).unwrap();
        let mut nvme = node("device:pci-0000:02:00.0", NodeType::Device);
        nvme.label = "block device nvme0n1".into();
        graph.add_node(nvme).unwrap();
        let mut wlan = node("device:pci-0000:03:00.0", NodeType::Device);
        wlan.label = "wireless controller".into();
        graph.add_node(wlan).unwrap();
        let mut fw = node("firmware:iwlwifi-ucode", NodeType::Firmware);
        fw.label = "iwlwifi firmware".into();
        graph.add_node(fw).unwrap();
        let mut drv = node("driver:ahci", NodeType::Driver);
        drv.label = "ahci kernel module".into();
        graph.add_node(drv).unwrap();
        graph
            .add_edge(EdgeMetadata {
                edge_id: EdgeId::new(),
                edge_type: EdgeType::DependsOn,
                source_node: NodeId("device:pci-0000:01:00.0".into()),
                target_node: NodeId("driver:ahci".into()),
                provenance: EdgeProvenance::Observed {
                    observed_by: PrincipalId::system("discovery"),
                    event_type: crate::protocol::EventType::DeviceAdded,
                },
                created_at: 1,
                last_observed: 1,
                expires_at: None,
                attributes: HashMap::new(),
            })
            .unwrap();
        graph
    }

    #[test]
    fn discovers_unclaimed_hardware_only() {
        let graph = hardware_graph();
        let specialist = DriversSpecialist::discover(&graph).unwrap();
        // The SATA controller is unclaimed; the GPU, the block device, and
        // the wireless controller belong to domain specialists.
        assert_eq!(
            specialist.devices,
            vec![NodeId("device:pci-0000:01:00.0".into())]
        );
        assert_eq!(
            specialist.firmware,
            vec![NodeId("firmware:iwlwifi-ucode".into())]
        );
        assert_eq!(specialist.drivers, vec![NodeId("driver:ahci".into())]);
    }

    #[test]
    fn gpu_class_devices_are_not_claimed() {
        let graph = hardware_graph();
        let specialist = DriversSpecialist::discover(&graph).unwrap();
        assert!(
            !specialist
                .devices
                .contains(&NodeId("device:pci-0000:00:02.0".into())),
            "GPU-class devices stay unclaimed for the Graphics domain"
        );
        let mut graph = hardware_graph();
        let _specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            graph.get_owner(&NodeId("device:pci-0000:00:02.0".into())),
            None,
            "the GPU has no owner until Graphics instantiates"
        );
    }

    #[test]
    fn skips_resources_already_owned() {
        let mut graph = hardware_graph();
        // The storage specialist claims the block device first.
        let storage_owner = NodeId("specialist:storage:0".into());
        graph.add_node(node(&storage_owner.0, NodeType::Specialist)).unwrap();
        graph
            .add_edge(EdgeMetadata {
                edge_id: EdgeId::new(),
                edge_type: EdgeType::Owns,
                source_node: storage_owner.clone(),
                target_node: NodeId("device:pci-0000:02:00.0".into()),
                provenance: EdgeProvenance::Declared {
                    declared_by: PrincipalId::system("coordinator"),
                    package: "storage.specialist".into(),
                },
                created_at: 1,
                last_observed: 1,
                expires_at: None,
                attributes: HashMap::new(),
            })
            .unwrap();
        let specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(graph.get_edges(&specialist.specialist, EdgeType::Owns).len(), 3);
        assert_ne!(
            graph.get_owner(&NodeId("device:pci-0000:02:00.0".into())).unwrap().node_id,
            specialist.specialist,
            "claimed block device stays with the storage specialist"
        );
    }

    #[test]
    fn fails_closed_without_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            DriversSpecialist::discover(&graph),
            Err(DriversError::NoHardwareResources)
        );
        assert!(DriversSpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = hardware_graph();
        let specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["drivers.observe_device", "drivers.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_driver_attachment_evidence() {
        let mut graph = hardware_graph();
        let specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.devices, 1);
        assert_eq!(health.devices_with_driver, 1);
        assert_eq!(health.drivers, 1);
        assert_eq!(health.firmware, 1);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = hardware_graph();
        let specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("devices").unwrap(), "1");
        assert_eq!(metrics.get("devices_with_driver").unwrap(), "1");
        assert_eq!(metrics.get("drivers").unwrap(), "1");
        assert_eq!(metrics.get("firmware").unwrap(), "1");
    }

    #[test]
    fn diagnose_flags_missing_driver_attachment() {
        let mut graph = hardware_graph();
        // Rename the attached driver out of the driver:* namespace: the
        // depends_on edge no longer points at a driver node, so DRIVER-001
        // evidence disappears.
        let mut drv = node("driver:ahci", NodeType::Driver);
        drv.label = "ahci kernel module".into();
        drv.node_id = NodeId("module:ahci-unloaded".into());
        graph.remove_node(&NodeId("driver:ahci".into())).unwrap();
        graph.add_node(drv).unwrap();
        let specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("DRIVER-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = hardware_graph();
        let specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("driver:"),
            vec![NodeId("driver:ahci".into())]
        );
        assert_eq!(specialist.resolve_target("all").len(), 3);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = hardware_graph();
        let specialist = DriversSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:net-none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}