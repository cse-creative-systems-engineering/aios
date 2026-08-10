# Aios Architecture

**Status:** Vision (essay, not contract — contracts are source of truth)  
**Depends on:** glossary.md, requirements.md, all contract docs, all ADRs

**Aios** means **Artificially Intelligent Operating System**.

This document is the **vision and principles** record for Aios. It describes
what Aios should be and why. Implementation contracts, protocol schemas, and
detailed specifications live in focused documents under `docs/`. This document
links to them where relevant and should be updated when a principle changes.

**Primary design goal:** make an operating environment that is intelligent and
useful without allowing probabilistic agents to become an uncontrolled safety
or security boundary.

## Document index

| Document | Purpose | Status |
|---|---|---|
| `architecture.md` (this doc) | Vision and principles | Accepted |
| `glossary.md` | Shared terminology | Draft |
| `requirements.md` | Functional and non-functional requirements | Draft |
| `decisions/` | Architecture Decision Records | 0001–0004 accepted |
| `doc-progress.md` | Documentation completion tracker | Living document |
| `security-model.md` | Threat model and trust boundaries | Draft |
| `capability-model.md` | Authorization system (capability × risk level) | Draft |
| `message-protocol.md` | Typed internal protocol | Draft |
| `action-state-machine.md` | Transaction and recovery states | Draft |
| `system-graph.md` | Graph specification | Draft |
| `agent-packages.md` | Package manifest and registry | Draft |
| `model-routing.md` | Model gateway and provider routing | Draft |
| `implementation-roadmap.md` | Milestones and dependencies | Draft |
| `testing-strategy.md` | Verification and evaluation | Draft |
| `observability.md` | Audit, tracing, and logging | Draft |
| `modules/` | Per-specialist specifications | (none yet) |

## 1. Core idea

Aios presents one conversational interface to the user, but internally it is a coordinated system of specialized agents and deterministic services.

The user should be able to express an intent such as:

> “Get this Wi-Fi device working.”

Aios should decompose that intent into diagnosis, planning, verification, execution, and recovery steps. It may discover hardware, research compatible drivers, build and test code, or ask for approval. It must not silently make an unsafe kernel-level change merely because an agent believes the change is reasonable.

The central safety principle is:

> No component should both make an autonomous decision and possess unrestricted authority to execute it.

### Development principle: fail fast, no silent fallbacks

During development, every error, ambiguity, missing capability, stale state, or
unexpected condition must cause an immediate and visible failure. No fallback
paths exist unless explicitly designed, discussed, and documented. See
[ADR-0003](decisions/0003-fail-fast-no-silent-fallbacks.md).

This is the development-time expression of the production fail-closed
principle (REQ-SAF-002). Silent fallbacks hide bugs; in a safety-critical
system, a hidden bug is a latent safety failure.

## 2. Architecture at a glance

```text
                         User
                          │
                          ▼
              Aios conversational facade
                          │
                          ▼
                   Session coordinator
                          │
              ┌───────────┴───────────┐
              ▼                       ▼
       Planner Agent          Verification Agent
              │                       │
              └───────────┬───────────┘
                          ▼
               Domain and hardware specialists
                          │
                          ▼
                Infrastructure Guardian
                   (read-only veto)
                          │
                          ▼
              Deterministic policy broker
             (capabilities and enforcement)
                          │
                          ▼
               Staged transaction executor
                          │
                          ▼
             OS services, drivers, and kernel
                          │
                          ▼
               Hardware and recovery layer
```

The diagram shows logical responsibility, not necessarily the path taken by every message. Agents may communicate directly through an internal message bus, but no message may bypass the policy and capability rules that govern its action.

## 3. Three planes of the system

### Agent plane

This is the reasoning and coordination layer:

- User-facing conversational facade
- Planner Agent
- Verification Agent
- Domain specialists
- Hardware specialists
- Diagnostics and explanation services

Agents propose, analyze, explain, and monitor. They do not automatically receive unrestricted operating-system authority.

### Enforcement plane

This is the execution and safety layer:

- Policy Broker (capability and clearance enforcement)
- Permission and policy engine
- Typed tool interfaces
- Staged Transaction Executor (checkpoint, stage, health, commit/rollback)
- Health checks
- Audit log

The enforcement plane validates and executes actions. It must remain deterministic wherever possible and must not trust a model’s response as proof of safety.

### Trust plane

This is the lowest-level recovery and integrity foundation:

- Firmware and boot verification
- Kernel primitives
- Memory and process isolation
- IOMMU and DMA protection
- Watchdogs
- Known-good kernel and recovery images
- Recovery supervisor

The trust plane must remain functional if the agent plane, message bus, or main operating environment fails. Aios should lose intelligence before it loses the ability to recover safely.

## 4. The dual-agent bridge

The dual-agent design refers to the two primary reasoning roles between the user and the operating system.

| Role | Responsibility | Direct execution authority |
| --- | --- | --- |
| Planner Agent | Understand intent, inspect available options, and produce a structured plan | None by default |
| Verification Agent | Independently challenge the intent, plan, assumptions, risks, and expected effects | None by default |

The user sees one Aios response. Internally, the Planner and Verification Agent may use different prompts, models, tools, or reasoning strategies. The Verification Agent should receive the original intent, the proposed plan, and relevant system state rather than simply trusting the Planner’s conclusions.

“Constantly checking” should mean checking at meaningful decision boundaries—not invoking two language models for every trivial read. Simple health queries can be answered by deterministic subsystem services. High-impact decisions receive deeper review.

Agreement between the two agents is not proof of correctness. They may share the same blind spot or be fooled by the same input. Their output is therefore advisory until accepted by the enforcement plane.

## 5. Domain and hardware specialists

Aios may have a specialist for each meaningful functional domain, but not necessarily an independent language model for every small kernel subsystem.

Possible domains include:

- Boot and recovery
- Processes and resource management
- Memory
- Storage and filesystems
- Networking and Wi-Fi
- Drivers and hardware
- Security and identity
- Packages and updates
- Power and thermal management
- Graphics and user sessions

A **Subsystem Specialist** is a bounded domain service. A **Hardware Specialist** is a Subsystem Specialist whose primary ownership boundary is a physical device or hardware family. Either may contain deterministic code, an AI-assisted diagnostic component, or both. Its system context should describe:

- The domain it owns
- Its dependencies
- Its health indicators
- Its allowed operations
- Its failure modes
- Its escalation rules
- Its recovery procedures

The prompt or context describes responsibility; a separate capability system grants authority.

### Hardware depth

Hardware agents should become less autonomous as they approach the hardware boundary.

```text
User-facing reasoning
        │ flexible, probabilistic
        ▼
Domain and hardware diagnosis
        │ constrained recommendations
        ▼
Driver and device services
        │ typed operations and isolation
        ▼
Firmware and hardware controllers
        │ deterministic limits and fail-safe behavior
        ▼
Physical hardware
```

Existing hardware already contains controllers and firmware in devices such as SSDs, network cards, GPUs, embedded controllers, and TPMs. Aios should initially supervise and coordinate those components rather than assume it can replace them.

Language models should not be placed directly in real-time control loops for memory protection, DMA, interrupt handling, voltage, thermal safety, or similar functions. AI may diagnose, predict, or recommend; deterministic controllers must enforce hard limits.

