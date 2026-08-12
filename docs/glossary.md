# Aios Glossary

**Status:** Draft  
**Depends on:** architecture.md

This document defines terminology used across all Aios design documents. Terms
should be used consistently. If a term is used differently in a focused
document, that document must note the deviation.

---

## A

### Action
A structured request to perform an operation on a resource. An action has a
lifecycle (see **Action State Machine**) and must pass through the **Policy
Broker** before execution. Actions are typed, not free-form commands.

### Action Plan
A structured proposal produced by the **Planner Agent** describing one or more
**Actions** to achieve a user intent. Includes affected systems, required
capabilities, expected risks, and rollback state.

### Action State Machine
The set of states an **Action** moves through: proposed → impact-analyzed →
reviewed → staged → health-verified → committed (or rolled back). Defined in
`docs/action-state-machine.md`.

### Agent
A runtime process that reasons, coordinates, diagnoses, or monitors. Agents
propose and analyze; they do not automatically receive unrestricted OS
authority. See also **Agent Package**, **Agent Instance**.

### Agent Instance
A running instantiation of an **Agent Package** bound to specific system
context (e.g., a device identifier, firmware version, health state). Its
authority comes from the **Policy Broker**, not from its context.

### Agent Package
A versioned, signed, deployable definition for a runtime agent. Contains
manifest, role, tools, capabilities, invariants, health checks, recovery
rules, model policy, tests, and resource budgets. See
`docs/agent-packages.md`.

### Agent Package Registry
The catalog of available **Agent Packages**. Discovery maps **System Graph**
nodes to packages in the registry to create **Agent Instances**.

### Approval
A scoped, expiring authorization from the user (or an authorized role) for a
specific **Action Plan**. Approval is not blanket authority. It does not
bypass fundamental safety invariants or the **Capability** system.

### Audit Log
Append-only record of intent, plans, evidence, approvals, tool calls, policy
decisions, results, and rollback events. Does not record chain-of-thought or
secrets. See `docs/observability.md`.

## B

### Broker
See **Policy Broker**.

## C

### Capability
A bounded permission to perform a specific class of operation on a specific
resource. Capabilities are granted by the **Policy Broker**, carried by
**Tool Requests**, and enforced deterministically. Context never grants
capability. See `docs/capability-model.md`.

### Capability Token
A scoped, expiring, revocable token that proves an agent holds a specific
**Capability**. Includes principal, resource, operation scope, expiration, and
provenance.

### Checkpoint
A saved system state that can be restored during **Rollback**. Part of the
**Staged Transaction Executor**'s commit-or-rollback mechanism.

### Conversational Facade
The single user-facing interface to Aios. Internally routes to the **Session
Coordinator**, **Planner Agent**, and **Verification Agent**. The user sees one
Aios response.

## D

### Dependency Graph
A model of how system components depend on each other. Used for impact
analysis, health propagation, and change safety. Not a tree — components may
depend on multiple unrelated components. See **System Graph**.

### Deterministic
Behavior that is fully specified and reproducible, with no probabilistic
component. The **Enforcement Plane** and **Trust Plane** must be deterministic
wherever possible.

### Discovery
The process of detecting hardware, OS resources, services, and relationships
to build the initial **System Graph**. Discovery is deterministic and does not
require AI.

## E

### Enforcement Plane
The execution and safety layer of Aios. Includes the **Policy Broker**,
permission engine, typed tool interfaces, **Staged Transaction Executor**,
health checks, and **Audit Log**. Must remain deterministic.

### Escalation Block
A **Guardian** decision that flags an action as risky or under-evidenced
even though it does not violate a fundamental invariant. In v0.1 escalation
is collapsed to a **denial** (ADR-0003): the action is blocked and the user
is notified. To proceed, a re-planned action with stronger evidence re-enters
the lifecycle for review and user **Approval**. See `docs/human-interaction.md`
§5.

## F

### Fail-Closed
A design principle where, on error, ambiguity, or missing information, the
system denies the action rather than allowing it. The default behavior for
the **Policy Broker** and **Capability** system.

## G

### Guardian
See **Infrastructure Guardian**.

## H

### Hard Block
A **Guardian** decision that prevents an action because it violates a
fundamental invariant or protected boundary. Cannot be overridden through the
normal interface.

### Hardware Specialist
A **Subsystem Specialist** whose primary ownership boundary is a physical
device or hardware family (e.g., Wi-Fi, NVMe, GPU).

### Health Check
A deterministic verification that a subsystem or resource is functioning
within its **Operational Contract**. Performed after staged changes and
periodically.

### Health Report
A structured report describing the health of a subsystem, including source,
timestamp, freshness, confidence, and any active warnings. See
`docs/message-protocol.md`.

