use crate::graph::{EdgeType, NodeId, NodeType, SystemGraph};
use std::collections::HashMap;

#[derive(Debug)]
pub enum ToolError {
    Unknown(String),
    Permission(String),
    NotFound(String),
    Ambiguous(Vec<String>),
    Usage(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::Unknown(name) => write!(f, "unknown tool: {name}"),
            ToolError::Permission(reason) => write!(f, "denied: {reason}"),
            ToolError::NotFound(target) => write!(f, "nothing matches: {target}"),
            ToolError::Ambiguous(matches) => {
                write!(f, "ambiguous, matches: {}", matches.join(", "))
            }
            ToolError::Usage(reason) => write!(f, "usage: {reason}"),
        }
    }
}

impl std::error::Error for ToolError {}

#[derive(Debug)]
pub struct ToolResult {
    pub tool: &'static str,
    pub text: String,
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn SpecialistTool>>,
}

pub trait SpecialistTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn summary(&self) -> &'static str;
    fn run(&self, graph: &SystemGraph, args: &str) -> Result<ToolResult, ToolError>;
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: vec![
                Box::new(ObserveDevice),
                Box::new(Diagnose),
                Box::new(QueryNodes),
                Box::new(Dependencies),
                Box::new(Impact),
                Box::new(GraphHealth),
            ],
        }
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|t| t.name()).collect()
    }

    pub fn help(&self) -> String {
        let mut lines = Vec::new();
        for tool in &self.tools {
            lines.push(format!("  {:<12} {}", tool.name(), tool.summary()));
        }
        lines.join("\n")
    }

    pub fn run(
        &self,
        graph: &SystemGraph,
        name: &str,
        args: &str,
    ) -> Result<ToolResult, ToolError> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| ToolError::Unknown(name.to_string()))?;
        tool.run(graph, args)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn require_args(args: &str, usage: &str) -> Result<String, ToolError> {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        Err(ToolError::Usage(usage.to_string()))
    } else {
        Ok(trimmed.to_string())
    }
}

fn find_nodes<'a>(
    graph: &'a SystemGraph,
    needle: &str,
) -> Vec<(&'a NodeId, &'a crate::graph::NodeMetadata)> {
    let needle = needle.to_lowercase();
    let mut found: Vec<(&NodeId, &crate::graph::NodeMetadata)> = graph
        .nodes()
        .iter()
        .filter(|(id, node)| {
            id.to_string().to_lowercase().contains(&needle)
                || node.label.to_lowercase().contains(&needle)
                || node
                    .attributes
                    .values()
                    .any(|v| v.to_lowercase().contains(&needle))
        })
        .collect();
    found.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));
    found
}

fn resolve_one(graph: &SystemGraph, needle: &str) -> Result<NodeId, ToolError> {
    if let Some(node) = graph.get_node(&NodeId(needle.to_string())) {
        return Ok(node.node_id);
    }
    let found = find_nodes(graph, needle);
    match found.len() {
        0 => Err(ToolError::NotFound(needle.to_string())),
        1 => Ok(found[0].0.clone()),
        n => Err(ToolError::Ambiguous(
            found
                .into_iter()
                .take(8)
                .map(|(id, _)| id.to_string())
                .collect::<Vec<_>>()
                .tap(|v| {
                    if n > 8 {
                        v.push(format!("... and {} more", n - 8));
                    }
                }),
        )),
    }
}

trait Tap: Sized {
    fn tap(self, f: impl FnOnce(&mut Self)) -> Self;
}

impl<T: Sized> Tap for T {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}