### Specialist agents as tools

The central Aios agents should not need to understand every hardware implementation detail. Instead, each meaningful hardware/system boundary exposes a specialized agent through a typed tool interface.

```text
                    Dual Aios agents
                 Planner + Verification
                            │
                    specialist tool calls
                            ▼
                 Domain and hardware agents
       ┌──────────────┬──────────────┬──────────────┐
       ▼              ▼              ▼              ▼
   Wi-Fi tool     Storage tool   Power tool    Driver tool
       │              │              │              │
       └──────────────┴──────────────┴──────────────┘
                            │
                    typed broker requests
                            ▼
                 OS services and kernel APIs
                            │
                         Hardware
```

For example, a Wi-Fi specialist may understand PCI or USB identifiers, firmware requirements, driver compatibility, link state, resets, and recovery. The Planner does not need to reproduce that expertise in its own prompt; it calls the Wi-Fi specialist as a tool.

The word “agent” describes the specialist’s behavior and ownership. The word “tool” describes how the higher-level agents access it. Internally, a specialist may be a deterministic Rust service, an AI-assisted diagnostic process, or a combination of both.

A specialist tool should expose bounded operations such as:

```text
observe_device()
get_health()
diagnose_fault()
propose_repair()
stage_change()
verify_change()
request_reset()
```

It should not expose an unrestricted operation such as `run_any_command()` or `write_any_memory_address()`. Read-only observation is the default. Mutating operations require capabilities and pass through the policy broker, staging system, and Infrastructure Guardian where applicable.

Hardware events should also be structured before reaching an AI agent:

```text
DeviceAdded { bus: "pci", id: "...", class: "network" }
LinkStateChanged { device: "wifi0", state: "down", reason: "firmware" }
TemperatureWarning { device: "nvme0", celsius: 78 }
MemoryEccError { bank: 3, corrected: true }
```

This keeps the specialist focused on meaningful system state rather than requiring it to interpret arbitrary register dumps or untrusted raw messages.

Each resource should have a clear owning specialist. Other agents may request information or actions from that owner, but two agents should not independently control the same hardware resource. Cross-domain operations are coordinated through the broker and dependency graph.

This creates a useful division of labor:

```text
Dual Aios agents:
  understand user intent, plan, compare, and explain

Specialist agents:
  understand a subsystem, observe it, and expose safe operations

Policy broker and trust plane:
  decide whether operations are permitted and enforce the boundary
```

## 6. Hierarchy versus dependency graph

A hierarchy is useful for organizing ownership and escalation:

```text
Aios
├── Security Specialist
├── Storage Specialist
├── Network Specialist
│   └── Wi-Fi Specialist
├── Driver and Hardware Specialist
├── Process and Resource Specialist
└── Boot and Recovery Specialist
```

However, hardware and software dependencies are not a tree. Wi-Fi may depend on PCIe, power management, firmware, storage, security, and the network stack at the same time.

Therefore:

- Use a hierarchy for ownership, delegation, and human-readable organization.
- Use a dependency graph for impact analysis, health, and change safety.

The same action may consult several specialists without forcing all communication through a single parent.

### Aios System Graph

Aios should maintain a live, typed graph of the system. The graph should describe more than agent communication: it should connect physical hardware, operating-system resources, services, specialists, models, gateways, capabilities, and recovery paths.

```text
[Wi-Fi hardware]
       │ managed_by
       ▼
[Wi-Fi driver service]
       │ observed_by
       ▼
[Wi-Fi Specialist]
       │ consulted_by
       ▼
[Planner / Verification Agents]
       │ requests
       ▼
[Policy Broker]
       │ authorizes
       ▼
[Staged Executor]
```

The graph can contain several layers:

```text
Physical layer:
  CPU, memory, buses, devices, firmware, sensors

Operating-system layer:
  kernel, drivers, services, filesystems, processes, namespaces

Agent layer:
  Planner, Verification, Subsystem Specialists, Guardian

Model and gateway layer:
  local models, LAN gateways, internet providers, fallback routes

Trust and recovery layer:
  capabilities, policies, boot images, snapshots, watchdogs
```

Edges should have explicit types rather than being treated as generic connections:

| Edge type | Meaning |
| --- | --- |
| `owns` | A component is the authoritative manager for a resource |
| `depends_on` | One component requires another to function |
| `communicates_with` | Messages have been exchanged or a channel is declared |
| `observes` | A component receives telemetry from another |
| `controls` | A capability permits bounded operations on a resource |
| `affects` | A proposed change may alter another component’s behavior |
| `hosted_on` | A service or agent runs on a machine or execution domain |
| `fallback_to` | A component or model has a defined fallback path |

Redis or another message bus can provide evidence for `communicates_with` edges. A direct message should not automatically create an ownership or authority edge. Communication topology and permission topology are different things.

Each node and edge should carry metadata such as identity, version, source, trust level, capabilities, health, timestamps, and expiration. Observed edges should be distinguishable from declared or attested edges:

```text
Declared edge:
  Network Specialist owns wifi0

Observed edge:
  Network Specialist sent ToolRequest req-9182 to wifi-driver-service

Authority edge:
  Policy Broker granted driver_staging for req-9182
```

The graph can support:

- Impact analysis before a change is executed
- Routing a request to the correct specialist
- Detecting missing, stale, or unexpected dependencies
- Discovering failed or isolated hardware
- Selecting model and gateway paths
- Building health and recovery plans
- Explaining why an operation is considered critical
- Providing a focused context subgraph to an agent

Agents should not receive the entire graph in every prompt. The graph service should return the relevant neighborhood for the current task, such as the Wi-Fi device, its bus, firmware, driver, network service, policy, and recovery dependencies.

The graph is a system map and analysis source, not the final authority for permissions. The policy broker remains the source of truth for capabilities, and the trust plane remains the source of truth for protected system boundaries.

The graph should be built and maintained through discovery, declarations, attestation, event observation, and reconciliation. Because it can become stale or be poisoned by false reports, graph entries need provenance, freshness, conflict handling, and a safe behavior when the graph is incomplete.

### Agent packages and instantiation

Every runtime agent should be instantiated from a versioned **Agent Package**. Once bootstrap and deterministic discovery have built the initial System Graph, Aios can map graph node types, roles, and ownership boundaries to packages in the Agent Package Registry.

```text
Deterministic discovery
          ↓
System Graph nodes and edges
          ↓
Agent Package Registry matches node type and role
          ↓
Agent instance created
          ↓
Context, tools, policies, and health checks attached
          ↓
Agent instance registered in the graph
```

For example:

```text
PCI network device + Wi-Fi class
          ↓
network.wifi Specialist Package
          ↓
Wi-Fi Specialist instance for device wifi0
```

An Agent Package is a complete, versioned, deployable definition for a runtime agent. A **Specialist Package** is an Agent Package whose ownership and authority are bounded to a particular subsystem, hardware family, or resource domain. A package should be substantially more than a system prompt. It may contain:

- A manifest and package identity
- Agent role, domain, and ownership description
- Required graph node and edge types
- System context and state schemas
- System prompt and reasoning instructions, when AI is used
- Event subscriptions and message handlers
- Typed tool interfaces and implementations
- Requested capability classes
- Operational invariants and verification rules
- Health checks and telemetry definitions
- Recovery, quarantine, and escalation rules
- Model requirements or model adapter configuration
- CPU, memory, storage, latency, and power budgets
- Logging, privacy, and data-handling policy
- Compatibility constraints and dependency declarations
- Unit tests, simulations, evaluations, and acceptance criteria

The running instance receives node-specific context—such as a device identifier, driver, firmware, dependencies, and health state—but its authority comes separately from the policy broker. A package can request capabilities; it cannot grant them. Context must never grant permission.

A conceptual Specialist Package manifest might look like:

```text
package: aios.specialist.network.wifi
package_type: specialist
version: 1
matches: [pci.network.wireless, usb.network.wireless]
tools: [observe_device, get_health, diagnose_fault, stage_driver, request_reset]
capabilities: [hardware_read, driver_staging, device_reset_request]
events: [DeviceAdded, LinkStateChanged, FirmwareError]
invariants: [DRIVER-001, NETWORK-002]
recovery: [quarantine_device, restore_previous_driver]
model_policy: local_or_approved_gateway
tests: [wifi.discovery, wifi.driver_staging, wifi.rollback]
data_policy: system_hardware_local_or_trusted_gateway
```

All Agent Packages should be signed or otherwise integrity-protected, versioned, and independently testable. A package update should not silently broaden an existing agent’s capabilities. Package installation, activation, update, revocation, and rollback are privileged lifecycle operations.

Package types may include:

| Package type | Examples | Typical scope |
| --- | --- | --- |
| Core Agent Package | Planner, Verification Agent | System or session singleton |
| Coordinator Package | Session Coordinator, model router | System or user session |
| Specialist Package | Wi-Fi, Storage, Security, Power | Domain or hardware resource |
| Guardian Package | Infrastructure Guardian, recovery monitor | System-wide safety boundary |
| Interface Package | Chat interface, System State panel | User session or desktop |
| Gateway Package | Local or LAN model gateway adapter | Host or trusted gateway |

Not every package creates an LLM process. Some packages create deterministic services, policy components, telemetry collectors, or UI components. The package model describes the deployable contract; implementation type is declared in the manifest.

Agent instances can have different lifetimes:

```text
System singleton:
  Planner, Verification Agent, Infrastructure Guardian

Per-session:
  User conversation, approval context, temporary coordinator

Per-domain:
  Network Specialist, Storage Specialist, Security Specialist

Per-resource:
  wifi0 Specialist, nvme0 Specialist, gpu0 Specialist

Per-gateway:
  LAN model gateway adapter
```

Not every graph node requires a new language-model process. An Agent Package may instantiate:

- A deterministic service for telemetry or hard real-time behavior
- A read-only diagnostic Specialist
- An AI-assisted Specialist with bounded tools
- A coordinator for a group of related resources

The registry should prefer one agent per meaningful role, ownership, or safety boundary rather than one agent per chip, register, or process. Unknown hardware should receive a minimal read-only inspector or remain quarantined until a reviewed package is available. Aios should never invent or activate a privileged Agent Package at runtime.

The package lifecycle is:

```text
Discover resource
       ↓
Match package
       ↓
Verify signature, compatibility, and dependencies
       ↓
Instantiate with bounded context
       ↓
Request capabilities from the policy broker
       ↓
Run package health checks and acceptance tests
       ↓
Activate, monitor, update, revoke, or quarantine
```

Model weights may be included in a package, referenced from the model registry, or supplied by an approved gateway. In all cases, model provenance and package provenance remain separately auditable.

### System State panel

Aios should expose the System Graph through a desktop panel or dashboard that gives the user a useful current-state summary at a glance.

```text
Hardware, kernel, services, and specialists
                    │
                    ▼
             Metrics collectors
                    │
                    ▼
       Redis streams / internal event bus
                    │
                    ▼
          Health and state aggregator
                    │
          ┌─────────┴─────────┐
          ▼                   ▼
    System Graph          Aios panel
     projection         desktop / chat UI
```

Redis should transport telemetry, not serve as the dashboard’s unquestioned source of truth. A state aggregator should validate events, track freshness, reconcile conflicts, calculate health against the Operational Contract, and publish a stable read model for the UI.

The overview should prioritize meaning over raw metric volume:

```text
┌─────────────────────────────────────────────────────┐
│ Aios System State              HEALTHY / DEGRADED    │
├─────────────────────────────────────────────────────┤
│ CPU        normal       Memory       normal          │
│ Storage    normal       Network      degraded        │
│ Wi-Fi      unavailable  Recovery     ready           │
│ Model      LAN GPU      Connectivity LAN_ONLY       │
├─────────────────────────────────────────────────────┤
│ Active operation: driver diagnosis                   │
│ Attention: Wi-Fi firmware is missing                 │
│ Last verified: 12 seconds ago                        │
└─────────────────────────────────────────────────────┘
```

Useful views include:

- **Overview:** overall status, subsystem health, connectivity, current model route, and active operations.
- **Subsystem view:** detailed metrics, recent events, dependencies, and responsible Specialist.
- **System Graph view:** affected nodes and edges for a warning or proposed change.
- **Recovery view:** snapshots, fallback images, failed operations, and available recovery actions.
- **Audit view:** changes, approvals, policy decisions, and tool results.

Every displayed health value should carry source, timestamp, freshness, and confidence. Missing or stale telemetry should appear as `UNKNOWN` or `STALE`, not silently as healthy. The dashboard should show structured evidence and recommended actions rather than exposing private model chain-of-thought.

The panel may offer controls, but any control that changes the system must use the same typed tools, policy broker, Guardian, approval, staging, and rollback path as a request made through chat. The dashboard is another interface to Aios, not a privileged bypass.

## 7. Infrastructure Guardian

The Infrastructure Guardian is a specialized safety sentinel. It should not repair anything and should have no direct write capability.

Its responsibilities are:

- Determine whether an action affects critical systems.
- Check the action against the Aios Operational Contract.
- Identify affected components and dependencies.
- Block or escalate unsafe changes.
- Explain what rule caused the decision.
- Verify system state after an approved change.

The actual block must be enforced by the deterministic policy broker. If the Guardian only returns “deny” but another agent can ignore it, it is not a real security boundary.

A denial should be structured and understandable:

```text
BLOCKED: critical system modification

Affected systems:
  Boot configuration and recovery manifest

Reason:
  The requested change could prevent the system from starting.
  Rule BOOT-001 requires a tested fallback image.

Required:
  Create a recovery snapshot, test the new configuration,
  and obtain user approval before activation.
```

### Block types

| Type | Meaning | Normal response |
| --- | --- | --- |
| Hard block | Violates a fundamental invariant or protected boundary | Cannot be overridden through the normal interface; use recovery procedures if appropriate |
| Escalation block | May be valid but has significant risk or insufficient evidence | Produce a repair plan, stage it, and request user approval |

The Guardian should block unauthorized critical mutations, not literally every operation that touches a critical subsystem. Normal scheduling, logging, device interrupts, and memory activity necessarily affect critical systems. Read-only observation and bounded routine operations should remain possible.