## I

### Infrastructure Guardian
A specialized safety sentinel that inspects proposed actions, checks them
against the **Operational Contract**, and blocks unsafe changes. In v0.1 a
block can only be lifted by re-planning, never by escalation through the
Guardian. It has no direct write capability. The actual block is enforced by the
**Policy Broker**. Read-only and veto-oriented.

### Invariant
A condition that must remain true for the system to be considered safe. Part
of the **Operational Contract**. Each invariant defines what must be true,
dependencies, verification method, severity, failure response, and recovery
method.

## M

### Message Bus
The internal transport layer for agent communication. May be Redis, local IPC,
or another mechanism. Provides transport and discovery only — it cannot bypass
the authority path or grant trust.

### Model Gateway
A service that routes LLM requests to configured providers (local, LAN, or
internet). Selection is deterministic based on connectivity state, health, and
data policy. See `docs/model-routing.md`.

### Model Router
The component within the **Model Gateway** that selects the provider and model
for a given task based on current system state and user policy.

## O

### Operational Contract
A maintained, executable list of system **Invariants**. Each invariant has a
severity level (0–4: Safety, Boot, Availability, Performance, Experience),
verification method, failure response, and recovery method.

## P

### Planner Agent
The primary reasoning role that understands user intent, inspects available
options, and produces a structured **Action Plan**. Has no direct execution
authority by default.

### Policy Broker
The deterministic authority that validates **Capabilities**, enforces
permissions, and gates all mutating operations. The source of truth for what
is permitted. Fail-closed.

### Principal
An authenticated identity that can request actions, hold capabilities, and be
held accountable. May be a user, an agent instance, or a system service.

## R

### Recovery
The process of restoring a system to a known-good state after a failure,
rollback, or unsafe change. Recovery paths must remain functional if the
**Agent Plane** or **Message Bus** fails.

### Resource
A hardware device, OS service, file, process, or other addressable system
component. Each resource has a clear owning **Specialist**.

### Rollback
The automatic or manual process of reverting a staged change and restoring the
previous **Checkpoint** when health verification fails or an error occurs.

## S

### Session Coordinator
The component that manages a user session, routes messages between the
**Conversational Facade** and the **Planner**/**Verification Agents**, and
tracks conversation context.

### Specialist
See **Subsystem Specialist** or **Hardware Specialist**.

### Specialist Package
An **Agent Package** whose ownership and authority are bounded to a particular
subsystem, hardware family, or resource domain.

### Staged Transaction Executor
The component that executes actions in an isolated, observable, reversible
manner: checkpoint → stage → canary → health check → commit or rollback.

### Subsystem Specialist
A bounded domain service that owns a functional area (e.g., storage,
networking, power). May be deterministic, AI-assisted, or both. Exposes
bounded **Tools**.

### System Graph
A live, typed graph connecting physical hardware, OS resources, services,
specialists, models, gateways, capabilities, and recovery paths. Edges have
explicit types (`owns`, `depends_on`, `observes`, `controls`, `affects`,
`hosted_on`, `fallback_to`). Advisory for routing and analysis — not the
authority for permissions. See `docs/system-graph.md`.

## T

### Tool
A typed interface exposed by a **Specialist** for use by higher-level agents.
Bounded operations such as `observe_device()`, `diagnose_fault()`,
`stage_change()`. Never exposes unrestricted operations like
`run_any_command()`.

### Tool Request
A structured message requesting a specific tool operation. Includes request
ID, origin, target, action, affected domains, required capabilities, risk
level, rollback reference, and deadline. See `docs/message-protocol.md`.

### Tool Result
A structured response to a **Tool Request**. Includes status, data, errors,
health impact, and provenance.

### Trust Plane
The lowest-level recovery and integrity foundation: firmware and boot
verification, kernel primitives, memory and process isolation, IOMMU/DMA
protection, watchdogs, known-good images, and recovery supervisor. Must
remain functional if all higher planes fail.

### Trusted Computing Base (TCB)
The minimal set of components that must be trusted for the system to operate
safely. If any TCB component is compromised, the system's safety properties
cannot be guaranteed. Defined in `docs/security-model.md`.

## V

### Verification Agent
The primary reasoning role that independently challenges the intent, plan,
assumptions, risks, and expected effects produced by the **Planner Agent**.
Receives original intent and system state, not just the Planner's conclusions.
Has no direct execution authority by default.

## W

### Workstream
A focused design or implementation effort that produces a concrete artifact
(protocol schema, test suite, simulation, service, etc.). Each workstream has
a specific question, existing constraints, and a definition of progress.
