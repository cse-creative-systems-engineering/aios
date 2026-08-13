//! M7: Power and thermal specialist (umbrella). Owns the system's power and
//! thermal domain: temperature sensors, fan state, and power/battery state.
//! Per `docs/modules/power-thermal.md`, v0.1 is read-only: bounded Observe and
//! Diagnose tools only. Bounded workload changes (throttling, fan curves) are
//! deferred and will pass through the staged executor and Guardian.
//!
//! Discovery represents the domain as `sensor:*` nodes (NodeType::Sensor)
//! from `sys/class/hwmon` (src/discovery.rs `discover_sensors`): temperature
//! (`temp*`, unit `millidegree_c`), fan (`fan*`, unit `rpm`), and power
//! (`in*`/`energy*`/`power*`/`curr*`) inputs. The specialist owns those nodes
//! via `owns` edges. ECC/memory sensors stay with the memory specialist
//! (one-owner rule, architecture §5). Invariants THERMAL-001 (temperature
//! sensors are present and report within limits) and THERMAL-002 (fan/power
//! state after a staged change) are evaluated from graph evidence; missing
//! evidence is reported as unknown, never as healthy.

use crate::capability::{Capability, Operation, PrincipalId, ResourceId, RiskLevel};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType, SystemGraph,
    TrustLevel,
};
use crate::protocol::{HealthState, now};
use std::collections::HashMap;

pub const PACKAGE_ID: &str = "power.specialist";

/// The power and thermal domain: the umbrella specialist and the resources it
/// owns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerSpecialist {
    pub specialist: NodeId,
    /// Thermal sensors (`sensor:*` nodes reporting temperature or fan state).
    pub thermal_sensors: Vec<NodeId>,
    /// Power sensors (`sensor:*` nodes reporting voltage/current/energy).
    pub power_sensors: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PowerHealth {
    /// Thermal sensors reporting a `value` attribute (THERMAL-001 evidence:
    /// present and reporting a reading).
    pub thermal_with_value: usize,
    pub thermal_sensors: usize,
    pub power_sensors: usize,
    /// Thermal or power sensors not reporting Healthy.
    pub degraded: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PowerError {
    NoPowerResources,
    Graph(String),
}

impl std::fmt::Display for PowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPowerResources => {
                f.write_str("no power or thermal resources were discovered")
            }
            Self::Graph(reason) => {
                write!(f, "could not instantiate power specialist: {reason}")
            }
        }
    }
}

impl std::error::Error for PowerError {}

/// ECC/memory sensors belong to the memory specialist (one-owner rule). The
/// power specialist skips any sensor the memory specialist would claim.
fn is_ecc_sensor(node: &NodeMetadata) -> bool {
    if node.node_type != NodeType::Sensor {
        return false;
    }
    let text = format!("{} {}", node.node_id, node.label).to_lowercase();
    text.contains("ecc") || text.contains("memory") || text.contains("edac")
}

/// The sensor kind is the part of the id after `sensor:<hwmon>-`, e.g.
/// `temp1`, `fan1`, `in0`, `energy1`, `power1`, `curr1`.
fn sensor_kind(node: &NodeMetadata) -> Option<&str> {
    let id = node.node_id.0.strip_prefix("sensor:")?;
    let kind = id.split_once('-').map(|(_, kind)| kind).unwrap_or(id);
    Some(kind)
}

/// Thermal sensors: temperature (`temp*`) and fan (`fan*`) inputs.
fn is_thermal_sensor(node: &NodeMetadata) -> bool {
    if node.node_type != NodeType::Sensor || is_ecc_sensor(node) {
        return false;
    }
    match sensor_kind(node) {
        Some(kind) => kind.starts_with("temp") || kind.starts_with("fan"),
        None => false,
    }
}