## 8. Operational Contract

Aios needs a maintained list of system invariants. This should become an executable contract over time, not merely a natural-language checklist.

Each invariant should define:

- What must remain true
- Dependencies
- Verification method
- Severity
- Failure response
- Recovery method

Example:

```text
DRIVER-001
Component: kernel driver activation
Invariant: an untested driver cannot be activated in the current boot environment
Severity: critical
Verification: staging, compatibility checks, and boot health checkpoint
Failure response: retain the previous driver and boot image
```

Suggested severity levels:

```text
Level 0: Safety        No data corruption or authority bypass
Level 1: Boot          System can start and recover
Level 2: Availability  Essential services remain usable
Level 3: Performance   Resource and latency budgets remain acceptable
Level 4: Experience    Convenience and quality features work correctly
```

## 9. Critical action lifecycle

The general action lifecycle is defined in detail in
`docs/action-state-machine.md`. The summary below shows the states and their
order. Note that the Policy Broker validates capabilities **before** the
Guardian reviews, because capability validation is a prerequisite — there is
no point in the Guardian reviewing a request that lacks authority.

```text
Proposed
   ↓
ImpactAnalyzed (System Graph consulted, affected systems identified)
   ↓
Reviewed (Planner and Verification Agent have weighed in)
   ↓
PolicyValidated (broker validates capability + clearance)
   ↓
GuardianChecked (risk level 2+: Guardian returns allow, escalate, or deny)
   ↓
Approved (risk level 3+: user approval obtained, scope checked)
   ↓
Staged (checkpoint created, change applied in staging)
   ↓
HealthVerified (health checks passed after staging)
   ↓
Committed or RolledBack
```

Risk level 0–1 actions skip Guardian review, staging, and health verification
—they go directly from `PolicyValidated` to `Committed`. See
`docs/capability-model.md` section 4 for the tool risk level table and
`docs/action-state-machine.md` section 3.2 for the risk-level fast paths.

User approval is scoped to a specific proposed plan. It should not mean “the
agents may do anything necessary.” An approval should show the affected
systems, required permissions, expected risks, rollback state, and
expiration. The broker checks that every request falls within the approval
scope (action, resource, operation) and that the plan hash matches — an
approval for one plan does not authorize operations outside that plan.

The user may authorize risk, but user approval should not bypass fundamental
safety invariants or the capability system.

## 10. Message routing and transport

The proposed routing model is hybrid:

> The hierarchy defines authority and ownership; the message bus provides transport and discovery.

**v0.1 transport:** In-process channels (Tokio `mpsc` / `oneshot`). Messages
are typed Rust values — no serialization on the wire. The `BrokerClient`
trait abstracts the transport so that v0.2 can swap channels for Unix domain
sockets without changing agent code.

**v0.2+ transport:** Unix domain sockets with serialized messages, or
optional Redis for distributed or multi-process deployments. Redis is
deferred to v0.2+ — it is not required for v0.1.

```text
Authority path:
User → Planner → Verifier → Guardian / Policy Broker → Executor

Transport path:
Authorized agent ⇄ internal message bus ⇄ responsible subsystem specialist
```

Messages may be routed directly to the specialist that owns a capability. They must not bypass the authority path.

### Message classes

| Message | Routing | Safety requirement |
| --- | --- | --- |
| Telemetry | Publish/subscribe | Freshness and provenance metadata |
| Read-only query | Direct to owning specialist | Read capability and access control |
| Routine action | Direct to owning specialist | Scoped capability token |
| Cross-domain action | Consult affected specialists | Dependency and transaction checks |
| Critical mutation | Policy broker and Guardian | Staging, rollback, and approval as required |

Redis could be useful in a later user-space prototype for discovery, telemetry, requests, responses, and audit events. Redis should not be required for boot, kernel protection, thermal control, memory safety, or recovery. If the bus fails, Aios should become less coordinated—not unsafe. The TCB communicates through direct channels, not through the bus — the bus is for agent coordination and telemetry, not for safety-critical communication.

For important commands, the protocol should support acknowledgements, deadlines, correlation IDs, rejection reasons, and replay or recovery behavior. Transient telemetry can use a less durable path.

A message should be structured rather than natural-language-only:

```text
request_id: req-9182
origin: user-session-42
target: wifi-driver-specialist
action: stage_driver
affected_domains: [hardware, kernel, boot, security]
capabilities: [driver_staging]
risk: critical
rollback: boot-image-previous
deadline: 2026-08-09T12:00:00Z
```

### Memory and hardware access

An agent should not send an unrestricted request such as `read address 0xffff1234`. Addresses may be invalid, may expose another process’s secrets, or may refer to device registers with side effects.

Prefer typed operations such as:

```text
ReadDeviceRegister {
    device: "wifi0",
    register: "STATUS",
    width: 32
}
```

The responsible driver or kernel service performs the read after validating the device, register, capability, and safety rules. Direct physical-memory access belongs only in tightly controlled diagnostic or recovery paths.

## 11. Model routing and offline operation

Aios should treat model availability as part of the current system state. The user configures approved providers and gateways during setup; after that, Aios can switch between them deterministically in the background.

```text
                         Aios Agent Runtime
                    ┌────────────────────────┐
                    │ Planner + Verification │
                    └───────────┬────────────┘
                                │
                       Model Gateway / Router
             ┌──────────────────┼──────────────────┐
             ▼                  ▼                  ▼
        Local Qwen       Trusted LAN GPU       Internet provider
         (offline)          gateway              (for example,
                                                   OpenRouter)
```

The initial state-based priority is:

```text
OFFLINE
  → Local Qwen

LAN_ONLY
  → Approved LAN gateway
  → Local Qwen fallback

INTERNET
  → Configured internet provider
  → Approved LAN gateway fallback
  → Local Qwen fallback
```

Here, “best” means the highest available provider tier for the current connectivity state—not an uncontrolled model-quality contest. Health, user policy, task type, and data sensitivity still determine whether the preferred provider is eligible.

The local Qwen model is the initial offline baseline. It should be supported as a first-class provider, but model weights should be separately packaged, verified, and selected according to the machine’s available CPU, memory, storage, and acceleration. Aios must retain a deterministic recovery path even if no language model can run.

### Routing rules

- Provider endpoints, credentials, trust relationships, and fallback order are configured during setup.
- Gateway discovery does not establish trust; LAN gateways require explicit pairing or configuration.
- Aios records the selected provider and exact model identifier for each task.
- An active task remains pinned to its selected provider and model when network state changes.
- A new provider may be selected for a later task after connectivity changes or a health failure.
- The model router never grants tool or system authority; all actions still pass through the policy broker.
- If the selected provider fails, fallback is allowed only within the user’s configured policy.

The Planner and Verification Agent can use the same provider in offline mode, but that does not provide full model independence. Deterministic policy checks and the Infrastructure Guardian remain necessary, especially when both roles use the same local model.

### Setup and data-sharing consent

Setup should establish trust and privacy rules once, instead of interrupting the user for every model request. Consent is attached to data classes and provider trust boundaries, not merely to network availability.

