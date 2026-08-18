# Aios UI

**Status:** Current M8 workstream
**Active renderer:** Tauri v2 plus Vite/TypeScript
**Current checkpoint:** `docs/milestones/0001-generative-surface-desktop-foundation.md`
**Next plan:** `docs/milestones/0002-multi-surface-lifecycle-plan.md`
**Sidebar plan:** `docs/milestones/0003-sidebar-administration-panel.md`

## Active Architecture

Aios has two separate desktop surfaces:

- The resident sidebar is the permanent chat and control surface.
- Generated surfaces are presentation-only widgets displayed in the detached
  transparent canvas overlay.

The sidebar must remain at the left edge below the desktop top bar. Generated
surfaces must not replace, resize, or render inside the sidebar.

The active frontend is `frontend/src/main.ts`. The Dioxus desktop tree under
`frontend/src/` is archived implementation material and is not part of the
Tauri runtime.

## Surface Contract

The backend gathers specialist evidence through the planner, broker, and owning
specialists. A separate groundless generation call receives only the user
request and the evidence relayed by Aios. It has no tools or system access.

The current checkpoint renders validated HTML because that is the accepted
short-term presentation path in ADR-0007. Aios verifies marked values before
display. The native canvas window is transparent outside the generated widget,
and its input region is limited to the widget so desktop clicks pass through.

The surface generator cannot execute commands, request capabilities, or change
system state. Generated presentation remains outside the authority boundary.

## Current Limits

The checkpoint currently supports one generated surface at a time. The next
work is the surface lifecycle plan in
`docs/milestones/0002-multi-surface-lifecycle-plan.md`, covering:

- multiple simultaneous surface IDs;
- independent movement and click-through regions;
- revisioned updates to an existing surface;
- one surface composed from evidence from multiple specialists;
- close, minimize, restore, and stale-evidence state.

## Sidebar Workstream

The sidebar is intentionally basic at the checkpoint. Its redesign is not a
cosmetic reskin. It becomes the resident Aios administration panel and the
entry point for the system's moving parts.

### Regression Firewall

Sidebar work must not change the generated-surface renderer, canvas geometry,
native input-region handling, or surface-generation protocol unless a change is
explicitly part of the surface lifecycle plan. The following checks remain
mandatory throughout the sidebar workstream:

- CPU surface still renders completely;
- RAM surface still renders completely;
- one explicit CPU plus RAM surface still renders both domains;
- generated surfaces remain movable;
- clicks outside surfaces still reach the desktop;
- the sidebar remains below the desktop top bar and at the left edge;
- surface-generation and input-region errors remain visible.

The sidebar and canvas should use separate frontend modules and state
boundaries. A visual sidebar change must be testable without changing the
canvas payload or native canvas commands.

The sidebar should expose, without overwhelming the conversation:

- current backend readiness and selected connectivity mode;
- provider health and model availability;
- planner, verifier, surface-composer, and specialist assignments;
- active surfaces and their lifecycle state;
- evidence freshness, warnings, and recent failures;
- settings and provider credential management;
- the conversation and prompt composer.

The visual direction is a refined system instrument, not a generic chatbot. The
chat remains important, but it is one instrument in a stable control surface.
The sidebar should always communicate what Aios is doing, what is waiting, and
what needs attention.

### Ultra-Premium Quality Bar

"Ultra-premium" is an acceptance requirement for every user-facing surface.
It means:

- deliberate typography, optical alignment, spacing, and hierarchy;
- a distinct Aios visual language rather than a generic chat template;
- polished loading, empty, degraded, stale, error, and recovery states;
- clear interaction feedback for focus, hover, press, drag, selection, and
  disabled controls;
- restrained motion that communicates state instead of adding noise;
- excellent narrow-panel density without cramped text or visual clutter;
- consistent details across the sidebar, administration views, and generated
  surface controls;
- accessibility, keyboard navigation, contrast, and readable status text;
- no placeholder-looking controls, dead settings, or unfinished visual states.

Visual work is not complete when it merely looks attractive in one screenshot.
It must remain coherent during real Aios activity and failure conditions.

### Sidebar Layout Design (2026-08-17)

The original sidebar direction was a single-column layout with a status rail,
chat, and prompt form. During design discussion we changed direction for three
reasons:

