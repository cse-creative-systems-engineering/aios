# Aios System Graph

**Status:** Draft — frozen for M1  
**Depends on:** architecture.md, glossary.md, requirements.md, security-model.md, capability-model.md, message-protocol.md, decisions/0001-v01-runs-above-linux.md, decisions/0003-fail-fast-no-silent-fallbacks.md

## Purpose

Specify the System Graph: what it contains, how it is created, how it is
queried, and how it stays accurate. The graph is a live, typed map of the
system — hardware, OS resources, agents, models, capabilities, and recovery
paths — used for impact analysis, routing, and health.

### Design principles

1. **The graph is advisory, not authoritative.** The Policy Broker is the
   source of truth for permissions. The graph informs routing and analysis
   but never grants authority.
2. **Fail-closed on missing graph data.** If the graph cannot provide
   reliable information about a resource, actions affecting that resource
   are denied (ADR-0003, REQ-SAF-002).
3. **Staleness is visible.** Missing or stale telemetry appears as `UNKNOWN`
   or `STALE`, never silently as healthy (REQ-UX-003).
4. **Discovery is deterministic.** The initial graph is built from Linux
   interfaces (udev, sysfs, procfs). No AI is required for discovery.
5. **Provenance on everything.** Every node and edge carries provenance —
   who or what created it, when, and from what source.

---

## 1. What the Graph Contains

The graph has five layers, each with specific node types:

### 1.1 Physical layer

| Node type | Examples | Discovered via |
|---|---|---|
| `Cpu` | `cpu:0`, `cpu:1` | `/proc/cpuinfo`, sysfs |
| `Memory` | `memory:dimm0` | `/proc/meminfo`, dmidecode |
| `Bus` | `bus:pci0`, `bus:usb0` | sysfs (`/sys/bus/pci`, `/sys/bus/usb`) |
| `Device` | `device:wifi0`, `device:nvme0` | udev, sysfs |
| `Firmware` | `firmware:iwlwifi-ucode` | sysfs, `/lib/firmware` |
| `Sensor` | `sensor:cpu_temp`, `sensor:fan0` | sysfs hwmon |

### 1.2 Operating-system layer

| Node type | Examples | Discovered via |
|---|---|---|
| `Kernel` | `kernel:linux-6.x` | `uname -r` |
| `Driver` | `driver:iwlwifi`, `driver:nvme` | `/proc/modules`, sysfs |
| `Service` | `service:networkd`, `service:systemd-resolved` | systemctl, D-Bus |
| `Filesystem` | `fs:ext4-root`, `fs:btrfs-home` | `/proc/mounts` |
| `Process` | `process:aios-broker` | `/proc/<pid>/` |
| `Namespace` | `net:default`, `mnt:default` | `/proc/<pid>/ns/` |

### 1.3 Agent layer

| Node type | Examples | Created via |
|---|---|---|
| `PlannerAgent` | `agent:planner` | Core package instantiation |
| `VerificationAgent` | `agent:verifier` | Core package instantiation |
| `Specialist` | `agent:wifi0-specialist` | Specialist package instantiation |
| `Guardian` | `agent:guardian` | Guardian package instantiation |
| `Coordinator` | `agent:coordinator` | Coordinator package instantiation |

Agent nodes are created during agent instantiation (see `agent-packages.md`),
not during deterministic discovery. They are linked to the resources they own
via `owns` edges.

### 1.4 Model and gateway layer

| Node type | Examples | Created via |
|---|---|---|
| `LocalModel` | `model:qwen-local` | Model registry |
| `LanGateway` | `gateway:lan-gpu-01` | Explicit pairing |
| `InternetProvider` | `provider:openrouter` | Setup configuration |
| `FallbackRoute` | `fallback:offline-to-local` | Model routing config |

### 1.5 Trust and recovery layer

| Node type | Examples | Created via |
|---|---|---|
| `Capability` | `cap:driver_staging:wifi0` | Broker at agent instantiation |
| `Policy` | `policy:operational-contract` | System configuration |
| `BootImage` | `boot:current`, `boot:previous` | Boot manager (v0.1: read-only) |
| `Snapshot` | `snapshot:pre-driver-update` | Staged executor |
| `Watchdog` | `watchdog:system` | System service |

---

## 2. Edge Types

### 2.1 Edge type registry