| Data class | Default routing |
| --- | --- |
| Public information | Any approved provider |
| Personal memory and documents | Local or explicitly trusted gateway |
| System configuration and logs | Local by default |
| Credentials, tokens, and encryption keys | Never sent to public models |
| Kernel, security, and recovery state | Local or tightly trusted gateway |

The setup choices may include:

```text
[ Local only ]
[ Local and trusted LAN gateways ]
[ Allow private memory to external providers ]
[ Allow providers that may retain or train on submitted data ]
```

The consent record should include the provider or gateway identity, policy version, data scope, timestamp, and revocation state. If a provider changes its retention or training policy, Aios should require renewed consent before sending affected private data.

Aggregators such as OpenRouter require an additional trust decision because a request may be forwarded to different underlying providers. Aios should either pin a specific downstream provider/model or require the gateway to expose its downstream data policy. Consent to one gateway should not silently imply consent to every unknown provider behind it.

External models may assist with planning or explanation only when the data-sharing policy permits it. Secrets and protected system state remain blocked by the policy broker even if a general private-memory consent exists.

## 12. Driver and hardware recovery example

The prior Wi-Fi driver experience is a representative Aios use case. Aios may discover hardware identifiers, investigate an unavailable driver, evaluate a port or alternative source, and prepare a build. The system should decide which technical approach is appropriate; the user should not need to manually guess.

Because a kernel driver executes with high privilege, the activation rules should be strict. In v0.1, Aios operates above Linux and does not manage boot images or watchdogs (ADR-0001). Staging and rollback happen at the filesystem and service level:

```text
New driver proposal
        ↓
Exact kernel and hardware compatibility checks
        ↓
Source, firmware, and build review
        ↓
Checkpoint current driver module and config
        ↓
Load new module in a test configuration
        ↓
Health check: does the interface come up?
        ↓
If healthy: commit (keep new module, remove checkpoint)
        ↓
If unhealthy: rollback (unload new module, restore checkpoint)
        ↓
User approval for permanent activation
```

In a future version (v0.2+), when Aios manages boot configurations, the
staging can extend to alternate boot images with watchdog-based automatic
rollback. That is out of scope for v0.1.

The desired failure mode is:

> “Wi-Fi unavailable; the driver was rejected or rolled back.”

It should not be:

> “The operating system is now unbootable.”

In v0.1, rollback is at the module/service level — the previous driver is
restored and the new module is unloaded. The system does not become
unbootable because the boot chain is never modified. In a future version,
boot-level rollback (A/B images, watchdog) will provide protection for
changes that affect the boot chain itself.

## 13. Main risks and design responses

| Risk | Design response |
| --- | --- |
| Hallucinated or unsafe plans | Independent verification, typed tools, deterministic policy |
| Correlated agent mistakes | Different review roles, executable invariants, staged testing |
| Prompt injection from files, devices, or web content | Treat all external data as untrusted; keep authority separate from context |
| Agent privilege escalation | Capability-based access and an external enforcement broker |
| Too many agents and coordination loops | Functional domains, hierarchy, deadlines, and bounded protocols |
| Stale subsystem knowledge | Live telemetry, versioned contracts, freshness checks |
| Alert fatigue | Risk tiers and automatic handling for safe reversible actions |
| Message-bus failure | Keep critical controls independent of Redis or any agent bus |
| Uncontrolled model or data routing | Setup-time provider policy, data classification, task pinning, and audit records |
| Corrupted kernel or firmware | Signed artifacts, staged boot, watchdogs, A/B images, recovery supervisor |
| User blindly approving prompts | Exact plans, clear impact summaries, scoped and expiring approvals |

## 14. Current design position

The current direction is:

1. Aios presents one user-facing conversational assistant.
2. A Planner and Verification Agent form the primary dual-agent bridge.
3. Specialized domain and hardware specialists provide local expertise as bounded tools.
4. The Infrastructure Guardian is read-only and veto-oriented.
5. A deterministic policy broker enforces permissions and capabilities.
6. Critical changes are staged, monitored, and reversible.
7. A hierarchy organizes ownership, while a dependency graph models impact.
8. A message bus such as Redis may provide transport, but it cannot bypass authority or become a trust anchor.
9. The closer a component is to hardware, the more deterministic and constrained it must be.
10. The kernel and recovery layer remain independent of AI availability.
11. Aios selects models deterministically from configured provider tiers based on current connectivity and health.
12. A local Qwen model provides the offline baseline; LAN and internet gateways are optional, configured providers.
13. Private-data routing is controlled by setup-time consent and data classification.
14. A live typed System Graph represents hardware, software, agents, communication, authority, dependencies, and recovery paths.
15. A System State panel presents an aggregated, freshness-aware view of health, dependencies, active operations, recovery, and model connectivity.
16. Deterministic discovery maps graph resources and system roles to versioned Agent Packages and creates bounded instances.
17. **Two-dimensional authorization:** an agent needs both a valid capability (resource + operation) and sufficient clearance (tool risk level 0–4) to execute. See [ADR-0004](decisions/0004-two-dimensional-authorization.md).
18. **Fail-fast, no silent fallbacks:** during development, every error, ambiguity, or missing condition causes an immediate and visible failure. See [ADR-0003](decisions/0003-fail-fast-no-silent-fallbacks.md).
19. **Token cost is not a design constraint for safety systems.** Per-resource granularity and per-stage validation are chosen for safety, not efficiency. Local models (Qwen, Nemotron) are effectively free; external providers are used when their capabilities are needed and data policy permits.

## 15. Architecture review: gaps and improvements

The current architecture has a strong conceptual foundation: reasoning is separated from execution, specialist agents expose bounded tools, critical changes can be blocked or rolled back, and model connectivity is treated as a changing system state. The review below identifies areas that needed sharper contracts before implementation, with their current status.

### Consistency observations

The following potentially conflicting ideas are currently compatible when interpreted together:

- **Hierarchy versus direct messaging:** the hierarchy defines ownership and authority; the message bus is only a transport shortcut.
- **Guardian veto versus Guardian read-only behavior:** the Guardian can inspect and reject a plan without having permission to repair or mutate the system.
- **Model fallback versus safety:** a local model may continue planning offline, but it never bypasses the same policy broker and capability checks.
- **Dual agents versus model independence:** two roles remain useful offline, but using the same model reduces independence and must lower confidence for high-risk decisions.
- **Private-memory consent versus protected data:** user consent can permit private context to leave the machine, but secrets and protected system state remain separate data classes.

### Gaps and their status