1. **Screen space is precious.** The sidebar is 420px. A single-column layout
   wastes width on navigation and status that could serve the chat or system
   feedback. An icon rail at 56px gives permanent navigation without consuming
   width.

2. **Chat must always be visible.** The original design treated chat as one
   section among many. But chat is the primary control interface. It should
   never disappear or be replaced by another view. The new layout keeps chat
   visible at all times.

3. **System feedback needs dedicated space.** Aios touches every part of the
    system down to kernel and hardware level. The user should see Aios working
    while waiting for a response — not a spinner, but real system state. The
    original status rail was too compact. The **top half (≈50%)** of the sidebar
    is reserved for the system feedback block — visualization, status, and
    controls — giving every system, sub-system, specialist, and model real
    estate without crowding the chat. The chat takes the bottom half.

The new layout uses a three-zone sidebar plus a slide-out panel:

```
┌─────────┬──────────────────────────┐
│  Rail   │  System Feedback Block   │
│  56px   │  (reserved per system)   │
│         │  ──────────────────────  │
│         │  Chat Interface          │
│         │  (messages + composer)   │
└─────────┴──────────────────────────┘
                                    ┌──────────────┐
                                    │ Slide-out    │
                                    │ Panel        │
                                    │ (admin views)│
                                    └──────────────┘
```

**Icon rail** (56px, always visible): permanent navigation skeleton with
grouped section icons (Chat, Providers, Roles, Surfaces, Audit, Settings).
The rail never hides. The rail is purely navigational; all system feedback
lives in the live system graph above chat.

**System feedback block** (top half of the sidebar, ≈50%): a live, animated
SVG graph visualization of the Aios runtime topology. This block holds the
visualization, system feedback, and controls — not just the graph. Every node
represents a real runtime component (see Live System Graph section below). The
graph shows
Planner, Verifier, Broker, Guardian, ModelGateway, all 11 specialists,
SystemGraph, and the surface pipeline. Nodes pulse when active, edges glow
when data flows, health states shift in real time. Expands when active,
contracts when idle. Below the graph, a compact text readout shows the
current phase, active route, provider health, and surface status.

**Chat interface** (always visible): occupies the bottom half (≈50%) of the
sidebar, directly below the system feedback block. Messages, composer,
evidence. The primary control interface. Never replaced by another view.

**Slide-out panel** (right edge, separate Tauri window): appears at x=420,
same z-level as sidebar (always on top). Shows detailed admin views for
Providers, Roles, Surfaces, Audit, Settings. Overlays the canvas. Nothing
shifts. Click icon again or click outside to close.

Design principles:
- Neither comprehensiveness nor complexity restricts design decisions. The
  right design is the one that serves Aios, regardless of complexity.
- The UI must convey that Aios is a deep system, not a generic chatbot.
- Chat is always visible. It is never replaced by another view.
- Screen space is precious. The rail is fixed at 56px. The system feedback
   block owns the top half of the sidebar for visualization, system feedback,
   and controls; the chat interface owns the bottom half. The slide-out panel
   appears on demand.

### Live System Graph (2026-08-17)

The system feedback block is a live, animated SVG graph visualization of
the Aios runtime topology. It is not a decorative diagram. Every node
represents a real runtime component confirmed in the source. The graph
comes alive as the system runs: nodes pulse when active, edges glow when
data flows, health states shift in real time.

#### Runtime Topology (from code)

The backend contains these confirmed runtime components:

**Orchestration layer:**
- Facade (`src/facade.rs`) — top-level entry, owns Coordinator
- Coordinator (`src/coordinator.rs`) — central hub, owns all subsystems

**Agent layer:**
- Planner (`src/planner.rs`) — generates plans via ModelGateway
- Verifier (`src/verifier.rs`) — reviews plans via ModelGateway
- Broker (`src/broker.rs`) — policy enforcement + async specialist message bus
- Guardian (`src/guardian.rs`) — invariant enforcement, invoked at risk >= 3

**Model gateway:**
- ModelGateway (`src/model.rs`) → ModelRouter → ModelRegistry
- Backends: HttpBackend (OpenAI-compatible) + LocalLlama (gguf)
- Connectivity states: Offline / LanOnly / Internet
- Provider tiers: Local / Lan / Internet