| Edge type | Meaning | Direction | Source |
|---|---|---|---|
| `owns` | A component is the authoritative manager for a resource | owner → resource | Declared |
| `depends_on` | One component requires another to function | dependent → dependency | Declared or discovered |
| `communicates_with` | Messages have been exchanged or a channel is declared | a → b (bidirectional) | Observed |
| `observes` | A component receives telemetry from another | observer → observed | Declared |
| `controls` | A capability permits bounded operations on a resource | agent → resource | Attested |
| `affects` | A proposed change may alter another component's behavior | action → resource | Observed |
| `hosted_on` | A service or agent runs on a machine or execution domain | service → host | Declared |
| `fallback_to` | A component or model has a defined fallback path | primary → fallback | Declared |

### 2.2 Edge provenance classes

| Class | Meaning | Trust level | Example |
|---|---|---|---|
| **Declared** | Stated in a package manifest or system configuration | High — verified at signing | "Wi-Fi Specialist owns wifi0" |
| **Attested** | Verified by a trusted component (broker, Guardian) | High — cryptographically verified | "Policy Broker granted driver_staging for req-9182" |
| **Observed** | Detected from runtime events or telemetry | Medium — may be stale or spoofed | "Network Specialist sent ToolRequest to wifi-driver-service" |

The broker treats edges differently by provenance:
- `Declared` and `Attested` edges are trusted for routing and analysis.
- `Observed` edges are used for situational awareness but not for permission
  decisions. They carry freshness metadata and may be marked `STALE`.

### 2.3 Edge constraints

- A resource has exactly one `owns` edge (single owner).
- `controls` edges require a corresponding capability token in the broker.
- `communicates_with` edges are created by observed message traffic and expire
  if no messages are seen within a TTL.
- `affects` edges are transient — created during action planning, removed
  after the action reaches a terminal state.
- `fallback_to` edges form a chain with no cycles (offline → local, LAN →
  local, internet → LAN → local).

---

## 3. Node and Edge Metadata

### 3.1 Node metadata

```rust
pub struct NodeMetadata {
    pub node_id: NodeId,
    pub node_type: NodeType,
    pub label: String,
    pub version: Option<String>,
    pub source: ProvenanceSource,
    pub trust_level: TrustLevel,
    pub health: HealthState,
    pub capabilities: Vec<Capability>,
    pub created_at: Timestamp,
    pub last_observed: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub attributes: HashMap<String, String>,
}
```

### 3.2 Edge metadata

```rust
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

pub enum EdgeProvenance {
    Declared { declared_by: PrincipalId, package: PackageId },
    Attested { attested_by: PrincipalId, signature_verified: bool },
    Observed { observed_by: PrincipalId, event_type: EventType },
}
```

### 3.3 Trust levels

```rust
pub enum TrustLevel {
    Trusted,      // Declared or attested, verified
    Provisional,  // Observed, not yet verified
    Untrusted,    // From an unknown or compromised source
    Unknown,      // No provenance information
}
```

Untrusted or Unknown nodes are not used for routing. Actions affecting
Unknown resources are denied (fail-closed).

---

## 4. Graph Integrity

### 4.1 Staleness detection

Every node and edge has a `last_observed` timestamp and optional `expires_at`.
A node or edge is `STALE` if `now() > expires_at` or if `now() - last_observed`
exceeds a type-specific TTL.

| Node type | TTL (v0.1 default) |
|---|---|
| `Device` | 30 seconds |
| `Service` | 60 seconds |
| `Agent` | 10 seconds |
| `Model` | 120 seconds |
| `Capability` | Does not expire (static) |
| `BootImage` | Does not expire |

Stale nodes are marked `STALE` and surfaced to the System State panel as
`STALE` or `UNKNOWN`, never as healthy.

### 4.2 Conflict detection

| Conflict type | Example | Resolution |
|---|---|---|
| Ownership conflict | Two agents claim to `own` the same resource | Declared ownership from signed package wins. Observed ownership is discarded. |
| Health conflict | Agent reports healthy, telemetry reports degraded | Telemetry from the owning specialist is preferred. Conflicting reports are logged. |
| Dependency conflict | Graph shows dependency A, package declares dependency B | Declared dependency from package is authoritative. Observed dependency is advisory. |

Unresolvable conflicts are marked `UNKNOWN` and the affected resource is
treated as unreliable. Actions affecting it are denied (fail-closed).

### 4.3 Poisoned data handling

If a source is detected as compromised:
- All nodes and edges from that source are marked `Untrusted`.
- `Untrusted` nodes are not used for routing.
- `Untrusted` edges are not used for impact analysis.
- The compromised source may be quarantined.

### 4.4 Incomplete graph

If the graph is incomplete:
- The broker does not infer missing relationships. No guessing.
- Actions affecting resources with incomplete graph data are denied
  (fail-closed).
- The System State panel shows `UNKNOWN` for affected subsystems.
- The user is notified that the graph is incomplete.

---