| Priority | Area | Status | Addressed in |
| --- | --- | --- | --- |
| ~~Critical~~ | ~~Trust boundaries~~ | ✅ Closed | `security-model.md` — TCB defined, trust boundaries diagrammed, compromise scenarios documented |
| ~~Critical~~ | ~~Capabilities and identity~~ | ✅ Closed | `capability-model.md` — principals, resources, operations, capability tokens, clearance, broker algorithm |
| ~~Critical~~ | ~~Action state and transactions~~ | ✅ Closed | `action-state-machine.md` — states, transitions, checkpoints, crash recovery, power-loss recovery |
| ~~Critical~~ | ~~Internal protocol~~ | ✅ Closed | `message-protocol.md` — versioned schemas, delivery semantics, error handling, security |
| ~~Critical~~ | ~~Secrets and memory~~ | ✅ Closed | `security-model.md` section 5 — secret store, redaction rules, provenance labels, hard boundary |
| High | Multi-agent concurrency | 🔶 Partial | Per-resource serialization in protocol; full concurrency model deferred to implementation |
| High | Failure and degraded modes | ✅ Closed | `security-model.md` section 4 (compromise scenarios), `model-routing.md` (provider failure), `action-state-machine.md` (crash recovery) |
| ~~High~~ | ~~Verification and evaluation~~ | ✅ Closed | `testing-strategy.md` — 6 test layers, safety-specific tests, Aios evaluations |
| High | Updates and supply chain | ✅ Closed | `agent-packages.md` — signed packages, versioning, lifecycle, no silent capability expansion |
| ~~High~~ | ~~Observability~~ | ✅ Closed | `observability.md` — audit log, trace propagation, metrics, health read model, privacy |
| ~~High~~ | ~~Health read model~~ | ✅ Closed | `observability.md` section 4 — state aggregator, freshness rules, conflict resolution |
| High | Resource management | 🔶 Partial | Resource budgets defined in agent-packages.md; enforcement deferred to v0.2+ (advisory in v0.1) |
| High | User identity and consent | 🔶 Partial | Single-user for v0.1 (ADR-0001); multi-user deferred to v0.2+ |
| Medium | Hardware model | ✅ Closed | `system-graph.md` section 2.3 — resource lifecycle (Discovered → Available → Degraded → Quarantined → Removed) |
| Medium | Gateway trust | ✅ Closed | `model-routing.md` section 6 — pairing, certificate rotation, downstream disclosure, replay protection |
| ~~Medium~~ | ~~System graph integrity~~ | ✅ Closed | `system-graph.md` section 4 — staleness, conflict detection, poisoned data, incomplete graph |
| ~~Medium~~ | ~~Agent Package integrity~~ | ✅ Closed | `agent-packages.md` — signed manifests, capability requests, no silent expansion, unknown device handling |
| ~~Medium~~ | ~~Implementation boundary~~ | ✅ Closed | ADR-0001 — v0.1 runs above Linux in user space |
| Medium | Provenance and licensing | 🔶 Partial | Model provenance in model-routing.md; full artifact provenance tracking deferred to implementation |

### Remaining gaps before implementation

The following gaps are partially addressed and will be refined during
implementation (Milestone 1):

1. **Multi-agent concurrency:** per-resource serialization is defined, but
   full concurrency control (deadlock handling, priority rules, cross-domain
   transactions) will be designed during M1.
2. **Resource budget enforcement:** budgets are declared in packages but
   advisory in v0.1. Enforcement requires process isolation (v0.2+).
3. **Multi-user identity:** deferred to v0.2+. v0.1 is single-user.
4. **Artifact provenance tracking:** model provenance is defined; full
   tracking for all imported artifacts (drivers, firmware, packages) will be
   refined during specialist implementation.

The document currently describes the intended structure more strongly than the
runtime contracts. That was appropriate for brainstorming. The focused design
docs now hold the detailed contracts. Implementation should begin with typed
read-only interfaces, explicit identities, deterministic policy decisions, and
testable failure behavior — as defined in the implementation roadmap (Milestone 1).

## 16. Open architecture questions

Questions that have been answered are marked with ✅ and reference the
decision or document. Remaining open questions are marked with ❌.

- ✅ Should the first Aios implementation run above Linux, use a microkernel, or eventually use a custom kernel? → [ADR-0001](decisions/0001-v01-runs-above-linux.md): v0.1 runs above Linux.
- ✅ What exact capability and authorization model should the broker implement? → [ADR-0004](decisions/0004-two-dimensional-authorization.md) and `capability-model.md`: two-dimensional authorization (capability × tool risk level).
- ✅ Should Redis remain the prototype message bus, or should local IPC be the default? → `message-protocol.md`: v0.1 is in-process channels; Redis deferred to v0.2+.
- ❌ How should agents authenticate each other and prove message origin? (v0.1: in-process identity; v0.2: IPC authentication — method TBD)
- ✅ What is the minimum trusted computing base outside the agent system? → `security-model.md` section 1: TCB defined (broker, Guardian, executor, capability verification, audit log).
- ✅ Which subsystems should be first-class specialists in the initial prototype? → `implementation-roadmap.md`: Wi-Fi first, then storage, network, power, security, processes, packages, boot, graphics.
- ❌ How should long-term agent memory be separated from operational system state?
- ❌ Which actions may receive automatic approval, and which always require the user? (Partially answered: risk levels 0–2 automatic, 3–4 require approval. Full list per operation TBD per specialist.)
- ❌ What is the recovery experience when the user is unavailable?
- ❌ How much hardware control can safely be exposed through user-space services?
- ✅ What local model sizes and quantizations should Aios support across different hardware profiles? → `model-routing.md`: Qwen, Nemotron Nano/Super/Ultra with resource requirements.
- ✅ How should provider policy changes be detected and represented to the user? → `model-routing.md` section 4.4: policy version detection, re-consent required.
- ✅ What graph storage and projection model should represent the Aios System Graph? → `system-graph.md`: in-memory for v0.1, SQLite for v0.2+.
- ✅ Which graph edges require declaration, attestation, or runtime observation? → `system-graph.md` section 2.2: Declared, Attested, Observed provenance classes.
- ✅ How should graph conflicts and stale topology affect routing and safety decisions? → `system-graph.md` section 4: fail-closed on conflicts and stale data.
- ✅ What Agent Package manifest and registry format should define agent instances? → `agent-packages.md`: YAML manifest with signing, versioning, dependencies.
- ❌ Which graph node types should create a process, a deterministic service, or only a logical Specialist? (Partially answered: package types defined in agent-packages.md; per-node mapping refined during implementation.)
- ❌ Which metrics and invariants define the top-level System State status?
- ❌ What dashboard data may be visible to each user or remote session? (v0.1: single-user, all visible. v0.2+: per-user visibility TBD.)

## 17. Design artifacts — status

The following artifacts were suggested before implementation. All have been
created:

1. ✅ A capability and permission model → `capability-model.md`
2. ✅ A typed internal message protocol → `message-protocol.md`
3. ✅ A domain registry and hardware dependency graph → `system-graph.md`
4. ✅ The first version of the Aios Operational Contract → `requirements.md` + `security-model.md`
5. ✅ A threat model and recovery model → `security-model.md` + `action-state-machine.md`
6. ✅ A small in-process Rust simulation → `implementation-roadmap.md` Milestone 1 (not yet implemented, but fully specified)

Additional artifacts created beyond the original list:

7. ✅ Agent Package manifest and registry → `agent-packages.md`
8. ✅ Model routing and data consent → `model-routing.md`
9. ✅ Testing strategy → `testing-strategy.md`
10. ✅ Observability and audit → `observability.md`
11. ✅ Implementation roadmap → `implementation-roadmap.md`
12. ✅ Glossary → `glossary.md`
13. ✅ ADRs 0001–0004 → `decisions/`

## 18. Design and implementation strategy

Aios is intentionally being designed through many focused discussions and work sessions. The purpose of the architecture document is to preserve the whole vision while allowing one bounded problem to be designed, prototyped, tested, and revised at a time.