**11 domain specialists** (Option<T> on Coordinator, None if no matching resources):
- WifiSpecialist (`src/wifi.rs`) — owns wireless devices
- StorageSpecialist (`src/storage.rs`) — owns block devices + filesystems
- NetworkSpecialist (`src/network.rs`) — owns wired interfaces + bluetooth
- DriversSpecialist (`src/drivers.rs`) — owns unclaimed PCI/USB + firmware + modules
- GraphicsSpecialist (`src/graphics.rs`) — owns GPUs + displays + sessions
- MemorySpecialist (`src/memory.rs`) — owns memory nodes + ECC sensors
- PowerSpecialist (`src/power.rs`) — owns thermal + power sensors
- ProcessesSpecialist (`src/processes.rs`) — owns process nodes
- SecuritySpecialist (`src/security.rs`) — owns Guardian/Capability/Policy nodes
- BootRecoverySpecialist (`src/boot.rs`) — owns boot images + snapshots + watchdogs
- PackagesSpecialist (`src/packages.rs`) — owns package nodes

**Infrastructure:**
- SystemGraph (`src/graph.rs`) — 27 node types, 8 edge types, health on every node
- SysfsDiscovery + ServiceDiscovery — scans /sys, /proc, systemctl
- AuditLog (`src/audit.rs`) — SHA-256 chained append-only log
- StagedExecutor (`src/executor.rs`) — checkpoint → stage → health → commit/rollback
- ToolRegistry (`src/tools.rs`) — 6 cross-cutting graph query tools

**Surface pipeline:**
- SurfaceComposer (`src/surface/composer.rs`) — model call producing typed Surface JSON
- EvidenceIndex (`src/surface/evidence.rs`) — value-presence verification
- SurfaceValidator (`src/surface/validator.rs`) — value fidelity check

#### Graph Layout

The graph occupies the system feedback block (364px wide, between the
56px rail and the right edge; the block spans the top half of the sidebar
height). Nodes are arranged in layers matching the architecture:

```
┌──────────────────────────────────────────────┐
│  Orchestration  [Facade] ─── [Coordinator]   │
│                    │                          │
│  Agents    [Planner] [Verifier] [Broker]      │
│                       │            │          │
│  Control       [Guardian]  [StagedExecutor]   │
│                    │                          │
│  Gateway       [ModelGateway]                 │
│              [Http] [Local] [Providers]       │
│                    │                          │
│  Specialists  [wifi][storage][network]        │
│               [drivers][graphics][memory]     │
│               [power][processes][security]    │
│               [boot][packages]                │
│                    │                          │
│  Infrastructure  [SystemGraph]                │
│               [AuditLog][ToolRegistry]        │
│                    │                          │
│  Surface       [Composer] → [Validator]       │
└──────────────────────────────────────────────┘
```

Edges connect layers: Facade→Coordinator→Planner/Verifier→Gateway→
Specialists→SystemGraph. The Broker fans out to all specialists.

#### Visual Language

**Health states** (from `src/protocol.rs`):
- Healthy — node color: muted green glow
- Degraded — amber pulse
- Unhealthy — red pulse, more urgent
- Unknown — gray, dim
- Stale — yellow, faded

**Active state:** When a request is in flight, the nodes involved in the
current phase pulse with a breathing animation. The phase is visible:
- Idle: no animation, static graph
- Planning: Planner node pulses, edge to Gateway glows
- Verifying: Verifier node pulses
- Gathering: active specialist(s) pulse, edges to SystemGraph glow
- Composing: Composer node pulses
- Policy check: Broker node pulses

**Node shape:** Small rounded rectangles with a 2-3 letter label. The
label is the component name (e.g. "Pln" for Planner, "Brok" for Broker,
"WiFi" for WifiSpecialist). On hover, a tooltip shows the full name,
health, and current detail.

**Edge style:** Thin lines connecting nodes. When data flows along an
edge, the line animates with a subtle directional pulse.

#### Text Readout

Below the graph, a compact text readout shows the most important system
details that don't fit in the graph:
- Current phase (idle/planning/verifying/gathering/composing)
- Active route (provider / model)
- Provider health summary
- Total graph nodes and health distribution
- Surface status (present/none)

