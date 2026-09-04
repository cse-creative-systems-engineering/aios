# Live System State and A2UI Runtime

**Status:** In progress — state/projection and durable surface-runtime foundation landed  
**Governing ADR:** `decisions/0012-live-system-state-and-a2ui-runtime.md`

## Goal

Make Aios continuously aware of relevant host state without constraining the
generative A2UI model. Rust owns collection, state, bindings, and lifecycle;
the A2UI model owns each surface's presentation and user-directed revisions.

## Work streams

### 1. SystemStateStore core

- Typed resource/metric keys, value/unit/schema validation, timestamps,
  provenance, freshness, and data classification.
- Current-value snapshots plus bounded history and subscriptions.
- A task-aware `ContextProjection` builder with explicit size limits.
- Reconciliation with the SystemGraph; neither grants Broker authority.

Initial implementation: graph observations seed the store at discovery and a
read-only procfs sampler refreshes CPU load, memory, and network counters just
before a model context projection. It intentionally reports only observations
it can read; collector absence remains absence.

### 2. Collector framework

- Prebuilt Rust collector catalog selected by deterministic discovery.
- Domain collector contracts: metrics, sampling/event triggers, health/failure
  state, correlation keys, and platform availability.
- Start with process, GPU, network, and thermal data to prove a multi-domain
  correlation; migrate other domains incrementally.

### 3. Findings and correlation

- Deterministic trends, thresholds, rankings, and bounded time-window overlap.
- Findings carry evidence keys, window, rule/version, confidence, and
  freshness.
- Explicitly prohibit causal claims from this layer.

### 4. Generative A2UI runtime

- Backend-owned `SurfaceRecord`: stable ID, intent, revision, HTML, bindings,
  layout, visibility, lifecycle, and evidence/projection version.
- Persist/restore surface records with sessions.
- Host arbitrary validated A2UI HTML/CSS in an isolated presentation boundary.
- Update live bindings without regenerating HTML; reject invalid/missing/stale
  bindings visibly.
- Revision protocol: use existing surface + edit intent; retain the last valid
  revision if generation or validation fails.

Initial implementation: backend-owned records now preserve stable identity,
revision, intent, declared `data-aios` bindings, placement, z-order, and
visibility through close, restore, and drag persistence. Binding replacement,
surface-edit routing, and presentation isolation remain the next slices.

### 5. Desktop resilience

- Make drag, resize, close, z-order, input-region calculation, and restore
  backend-owned and testable rather than ephemeral frontend side effects.
- Handle monitor scale/geometry changes and canvas restart without losing
  reachable surfaces.
- Keep click-through correct between, outside, and inside surfaces.

## Acceptance gates

1. A CPU/process request receives a fresh bounded projection without a
   conversational specialist round trip.
2. A GPU/process/network/thermal overlap is represented as evidence-backed
   correlation, not a causal assertion.
3. A generated CPU surface may have any A2UI-authored visual design and updates
   its bound values without regeneration.
4. User edits produce a revision while retaining placement and a recoverable
   last-valid surface.
5. A stale or unavailable collector renders explicit freshness/failure state;
   it never becomes healthy by default.
6. Generated presentation cannot access Tauri IPC, tools, filesystem, network,
   or privileged state.
7. Existing broker, Guardian, staged-execution, audit, and desktop regression
   gates remain green.

## Deliberate non-goals

- No fixed widget vocabulary or deterministic surface layouts.
- No model-written executable collector code or hardware-derived authority.
- No causal inference from metric co-occurrence.
- No mutation path outside the existing enforcement plane.
