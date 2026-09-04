# ADR-0012: Live System State and Generative A2UI Runtime

**Status:** Accepted  
**Date:** 2026-09-04  
**Amends:** ADR-0007 (surface lifecycle and data delivery only)  
**Does not amend:** ADR-0004 (authorization), ADR-0010 (execution safety)

## Context

Aios currently obtains most system evidence on demand through bounded Rust
specialist tools. That is safe, but it makes ordinary conversation depend on a
sequence of model decisions and tool round trips. It also gives generative
surfaces only a static evidence snapshot. The current canvas proves that
unrestricted HTML/CSS can produce useful, user-customizable presentation, but
surface identity, placement, updates, and recovery are split across ephemeral
frontend and backend state.

The desired experience is an always-aware Aios: deterministic Rust code keeps
an accurate, fresh view of the host; Aios receives only the relevant projection
for a conversation or surface; and the A2UI model remains free to create or
revise any presentation the user asks for.

## Decision

### 1. Separate observation, topology, authority, and presentation

| Concern | Authoritative component | Purpose |
|---|---|---|
| Observations and trends | `SystemStateStore` | Timestamped current values, bounded history, freshness, provenance, and deterministic findings. |
| Relationships and ownership | `SystemGraph` | Hardware/service topology, dependencies, and declared ownership. |
| Permission and execution | Policy Broker | Capabilities, clearance, approvals, Guardian review, staging, audit. |
| Presentation | A2UI surface model | Unrestricted visual composition and iterative user-directed redesign. |

No observation, trend, graph edge, generated surface, or model output grants
authority. The Broker remains the only authorization authority.

### 2. Replace conversational specialists with deterministic collectors

The existing domain specialists evolve into Rust collectors and adapters. They
discover applicable hardware, sample permitted OS interfaces, normalize their
domain data, and publish updates to the `SystemStateStore`. They do not require
a model call to supply routine facts.

Collectors are selected from a prebuilt, signed Rust capability catalog at
install/first run. Discovery may configure a collector for actual hardware and
generate local descriptors, but untrusted device strings, driver metadata, or
firmware data must never compile code or expand authority dynamically.

Each collector declares its metrics, units, resource keys, relationships,
sampling/event triggers, retention class, freshness policy, and failure state.
Missing data is `Unknown` or `Stale`, never synthesized as healthy.

### 3. Project relevant context, never the whole machine

Before a model call, Rust derives a bounded `ContextProjection` from the active
conversation, visible surfaces, user request, and allowed data classifications.
It contains current facts, recent deterministic trends, provenance, freshness,
and stable query keys. Aios can use typed read-only queries to request narrow
detail or an explicit refresh.

The projection is not a raw database dump. It is size-bounded, classification
aware, and task-specific. Protected and secret data stay subject to the current
data-policy and consent rules.

### 4. Deterministic findings distinguish fact from interpretation

Rust may calculate deltas, thresholds, rates, overlap windows, rankings, and
temporal correlation. It may publish statements such as “GPU temperature rose
13 C in six minutes” or “process X GPU activity and outbound traffic overlapped
for five minutes.” It must not publish causal conclusions. Aios may present a
causal explanation only as a labeled hypothesis or after a diagnostic verifies
it.

Every finding includes its source keys, observation window, confidence rule,
and freshness so Aios can explain why it noticed something.

### 5. Keep A2UI presentation unconstrained; harden its platform

ADR-0007 remains in force: the A2UI model may generate arbitrary HTML/CSS,
layout, density, typography, color, controls, and visual language. This ADR
does not introduce a closed widget vocabulary or deterministic surface
templates.

The deterministic runtime owns only surface identity, revision, placement,
size, z-order, visibility, persistence, isolation, input regions, and data
delivery. A surface receives a read-only projection and may declare stable
bindings to projection keys. The model controls where and how those values
appear; Rust supplies replacement values and validates them. A live data update
must not require regenerating the surface. A user-directed visual edit creates
a new revision from the existing surface and edit request, preserving its
lifecycle state unless the user requests a change.

Generated presentation has no privileged IPC, tool, network, or filesystem
access. A failed generation, invalid revision, or stale binding leaves the last
valid revision visible and reports the failure; it must not silently substitute
a template.

### 6. Adopt incrementally and preserve the runnable path

The migration runs the existing on-demand evidence path beside the store:

1. Define state schemas and collector contracts.
2. Publish current specialist output into the store without changing answers.
3. Compare projection-backed evidence with the existing path in tests.
4. Move Aios context and A2UI bindings to projections.
5. Retire conversational collection only after equivalent coverage passes.

All mutating tools, approvals, sandboxing, staging, rollback, and audit remain
on the existing Broker path throughout.

## Consequences

- Routine answers need fewer model/tool round trips and can use fresh context.
- Aios can surface deterministic, evidence-backed observations during a
  conversation without claiming omniscience or causation.
- Generated surfaces become live and revisable without constraining their
  visual design.
- The implementation gains explicit state, retention, subscription, and
  recovery contracts that the current ephemeral surface path lacks.
- The store is a new high-value integrity component but is not a TCB authority
  boundary; corrupted or unavailable state fails closed for affected actions.

## Required follow-up documentation

- `system-graph.md`: define graph/store reconciliation and key relationships.
- `message-protocol.md`: define projection, delta, refresh, and binding
  messages.
- `observability.md`: define telemetry retention, aggregation, and finding
  provenance.
- `capability-model.md` and `model-routing.md`: define projection data-policy
  enforcement and read-only queries.
- `ui.md`: define the A2UI runtime lifecycle, bindings, revision recovery, and
  user-visible freshness.
- Module specifications: replace per-request specialist contracts with
  collector schemas and triggers.

## Related

- ADR-0007 — unrestricted groundless HTML/CSS presentation
- ADR-0004 — capability × clearance authorization
- ADR-0010 — sandboxed execution primitive
- `docs/milestones/0006-live-system-state-a2ui-runtime.md` — staged adoption