## 5. Storage Model

### 5.1 v0.1: In-memory

```rust
pub struct SystemGraph {
    nodes: HashMap<NodeId, NodeMetadata>,
    edges: HashMap<EdgeId, EdgeMetadata>,
    adjacency: HashMap<NodeId, Vec<EdgeId>>,
    reverse_adjacency: HashMap<NodeId, Vec<EdgeId>>,
}
```

The graph lives in memory for the duration of the Aios process. It is
rebuilt from discovery on restart. No persistence in v0.1.

### 5.2 v0.2+: Persistence

- Graph state persisted to an embedded database (SQLite or similar).
- On restart, the graph is loaded from storage and reconciled with fresh
  discovery.

### 5.3 Concurrency

- Reads are lock-free (agents get a snapshot via `Arc<SystemGraph>`).
- Writes are serialized through a single writer (the graph service).
- Agents never mutate the graph directly. They send `Event` messages, and
  the graph service applies them.

---

## 6. Query and Projection API

### 6.1 Subgraph extraction

Agents do not receive the entire graph. The graph service returns the
relevant neighborhood for the current task:

```rust
pub trait GraphQuery {
    fn get_node(&self, id: &NodeId) -> Option<NodeMetadata>;
    fn get_edges(&self, from: &NodeId, edge_type: EdgeType) -> Vec<EdgeMetadata>;
    fn get_subgraph(&self, root: &NodeId, max_hops: usize) -> Subgraph;
    fn get_nodes_by_type(&self, node_type: NodeType) -> Vec<NodeMetadata>;
    fn get_owner(&self, resource: &NodeId) -> Option<NodeMetadata>;
    fn get_dependencies(&self, node: &NodeId) -> Vec<NodeMetadata>;
    fn get_affected(&self, node: &NodeId) -> Vec<NodeMetadata>;
    fn get_health(&self, node: &NodeId) -> HealthState;
}
```

### 6.2 Example: Wi-Fi diagnosis context

When the Planner asks about `device:wifi0`, the graph service returns:

```text
Subgraph rooted at device:wifi0 (3 hops):

device:wifi0
  ├── owns ← agent:wifi0-specialist
  ├── depends_on → bus:pci0
  ├── depends_on → driver:iwlwifi
  ├── depends_on → firmware:iwlwifi-ucode
  ├── depends_on → service:networkd
  ├── observes ← agent:wifi0-specialist
  └── controls ← agent:wifi0-specialist (cap:observe, cap:diagnose, cap:stage)

driver:iwlwifi
  ├── depends_on → kernel:linux-6.x
  └── affects → device:wifi0

service:networkd
  ├── depends_on → process:systemd
  └── communicates_with → service:systemd-resolved
```

### 6.3 Impact analysis

Before a staged change, the broker queries the graph for affected nodes:

```rust
fn analyze_impact(&self, resource: &NodeId) -> ImpactReport {
    let affected = self.get_affected(resource);
    let dependencies = self.get_dependencies(resource);
    // ...
}
```

The impact report is passed to the Guardian for review.

---

## 7. Discovery and Maintenance

### 7.1 Phase 1: Deterministic discovery (no AI)

On startup, the graph service builds the initial graph from Linux interfaces:

```text
1. CPU discovery        → /proc/cpuinfo → Cpu nodes
2. Memory discovery     → /proc/meminfo, dmidecode → Memory nodes
3. Bus and device        → udev enumerate → Bus and Device nodes
4. Driver discovery     → /proc/modules, sysfs → Driver nodes
5. Service discovery    → systemctl, D-Bus → Service nodes
6. Filesystem discovery → /proc/mounts → Filesystem nodes
7. Network discovery    → /sys/class/net → Network interface nodes
8. Sensor discovery     → sysfs hwmon → Sensor nodes
9. Boot discovery       → Boot manager config → BootImage nodes (read-only)
```

All discovered nodes have `ProvenanceSource::Discovered` and
`TrustLevel::Trusted`.

### 7.2 Phase 2: Agent instantiation

After discovery, the Agent Package Registry maps graph nodes to packages:

```text
For each Device node:
  Match node type and class to packages in registry
  → If match found: instantiate agent from package
  → Create Agent node in graph
  → Create owns edge: agent → device
  → Create observes edge: agent → device
  → Request capabilities from broker
  → Create controls edges: agent → device (per capability)

For unknown devices:
  → No matching package
  → Create read-only inspector agent (if available)
  → Or leave device unowned (quarantine until package available)
```

Detailed pipeline is defined in `agent-packages.md`.

### 7.3 Event-driven updates

