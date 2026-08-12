use crate::broker::{PolicyBroker, build_request};
use crate::capability::{
    Capability, CapabilityToken, Clearance, Operation, PrincipalId, ResourceId, ResourceState,
    RiskLevel, ToolDefinition,
};
use crate::graph::{
    EdgeId, EdgeMetadata, EdgeProvenance, EdgeType, NodeId, NodeMetadata, NodeType,
    ProvenanceSource, SystemGraph, TrustLevel,
};
use crate::guardian::Guardian;
use crate::protocol::{PolicyVerdict, ToolParameters, now};
use std::collections::HashMap;

pub const HARNESS_PRINCIPAL: &str = "harness.observation";
pub const HARNESS_INSTANCE: &str = "inst-001";
pub const HARNESS_PACKAGE: &str = "harness.specialist";

const RESOURCE_POOL: &[(&str, &str)] = &[
    ("nvme0", "block"),
    ("nvme1", "block"),
    ("eth0", "net"),
    ("eth1", "net"),
    ("usb-1-1", "pci"),
    ("hpet", "char"),
];

const PLAN_OPS: [Operation; 3] = [Operation::Observe, Operation::Diagnose, Operation::Query];

pub fn harness_principal() -> PrincipalId {
    PrincipalId::agent(HARNESS_PRINCIPAL, HARNESS_INSTANCE)
}

pub fn resource_id(name: &str) -> ResourceId {
    ResourceId(format!("device:{name}"))
}

pub fn op_name(operation: Operation) -> &'static str {
    match operation {
        Operation::Observe => "observe",
        Operation::Diagnose => "diagnose",
        Operation::Query => "query",
        _ => "op",
    }
}

pub fn tool_id(resource: &str, operation: Operation) -> String {
    format!("{resource}:{}", op_name(operation))
}

pub fn parameters_for(operation: Operation) -> ToolParameters {
    match operation {
        Operation::Observe => ToolParameters::Observe { fields: Vec::new() },
        Operation::Diagnose => ToolParameters::Diagnose {
            symptom: "report".into(),
        },
        Operation::Query => ToolParameters::Query {
            query: "metrics".into(),
        },
        _ => ToolParameters::Observe { fields: Vec::new() },
    }
}

pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    pub fn range(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

#[derive(Clone, Debug)]
pub struct ResourceSpec {
    pub name: String,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessStep {
    pub index: usize,
    pub resource: String,
    pub operation: Operation,
    pub description: String,
}

#[derive(Clone, Debug)]
pub struct HarnessPlan {
    pub intent: String,
    pub steps: Vec<HarnessStep>,
    pub resources: Vec<ResourceSpec>,
}

pub fn plan(seed: u64) -> HarnessPlan {
    let mut rng = SplitMix64::new(seed.wrapping_mul(0x517CC1B727220A95).wrapping_add(1));
    let resource_count = 2 + rng.range(4);
    let mut pool = RESOURCE_POOL.to_vec();
    let mut resources = Vec::new();
    for _ in 0..resource_count {
        let pick = rng.range(pool.len());
        let (name, kind) = pool.remove(pick);
        resources.push(ResourceSpec {
            name: name.to_string(),
            kind: kind.to_string(),
        });
    }
    resources.sort_by(|a, b| a.name.cmp(&b.name));

    let step_count = 4 + rng.range(6);
    let steps = (0..step_count)
        .map(|index| {
            let resource = &resources[rng.range(resources.len())];
            let operation = PLAN_OPS[rng.range(PLAN_OPS.len())];
            HarnessStep {
                index,
                resource: resource.name.clone(),
                operation,
                description: format!("{} {}", op_name(operation), resource.name),
            }
        })
        .collect();

    HarnessPlan {
        intent: format!("harness campaign seed={seed}"),
        steps,
        resources,
    }
}

pub fn capabilities(plan: &HarnessPlan) -> Vec<Capability> {
    plan.resources
        .iter()
        .flat_map(|r| {
            PLAN_OPS.iter().map(|&operation| Capability {
                resource: resource_id(&r.name),
                operation,
            })
        })
        .collect()
}

pub fn tool_definitions(plan: &HarnessPlan) -> Vec<ToolDefinition> {
    plan.resources
        .iter()
        .flat_map(|r| {
            PLAN_OPS.iter().map(|&operation| ToolDefinition {
                tool_id: tool_id(&r.name, operation),
                specialist_package: HARNESS_PACKAGE.into(),
                risk_level: RiskLevel::ReadOnly,
                required_capabilities: vec![Capability {
                    resource: resource_id(&r.name),
                    operation,
                }],
                description: format!("{} {}", op_name(operation), r.name),
            })
        })
        .collect()
}

pub fn harness_tool_ids() -> Vec<String> {
    RESOURCE_POOL
        .iter()
        .flat_map(|(name, _)| PLAN_OPS.iter().map(move |&op| tool_id(name, op)))
        .collect()
}

#[derive(Default)]
pub struct VirtualFs {
    devices: HashMap<String, DeviceHistory>,
}

#[derive(Default)]
struct DeviceHistory {
    observations: Vec<String>,
    diagnoses: Vec<String>,
    queries: Vec<String>,
}

impl VirtualFs {
    pub fn apply(&mut self, resource: &str, kind: &str, operation: Operation) -> String {
        let device = self.devices.entry(resource.to_string()).or_default();
        match operation {
            Operation::Observe => {
                let line = match kind {
                    "block" => format!("{resource}: firmware=9C2QO8Y3 temp=41C smart=ok"),
                    "net" => format!("{resource}: carrier=1 speed=1000Mb/s link=up"),
                    "pci" => format!("{resource}: speed=5Gbps vendor=0x8086"),
                    _ => format!(
                        "{resource}: resolution=1ns ticks={}",
                        device.observations.len() + 1
                    ),
                };
                device.observations.push(line.clone());
                line
            }
            Operation::Diagnose => {
                let line = format!(
                    "{resource}: no anomalies ({} observation(s))",
                    device.observations.len()
                );
                device.diagnoses.push(line.clone());
                line
            }
            Operation::Query => {
                let line = format!(
                    "{resource}: reads={} writes=0",
                    device.queries.len() + device.observations.len() + 1
                );
                device.queries.push(line.clone());
                line
            }
            _ => format!("{resource}: unsupported operation"),
        }
    }
}

pub struct TestHarness {
    pub broker: PolicyBroker,
    pub guardian: Guardian,
    pub graph: SystemGraph,
    pub plan: HarnessPlan,
    pub vfs: VirtualFs,
    pub enforce: bool,
    seed: u64,
    nonce: u64,
}

impl TestHarness {
    pub fn new(seed: u64, enforce: bool) -> Self {
        let plan = plan(seed);
        let mut broker = PolicyBroker::new();
        for tool in tool_definitions(&plan) {
            broker.register_tool(tool);
        }
        let principal = harness_principal();
        broker.register_principal(principal.clone(), capabilities(&plan), Clearance::max());
        for resource in &plan.resources {
            let id = resource_id(&resource.name);
            broker.set_resource_state(id.clone(), ResourceState::Available);
            broker.set_resource_owner(id, principal.clone());
        }
        let graph = build_graph(&plan);
        Self {
            broker,
            guardian: Guardian::new(),
            graph,
            plan,
            vfs: VirtualFs::default(),
            enforce,
            seed,
            nonce: 0,
        }
    }

    pub fn principal(&self) -> PrincipalId {
        harness_principal()
    }

    pub fn quarantine(&mut self, resource: &str) {
        self.broker
            .set_resource_state(resource_id(resource), ResourceState::Quarantined);
    }

    pub fn submit_tool_request(&mut self, request: crate::protocol::ToolRequest) -> PolicyVerdict {
        self.broker.evaluate(&request)
    }

    pub fn evaluate_step(&mut self, step: &HarnessStep) -> PolicyVerdict {
        let request = self.build_request(step);
        self.submit_tool_request(request)
    }

    fn build_request(&mut self, step: &HarnessStep) -> crate::protocol::ToolRequest {
        self.nonce += 1;
        let token = self.token_for(step);
        build_request(
            harness_principal(),
            resource_id(&step.resource),
            step.operation,
            tool_id(&step.resource, step.operation),
            &token,
            parameters_for(step.operation),
            uuid::Uuid::new_v4(),
            self.nonce,
        )
    }

    fn token_for(&self, step: &HarnessStep) -> CapabilityToken {
        let target = Capability {
            resource: resource_id(&step.resource),
            operation: step.operation,
        };
        self.broker
            .capability_tokens(&harness_principal())
            .into_iter()
            .find(|token| token.capability == target)
            .expect("capability granted for plan step")
    }

    pub fn list_tools(&self) -> Vec<String> {
        tool_definitions(&self.plan)
            .iter()
            .map(|tool| tool.tool_id.clone())
            .collect()
    }

    pub fn run_step(&mut self, index: usize) -> StepReport {
        let step = self.plan.steps.get(index).cloned().expect("step exists");
        let verdict = self.evaluate_step(&step);
        let (allowed, gate) = match verdict {
            PolicyVerdict::Allow => (true, "allow".to_string()),
            PolicyVerdict::Deny(reason) => (false, format!("deny: {reason}")),
        };
        let resource = self
            .plan
            .resources
            .iter()
            .find(|r| r.name == step.resource)
            .expect("resource in plan");
        let (ran, output) = if allowed || !self.enforce {
            let output = self
                .vfs
                .apply(&step.resource, &resource.kind, step.operation);
            (true, output)
        } else {
            (false, String::new())
        };
        StepReport {
            index,
            resource: step.resource.clone(),
            op: op_name(step.operation).to_string(),
            tool_id: tool_id(&step.resource, step.operation),
            gate,
            ran,
            output,
        }
    }

    pub fn run_all(&mut self) -> RunReport {
        let mut report = RunReport {
            intent: self.plan.intent.clone(),
            seed: self.seed,
            enforce: self.enforce,
            total: self.plan.steps.len(),
            allowed: 0,
            denied: 0,
            ran: 0,
            steps: Vec::new(),
            graph_nodes: self.graph.nodes().len(),
            audit_entries: 0,
            stopped_early: false,
        };
        for index in 0..self.plan.steps.len() {
            let step_report = self.run_step(index);
            if step_report.gate.starts_with("deny") {
                report.denied += 1;
                if self.enforce {
                    report.stopped_early = true;
                    report.steps.push(step_report);
                    break;
                }
            } else {
                report.allowed += 1;
            }
            if step_report.ran {
                report.ran += 1;
            }
            report.steps.push(step_report);
        }
        report.audit_entries = self.broker.audit_entries().len();
        report
    }
}

pub fn build_graph(plan: &HarnessPlan) -> SystemGraph {
    let t = now();
    let mut graph = SystemGraph::new();
    for resource in &plan.resources {
        let node = NodeId(format!("device:{}", resource.name));
        let _ = graph.add_node(NodeMetadata::new(
            node,
            NodeType::Device,
            ProvenanceSource::Declared {
                package: HARNESS_PACKAGE.into(),
            },
            TrustLevel::Trusted,
            t,
        ));
    }
    let agent = NodeId(format!("agent:{}", HARNESS_PRINCIPAL));
    let _ = graph.add_node(NodeMetadata::new(
        agent.clone(),
        NodeType::Specialist,
        ProvenanceSource::Declared {
            package: HARNESS_PACKAGE.into(),
        },
        TrustLevel::Trusted,
        t,
    ));
    for resource in &plan.resources {
        let _ = graph.add_edge(EdgeMetadata {
            edge_id: EdgeId::new(),
            edge_type: EdgeType::Observes,
            source_node: agent.clone(),
            target_node: NodeId(format!("device:{}", resource.name)),
            provenance: EdgeProvenance::Declared {
                declared_by: harness_principal(),
                package: HARNESS_PACKAGE.into(),
            },
            created_at: t,
            last_observed: t,
            expires_at: None,
            attributes: Default::default(),
        });
    }
    graph
}

#[derive(Clone, Debug)]
pub struct StepReport {
    pub index: usize,
    pub resource: String,
    pub op: String,
    pub tool_id: String,
    pub gate: String,
    pub ran: bool,
    pub output: String,
}

#[derive(Clone, Debug)]
pub struct RunReport {
    pub intent: String,
    pub seed: u64,
    pub enforce: bool,
    pub total: usize,
    pub allowed: usize,
    pub denied: usize,
    pub ran: usize,
    pub steps: Vec<StepReport>,
    pub graph_nodes: usize,
    pub audit_entries: usize,
    pub stopped_early: bool,
}

impl RunReport {
    pub fn render(&self) -> String {
        let mut lines = vec![
            format!("harness run: {}", self.intent),
            format!(
                "seed={} enforce={} total={} allowed={} denied={} ran={}",
                self.seed, self.enforce, self.total, self.allowed, self.denied, self.ran
            ),
        ];
        for step in &self.steps {
            let output = if step.ran {
                format!("output: {}", step.output)
            } else {
                "NOT RUN".to_string()
            };
            lines.push(format!(
                "  [{:2}] {:<8} {:<8} -> {:<24} {}",
                step.index, step.resource, step.op, step.gate, output
            ));
        }
        if self.stopped_early {
            lines.push("  stopped early: enforcement blocked the campaign".to_string());
        }
        lines.push(format!(
            "graph nodes={} policy decisions logged={}",
            self.graph_nodes, self.audit_entries
        ));
        lines.join("\n")
    }

    pub fn as_json(&self) -> String {
        let steps: Vec<serde_json::Value> = self
            .steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "index": s.index,
                    "resource": s.resource,
                    "op": s.op,
                    "tool_id": s.tool_id,
                    "gate": s.gate,
                    "ran": s.ran,
                    "output": s.output,
                })
            })
            .collect();
        serde_json::json!({
            "intent": self.intent,
            "seed": self.seed,
            "enforce": self.enforce,
            "total": self.total,
            "allowed": self.allowed,
            "denied": self.denied,
            "ran": self.ran,
            "graph_nodes": self.graph_nodes,
            "audit_entries": self.audit_entries,
            "stopped_early": self.stopped_early,
            "steps": steps,
        })
        .to_string()
    }
}