fn node_line(node: &crate::graph::NodeMetadata) -> String {
    let mut line = format!(
        "  {:?} {} ({}) health={:?}",
        node.node_type,
        node.node_id,
        if node.label.is_empty() {
            "-"
        } else {
            &node.label
        },
        node.health
    );
    if let Some(version) = &node.version {
        line.push_str(&format!(" version={version}"));
    }
    if let Some(expires) = node.expires_at {
        let now = crate::protocol::now();
        if now > expires {
            line.push_str(" [stale]");
        }
    }
    if !node.attributes.is_empty() {
        let mut attrs: Vec<(&String, &String)> = node.attributes.iter().collect();
        attrs.sort_by(|a, b| a.0.cmp(b.0));
        line.push_str(&format!(
            " {{{}}}",
            attrs
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    line
}

fn neighbors(graph: &SystemGraph, id: &NodeId) -> Vec<(EdgeType, NodeId)> {
    let mut out = Vec::new();
    for edge in graph.edges() {
        if edge.source_node == *id {
            out.push((edge.edge_type, edge.target_node.clone()));
        }
        if edge.target_node == *id && edge.edge_type == EdgeType::DependsOn {
            out.push((EdgeType::DependsOn, edge.source_node.clone()));
        }
    }
    out.sort_by(|a, b| format!("{:?}{}", a.0, a.1).cmp(&format!("{:?}{}", b.0, b.1)));
    out
}

struct ObserveDevice;

impl SpecialistTool for ObserveDevice {
    fn name(&self) -> &'static str {
        "observe"
    }
    fn summary(&self) -> &'static str {
        "observe <id|label|attr> - node details, links, owner, attributes"
    }
    fn run(&self, graph: &SystemGraph, args: &str) -> Result<ToolResult, ToolError> {
        let needle = require_args(args, "observe <id|label|attr>")?;
        let id = resolve_one(graph, &needle)?;
        let node = graph
            .get_node(&id)
            .ok_or_else(|| ToolError::NotFound(needle.clone()))?;

        let mut lines = Vec::new();
        lines.push(format!("node: {}", node_line(&node)));

        if let Some(owner) = graph.get_owner(&id) {
            lines.push(format!("owner: {}", owner.node_id));
        }
        let links = neighbors(graph, &id);
        if !links.is_empty() {
            lines.push("links:".into());
            for (edge_type, target) in &links {
                if let Some(target_node) = graph.get_node(target) {
                    lines.push(format!(
                        "  {edge_type:?} -> {} ({:?})",
                        target_node.node_id, target_node.node_type
                    ));
                } else {
                    lines.push(format!("  {edge_type:?} -> {target}"));
                }
            }
        } else {
            lines.push("links: none".into());
        }
        Ok(ToolResult {
            tool: self.name(),
            text: lines.join("\n"),
        })
    }
}

struct Diagnose;

impl SpecialistTool for Diagnose {
    fn name(&self) -> &'static str {
        "diagnose"
    }
    fn summary(&self) -> &'static str {
        "diagnose <id|label|attr> - health and dependency summary for a node"
    }
    fn run(&self, graph: &SystemGraph, args: &str) -> Result<ToolResult, ToolError> {
        let needle = require_args(args, "diagnose <id|label|attr>")?;
        let id = resolve_one(graph, &needle)?;
        let node = graph
            .get_node(&id)
            .ok_or_else(|| ToolError::NotFound(needle.clone()))?;

        let mut lines = Vec::new();
        lines.push(format!("{} ({:?})", node.node_id, node.node_type));
        lines.push(format!("health: {:?}", node.health));
        if let Some(expires) = node.expires_at {
            if crate::protocol::now() > expires {
                lines.push("staleness: expired (stale)".into());
            } else {
                lines.push("staleness: fresh".into());
            }
        }

        let deps = graph.get_dependencies(&id);
        if deps.is_empty() {
            lines.push("dependencies: none".into());
        } else {
            let mut unhealthy = Vec::new();
            lines.push("dependencies:".into());
            for dep in &deps {
                lines.push(format!("  {} health={:?}", dep.node_id, dep.health));
                if matches!(
                    dep.health,
                    crate::protocol::HealthState::Degraded
                        | crate::protocol::HealthState::Unhealthy
                        | crate::protocol::HealthState::Stale
                ) {
                    unhealthy.push(dep.node_id.to_string());
                }
            }
            if unhealthy.is_empty() {
                lines.push("all dependencies look healthy".into());
            } else {
                lines.push(format!(
                    "warning: unhealthy dependencies: {}",
                    unhealthy.join(", ")
                ));
            }
        }

        let dependents = graph.get_dependents(&id);
        if !dependents.is_empty() {
            lines.push(format!(
                "depended on by: {}",
                dependents
                    .iter()
                    .map(|n| n.node_id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(ToolResult {
            tool: self.name(),
            text: lines.join("\n"),
        })
    }
}

struct QueryNodes;

impl SpecialistTool for QueryNodes {
    fn name(&self) -> &'static str {
        "query"
    }
    fn summary(&self) -> &'static str {
        "query <type> - list nodes of a type (device, service, driver, sensor, cpu, all)"
    }
    fn run(&self, graph: &SystemGraph, args: &str) -> Result<ToolResult, ToolError> {
        let needle = require_args(args, "query <type>")?;
        // Deterministic resolution: if the needle is a recognized node type,
        // list all nodes of that type. Otherwise resolve it as a resource
        // reference (system-graph §6) and return the matching node(s).
        let node_type = parse_node_type(&needle).ok();
        let mut nodes: Vec<crate::graph::NodeMetadata> = match node_type {
            Some(Some(t)) => graph.get_nodes_by_type(t),
            Some(None) => graph.nodes().values().cloned().collect(),
            None => {
                let found = find_nodes(graph, &needle);
                if found.is_empty() {
                    return Err(ToolError::NotFound(needle.clone()));
                }
                found.into_iter().map(|(_, node)| node.clone()).collect()
            }
        };
        nodes.sort_by(|a, b| a.node_id.to_string().cmp(&b.node_id.to_string()));
        nodes.dedup_by(|a, b| a.node_id == b.node_id);

        if nodes.is_empty() {
            return Ok(ToolResult {
                tool: self.name(),
                text: format!("no {needle} nodes found"),
            });
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        for node in &nodes {
            *counts.entry(format!("{:?}", node.node_type)).or_default() += 1;
        }
        let mut counts: Vec<(String, usize)> = counts.into_iter().collect();
        counts.sort();
        let header = counts
            .iter()
            .map(|(t, c)| format!("{c} {t}"))
            .collect::<Vec<_>>()
            .join(", ");

        let mut lines = vec![format!("{}: {header}", nodes.len())];
        for node in nodes {
            lines.push(node_line(&node));
        }
        Ok(ToolResult {
            tool: self.name(),
            text: lines.join("\n"),
        })
    }
}

fn parse_node_type(needle: &str) -> Result<Option<NodeType>, ToolError> {
    match needle {
        "all" => Ok(None),
        "device" | "devices" => Ok(Some(NodeType::Device)),
        "service" | "services" => Ok(Some(NodeType::Service)),
        "driver" | "drivers" => Ok(Some(NodeType::Driver)),
        "sensor" | "sensors" => Ok(Some(NodeType::Sensor)),
        "cpu" | "cpus" => Ok(Some(NodeType::Cpu)),
        "kernel" => Ok(Some(NodeType::Kernel)),
        "memory" => Ok(Some(NodeType::Memory)),
        "process" | "processes" => Ok(Some(NodeType::Process)),
        "bus" | "buses" => Ok(Some(NodeType::Bus)),
        "filesystem" | "filesystems" => Ok(Some(NodeType::Filesystem)),
        "firmware" => Ok(Some(NodeType::Firmware)),
        "package" | "packages" => Ok(Some(NodeType::Package)),
        other => Err(ToolError::Usage(format!(
            "unknown type '{other}' (device, service, driver, sensor, cpu, kernel, memory, bus, filesystem, firmware, package, all)"
        ))),
    }
}

struct Dependencies;

impl SpecialistTool for Dependencies {
    fn name(&self) -> &'static str {
        "deps"
    }
    fn summary(&self) -> &'static str {
        "deps <id|label|attr> - full dependency chain of a node"
    }
    fn run(&self, graph: &SystemGraph, args: &str) -> Result<ToolResult, ToolError> {
        let needle = require_args(args, "deps <id|label|attr>")?;
        let id = resolve_one(graph, &needle)?;
        let mut lines = Vec::new();
        walk_deps(graph, &id, 0, &mut lines, &mut Vec::new());
        Ok(ToolResult {
            tool: self.name(),
            text: lines.join("\n"),
        })
    }
}