/// Power sensors: voltage (`in*`), energy (`energy*`), power (`power*`), and
/// current (`curr*`) inputs.
fn is_power_sensor(node: &NodeMetadata) -> bool {
    if node.node_type != NodeType::Sensor || is_ecc_sensor(node) {
        return false;
    }
    match sensor_kind(node) {
        Some(kind) => {
            kind.starts_with("in")
                || kind.starts_with("energy")
                || kind.starts_with("power")
                || kind.starts_with("curr")
        }
        None => false,
    }
}

impl PowerSpecialist {
    /// Deterministically resolve the power and thermal domain: every thermal
    /// and power sensor in the graph. Lists are sorted by id so the result is
    /// stable across boots. An empty domain is an error (fail closed,
    /// read-only).
    pub fn discover(graph: &SystemGraph) -> Result<Self, PowerError> {
        let mut thermal_sensors: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_thermal_sensor(node))
            .map(|node| node.node_id.clone())
            .collect();
        let mut power_sensors: Vec<NodeId> = graph
            .nodes()
            .values()
            .filter(|node| is_power_sensor(node))
            .map(|node| node.node_id.clone())
            .collect();
        thermal_sensors.sort_by(|a, b| a.0.cmp(&b.0));
        power_sensors.sort_by(|a, b| a.0.cmp(&b.0));
        if thermal_sensors.is_empty() && power_sensors.is_empty() {
            return Err(PowerError::NoPowerResources);
        }
        Ok(Self {
            specialist: NodeId("specialist:power:0".into()),
            thermal_sensors,
            power_sensors,
        })
    }

    pub fn instantiate(graph: &mut SystemGraph) -> Result<Self, PowerError> {
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
        node.label = "Power and thermal specialist".into();
        node.health = HealthState::Healthy;
        node.attributes.insert("package".into(), PACKAGE_ID.into());
        graph.add_node(node).map_err(PowerError::Graph)?;
        for resource in specialist
            .thermal_sensors
            .iter()
            .chain(specialist.power_sensors.iter())
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
    ) -> Result<(), PowerError> {
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
            .map_err(PowerError::Graph)
    }

    pub fn tool_definitions(&self) -> Vec<crate::capability::ToolDefinition> {
        let resource = ResourceId("power:domain".into());
        vec![
            tool(
                "observe_thermal",
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

    /// Cross-layer health: THERMAL-001 evidence is a thermal sensor reporting
    /// a reading (a `value` attribute), plus thermal and power-sensor counts
    /// for the domain.
    pub fn health(&self, graph: &SystemGraph) -> PowerHealth {
        let thermal_with_value = self
            .thermal_sensors
            .iter()
            .filter(|id| {
                graph
                    .get_node(id)
                    .is_some_and(|node| node.attributes.contains_key("value"))
            })
            .count();
        let degraded = self
            .thermal_sensors
            .iter()
            .chain(self.power_sensors.iter())
            .filter(|id| {
                graph
                    .get_node(id)
                    .map(|node| node.health != HealthState::Healthy)
                    .unwrap_or(true)
            })
            .count();
        PowerHealth {
            thermal_with_value,
            thermal_sensors: self.thermal_sensors.len(),
            power_sensors: self.power_sensors.len(),
            degraded,
        }
    }

    /// Resolve a tool target to the resources it covers: an exact node id,
    /// a prefix that matches several ids (e.g. `sensor:hwmon0-temp`), or the
    /// whole domain when the target is empty or `all`.
    pub fn resolve_target(&self, target: &str) -> Vec<NodeId> {
        let target = target.trim();
        if target.is_empty() || target == "all" {
            return self
                .thermal_sensors
                .iter()
                .chain(self.power_sensors.iter())
                .cloned()
                .collect();
        }
        let mut matched: Vec<NodeId> = self
            .thermal_sensors
            .iter()
            .chain(self.power_sensors.iter())
            .filter(|id| id.0 == target || id.0.starts_with(target))
            .cloned()
            .collect();
        matched.sort_by(|a, b| a.0.cmp(&b.0));
        matched
    }

    /// Bounded observe tool: temperature, fan, and power state for the target
    /// resources (docs/modules/power-thermal.md). Domain-wide when the target
    /// is empty or `all`.
    pub fn observe(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut metrics = HashMap::new();
        metrics.insert("thermal_sensors".into(), health.thermal_sensors.to_string());
        metrics.insert(
            "thermal_with_value".into(),
            health.thermal_with_value.to_string(),
        );
        metrics.insert("power_sensors".into(), health.power_sensors.to_string());
        metrics.insert("degraded".into(), health.degraded.to_string());
        metrics.insert(
            "resources".into(),
            resources.iter().map(|id| id.0.clone()).collect::<Vec<_>>().join(","),
        );
        for id in resources.iter().take(8) {
            if let Some(node) = graph.get_node(id) {
                metrics.insert(format!("state:{id}"), format!("{:?}", node.health));
                if let Some(value) = node.attributes.get("value") {
                    metrics.insert(format!("value:{id}"), value.clone());
                }
                if let Some(unit) = node.attributes.get("unit") {
                    metrics.insert(format!("unit:{id}"), unit.clone());
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

    /// Bounded diagnose tool: compare observations with the thermal invariants
    /// (THERMAL-001; THERMAL-002 belongs to the mutation pass). Missing
    /// evidence is reported as unknown findings, never as healthy.
    pub fn diagnose(&self, graph: &SystemGraph, target: &str) -> crate::protocol::ToolResult {
        let resources = self.resolve_target(target);
        if resources.is_empty() {
            return not_found(target);
        }
        let health = self.health(graph);
        let mut findings: Vec<String> = Vec::new();
        let mut confidence: f64 = 0.5;
        if health.thermal_with_value < health.thermal_sensors {
            findings.push(format!(
                "THERMAL-001: {} of {} thermal sensors lack a reading (present but not confirmed reporting)",
                health.thermal_sensors - health.thermal_with_value,
                health.thermal_sensors
            ));
            confidence = 0.7;
        }
        if health.degraded > 0 {
            findings.push(format!(
                "{} thermal or power resources report non-healthy state",
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
        tool_id: format!("power.{name}"),
        specialist_package: PACKAGE_ID.into(),
        risk_level,
        required_capabilities: vec![Capability {
            resource: resource.clone(),
            operation,
        }],
        description: format!("Power and thermal specialist {name}"),
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

    fn sensor(id: &str, value: Option<&str>) -> NodeMetadata {
        let mut node = node(id, NodeType::Sensor);
        if let Some(value) = value {
            node.attributes.insert("value".into(), value.into());
        }
        node
    }

    fn power_graph() -> SystemGraph {
        let mut graph = SystemGraph::new();
        let mut temp = sensor("sensor:hwmon0-temp1", Some("52000"));
        temp.label = "coretemp temp1".into();
        temp.attributes.insert("unit".into(), "millidegree_c".into());
        graph.add_node(temp).unwrap();
        let mut fan = sensor("sensor:hwmon1-fan1", Some("1200"));
        fan.label = "nct6798 fan1".into();
        fan.attributes.insert("unit".into(), "rpm".into());
        graph.add_node(fan).unwrap();
        let mut in0 = sensor("sensor:hwmon1-in0", Some("12000"));
        in0.label = "nct6798 in0".into();
        in0.attributes.insert("unit".into(), "millivolt".into());
        graph.add_node(in0).unwrap();
        // An ECC sensor belongs to the memory specialist, not the power one.
        let mut ecc = sensor("sensor:edac0-ecc", Some("0"));
        ecc.label = "edac memory ECC".into();
        graph.add_node(ecc).unwrap();
        graph
    }

    #[test]
    fn discovers_thermal_and_power_sensors() {
        let graph = power_graph();
        let specialist = PowerSpecialist::discover(&graph).unwrap();
        assert_eq!(
            specialist.thermal_sensors,
            vec![
                NodeId("sensor:hwmon0-temp1".into()),
                NodeId("sensor:hwmon1-fan1".into())
            ]
        );
        assert_eq!(
            specialist.power_sensors,
            vec![NodeId("sensor:hwmon1-in0".into())]
        );
    }

    #[test]
    fn instantiates_with_owns_edges_for_each_resource() {
        let mut graph = power_graph();
        let specialist = PowerSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            graph.get_edges(&specialist.specialist, EdgeType::Owns).len(),
            3
        );
        for resource in specialist
            .thermal_sensors
            .iter()
            .chain(specialist.power_sensors.iter())
        {
            assert_eq!(
                graph.get_owner(resource).unwrap().node_id,
                specialist.specialist,
                "{resource} must have exactly one owner"
            );
        }
    }

    #[test]
    fn fails_closed_without_power_resources() {
        let graph = SystemGraph::new();
        assert_eq!(
            PowerSpecialist::discover(&graph),
            Err(PowerError::NoPowerResources)
        );
        assert!(PowerSpecialist::instantiate(&mut graph.clone()).is_err());
    }

    #[test]
    fn exposes_only_read_only_tools() {
        let mut graph = power_graph();
        let specialist = PowerSpecialist::instantiate(&mut graph).unwrap();
        let tools = specialist.tool_definitions();
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.tool_id.as_str())
                .collect::<Vec<_>>(),
            vec!["power.observe_thermal", "power.diagnose_fault"]
        );
        for tool in tools {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn health_counts_reading_evidence() {
        let mut graph = power_graph();
        let specialist = PowerSpecialist::instantiate(&mut graph).unwrap();
        let health = specialist.health(&graph);
        assert_eq!(health.thermal_sensors, 2);
        assert_eq!(health.thermal_with_value, 2);
        assert_eq!(health.power_sensors, 1);
        assert_eq!(health.degraded, 0);
    }

    #[test]
    fn observe_reports_domain_metrics() {
        let mut graph = power_graph();
        let specialist = PowerSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "all");
        let metrics = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::DeviceState { metrics, .. } => metrics,
            _ => panic!("expected device state"),
        };
        assert_eq!(metrics.get("thermal_sensors").unwrap(), "2");
        assert_eq!(metrics.get("thermal_with_value").unwrap(), "2");
        assert_eq!(metrics.get("power_sensors").unwrap(), "1");
    }

    #[test]
    fn diagnose_flags_missing_reading_evidence() {
        let mut graph = power_graph();
        // Remove the value attribute from one thermal sensor: THERMAL-001
        // evidence disappears.
        let temp = NodeId("sensor:hwmon0-temp1".into());
        let mut node = graph.get_node(&temp).unwrap().clone();
        node.attributes.remove("value");
        graph.upsert_node(node);
        let specialist = PowerSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.diagnose(&graph, "all");
        let (findings, _confidence) = match result.data.as_ref().unwrap() {
            crate::protocol::ToolData::Diagnosis {
                findings,
                confidence,
            } => (findings.clone(), *confidence),
            _ => panic!("expected diagnosis"),
        };
        assert!(
            findings.iter().any(|f| f.starts_with("THERMAL-001")),
            "{findings:?}"
        );
    }

    #[test]
    fn target_resolution_supports_prefix_and_domain() {
        let mut graph = power_graph();
        let specialist = PowerSpecialist::instantiate(&mut graph).unwrap();
        assert_eq!(
            specialist.resolve_target("sensor:hwmon1"),
            vec![
                NodeId("sensor:hwmon1-fan1".into()),
                NodeId("sensor:hwmon1-in0".into())
            ]
        );
        assert_eq!(specialist.resolve_target("all").len(), 3);
    }

    #[test]
    fn observe_unknown_target_fails_with_not_found() {
        let mut graph = power_graph();
        let specialist = PowerSpecialist::instantiate(&mut graph).unwrap();
        let result = specialist.observe(&graph, "device:none");
        assert_eq!(result.status, crate::protocol::ToolStatus::Failed);
        assert!(result
            .error
            .unwrap()
            .message
            .contains("nothing matches"));
    }
}