#### Data Source

The graph is driven by a `system_graph` IPC command that returns a
frontend-friendly projection of the backend's `SystemGraph` plus
`PanelSnapshot`. The backend already computes all necessary data. The
IPC command exposes:
- All graph nodes with type, label, health, and layer
- All edges with type and direction
- Current phase (inferred from request state)
- Active node IDs (which nodes are involved in the current request)
- Aggregated health counts and subsystem rollups

The graph data refreshes after each prompt completion and can be polled
periodically during idle state.

## Provider and Model Administration

Provider configuration belongs behind a dedicated administration view reached
from the sidebar. It must not be implemented as a raw configuration-file dump
or a plain API-key textarea.

The administration view should support:

- adding and removing provider records;
- entering credentials through a trusted secret-entry path;
- discovering models from a provider when the provider supports it;
- showing provider connectivity and health;
- showing model capabilities and limits;
- assigning a provider and model to an operation role;
- assigning a default provider/model to all specialists, with per-specialist
  overrides;
- showing which assignment is active for the current task.

Assignments should be layered:

1. System default
2. Role default, such as Planner or SurfaceComposition
3. Specialist override, such as Memory or Processes
4. Explicit task pin while a request is active

The backend owns the assignment registry and task pins. The sidebar edits
typed settings through backend commands; it does not route model requests or
hold provider credentials.

The Policy Broker is not a model role. It remains deterministic authority for
capability, risk, Guardian, approval, staging, and audit decisions. A model may
provide analysis around a broker decision later, but it must never be assigned
the authority to make or override that decision.

## Assignment Validation

Selecting a model for a role should run capability checks before the assignment
is accepted. Examples:

- Planner requires reliable tool calling and sufficient context length.
- Verifier requires structured, bounded review output.
- SurfaceComposition requires generation quality and must not receive tools or
  system access.
- A specialist assignment must support the data classification and tool
  contract required by that specialist.
- A provider must be reachable and credentialed before it is marked usable.

An incompatible assignment is rejected with a specific reason. The UI should
show the reason and the required capabilities rather than allowing a broken
configuration that fails later during a request.

Provider metadata, model lists, health checks, and assignment errors must never
include secret values in frontend state, logs, prompts, or generated surfaces.

## Persistent Status Feedback

The sidebar should have a compact status area that is always present and an
expandable system panel for detail. At minimum it should expose:

- Aios readiness;
- connectivity state;
- active provider/model route;
- provider and model health;
- current operation phase, such as gathering, verifying, composing, or idle;
- active surface count and update state;
- stale or unknown evidence;
- actionable errors and recovery state.

Status must be event-driven where possible. It should not make the user infer
system state from a spinner or from a missing widget. Errors remain visible
until acknowledged or resolved.

## Full Chat Contract

The chat is the primary Aios control interface. It must grow into a complete
operational chat experience rather than remain a test prompt:

- streaming assistant output with a visible phase and cancellation;
- clear pending, complete, failed, cancelled, and retry states;
- prompt submission with Enter, Shift+Enter, autosizing, and disabled-state
  handling;
- retry and edit-and-resubmit for user messages;
- conversation history and session switching;
- tool-gathering progress that names the active specialist without exposing
  secrets or model chain-of-thought;
- collapsible specialist evidence with timestamps and freshness state;
- copy, select, and accessible keyboard navigation;
- clear backend, provider, policy, and validation errors;
- current surface references and actions once the surface manager exists;
- a command or settings entry point for administration tasks without mixing
  configuration into ordinary chat text.

The chat must preserve the existing backend boundary. It submits user intent
to Aios and renders backend events; it does not call specialists, providers,
the broker, or the operating system directly.

## Desktop Compatibility

On the current Ubuntu GNOME Wayland session, Aios selects XWayland when
available so the sidebar can use EWMH dock behavior. The selected native mode is
logged at startup. Unsupported desktop behavior must be visible and must not
be presented as a working dock.

## Safety Rules

- Render only evidence accepted by the backend validation boundary.
- Preserve `UNKNOWN`, `STALE`, and missing states.
- Do not add silent widget or surface fallbacks during development.
- Report IPC, native-window, and generation errors at their boundary.
- Keep all mutating actions on the existing broker and approval path.