The project should distinguish between:

```text
Vision       What Aios may ultimately become
Principle    A rule that should remain true across implementations
Decision     An accepted architectural choice
Proposal     A promising design that still needs validation
Open question A decision intentionally deferred
Constraint   A hardware, security, legal, or operational limit
```

Each workstream should produce an artifact, not just more discussion. Useful artifacts include a protocol schema, threat model, simulator, test suite, read-only service, dashboard view, or documented decision.

### Suggested workstream order

The workstreams may overlap, but their dependencies suggest this progression:

```text
1. Scope, trust model, and terminology
                  ↓
2. Capabilities, identities, and message protocol
                  ↓
3. In-process simulation of agents, tools, broker, and graph
                  ↓
4. Read-only Linux discovery, telemetry, and System State panel
                  ↓
5. Local model runtime, provider routing, and data policy
                  ↓
6. Dual-agent orchestration with safe read-only tools
                  ↓
7. Transactions, approvals, staging, rollback, and recovery
                  ↓
8. Hardware Specialists and bounded device control
                  ↓
9. Driver, firmware, kernel, or eventual custom-OS decisions
```

The first implementation should not require the final kernel architecture. A user-space prototype above Linux can validate the agent, specialist, graph, message, policy, and dashboard concepts before Aios takes responsibility for lower-level operating-system behavior.

### Session discipline

At the beginning of each focused workstream, record:

- The specific question being solved
- Existing principles and constraints
- Decisions already made
- Alternatives being considered
- The artifact or test that will indicate progress
- Questions intentionally left open

At the end, record the decision, evidence, unresolved risks, and next dependency. This keeps long-running design sessions connected without forcing premature decisions in unrelated areas.

The architecture should be updated when a principle or decision changes. Detailed contracts live in focused documents:

```text
docs/
├── architecture.md              ← this document (vision and principles)
├── glossary.md                   ← shared terminology
├── requirements.md               ← functional and non-functional requirements
├── doc-progress.md               ← documentation completion tracker
├── security-model.md             ← threat model, TCB, trust boundaries
├── capability-model.md           ← two-dimensional authorization
├── message-protocol.md           ← typed internal protocol
├── action-state-machine.md       ← transaction and recovery states
├── system-graph.md               ← graph specification
├── agent-packages.md             ← package manifest and registry
├── model-routing.md              ← model gateway and provider routing
├── implementation-roadmap.md     ← milestones and dependencies
├── testing-strategy.md           ← verification and evaluation
├── observability.md              ← audit, tracing, and logging
├── decisions/                    ← architecture decision records
│   ├── 0001-v01-runs-above-linux.md
│   ├── 0002-rust-as-implementation-language.md
│   ├── 0003-fail-fast-no-silent-fallbacks.md
│   └── 0004-two-dimensional-authorization.md
└── modules/                      ← per-specialist specifications (created during implementation)
```

`architecture.md` remains the navigational overview; the focused documents hold the detailed contracts and decisions that emerge from individual workstreams.

## 19. Scenario fit

Aios is most likely to outperform manual administration when a task is continuous, cross-domain, repetitive, evidence-heavy, and reversible. Its advantage is persistent observation and safe coordination—not unlimited authority or perfect judgment.

| Scenario | Aios advantage over manual work | Appropriate autonomy |
| --- | --- | --- |
| Hardware and driver recovery | Correlates hardware identifiers, firmware, drivers, kernel compatibility, package provenance, boot state, and recovery paths. A human does not need to rediscover the same context across many tools. | Diagnose automatically; stage and test changes; require approval for activation. |
| “Why is my system slow?” | Correlates CPU, memory, storage, thermal, process, network, and recent-change data instead of requiring a human to inspect each source separately. | Read-only diagnosis and explanation; bounded reversible remediation. |
| Safe system updates | Builds the dependency graph, checks signatures and compatibility, creates snapshots, stages updates, runs health checks, and rolls back failed changes. | Automatic for low-risk updates; approval for kernel, boot, security, or irreversible changes. |
| Wi-Fi or network setup | Identifies devices, selects the correct Specialist Package, checks firmware and drivers, validates network policy, and remembers prior failures. | Automatic observation and configuration; approval for privileged driver or security changes. |
| Backup and recovery | Tracks what is protected, verifies backup freshness, detects incomplete backups, tests restoration, and explains recovery options. | Automatic for scheduled, reversible backup operations; approval for destructive restoration or replacement. |
| Security anomaly response | Correlates process, file, identity, device, and network events quickly and can quarantine a capability while preserving evidence. | Automatic containment within predefined boundaries; human approval for deletion, credential rotation, or broader isolation. |
| Power and thermal management | Continuously monitors workload, temperature, battery, and device state and coordinates resource policies across domains. | Deterministic controllers enforce hard limits; Aios recommends or applies bounded workload changes. |
| Service degradation and recovery | Detects failed heartbeats, identifies dependencies, restarts or isolates affected services, and verifies recovery without requiring a human to watch logs. | Automatic for known safe restart/failover procedures; escalation for unknown failures. |
| Preparing a machine for a goal | Converts an intent such as “prepare this laptop for travel” into checks for updates, backups, battery, VPN, firewall, synchronization, and offline model availability. | Plan automatically; show a summary and request approval for meaningful changes. |
| Development environment setup | Discovers project requirements, provisions tools, checks versions, runs tests, records provenance, and can reproduce or undo the environment. | Automatic in isolated environments; approval for system-wide or privileged changes. |

### Where humans remain better

Aios should not pretend to replace human judgment in every situation. Humans remain primary for:

- Ambiguous goals where the desired outcome is not measurable
- Physical repairs or observations that sensors cannot provide
- Novel hardware with no trusted Specialist Package
- Irreversible deletion, financial, legal, or safety-critical decisions
- Exceptions that require personal values, accountability, or consent
- Reviewing a proposed architectural change to Aios itself

The intended division is:

```text
Observe and correlate       Aios excels
Diagnose and propose        Aios assists strongly
Stage and verify            Aios assists under policy
Commit risky changes        User or explicit authority
Define values and goals     Human
```

The first demonstrations should favor scenarios where the benefit is easy to measure: read-only hardware inventory, system-health diagnosis, model/gateway status, backup verification, and safe Wi-Fi troubleshooting. These validate the graph, Specialist Packages, telemetry, model routing, and dashboard before Aios is trusted with destructive operations.

## 20. Worked scenario: unsupported Wi-Fi hardware

This scenario is based on the kind of failure that motivated the Aios design: a computer contains a Wi-Fi device with no working Linux driver, and an improvised driver installation causes the system to lock up during boot.

### Situation

The user tells Aios:

> “My Wi-Fi does not work. Find out why and try to get it working.”

The system may have no internet connection, but it could have Ethernet, a phone tether, a trusted LAN gateway, or only the local Qwen model. Aios does not assume that a particular model or network path is available.

### Example user interaction

The user does not need to know the device identifier, kernel version, driver name, or diagnostic commands:

```text
User:
  My laptop's Wi-Fi is not working. Diagnose it and, if possible,
  prepare a safe fix. You may search approved sources and build a
  driver, but do not change the current boot environment without
  asking me first. Keep an automatic rollback available.
```

