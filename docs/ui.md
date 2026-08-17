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
