use crate::capability::{Capability, PrincipalId};
use crate::protocol::{EventType, PackageId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeType {
    Cpu,
    Memory,
    Bus,
    Device,
    Firmware,
    Sensor,
    Kernel,
    Driver,
    Service,
    Filesystem,
    Process,
    Namespace,
    PlannerAgent,
    VerificationAgent,
    Specialist,
    Guardian,
    Coordinator,
    LocalModel,
    LanGateway,
    InternetProvider,
    FallbackRoute,
    Capability,
    Policy,
    BootImage,
    Snapshot,
    Watchdog,
}

impl NodeType {
    /// Every node type in the model, in declaration order.
    pub fn all() -> Vec<NodeType> {
        vec![
            NodeType::Cpu,
            NodeType::Memory,
            NodeType::Bus,
            NodeType::Device,
            NodeType::Firmware,
            NodeType::Sensor,
            NodeType::Kernel,
            NodeType::Driver,
            NodeType::Service,
            NodeType::Filesystem,
            NodeType::Process,
            NodeType::Namespace,
            NodeType::PlannerAgent,
            NodeType::VerificationAgent,
            NodeType::Specialist,
            NodeType::Guardian,
            NodeType::Coordinator,
            NodeType::LocalModel,
            NodeType::LanGateway,
            NodeType::InternetProvider,
            NodeType::FallbackRoute,
            NodeType::Capability,
            NodeType::Policy,
            NodeType::BootImage,
            NodeType::Snapshot,
            NodeType::Watchdog,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenanceSource {
    Discovered { via: String },
    Declared { package: PackageId },
    Attested { by: PrincipalId },
    Observed { by: PrincipalId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    Trusted,
    Provisional,
    Untrusted,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMetadata {
    pub node_id: NodeId,
    pub node_type: NodeType,
    pub label: String,
    pub version: Option<String>,
    pub source: ProvenanceSource,
    pub trust_level: TrustLevel,
    pub health: crate::protocol::HealthState,
    pub capabilities: Vec<Capability>,
    pub created_at: Timestamp,
    pub last_observed: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub attributes: HashMap<String, String>,
}

impl NodeMetadata {
    pub fn new(
        node_id: NodeId,
        node_type: NodeType,
        source: ProvenanceSource,
        trust_level: TrustLevel,
        now: Timestamp,
    ) -> Self {
        Self {
            node_id,
            node_type,
            label: String::new(),
            version: None,
            source,
            trust_level,
            health: crate::protocol::HealthState::Unknown,
            capabilities: Vec::new(),
            created_at: now,
            last_observed: now,
            expires_at: None,
            attributes: HashMap::new(),
        }
    }

    pub fn is_stale(&self, now: Timestamp) -> bool {
        match self.expires_at {
            Some(expires) => now > expires,
            None => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EdgeId(uuid::Uuid);

impl EdgeId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for EdgeId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    Owns,
    DependsOn,
    CommunicatesWith,
    Observes,
    Controls,
    Affects,
    HostedOn,
    FallbackTo,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeProvenance {
    Declared {
        declared_by: PrincipalId,
        package: PackageId,
    },
    Attested {
        attested_by: PrincipalId,
        signature_verified: bool,
    },
    Observed {
        observed_by: PrincipalId,
        event_type: EventType,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeMetadata {
    pub edge_id: EdgeId,
    pub edge_type: EdgeType,
    pub source_node: NodeId,
    pub target_node: NodeId,
    pub provenance: EdgeProvenance,
    pub created_at: Timestamp,
    pub last_observed: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub attributes: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct Subgraph {
    pub nodes: Vec<NodeMetadata>,
    pub edges: Vec<EdgeMetadata>,
    pub root: NodeId,
    pub max_hops: usize,
}

#[derive(Clone, Debug)]
pub struct ImpactReport {
    pub resource: NodeId,
    pub affected_nodes: Vec<NodeMetadata>,
    pub dependencies: Vec<NodeMetadata>,
    pub risk_assessment: String,
}

#[derive(Clone, Debug, Default)]
pub struct SystemGraph {
    nodes: HashMap<NodeId, NodeMetadata>,
    edges: HashMap<EdgeId, EdgeMetadata>,
    adjacency: HashMap<NodeId, Vec<EdgeId>>,
    reverse_adjacency: HashMap<NodeId, Vec<EdgeId>>,
}

impl SystemGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: NodeMetadata) -> Result<(), String> {
        let id = node.node_id.clone();
        if self.nodes.insert(id.clone(), node).is_some() {
            return Err(format!("node {id} already exists"));
        }
        Ok(())
    }

    pub fn upsert_node(&mut self, node: NodeMetadata) {
        self.nodes.insert(node.node_id.clone(), node);
    }

    pub fn remove_node(&mut self, id: &NodeId) -> Option<NodeMetadata> {
        let removed = self.nodes.remove(id)?;
        let mut edges_to_remove = Vec::new();
        for (edge_id, edge) in &self.edges {
            if &edge.source_node == id || &edge.target_node == id {
                edges_to_remove.push(edge_id.clone());
            }
        }
        for edge_id in edges_to_remove {
            let edge = self.edges.remove(&edge_id).expect("edge present");
            if let Some(list) = self.adjacency.get_mut(&edge.source_node) {
                list.retain(|e| e != &edge_id);
            }
            if let Some(list) = self.reverse_adjacency.get_mut(&edge.target_node) {
                list.retain(|e| e != &edge_id);
            }
        }
        self.adjacency.remove(id);
        self.reverse_adjacency.remove(id);
        Some(removed)
    }

    pub fn get_node(&self, id: &NodeId) -> Option<NodeMetadata> {
        self.nodes.get(id).cloned()
    }

    pub fn nodes(&self) -> &HashMap<NodeId, NodeMetadata> {
        &self.nodes
    }

    pub fn add_edge(&mut self, edge: EdgeMetadata) -> Result<(), String> {
        if !self.nodes.contains_key(&edge.source_node) {
            return Err(format!("edge source {} missing", edge.source_node));
        }
        if !self.nodes.contains_key(&edge.target_node) {
            return Err(format!("edge target {} missing", edge.target_node));
        }
        if edge.edge_type == EdgeType::Owns && self.get_owner(&edge.target_node).is_some() {
            return Err(format!(
                "resource {} already has an owner",
                edge.target_node
            ));
        }
        let id = edge.edge_id.clone();
        let source = edge.source_node.clone();
        let target = edge.target_node.clone();
        if self.edges.insert(id.clone(), edge).is_some() {
            return Err(format!("edge {id:?} already exists"));
        }
        self.adjacency
            .entry(source)
            .or_default()
            .push(id.clone());
        self.reverse_adjacency.entry(target).or_default().push(id);
        Ok(())
    }

    pub fn get_edges(&self, from: &NodeId, edge_type: EdgeType) -> Vec<EdgeMetadata> {
        self.adjacency
            .get(from)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.edges.get(id))
                    .filter(|e| e.edge_type == edge_type)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn has_edge(&self, from: &NodeId, to: &NodeId, edge_type: EdgeType) -> bool {
        self.edges
            .values()
            .any(|e| e.source_node == *from && e.target_node == *to && e.edge_type == edge_type)
    }

    pub fn edges(&self) -> Vec<EdgeMetadata> {
        self.edges.values().cloned().collect()
    }

    pub fn get_incoming_edges(&self, to: &NodeId, edge_type: EdgeType) -> Vec<EdgeMetadata> {
        self.reverse_adjacency
            .get(to)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.edges.get(id))
                    .filter(|e| e.edge_type == edge_type)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn get_nodes_by_type(&self, node_type: NodeType) -> Vec<NodeMetadata> {
        self.nodes
            .values()
            .filter(|n| n.node_type == node_type)
            .cloned()
            .collect()
    }

    pub fn get_subgraph(&self, root: &NodeId, max_hops: usize) -> Option<Subgraph> {
        let root_node = self.nodes.get(root).cloned()?;
        let mut visited: HashMap<NodeId, usize> = HashMap::new();
        let mut queue = VecDeque::new();
        queue.push_back((root.clone(), 0usize));
        visited.insert(root.clone(), 0);

        while let Some((node, hops)) = queue.pop_front() {
            if hops >= max_hops {
                continue;
            }
            let edge_ids = self.adjacency.get(&node).cloned().unwrap_or_default();
            for edge_id in edge_ids {
                if let Some(edge) = self.edges.get(&edge_id) {
                    let next = edge.target_node.clone();
                    if !visited.contains_key(&next) {
                        visited.insert(next.clone(), hops + 1);
                        queue.push_back((next, hops + 1));
                    }
                }
            }
        }

        let node_ids: std::collections::HashSet<&NodeId> = visited.keys().collect();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for (id, _) in &visited {
            if let Some(n) = self.nodes.get(id) {
                nodes.push(n.clone());
            }
        }
        for edge in self.edges.values() {
            if node_ids.contains(&edge.source_node) && node_ids.contains(&edge.target_node) {
                edges.push(edge.clone());
            }
        }

        Some(Subgraph {
            nodes,
            edges,
            root: root_node.node_id,
            max_hops,
        })
    }

    pub fn get_owner(&self, resource: &NodeId) -> Option<NodeMetadata> {
        let edges = self.get_incoming_edges(resource, EdgeType::Owns);
        if edges.is_empty() {
            return None;
        }
        self.nodes.get(&edges[0].source_node).cloned()
    }

    pub fn get_dependencies(&self, node: &NodeId) -> Vec<NodeMetadata> {
        self.get_edges(node, EdgeType::DependsOn)
            .iter()
            .filter_map(|e| self.nodes.get(&e.target_node))
            .cloned()
            .collect()
    }

    pub fn get_dependents(&self, node: &NodeId) -> Vec<NodeMetadata> {
        self.get_incoming_edges(node, EdgeType::DependsOn)
            .iter()
            .filter_map(|e| self.nodes.get(&e.source_node))
            .cloned()
            .collect()
    }

    pub fn get_affected(&self, node: &NodeId) -> Vec<NodeMetadata> {
        self.get_incoming_edges(node, EdgeType::Affects)
            .iter()
            .filter_map(|e| self.nodes.get(&e.source_node))
            .cloned()
            .collect()
    }

    pub fn get_health(&self, node: &NodeId) -> crate::protocol::HealthState {
        self.nodes
            .get(node)
            .map(|n| n.health)
            .unwrap_or(crate::protocol::HealthState::Unknown)
    }

    pub fn update_health(&mut self, node: &NodeId, state: crate::protocol::HealthState) -> bool {
        if let Some(n) = self.nodes.get_mut(node) {
            n.health = state;
            n.last_observed = crate::protocol::now();
            true
        } else {
            false
        }
    }

    pub fn mark_stale(&mut self, now: Timestamp) -> Vec<NodeId> {
        let mut stale = Vec::new();
        for node in self.nodes.values_mut() {
            if node.is_stale(now) && node.health != crate::protocol::HealthState::Stale {
                node.health = crate::protocol::HealthState::Stale;
                stale.push(node.node_id.clone());
            }
        }
        stale
    }

    pub fn analyze_impact(&self, resource: &NodeId) -> Option<ImpactReport> {
        if !self.nodes.contains_key(resource) {
            return None;
        }
        let affected = self.get_affected(resource);
        let dependencies = self.get_dependencies(resource);
        let risk_assessment = match affected.len() + dependencies.len() {
            0 => "no known dependencies".to_string(),
            n => format!("{n} related components"),
        };
        Some(ImpactReport {
            resource: resource.clone(),
            affected_nodes: affected,
            dependencies,
            risk_assessment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::HealthState;

    fn t() -> Timestamp {
        1000
    }

    fn node(id: &str, node_type: NodeType) -> NodeMetadata {
        NodeMetadata::new(
            NodeId(id.into()),
            node_type,
            ProvenanceSource::Discovered { via: "udev".into() },
            TrustLevel::Trusted,
            t(),
        )
    }

    fn edge(from: &str, to: &str, edge_type: EdgeType) -> EdgeMetadata {
        EdgeMetadata {
            edge_id: EdgeId::new(),
            edge_type,
            source_node: NodeId(from.into()),
            target_node: NodeId(to.into()),
            provenance: EdgeProvenance::Observed {
                observed_by: PrincipalId::system("mock-discovery"),
                event_type: EventType::DeviceAdded,
            },
            created_at: t(),
            last_observed: t(),
            expires_at: None,
            attributes: HashMap::new(),
        }
    }

    #[test]
    fn add_node_rejects_duplicate() {
        let mut graph = SystemGraph::new();
        graph.add_node(node("dev1", NodeType::Device)).unwrap();
        assert!(graph.add_node(node("dev1", NodeType::Device)).is_err());
    }

    #[test]
    fn add_edge_requires_both_endpoints() {
        let mut graph = SystemGraph::new();
        graph.add_node(node("dev1", NodeType::Device)).unwrap();
        assert!(graph.add_edge(edge("dev1", "missing", EdgeType::DependsOn)).is_err());
        assert!(graph.add_edge(edge("missing", "dev1", EdgeType::DependsOn)).is_err());
        graph.add_node(node("dev2", NodeType::Device)).unwrap();
        let same = edge("dev1", "dev2", EdgeType::DependsOn);
        graph.add_edge(same.clone()).unwrap();
        assert!(graph.add_edge(same).is_err());
    }

    #[test]
    fn owns_edge_enforces_single_owner() {
        let mut graph = SystemGraph::new();
        graph.add_node(node("wifi0", NodeType::Device)).unwrap();
        graph.add_node(node("wifi-specialist", NodeType::Specialist)).unwrap();
        graph.add_node(node("storage-specialist", NodeType::Specialist)).unwrap();
        graph
            .add_edge(edge("wifi-specialist", "wifi0", EdgeType::Owns))
            .unwrap();
        let err = graph
            .add_edge(edge("storage-specialist", "wifi0", EdgeType::Owns))
            .unwrap_err();
        assert!(err.contains("already has an owner"));
        assert_eq!(graph.get_owner(&NodeId("wifi0".into())).unwrap().node_id.0, "wifi-specialist");
    }

    #[test]
    fn dependencies_and_dependents_track_both_directions() {
        let mut graph = SystemGraph::new();
        graph.add_node(node("wifi0", NodeType::Device)).unwrap();
        graph.add_node(node("iwlwifi", NodeType::Driver)).unwrap();
        graph.add_node(node("networkd", NodeType::Service)).unwrap();
        graph.add_edge(edge("wifi0", "iwlwifi", EdgeType::DependsOn)).unwrap();
        graph.add_edge(edge("wifi0", "networkd", EdgeType::DependsOn)).unwrap();

        let wifi = NodeId("wifi0".into());
        let driver = NodeId("iwlwifi".into());
        let deps: Vec<NodeId> = graph
            .get_dependencies(&wifi)
            .into_iter()
            .map(|n| n.node_id)
            .collect();
        assert_eq!(deps.len(), 2);
        let dependents: Vec<NodeId> = graph
            .get_dependents(&driver)
            .into_iter()
            .map(|n| n.node_id)
            .collect();
        assert_eq!(dependents, vec![NodeId("wifi0".into())]);
    }

    #[test]
    fn subgraph_respects_hop_limit() {
        let mut graph = SystemGraph::new();
        for id in ["a", "b", "c"] {
            graph.add_node(node(id, NodeType::Device)).unwrap();
        }
        graph.add_edge(edge("a", "b", EdgeType::DependsOn)).unwrap();
        graph.add_edge(edge("b", "c", EdgeType::DependsOn)).unwrap();
        let one_hop = graph.get_subgraph(&NodeId("a".into()), 1).unwrap();
        assert_eq!(one_hop.nodes.len(), 2);
        let two_hop = graph.get_subgraph(&NodeId("a".into()), 2).unwrap();
        assert_eq!(two_hop.nodes.len(), 3);
        assert!(graph.get_subgraph(&NodeId("missing".into()), 1).is_none());
    }

    #[test]
    fn owner_edge_resolves_owner() {
        let mut graph = SystemGraph::new();
        graph.add_node(node("wifi0", NodeType::Device)).unwrap();
        graph.add_node(node("wifi-specialist", NodeType::Specialist)).unwrap();
        graph
            .add_edge(edge("wifi-specialist", "wifi0", EdgeType::Owns))
            .unwrap();
        let owner = graph.get_owner(&NodeId("wifi0".into())).unwrap();
        assert_eq!(owner.node_id, NodeId("wifi-specialist".into()));
        assert!(graph.get_owner(&NodeId("nonexistent".into())).is_none());
    }

    #[test]
    fn stale_nodes_are_marked() {
        let mut graph = SystemGraph::new();
        let mut stale_node = node("old", NodeType::Sensor);
        stale_node.expires_at = Some(500);
        graph.add_node(stale_node).unwrap();
        graph.add_node(node("fresh", NodeType::Sensor)).unwrap();
        let stale = graph.mark_stale(600);
        assert_eq!(stale, vec![NodeId("old".into())]);
        assert_eq!(graph.get_health(&NodeId("old".into())), HealthState::Stale);
        assert_eq!(
            graph.get_health(&NodeId("fresh".into())),
            HealthState::Unknown
        );
    }

    #[test]
    fn impact_report_counts_related_components() {
        let mut graph = SystemGraph::new();
        graph.add_node(node("wifi0", NodeType::Device)).unwrap();
        graph.add_node(node("iwlwifi", NodeType::Driver)).unwrap();
        graph.add_node(node("planner", NodeType::PlannerAgent)).unwrap();
        graph.add_edge(edge("wifi0", "iwlwifi", EdgeType::DependsOn)).unwrap();
        graph
            .add_edge(edge("planner", "wifi0", EdgeType::Affects))
            .unwrap();
        let report = graph.analyze_impact(&NodeId("wifi0".into())).unwrap();
        assert_eq!(report.dependencies.len(), 1);
        assert_eq!(report.affected_nodes.len(), 1);
        assert_eq!(report.risk_assessment, "2 related components");
        assert!(graph.analyze_impact(&NodeId("missing".into())).is_none());
    }

    #[test]
    fn update_health_returns_whether_node_existed() {
        let mut graph = SystemGraph::new();
        graph.add_node(node("wifi0", NodeType::Device)).unwrap();
        assert!(graph.update_health(&NodeId("wifi0".into()), HealthState::Degraded));
        assert_eq!(
            graph.get_health(&NodeId("wifi0".into())),
            HealthState::Degraded
        );
        assert!(!graph.update_health(&NodeId("missing".into()), HealthState::Healthy));
    }
}