pub fn run_campaign(
    seed: u64,
    enforce: bool,
    quarantine: &[String],
    tool_filter: Option<&str>,
) -> RunReport {
    let mut harness = TestHarness::new(seed, enforce);
    for resource in quarantine {
        harness.quarantine(resource);
    }
    let mut report = harness.run_all();
    if let Some(prefix) = tool_filter {
        report.steps.retain(|s| s.tool_id.starts_with(prefix));
        report.allowed = report.steps.iter().filter(|s| s.gate == "allow").count();
        report.denied = report
            .steps
            .iter()
            .filter(|s| s.gate.starts_with("deny"))
            .count();
        report.ran = report.steps.iter().filter(|s| s.ran).count();
        report.total = report.steps.len();
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_mix_is_deterministic_and_varied() {
        let mut a = SplitMix64::new(7);
        let mut b = SplitMix64::new(7);
        assert_eq!(a.next_u64(), b.next_u64());
        assert_ne!(a.next_u64(), a.next_u64());
    }

    #[test]
    fn plan_is_deterministic_per_seed() {
        let a = plan(42);
        let b = plan(42);
        assert_eq!(a.steps.len(), b.steps.len());
        assert_eq!(a.steps[0].resource, b.steps[0].resource);
        assert_eq!(a.steps[0].operation, b.steps[0].operation);
        let c = plan(43);
        assert_ne!(a.steps.len(), 0);
        assert_ne!(c.steps, a.steps);
    }

    #[test]
    fn plan_resources_are_unique() {
        let p = plan(99);
        let names: Vec<&str> = p.resources.iter().map(|r| r.name.as_str()).collect();
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), names.len());
        assert!(p.resources.len() >= 2);
    }

    #[test]
    fn all_steps_allow_when_capabilities_granted() {
        let mut harness = TestHarness::new(1, false);
        let report = harness.run_all();
        assert_eq!(report.denied, 0, "{report:?}");
        assert_eq!(report.allowed, report.total);
        assert_eq!(report.ran, report.total);
    }

    #[test]
    fn quarantine_denies_affected_steps() {
        let mut harness = TestHarness::new(1, false);
        let affected: Vec<String> = harness
            .plan
            .steps
            .iter()
            .map(|s| s.resource.clone())
            .collect();
        harness.quarantine(&affected[0]);
        let report = harness.run_all();
        let denied: Vec<&StepReport> = report
            .steps
            .iter()
            .filter(|s| s.gate.starts_with("deny"))
            .collect();
        assert!(!denied.is_empty());
        assert!(
            denied.iter().all(|s| s.resource == affected[0]),
            "denials should only hit quarantined resource"
        );
    }

    #[test]
    fn enforce_stops_campaign_at_first_denial() {
        let mut harness = TestHarness::new(1, true);
        let target = harness.plan.steps[0].resource.clone();
        harness.quarantine(&target);
        let report = harness.run_all();
        assert!(report.stopped_early);
        assert!(report.steps.iter().any(|s| s.gate.starts_with("deny")));
        assert!(!report.steps.last().expect("a step ran").ran);
    }

    #[test]
    fn run_all_without_enforce_runs_denied_steps_anyway() {
        let mut harness = TestHarness::new(1, false);
        let target = harness.plan.steps[0].resource.clone();
        harness.quarantine(&target);
        let report = harness.run_all();
        assert!(!report.stopped_early);
        assert!(report.denied > 0);
        assert_eq!(report.ran, report.total);
    }

    #[test]
    fn tools_are_all_read_only() {
        let p = plan(5);
        for tool in tool_definitions(&p) {
            assert_eq!(tool.risk_level, RiskLevel::ReadOnly);
        }
    }

    #[test]
    fn vfs_records_history() {
        let mut vfs = VirtualFs::default();
        let first = vfs.apply("nvme0", "block", Operation::Observe);
        let second = vfs.apply("nvme0", "block", Operation::Observe);
        assert!(first.contains("firmware"));
        assert!(second.contains("firmware"));
        let query = vfs.apply("nvme0", "block", Operation::Query);
        assert!(query.contains("reads=3"), "{query}");
    }
}