### Live System Graph — Implementation Decisions (2026-08-17)

This section records binding design decisions made while implementing the
system feedback block. It overrides the softer wording above where they differ.
Full restart context: `docs/grounding/project_grounding_2026-08-17_18-05-00.md`.

**The "animated SVG graph" described earlier is implemented as a mermaid
diagram driven by real backend events.** The prior agent misread "mermaid" as
"homemade SVG" and built a decorative renderer with hardcoded `"Healthy"`
health (broker, graph, composer) and a fake frontend timer for activity. That
is being replaced entirely — no trickery.

**Core decision: every `ui.md` component is a real `SystemGraph` node.**
The earlier "Options A/B" framing (derived state vs. only-real-nodes) was
rejected. Instead, `src/graph.rs` `NodeType` is extended with real variants for
the control plane: `Facade`, `Broker`, `ModelGateway`, `SurfaceComposer`,
`EvidenceIndex`, `SurfaceValidator`, `StagedExecutor`, `AuditLog`,
`ToolRegistry`. Each gets real health from a named backend signal; a missing
signal renders `Unknown`, never a silent green.

**Activity is real and event-driven.** A `graph_activity` Tauri event carries
`phase` + `active_node_ids`. It is emitted from genuine seams: `Planning`
before each planner call (`coordinator.rs:1285`/`:1324`), `Verifying` in
`plan_and_review` (`:1609`), `Gathering` in `run_tool_as` (`:1361`, active node
derived from the tool's resource), `Composing` in the worker before each
surface compose (`main.rs:443`/`:474`), `PolicyCheck` when the broker consults
Guardian, and `Idle` when a request settles. The fake `flightProgress` timer
(`main.ts:333`) and the 8s `refreshGraph` poll (`main.ts:216`/`:576`) are
deleted.

**A node that is expected to fire in a phase but does not is treated as a
possible bug, not hidden.** This diagnostic property is intentional.

**Renderer choice:** mermaid (user's explicit request). The frontend
`GraphState` is decoupled from the renderer so it can later be swapped for a
custom SVG without touching the backend event/data plumbing.

**Activity latch:** v1 lights a node only while it is genuinely active. Keeping
a node lit after activation is deferred to a later discussion.

**Layout proportion:** the system feedback block owns the top half (≈50%) of
the sidebar for visualization, system feedback, and controls; the chat
interface owns the bottom half (≈50%). The prior agent crammed the whole
visualization into the top ~25%, which is unnecessary — the chat is fine with
the bottom half.

**Firewall:** changes are confined to `src/graph.rs`, `src/coordinator.rs`,
`src/facade.rs`, `src/tauri/src/main.rs`, and the frontend sidebar module. The
generated-surface renderer, canvas geometry, and input-region handling are
untouched.

### Bespoke Graph Wiring (2026-08-18)

The existing bespoke graph renderer is intentionally retained as the visual
layer. It provides the compact topology, fixed positions, labels, health
colors, edge treatment, and hover details that the first mermaid replacement
did not match.

Its data is now wired to the backend:

- `Facade`, `Coordinator`, `Planner`, `Verifier`, `Broker`, `Gateway`,
  `Composer`, `EvidenceIndex`, `SurfaceValidator`, `StagedExecutor`,
  `AuditLog`, `ToolRegistry`, and `SystemGraph` are real graph nodes.
- All eleven specialist slots and `Guardian` have stable nodes. A component
  that is not instantiated on the machine remains a real `Unknown` node rather
  than disappearing or being shown as healthy.
- Snapshot health comes from the corresponding `SystemGraph` node. The
  renderer no longer marks broker, graph, or composer healthy unconditionally.
- The frontend listens to `graph_activity`. Planning, verifying, gathering,
  composing, policy checks, and idle states come from backend seams; specialist
  activity is resolved from the real resource-owner edge. `PolicyCheck` fires
  immediately before the broker calls `Guardian::review()` for a risk level
  that requires Guardian review.
- The old frontend phase timers are removed. The 8-second refresh only refreshes
  the real graph snapshot and does not invent activity.