fn walk_deps(
    graph: &SystemGraph,
    id: &NodeId,
    depth: usize,
    lines: &mut Vec<String>,
    visited: &mut Vec<NodeId>,
) {
    if visited.contains(id) {
        lines.push(format!("{}-> {id} (cycle)", "  ".repeat(depth)));
        return;
    }
    visited.push(id.clone());
    if let Some(node) = graph.get_node(id) {
        lines.push(format!(
            "{}-> {} ({:?}) {:?}",
            "  ".repeat(depth),
            node.node_id,
            node.node_type,
            node.health
        ));
    } else {
        lines.push(format!("{}-> {id} (missing)", "  ".repeat(depth)));
        return;
    }
    let mut deps = graph.get_dependencies(id);
    deps.sort_by(|a, b| a.node_id.to_string().cmp(&b.node_id.to_string()));
    for dep in deps {
        walk_deps(graph, &dep.node_id, depth + 1, lines, visited);
    }
}

struct Impact;

impl SpecialistTool for Impact {
    fn name(&self) -> &'static str {
        "impact"
    }
    fn summary(&self) -> &'static str {
        "impact <id|label|attr> - what depends on this node and what it depends on"
    }
    fn run(&self, graph: &SystemGraph, args: &str) -> Result<ToolResult, ToolError> {
        let needle = require_args(args, "impact <id|label|attr>")?;
        let id = resolve_one(graph, &needle)?;
        let report = graph
            .analyze_impact(&id)
            .ok_or_else(|| ToolError::NotFound(needle.clone()))?;
        let mut lines = vec![format!(
            "resource: {} ({})",
            report.resource,
            graph
                .get_node(&report.resource)
                .map(|n| format!("{:?}", n.node_type))
                .unwrap_or_default()
        )];
        lines.push(format!("risk: {}", report.risk_assessment));
        if report.dependencies.is_empty() {
            lines.push("dependencies: none".into());
        } else {
            lines.push("dependencies:".into());
            for node in &report.dependencies {
                lines.push(format!(
                    "  {} ({:?}) {:?}",
                    node.node_id, node.node_type, node.health
                ));
            }
        }
        if report.affected_nodes.is_empty() {
            lines.push("affected by: none".into());
        } else {
            lines.push("affected by:".into());
            for node in &report.affected_nodes {
                lines.push(format!("  {} ({:?})", node.node_id, node.node_type));
            }
        }
        Ok(ToolResult {
            tool: self.name(),
            text: lines.join("\n"),
        })
    }
}

