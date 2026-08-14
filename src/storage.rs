//! M7: Storage specialist (umbrella). Owns the storage domain: block devices
//! (`device:sd*`/`device:nvme*`, discovered with capacity/read-only/removable
//! attributes) and mounted filesystems (`fs:*` nodes). Per
//! `docs/modules/storage.md`, v0.1 is read-only: bounded Observe and Diagnose
//! tools only. Mutating operations (partitioning, formatting, device reset)
//! are deferred and will pass through the staged executor and Guardian.
//!
//! Ownership follows the one-owner-per-resource rule (architecture §5): every
//! block device and filesystem in the graph gets exactly one `owns` edge from
//! the storage specialist. Invariants STORAGE-001 (block device present,
//! readable, reports capacity) and STORAGE-002 (filesystem state after a
//! change) are evaluated from graph evidence; missing evidence is reported as
//! unknown, never as healthy.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "storage.specialist";

/// The storage domain: the umbrella specialist and the resources it owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageSpecialist {
    pub specialist: NodeId,
    pub block_devices: Vec<NodeId>,
    pub filesystems: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageHealth {
    /// Block devices that report a `size_bytes` attribute (STORAGE-001 evidence).
    pub devices_reporting_capacity: usize,
    pub block_devices: usize,
    /// Filesystems with a backing block device in the graph (device attribute
    /// or a `depends_on` edge to a block device).
    pub filesystems_with_backing: usize,
    pub filesystems: usize,
    /// Filesystems not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageError {
    NoStorageResources,
    Graph(String),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoStorageResources => {
                f.write_str("no block devices or filesystems were discovered")
            }
            Self::Graph(reason) => write!(f, "could not instantiate storage specialist: {reason}"),
        }
    }
}

impl std::error::Error for StorageError {}

fn is_block_device(node: &NodeMetadata) -> bool {
    node.node_type == NodeType::Device && node.label.starts_with("block device ")
}

fn is_filesystem(node: &NodeMetadata) -> bool {
    node.node_type == NodeType::Filesystem
}