| Event | Graph update |
|---|---|
| `DeviceAdded` | New `Device` node, `depends_on` edges to bus and driver |
| `DeviceRemoved` | Node marked `Removed`, `owns` edge deleted, capabilities revoked |
| `LinkStateChanged` | Device node health updated |
| `ServiceStateChanged` | Service node health updated |
| `ResourceHealthChanged` | Node health updated, `last_observed` refreshed |
| `AgentStarted` | New `Agent` node, `owns`/`observes` edges |
| `AgentTerminated` | Agent node removed, `owns` edges deleted, capabilities revoked |
| `PackageActivated` | Package node created or updated |
| `PackageRevoked` | Package node marked revoked, all instances terminated |

### 7.4 Reconciliation

Periodically (v0.1: every 60 seconds), the graph service re-runs discovery
and reconciles:

- New devices not in the graph → add nodes, trigger agent instantiation.
- Devices in the graph but not discovered → mark `Removed`.
- Services that changed state → update health.
- Stale nodes → mark `STALE`.

---

## 8. Rust Types

```rust
use std::collections::HashMap;
use crate::capability::{Capability, ResourceId, PrincipalId};
use crate::protocol::{Timestamp, EventType, PackageId};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeType {
    Cpu, Memory, Bus, Device, Firmware, Sensor,
    Kernel, Driver, Service, Filesystem, Process, Namespace,
    PlannerAgent, VerificationAgent, Specialist, Guardian, Coordinator,
    LocalModel, LanGateway, InternetProvider, FallbackRoute,
    Capability, Policy, BootImage, Snapshot, Watchdog,
}

#[derive(Clone, Debug)]
pub struct NodeMetadata {
    pub node_id: NodeId,
    pub node_type: NodeType,
    pub label: String,
    pub version: Option<String>,
    pub source: ProvenanceSource,
    pub trust_level: TrustLevel,
    pub health: HealthState,
    pub capabilities: Vec<Capability>,
    pub created_at: Timestamp,
    pub last_observed: Timestamp,
    pub expires_at: Option<Timestamp>,
    pub attributes: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub enum ProvenanceSource {
    Discovered { via: String },
    Declared { package: PackageId },
    Attested { by: PrincipalId },
    Observed { by: PrincipalId },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustLevel { Trusted, Provisional, Untrusted, Unknown }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HealthState { Healthy, Degraded, Unhealthy, Unknown, Stale }

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EdgeId(Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeType {
    Owns, DependsOn, CommunicatesWith, Observes,
    Controls, Affects, HostedOn, FallbackTo,
}

#[derive(Clone, Debug)]
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
pub enum EdgeProvenance {
    Declared { declared_by: PrincipalId, package: PackageId },
    Attested { attested_by: PrincipalId, signature_verified: bool },
    Observed { observed_by: PrincipalId, event_type: EventType },
}

pub struct SystemGraph {
    nodes: HashMap<NodeId, NodeMetadata>,
    edges: HashMap<EdgeId, EdgeMetadata>,
    adjacency: HashMap<NodeId, Vec<EdgeId>>,
    reverse_adjacency: HashMap<NodeId, Vec<EdgeId>>,
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
```

---

## 9. Open questions

1. **Graph persistence format.** v0.1 is in-memory. When v0.2 adds
   persistence, should it be SQLite, a graph database, or a custom format?
   (Recommendation: SQLite — simple, embedded, sufficient for v0.2 scale.)
2. **Graph versioning.** Should the graph maintain history for audit and
   analysis? (Recommendation: no in v0.1. The audit log captures events.)
3. **Cross-host graph.** How does the graph represent remote resources?
   (Recommendation: out of scope for v0.1. Single-host graph only.)
4. **Graph query language.** Should agents query via a structured query
   language, or only via the typed API? (Recommendation: typed API only.)
5. **Dynamic TTL.** Should node TTLs be configurable per resource type, or
   per individual node? (Recommendation: per-type defaults in v0.1.)

---

## References

- `docs/architecture.md` — section 6 (System Graph, Agent Packages), section
  15 (gaps: system graph integrity)
- `docs/security-model.md` — section 4.5 (graph state poisoned), section 3.2
  (tampering: graph state poisoning)
- `docs/capability-model.md` — section 2 (resources), section 7 (tool
  registry)
- `docs/message-protocol.md` — section 2.6 (Event), section 2.8 (HealthReport)
- `docs/agent-packages.md` — will define the agent instantiation pipeline
- `docs/requirements.md` — REQ-FUNC-005, REQ-FUNC-008, REQ-FUNC-009,
  REQ-UX-003
- `docs/decisions/0001-v01-runs-above-linux.md` — discovery via Linux
  interfaces