struct GraphHealth;

impl SpecialistTool for GraphHealth {
    fn name(&self) -> &'static str {
        "health"
    }
    fn summary(&self) -> &'static str {
        "health - roll up node health across the graph"
    }
    fn run(&self, graph: &SystemGraph, args: &str) -> Result<ToolResult, ToolError> {
        if !args.trim().is_empty() {
            return Err(ToolError::Usage("health takes no arguments".into()));
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut by_type: HashMap<String, HashMap<String, usize>> = HashMap::new();
        for node in graph.nodes().values() {
            *counts.entry(format!("{:?}", node.health)).or_default() += 1;
            let t = format!("{:?}", node.node_type);
            *by_type
                .entry(t.clone())
                .or_default()
                .entry(format!("{:?}", node.health))
                .or_default() += 1;
        }
        let mut lines = vec![format!("{} nodes total", graph.nodes().len())];
        let mut health: Vec<(String, usize)> = counts.into_iter().collect();
        health.sort();
        lines.push(
            health
                .iter()
                .map(|(h, c)| format!("{h}: {c}"))
                .collect::<Vec<_>>()
                .join(", "),
        );
        let mut types: Vec<(String, HashMap<String, usize>)> = by_type.into_iter().collect();
        types.sort_by(|a, b| a.0.cmp(&b.0));
        for (t, states) in types {
            let mut states: Vec<(String, usize)> = states.into_iter().collect();
            states.sort();
            lines.push(format!(
                "  {t}: {}",
                states
                    .iter()
                    .map(|(h, c)| format!("{c} {h}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        Ok(ToolResult {
            tool: self.name(),
            text: lines.join("\n"),
        })
    }
}

pub fn tools_context(graph: &SystemGraph) -> String {
    let mut lines = Vec::new();
    let sensors: Vec<crate::graph::NodeMetadata> = graph.get_nodes_by_type(NodeType::Sensor);
    let memory: Vec<crate::graph::NodeMetadata> = graph.get_nodes_by_type(NodeType::Memory);
    let processes: Vec<crate::graph::NodeMetadata> = graph.get_nodes_by_type(NodeType::Process);
    let devices: Vec<crate::graph::NodeMetadata> = graph.get_nodes_by_type(NodeType::Device);
    let services: Vec<crate::graph::NodeMetadata> = graph.get_nodes_by_type(NodeType::Service);
    if devices.is_empty()
        && services.is_empty()
        && sensors.is_empty()
        && memory.is_empty()
        && processes.is_empty()
    {
        return String::new();
    }
    if !devices.is_empty() {
        lines.push(format!("{} devices:", devices.len()));
        for node in devices.iter().take(24) {
            lines.push(format!(
                "  {} {} {:?}",
                node.node_id, node.label, node.health
            ));
        }
        if devices.len() > 24 {
            lines.push(format!("  ... and {} more", devices.len() - 24));
        }
    }
    if !services.is_empty() {
        lines.push(format!("{} services:", services.len()));
        for node in services.iter().take(24) {
            lines.push(format!("  {} {:?}", node.node_id, node.health));
        }
        if services.len() > 24 {
            lines.push(format!("  ... and {} more", services.len() - 24));
        }
    }
    for (label, nodes) in [("sensors", sensors), ("memory", memory)] {
        if !nodes.is_empty() {
            lines.push(format!("{label}:"));
            for node in nodes {
                lines.push(format!(
                    "  {} {} {:?}",
                    node.node_id, node.label, node.attributes
                ));
            }
        }
    }
    if !processes.is_empty() {
        lines.push(format!("{} processes:", processes.len()));
        for node in processes.iter().take(8) {
            lines.push(format!(
                "  {} {} {:?}",
                node.node_id, node.label, node.attributes
            ));
        }
        if processes.len() > 8 {
            lines.push(format!("  ... and {} more", processes.len() - 8));
        }
    }
    lines.join("\n")
}

/// Compact, discoverable index of all resources by type.
///
/// Per system-graph §6.1, agents receive a relevant projection of the graph,
/// not the entire graph. This function returns a compact listing of valid
/// target identifiers (node IDs and labels) that the model can use with
/// observe/diagnose/deps/impact tools. Full attribute dumps are NOT included
/// here — the model calls the tool with a target to get full details.
pub fn resource_index(graph: &SystemGraph) -> String {
    let mut lines = Vec::new();

    let devices = graph.get_nodes_by_type(NodeType::Device);
    if !devices.is_empty() {
        lines.push(format!("{} devices:", devices.len()));
        for node in &devices {
            lines.push(format!("  {} ({})", node.node_id, node.label));
        }
    }

    let services: Vec<_> = graph.get_nodes_by_type(NodeType::Service);
    if !services.is_empty() {
        lines.push(format!("{} services:", services.len()));
        for node in services.iter().take(24) {
            lines.push(format!("  {} health={:?}", node.node_id, node.health));
        }
        if services.len() > 24 {
            lines.push(format!("  ... and {} more", services.len() - 24));
        }
    }

    let sensors = graph.get_nodes_by_type(NodeType::Sensor);
    if !sensors.is_empty() {
        lines.push(format!("{} sensors:", sensors.len()));
        for node in &sensors {
            lines.push(format!("  {} ({})", node.node_id, node.label));
        }
    }

    let drivers = graph.get_nodes_by_type(NodeType::Driver);
    if !drivers.is_empty() {
        lines.push(format!("{} drivers:", drivers.len()));
        for node in &drivers {
            lines.push(format!("  {} ({})", node.node_id, node.label));
        }
    }

    let cpus = graph.get_nodes_by_type(NodeType::Cpu);
    if !cpus.is_empty() {
        lines.push(format!("{} cpus", cpus.len()));
    }

    let memory = graph.get_nodes_by_type(NodeType::Memory);
    if !memory.is_empty() {
        lines.push(format!("{} memory nodes", memory.len()));
    }

    let processes = graph.get_nodes_by_type(NodeType::Process);
    if !processes.is_empty() {
        lines.push(format!("{} processes", processes.len()));
    }

    let firmware = graph.get_nodes_by_type(NodeType::Firmware);
    if !firmware.is_empty() {
        lines.push(format!("{} firmware", firmware.len()));
    }

    let filesystems = graph.get_nodes_by_type(NodeType::Filesystem);
    if !filesystems.is_empty() {
        lines.push(format!("{} filesystems", filesystems.len()));
    }

    let packages = graph.get_nodes_by_type(NodeType::Package);
    if !packages.is_empty() {
        lines.push(format!("{} packages", packages.len()));
    }

    if lines.is_empty() {
        return "no resources discovered".to_string();
    }
    lines.join("\n")
}

pub fn model_tool_instructions() -> &'static str {
    "Read-only machine tools are available. Never invent command output and never claim to run shell commands. The available tools are: observe, diagnose, query, deps, impact, health, wifi.observe_device, wifi.diagnose_fault, storage.observe_storage, storage.diagnose_fault, network.observe_network, network.diagnose_fault, drivers.observe_device, drivers.diagnose_fault, graphics.observe_graphics, graphics.diagnose_fault, memory.observe_memory, memory.diagnose_fault, processes.observe_process, processes.diagnose_fault, power.observe_thermal, power.diagnose_fault, security.observe_security, security.diagnose_fault, packages.observe_package, packages.diagnose_fault, boot.observe_boot, boot.diagnose_fault. To use a tool, emit a native function call: {\"tool_calls\":[{\"function\":{\"name\":\"<tool>\",\"arguments\":\"<args>\"}}]} where <tool> is exactly one of the available tools and <args> is a plain string argument. For every question about CPU utilization or process load, always call processes.observe_process with target 'all' before answering. Never answer those questions from context alone. Use query sensor for sensor readings, query memory for memory data, and query device for hardware. For a Wi-Fi device, use wifi.observe_device to read its state and wifi.diagnose_fault to diagnose it. For storage, use storage.observe_storage to read disk and filesystem state and storage.diagnose_fault to diagnose it. For the network domain, use network.observe_network to read interface and link state and network.diagnose_fault to diagnose it. For generic hardware, use drivers.observe_device to read device, driver, firmware, and module state and drivers.diagnose_fault to diagnose it (target 'all' for the whole domain). For graphics, use graphics.observe_graphics to read GPU, display, and session state and graphics.diagnose_fault to diagnose it (target 'all' for the whole domain). For memory, use memory.observe_memory to read total, used, free, swap, and pressure state and memory.diagnose_fault to diagnose it (target 'all' for the whole domain). For processes, use processes.observe_process to read per-process CPU percent, memory (RSS), state, and command line plus system-wide CPU utilization and core count, and processes.diagnose_fault to diagnose it (target 'all' reports the domain's top processes by CPU percent). For power and thermal, use power.observe_thermal to read temperature, fan, and power state and power.diagnose_fault to diagnose it (target 'all' for the whole domain). For security and identity, use security.observe_security to read identity, trust, and security state and security.diagnose_fault to diagnose it (target 'all' for the whole domain). For packages, use packages.observe_package to read package, version, and signature state and packages.diagnose_fault to diagnose it (target 'all' for the whole domain). For boot and recovery, use boot.observe_boot to read boot state and recovery-image availability and boot.diagnose_fault to diagnose it (target 'all' for the whole domain). After receiving tool results, answer only from those results. If a tool cannot establish a fact, say so."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::PrincipalId;
    use crate::graph::{
        EdgeId, EdgeMetadata, EdgeProvenance, NodeMetadata, ProvenanceSource, TrustLevel,
    };
    use crate::protocol::{EventType, HealthState};

    fn t() -> crate::protocol::Timestamp {
        1000
    }

    fn node(id: &str, node_type: NodeType, label: &str, health: HealthState) -> NodeMetadata {
        let mut n = NodeMetadata::new(
            NodeId(id.into()),
            node_type,
            ProvenanceSource::Discovered { via: "test".into() },
            TrustLevel::Provisional,
            t(),
        );
        n.label = label.into();
        n.health = health;
        n
    }

    fn edge(from: &str, to: &str, edge_type: EdgeType) -> EdgeMetadata {
        EdgeMetadata {
            edge_id: EdgeId::new(),
            edge_type,
            source_node: NodeId(from.into()),
            target_node: NodeId(to.into()),
            provenance: EdgeProvenance::Observed {
                observed_by: PrincipalId::system("test"),
                event_type: EventType::DeviceAdded,
            },
            created_at: t(),
            last_observed: t(),
            expires_at: None,
            attributes: HashMap::new(),
        }
    }

    fn fixture() -> SystemGraph {
        let mut graph = SystemGraph::new();
        graph
            .add_node(node(
                "dev:wifi0",
                NodeType::Device,
                "Wireless controller",
                HealthState::Healthy,
            ))
            .unwrap();
        graph
            .add_node(node(
                "dev:eth0",
                NodeType::Device,
                "Ethernet controller",
                HealthState::Healthy,
            ))
            .unwrap();
        graph
            .add_node(node(
                "driver:iwlwifi",
                NodeType::Driver,
                "kernel driver iwlwifi",
                HealthState::Healthy,
            ))
            .unwrap();
        graph
            .add_node(node(
                "svc:networkd",
                NodeType::Service,
                "systemd-networkd",
                HealthState::Degraded,
            ))
            .unwrap();
        graph
            .add_edge(edge("dev:wifi0", "driver:iwlwifi", EdgeType::DependsOn))
            .unwrap();
        graph
            .add_edge(edge("dev:wifi0", "svc:networkd", EdgeType::DependsOn))
            .unwrap();
        graph
    }

    #[test]
    fn observe_resolves_by_label_and_shows_links() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let result = registry.run(&graph, "observe", "wifi0").expect("observe");
        assert!(result.text.contains("dev:wifi0"), "{}", result.text);
        assert!(result.text.contains("driver:iwlwifi"), "{}", result.text);
        assert!(result.text.contains("svc:networkd"), "{}", result.text);
    }

    #[test]
    fn observe_ambiguous_match_reports_all() {
        let mut graph = fixture();
        graph
            .add_node(node(
                "dev:wifi1",
                NodeType::Device,
                "Wireless controller 2",
                HealthState::Unknown,
            ))
            .unwrap();
        let registry = ToolRegistry::new();
        let err = registry.run(&graph, "observe", "wireless").unwrap_err();
        assert!(matches!(err, ToolError::Ambiguous(_)), "{err}");
        assert!(err.to_string().contains("dev:wifi0"));
        assert!(err.to_string().contains("dev:wifi1"));
    }

    #[test]
    fn observe_missing_is_not_found() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let err = registry
            .run(&graph, "observe", "definitely-missing")
            .unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)), "{err}");
    }

    #[test]
    fn diagnose_reports_unhealthy_dependency() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let result = registry
            .run(&graph, "diagnose", "dev:wifi0")
            .expect("diagnose");
        assert!(
            result.text.contains("warning: unhealthy dependencies"),
            "{}",
            result.text
        );
        assert!(result.text.contains("svc:networkd"), "{}", result.text);
    }

    #[test]
    fn query_lists_by_type_and_counts() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let result = registry.run(&graph, "query", "device").expect("query");
        assert!(result.text.contains("2 Device"), "{}", result.text);
        assert!(result.text.contains("dev:wifi0"), "{}", result.text);
        assert!(result.text.contains("dev:eth0"), "{}", result.text);
    }

    #[test]
    fn query_unknown_type_resolves_as_resource_or_not_found() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        // A free-text target that matches a node resolves (system-graph §6).
        let result = registry
            .run(&graph, "query", "wifi0")
            .expect("free-text resolves");
        assert!(result.text.contains("dev:wifi0"), "{}", result.text);
        // A target matching nothing is NotFound, not a usage error.
        let err = registry.run(&graph, "query", "teleporter").unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)), "{err}");
    }

    #[test]
    fn deps_renders_chain() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let result = registry.run(&graph, "deps", "dev:wifi0").expect("deps");
        assert!(result.text.contains("driver:iwlwifi"), "{}", result.text);
        assert!(result.text.contains("svc:networkd"), "{}", result.text);
    }

    #[test]
    fn impact_counts_related() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let result = registry.run(&graph, "impact", "dev:wifi0").expect("impact");
        assert!(
            result.text.contains("2 related components"),
            "{}",
            result.text
        );
    }

    #[test]
    fn health_rolls_up_states() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let result = registry.run(&graph, "health", "").expect("health");
        assert!(result.text.contains("Healthy: 3"), "{}", result.text);
        assert!(result.text.contains("Degraded: 1"), "{}", result.text);
    }

    #[test]
    fn unknown_tool_is_rejected() {
        let graph = fixture();
        let registry = ToolRegistry::new();
        let err = registry.run(&graph, "rm", "-rf /").unwrap_err();
        assert!(matches!(err, ToolError::Unknown(_)), "{err}");
    }

    #[test]
    fn tools_context_mentions_devices_and_services() {
        let graph = fixture();
        let ctx = tools_context(&graph);
        assert!(ctx.contains("dev:wifi0"), "{ctx}");
        assert!(ctx.contains("svc:networkd"), "{ctx}");
    }

    #[test]
    fn resource_index_lists_valid_targets_compactly() {
        let graph = fixture();
        let index = resource_index(&graph);
        // Device labels are included so the model can discover valid targets.
        assert!(index.contains("dev:wifi0"), "{index}");
        assert!(index.contains("Wireless controller"), "{index}");
        assert!(index.contains("dev:eth0"), "{index}");
        // The driver node is listed as a target.
        assert!(index.contains("driver:iwlwifi"), "{index}");
        // The index is a compact projection: it must not dump dependency
        // chains or full attribute maps (system-graph §6.1).
        assert!(!index.contains("->"), "{index}");
    }
}