impl StorageSpecialist {
    /// Deterministically resolve the storage domain: every block device and
    /// mounted filesystem in the graph. Both lists are sorted by id so the
    /// result is stable across boots. An empty domain is an error (fail
    /// closed, read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, StorageError> {
        let mut block_devices: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_block_device(node))
            .map(|node| node.node_id.clone())
            .collect();
        let mut filesystems: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_filesystem(node))
            .map(|node| node.node_id.clone())
            .collect();
        block_devices.sort_by(|a, b| a.0.cmp(&b.0));
        filesystems.sort_by(|a, b| a.0.cmp(&b.0));
        if block_devices.is_empty() && filesystems.is_empty() {
            return Err(StorageError::NoStorageResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:storage:0".into()),
            block_devices,
            filesystems,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, StorageError> {
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
        node.label = "Storage specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(StorageError::Graph)?;
        for resource in specialist
            .block_devices
            .iter()
            .chain(specialist.filesystems.iter())
        {
            specialist.add_ownership(graph, resource, t)?;
        }
        Ok(specialist)
    }

    fn add_ownership(
        &self,
        graph: &mut SystemGraph,
        resource: &NodeId,
        t: crate::protocol::Timestamp,
    ) -> Result<(), StorageError> {
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
            .map_err(StorageError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("storage:domain".into());
        vec![
            tool(
                "observe_storage",
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

    pub fn health(&self, graph: &SystemGraph) -> StorageHealth {
        let devices_reporting_capacity = self
            .block_devices
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .is_some_and(|node| node.attributes.contains_key("size_bytes"))
            })
            .count();
        let filesystems_with_backing = self
            .filesystems
            .iter()
            .filter(|id| {
                graph.get_node(id).is_some_and(|node| {
                    let device_attr = node.attributes.get("device").is_some();
                    let depends_on_device = graph.get_edges(id, EdgeType::DependsOn).iter().any(
                        |edge| edge.target_node.0.starts_with("device:"),
                    );
                    device_attr || depends_on_device
                })
            })
            .count();
        let degraded = self
            .filesystems
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        StorageHealth {
            devices_reporting_capacity,
            block_devices: self.block_devices.len(),
            filesystems_with_backing,
            filesystems: self.filesystems.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `device:nvme`), or the whole
    /// domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self
                .block_devices
                .iter()
                .chain(self.filesystems.iter())
                .cloned()
                .collect();
        }
        let mut matched: Vec<NodeId> = self
            .block_devices
            .iter()
            .chain(self.filesystems.iter())
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: capacity and cross-layer state for the target
    /// resources (docs/modules/storage.md). Domain-wide when the target is
    /// empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("block_devices".into(), health.block_devices.to_string());
        metrics.insert(
            "devices_reporting_capacity".into(),
            health.devices_reporting_capacity.to_string(),
        );
        metrics.insert("filesystems".into(), health.filesystems.to_string());
        metrics.insert(
            "filesystems_with_backing".into(),
            health.filesystems_with_backing.to_string(),
        );
        metrics.insert("degraded".into(), health.degraded.to_string());

        const ROW_KEYS: &[&str] = &[
            "size_bytes",
            "read_only",
            "removable",
            "reads",
            "read_sectors",
            "writes",
            "write_sectors",
            "in_flight",
            "io_ticks",
            "time_in_queue",
            "rotational",
            "scheduler",
            "logical_block_size",
            "physical_block_size",
        ];
        for (i, id) in self.block_devices.iter().take(12).enumerate() {
            if let Some(node) = graph.get_node(id) {
                let mut fields: Vec<String> = Vec::new();
                if let Some(name) = id.0.strip_prefix("device:") {
                    fields.push(format!("device={name}"));
                }
                for key in ROW_KEYS {
                    if let Some(value) = node.attributes.get(*key) {
                        fields.push(format!("{key}={value}"));
                    }
                }
                metrics.insert(format!("disk_{i}"), fields.join(" "));
            }
        }

        const FS_ROW_KEYS: &[&str] = &[
            "fstype",
            "mount",
            "device",
            "options",
            "read_only",
            "usage_total_kb",
            "usage_used_kb",
            "usage_available_kb",
            "usage_used_percent",
        ];
        for (i, id) in self.filesystems.iter().take(12).enumerate() {
            if let Some(node) = graph.get_node(id) {
                let mut fields: Vec<String> = Vec::new();
                for key in FS_ROW_KEYS {
                    if let Some(value) = node.attributes.get(*key) {
                        fields.push(format!("{key}={value}"));
                    }
                }
                metrics.insert(format!("fs_{i}"), fields.join(" "));
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

    /// Bounded diagnose tool: compare observations with the storage
    /// invariants (STORAGE-001, STORAGE-002). Missing evidence is reported
    /// as unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.devices_reporting_capacity < health.block_devices {
            findings.push(format!(
                "STORAGE-001: {} of {} block devices lack capacity evidence (present but not confirmed readable)",
                health.block_devices - health.devices_reporting_capacity,
                health.block_devices
            ));
            confidence = 0.7;
        }
        if health.filesystems_with_backing < health.filesystems {
            findings.push(format!(
                "STORAGE-002: {} of {} filesystems have no backing block device evidence",
                health.filesystems - health.filesystems_with_backing,
                health.filesystems
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!("{} filesystems report non-healthy state", health.degraded));
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
        tool_id: format!("storage.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Storage specialist {name}"),
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

    fn storage_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut nvme = node("device:nvme0n1", NodeType::Device);
        nvme.label = "block device nvme0n1".into();
        nvme.attributes.insert("size_bytes".into(), "500107862016".into());
        nvme.attributes.insert("reads".into(), "1284416".into());
        nvme.attributes.insert("writes".into(), "1717812".into());
        nvme.attributes.insert("read_sectors".into(), "22558160".into());
        nvme.attributes.insert("write_sectors".into(), "85627488".into());
        nvme.attributes.insert("io_ticks".into(), "284136".into());
        nvme.attributes.insert("rotational".into(), "0".into());
        nvme.attributes.insert("scheduler".into(), "[mq-deadline] none".into());
        nvme.attributes.insert("logical_block_size".into(), "512".into());
        nvme.attributes.insert("physical_block_size".into(), "4096".into());
        nvme.attributes.insert("read_only".into(), "false".into());
        graph.add_node(nvme).unwrap();
        let mut sda = node("device:sda", NodeType::Device);
        sda.label = "block device sda".into();
        graph.add_node(sda).unwrap();
        let mut fs = node("fs:ext4-", NodeType::Filesystem);
        fs.label = "ext4 mounted at /".into();
        fs.attributes.insert("device".into(), "/dev/nvme0n1p2".into());
        fs.attributes.insert("fstype".into(), "ext4".into());
        fs.attributes.insert("mount".into(), "/".into());
        fs.attributes.insert("options".into(), "rw,relatime".into());
        fs.attributes.insert("read_only".into(), "false".into());
        fs.attributes.insert("usage_total_kb".into(), "488386496".into());
        fs.attributes.insert("usage_used_kb".into(), "205122328".into());
        fs.attributes.insert("usage_available_kb".into(), "258537064".into());
        fs.attributes.insert("usage_used_percent".into(), "42".into());
        graph.add_node(fs).unwrap();
        let mut tmp = node("fs:tmpfs-devmapper", NodeType::Filesystem);
        tmp.label = "xfs mounted at /var".into();
        tmp.attributes.insert("fstype".into(), "xfs".into());
        tmp.attributes.insert("mount".into(), "/var".into());
        graph.add_node(tmp).unwrap();
        graph
    }

    #[test]
    fn discovers_block_devices_and_filesystems() {
        let graph = storage_graph();
        let specialist = StorageSpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.block_devices,
            vec![NodeId("device:nvme0n1".into()), NodeId("device:sda".into())]
        );
        assert_eq!(
            specialist.filesystems,
            vec![
                NodeId("fs:ext4-".into()),
                NodeId("fs:tmpfs-devmapper".into())
            ]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        for resource in specialist
            .block_devices
            .iter()
            .chain(specialist.filesystems.iter())
        {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
        assert_eq!(
            graph
                .get_edges(&specialist.specialist, EdgeType::Owns)
                .len(),
            4
        );
    }

    #[test]
    fn fails_closed_without_storage_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            StorageSpecialist::discover(&graph),
            Err(StorageError::NoStorageResources)
        );
        assert!(StorageSpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["storage.observe_storage", "storage.diagnose_fault"]
        );
        assert!(tools
            .iter()
            .all(|tool| tool.risk_level == RiskLevel::ReadOnly));
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("all").len(),
            4,
            "domain-wide target must cover every resource"
        );
        assert_eq!(
            specialist.resolve_target("device:nvme"),
            vec![NodeId("device:nvme0n1".into())]
        );
        assert_eq!(specialist.resolve_target("device:missing"), Vec::new());
    }

    #[test]
    fn health_counts_capacity_and_backing_evidence() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.block_devices, 2);
        assert_eq!(health.devices_reporting_capacity, 1);
        assert_eq!(health.filesystems, 2);
        assert_eq!(health.filesystems_with_backing, 1);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        assert_eq!(result.status, crate::protocol::ToolStatus::Success);
        match result.data {
            Some(crate::protocol::ToolData::DeviceState { metrics, .. }) => {
                assert_eq!(
                    metrics.get("block_devices").map(|v| v.as_str()),
                    Some("2")
                );
                assert_eq!(metrics.get("filesystems").map(|v| v.as_str()), Some("2"));
                assert_eq!(metrics.get("degraded").map(|v| v.as_str()), Some("0"));
                assert_eq!(
                    metrics.get("devices_reporting_capacity").map(|v| v.as_str()),
                    Some("1")
                );
                let disk0 = metrics.get("disk_0").expect("disk_0 row");
                assert!(disk0.contains("device=nvme0n1"), "{disk0}");
                assert!(disk0.contains("reads=1284416"), "{disk0}");
                assert!(disk0.contains("writes=1717812"), "{disk0}");
                assert!(disk0.contains("rotational=0"), "{disk0}");
                let fs0 = metrics.get("fs_0").expect("fs_0 row");
                assert!(fs0.contains("fstype=ext4"), "{fs0}");
                assert!(fs0.contains("mount=/"), "{fs0}");
                assert!(fs0.contains("usage_used_percent=42"), "{fs0}");
                assert!(metrics.contains_key("fs_1"));
                assert!(!metrics.iter().any(|(k, _)| k.starts_with("state:") || k.starts_with("resources")),
                    "observe must not emit the old resources blob or state:<id> rows");
            }
            other => panic!("expected DeviceState, got {other:?}"),
        }
    }

    #[test]
    fn observe_implements_the_storage_tool_claim() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        let disk0 = metrics.get("disk_0").cloned().unwrap_or_default();
        let fs0 = metrics.get("fs_0").cloned().unwrap_or_default();
        let claim = crate::tools::STORAGE_TOOL_CLAIM;
        for (capability, needle) in [
            ("reads", "reads="),
            ("writes", "writes="),
            ("sector", "read_sectors="),
            ("latency", "io_ticks="),
            ("rotational", "rotational="),
            ("scheduler", "scheduler="),
            ("block size", "logical_block_size="),
            ("usage", "usage_used_percent="),
            ("read-only", "read_only="),
            ("options", "options="),
        ] {
            assert!(
                claim.contains(capability),
                "tool claim must mention {capability}: {claim}"
            );
            assert!(
                disk0.contains(needle) || fs0.contains(needle),
                "claim advertises {capability} but observe emits no {needle} row field"
            );
        }
        assert!(
            disk0.contains("device=nvme0n1") && fs0.contains("fstype=ext4"),
            "disk_0/fs_0 rows must name their resource"
        );
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:missing");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(
            result
                .error
                .as_ref()
                .map(|e| e.message.contains("nothing matches"))
                .unwrap_or(false),
            "{result:?}"
        );
    }

    #[test]
    fn diagnose_flags_missing_capacity_and_backing() {
        let mut graph = storage_graph();
        let specialist = StorageSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        assert_eq!(result.status, crate::protocol::ToolStatus::Success);
        match result.data {
            Some(crate::protocol::ToolData::Diagnosis { findings, confidence }) => {
                assert!(
                    findings.iter().any(|f| f.contains("STORAGE-001")),
                    "{findings:?}"
                );
                assert!(
                    findings.iter().any(|f| f.contains("STORAGE-002")),
                    "{findings:?}"
                );
                assert!(confidence > 0.0, "{confidence}");
            }
            other => panic!("expected Diagnosis, got {other:?}"),
        }
    }
}