Aios responds before making changes:

```text
Aios:
  I’ll begin with a read-only diagnosis. I won’t install or activate
  anything yet.

  I’ll check the wireless hardware, firmware state, kernel support,
  recent changes, and available recovery paths. If I find a candidate
  fix, I’ll show you exactly what would change before asking for approval.
```

The user sees a concise live status panel:

```text
Aios is diagnosing Wi-Fi

Hardware:    wireless controller detected
Interface:   no usable Wi-Fi interface
Driver:      no compatible driver active
Boot state:  known-good recovery image available
Changes:     none
Model:       local Qwen
```

Behind that single conversation, the Planner and Verification Agents use the Hardware, Network, Driver, Security, and Recovery Specialists as bounded tools. The user sees structured progress and evidence, not private model chain-of-thought.

After diagnosis, Aios reports:

```text
Aios:
  I found a wireless controller with no compatible active driver.
  I also found a possible candidate source, but it requires building
  a kernel module for this system.

  The candidate has not been installed. I verified the target hardware
  and kernel version, but physical hardware behavior cannot be proven
  without a controlled boot test.
```

Aios then presents a specific approval request:

```text
Aios:
  I can prepare one controlled hardware test.

  This will:
    - build the candidate in isolation;
    - checkpoint the current driver module and config;
    - load the new module in a test configuration;
    - preserve the checkpoint for automatic rollback.

  It will not permanently activate the driver without another approval.

  Approve the controlled test? [Yes] [No]
```

The user replies:

```text
User:
  Yes. Test it, but do not make it permanent without asking again.
```

If the test fails, Aios reports:

```text
Aios:
  The candidate driver failed to reach the required health checkpoint.
  The previous driver module has been restored.

  Result:
    Wi-Fi remains unavailable.
    The current system is intact.
    No permanent changes were made.

  I can provide the failure evidence and suggest the next safest options.
```

If the test succeeds, the result is still not silently committed:

```text
Aios:
  The candidate driver passed the controlled hardware test and Wi-Fi
  is working with the new driver module.

  The change is not permanent yet. Would you like to commit it?
  [Commit] [Keep staged] [Rollback]
```

### Aios response flow

```text
Read-only hardware discovery
            ↓
System Graph and health state
            ↓
Network, Hardware, Driver, Security, and Recovery Specialists
            ↓
Evidence collection and compatibility analysis
            ↓
Candidate driver package
            ↓
Build, provenance, and isolated tests
            ↓
Checkpoint current driver module and config
            ↓
User approval
            ↓
Load new module and run health checks
            ↓
Commit or automatic rollback (module-level)
```

### Phase 1: discover without changing anything

The Hardware Specialist uses deterministic tools to inspect PCI and USB devices, hardware identifiers, firmware state, kernel messages, loaded modules, and network interfaces. It does not attempt to install a driver.

The System Graph may produce a subgraph like:

```text
wifi0 hardware
   ├─ connected through PCIe
   ├─ requires firmware: unknown
   ├─ driver: missing
   ├─ network interface: absent
   ├─ kernel: aios-kernel-x
   └─ recovery image: available
```

The dashboard reports:

```text
Aios System State: DEGRADED

Network:      Wi-Fi unavailable
Hardware:     Wireless device detected
Driver:       No compatible driver active
Recovery:     Module-level checkpoint available (v0.1: no boot-level recovery)
Model route:  Local Qwen / LAN gateway / configured provider
Action:       Diagnosis only; no system changes made
```

### Phase 2: investigate possible solutions

The Planner asks the Network and Driver Specialists for candidate approaches. Depending on policy and connectivity, Aios may inspect local package caches, a trusted LAN gateway, or approved external sources. It could discover a Linux driver, a firmware package, a community port, or a Windows driver that might provide useful hardware information or code reference.

The Verification Agent checks the proposed sources and identifies uncertainties:

- Does the source actually target the detected hardware revision?
- Is it a kernel driver, user-space driver, firmware blob, or unrelated Windows component?
- Does its license permit the proposed use or redistribution?
- Does it match the exact kernel ABI and architecture?
- Does it require unsafe firmware, undocumented registers, or unrestricted DMA?
- Is the source trustworthy enough to compile or execute?

Aios may decide that a Windows driver port is technically possible, but it must not treat “found code online” as evidence that the code is safe or correct.

### Phase 3: create a candidate package

If a candidate is viable, Aios creates a staged candidate based on a reviewed Driver or Wi-Fi Specialist Package. The package records the source, license, hashes, build inputs, kernel compatibility, requested capabilities, tests, and rollback plan.

The candidate is built in an isolated environment against the exact target kernel. Static checks, compilation, package tests, and simulated device tests run before the driver is allowed near the physical device.

Because a virtual machine may not reproduce the real Wi-Fi hardware, passing the virtual tests is not considered proof of correctness. It only qualifies the candidate for a controlled hardware test.

### Phase 4: stage and obtain approval

The candidate driver is loaded as a kernel module in a test configuration.
The current driver module and config are checkpointed (saved to disk).
The Infrastructure Guardian blocks activation until the checkpoint is
verified and health checks are defined.

The user sees a concrete approval request:

```text
Aios has prepared a candidate Wi-Fi driver.

Affected:
  Wi-Fi device wifi0, kernel module, firmware configuration

Risk:
  A faulty driver could cause the Wi-Fi interface to fail.

Protection:
  Current driver module and config checkpointed
  New driver loaded in test configuration
  Health check: interface comes up and connects
  Automatic rollback if health check fails

Evidence:
  Source and license recorded
  Exact kernel build verified
  Static and virtual-device tests passed

Approve one controlled hardware test? [Yes] [No]
```

### Phase 5: test, commit, or recover

On approval, the system loads the new driver module and runs health checks:
the interface comes up, the link state is correct, and the driver reports
no errors. The checkpoint (previous module and config) remains available
for rollback.

If the driver works, Aios reports the result and leaves the change staged
until the user confirms permanent activation. If the health check fails,
the system unloads the new module and restores the checkpoint.

The failure result is:

> “The Wi-Fi driver was rejected and the previous driver was restored.”

It is not:

> “The computer is now unbootable and the user must recover it by accident.”

In v0.1, the boot chain is never touched, so the system remains bootable
regardless of driver staging outcome. In a future version, boot-level
rollback (A/B images, watchdog) will protect changes that affect the boot
chain itself.

If no candidate is safe or viable, Aios leaves the system unchanged, explains the evidence, and may suggest Ethernet, tethering, an external adapter, or waiting for a supported driver. That is still a successful outcome because the system remained intact and the user received a useful diagnosis.

### Why Aios shines here

The advantage is the coordination of many small but important tasks:

- The hardware identity is preserved across the investigation.
- The relevant Specialists share a graph rather than isolated command histories.
- The model can work offline or use an approved gateway when available.
- The candidate is treated as a privileged package with provenance and tests.
- The user is asked for one precise approval instead of many low-level commands.
- The system preserves a recovery path before touching the kernel.
- The result is observable, auditable, and reversible.

No part of this guarantees that Aios can invent a correct driver for unsupported hardware. It changes the experiment from a risky manual guess into a controlled engineering process.
