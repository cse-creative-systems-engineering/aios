# Aios UI

**Status:** Draft — v0.1 module specification (design session in progress)
**Depends on:** architecture.md, agent-packages.md, human-interaction.md,
security-model.md, observability.md

## Scope

This document covers the Aios user interface as a first-class component. The
current docs describe a conversational facade and a System State panel, but
the full interface is larger than either. This document scopes the whole UI so
the design gap is visible and can be worked through in its own design session.

Aios is not headless. It is a resident interface that is always present on
the screen. This is a fundamental architectural assumption that the other
docs under-specify; it is recorded here so it is not lost.

## The three faces of the UI

The Aios UI is best understood as three distinct capabilities, because each
has different safety and engineering implications:

### 1. Presence

Aios is always present on the screen rather than summoned on demand. It is a
resident part of the desktop, not a tool you open for a question.

- **Engineering:** a persistent Tauri window with a stable presence.
- **Safety:** low. A resident agent is mostly a long-running process.

### 2. Screen space

Aios owns a narrow resident sidebar docked to the left side of the desktop.
The sidebar is the persistent presence and contains the conversational chat
interface only. It does not contain a canvas and does not become a general
workspace.

Investigation results are displayed in separate floating canvas/panel windows
created on demand. A generated panel is disconnected from the resident chat
sidebar: it has its own window identity, lifecycle, position, and size. It may
float freely or dock to any edge of the desktop. Floating panels may be
resized, minimized, and closed by the user. They are display surfaces for
validated generative-UI output; they are not authority or execution surfaces.

- **Engineering:** Tauri v2 window management (native Ubuntu windowing,
  system-tray access). One persistent sidebar window and zero or more
  floating panel windows.
- **Safety:** moderate. The sidebar and floating panels occupy screen space
  and can obscure other applications. The user controls their position and size.

### Linux docking contract

A true resident dock is a native desktop-shell surface, not merely a narrow
Tauri window. It is conceptually closer to a taskbar/panel than to an ordinary
application window:

- On Wayland compositors advertising `zwlr_layer_shell_v1`, the sidebar uses
  GTK Layer Shell. It is anchored to the left, top, and bottom edges, requests
  a fixed width, and reserves an exclusive zone for that width.
- Layer Shell must be initialized before the GTK window is realized or mapped.
  The sidebar therefore starts hidden in Tauri configuration and is shown only
  after native configuration completes. Ordinary `set_position` calls are not
  used for a Layer Shell sidebar because the compositor owns its placement.
- On X11, the fallback uses EWMH dock properties and a partial strut
  (`_NET_WM_WINDOW_TYPE_DOCK` and `_NET_WM_STRUT_PARTIAL`) to reserve work
  area. A normal Tauri position is not sufficient.
- On GNOME Wayland, the desktop-native equivalent would be a GNOME Shell
  extension; ordinary Tauri windows cannot request a reserved strut there.
- On Wayland compositors without Layer Shell support, including GNOME Shell's
  normal Wayland session, Aios first prefers an available XWayland backend so
  it can use X11 edge positioning. If XWayland is unavailable, Aios can provide
  only a best-effort borderless window. It must not claim that it is permanently
  docked or that it resizes other applications in that final fallback mode.

This capability is compositor-dependent. The application must detect support,
log the selected mode, and expose configuration failures rather than silently
falling back while presenting the result as a dock. Sources consulted for this
contract include the [Wayland Layer Shell protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1),
[GTK Layer Shell documentation](https://wmww.github.io/gtk-layer-shell/gtk-layer-shell.html),
[Tauri window configuration](https://v2.tauri.app/reference/config/#windowconfig),
and the [TAO Wayland positioning limitation](https://docs.rs/tao/latest/tao/window/struct.WindowAttributes.html).

### 3. Screen vision

Aios has tools to see the screen — it can perceive what is displayed.

- **Engineering:** display capture and/or accessibility-tree reading.
- **Safety:** highest of the three. Seeing the screen means ingesting whatever
  the user has open, which will intermittently include protected data
  (passwords, documents, credentials). This lands directly in the
  security-model framing: all external/untrusted data, data classification,
  consent-by-class, and redaction. The doc must answer how screen content is
  classified, whether it is ever sent to models, what is redacted, and whether
  the user consents per region.

## Design decisions (2026-08-13)

### v0.1 form

The v0.1 UI consists of a persistent sidebar plus on-demand floating panels:

1. **Resident sidebar** — a narrow panel docked to the left side of the
   desktop. It contains the chat interface and a settings trigger. It does not
   contain a canvas or evidence dashboard. The sidebar remains available while
   floating result panels are open.

2. **Floating result panels** — separate desktop windows created when Aios
   has a response and validated evidence to display. A panel is a
   self-contained generative-UI composition, disconnected from the sidebar.
   It can float freely or dock to the left, right, top, or bottom desktop edge;
   docking never changes the sidebar's chat-only role. Panels can be resized,
   minimized, and closed by the user. No panel can execute tools or create
   authority; all input continues through the conversational/backend path.

### Chat interface

The sidebar contains the chat interface. It is the primary user interaction
surface.

- **Layout:** A scrollable message area occupies the upper 70% of the sidebar.
  A text input field with a send button occupies the bottom 30%.
- **Scope:** The sidebar is chat-only. Evidence is not rendered inline in the
  sidebar; it appears in floating generative-UI panels.
- **Messages:** Each message is rendered as a chat bubble. User messages are
  right-aligned with a primary color background. AI responses are left-aligned
  with a surface color background. Tool results are rendered as collapsible
  sections within the AI response bubble.
- **Streaming:** AI responses stream token-by-token from the LLM. The chat
  area auto-scrolls to the latest message.
- **Approval queue:** A dedicated section at the top of the sidebar shows
  pending approvals. Each approval item shows the tool name, risk level,
  and a brief summary. The user can approve/deny from this queue.
- **Input:** The user types a prompt in the input field and presses Enter
  or clicks the send button. The prompt is sent to the planner, which
  routes it through the broker and specialist tools.

### Generative UI stack

The canvas uses a structured JSON schema approach — the LLM never streams raw
UI code. Instead, it outputs JSON matching a predefined widget enum, which the
Rust frontend maps to compiled Dioxus/Leptos components.

**Stack:**
- **Tauri v2** — desktop shell (native Ubuntu windowing, system-tray, IPC)
- **Dioxus** — reactive Rust frontend framework (rsx! macros, component-based,
  dynamic instantiation from JSON state). Chosen over Leptos for better
  dynamic component support required by generative UI.
- **Rig or Kalasm** — Rust-native LLM orchestration (tool-calling pipelines,
  structured JSON output, local models via Ollama)
- **Ollama** — local model runtime for zero-latency inference
- **Tailwind CSS** — utility-first styling for rapid iteration and consistent
  design system

**Data flow:**
```
[User prompt in sidebar chat]
       │
       ▼
[Planner routes to specialist tools via broker]
       │
       ▼ (Tool results pass through broker authorization)
[Result data passed to LLM for UI generation]
       │
       ▼ (LLM streams structured JSON schema matching widget enum)
[Tauri Backend (Rust / Rig) validates JSON against enum]
       │
       ▼ (IPC Bridge / Window Events)
[Tauri Frontend (Dioxus or Leptos)]
       │
       ▼ (Matches JSON to Compiled RSX Component)
[Rendered Generative UI in canvas panel]
```

### Widget enum

The canvas renders widgets from a defined enum of component types. The model
chooses which widgets to instantiate and what data to pass to them, but cannot
create new widget types or override component templates. This constrains the
model to a safe, testable set of rendering primitives while leaving it free
to compose them in any arrangement.

Each widget variant derives `Deserialize` so it can be built directly from the
LLM's JSON stream. The frontend matches the JSON `type` tag to the corresponding
component and renders it with the provided props.

**v0.1 widget enum:**

```rust
#[derive(Deserialize, Clone, PartialEq)]
#[serde(tag = "type", content = "props")]
pub enum GenerativeWidget {
    MetricCard {
        label: String,
        value: String,
        unit: Option<String>,
        status: Option<WidgetStatus>,
    },
    SensorGauge {
        label: String,
        value: f64,
        min: Option<f64>,
        max: Option<f64>,
        unit: Option<String>,
    },
    StatusList {
        title: String,
        items: Vec<StatusItem>,
    },
    Chart {
        title: String,
        data: Vec<ChartDataPoint>,
        chart_type: ChartType,
    },
    ActionForm {
        action_name: String,
        description: String,
        fields: Vec<FormField>,
        risk_level: RiskLevel,
    },
}

#[derive(Deserialize, Clone, PartialEq)]
pub enum WidgetStatus {
    Healthy,
    Degraded,
    Unknown,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct StatusItem {
    pub label: String,
    pub status: WidgetStatus,
    pub detail: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct ChartDataPoint {
    pub label: String,
    pub value: f64,
}

#[derive(Deserialize, Clone, PartialEq)]
pub enum ChartType {
    Line,
    Bar,
    Area,
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct FormField {
    pub name: String,
    pub field_type: FormFieldType,
    pub placeholder: Option<String>,
    pub required: bool,
}

#[derive(Deserialize, Clone, PartialEq)]
pub enum FormFieldType {
    Text,
    Number,
    Select { options: Vec<String> },
    Boolean,
}
```

The full widget enum is a first-class design artifact. New widget types are
added through the spec, not by the model at runtime.

### Approval flow

The sidebar has a consolidated approval queue. The model's UI generation is
unhindered within the widget enum — it can compose any combination of widgets
and pass any broker-authorized data to them. But when a widget triggers a
mutating action (e.g., an `ActionForm` submission), the approval flow applies.

The sidebar shows a single queue of pending approvals at the top (not
individual popups), and the user can approve/deny in batch or set auto-approve
preferences for low-risk tool categories. Read-only operations (observe/diagnose)
never trigger approval. Canvas rendering of read-only data never requires approval.

The approval queue displays: tool name, risk level, affected resources, and a
brief summary. The user can approve or deny each item. When approved, the
action is submitted through the broker and the result is streamed back to the
canvas.

### Error handling

The canvas handles errors at three levels:

1. **LLM output errors** — if the LLM returns invalid JSON or output that
   doesn't match the widget enum, the Tauri backend catches the deserialization
   failure and displays a generic error widget in the canvas: "Could not
   generate UI. The model returned an unexpected format."

2. **Missing data errors** — if a widget's expected data is unavailable (e.g.,
   no GPU when a `SensorGauge` for GPU temp is requested), the widget renders
   with a placeholder state: the label is shown but the value displays "N/A"
   with a muted style. The widget does not crash or hide.

3. **Broker/LLM unavailability** — if the broker or LLM is unavailable, the
   sidebar chat displays a system message: "Aios is temporarily unavailable.
   The broker or model service is not responding." The canvas does not render
   new content.

### Floating panel lifecycle

- **Creation:** A floating panel is created when the model returns a validated
  generative-UI composition that requires a panel. The panel contains only
  compiled widget components and broker-authorized evidence.
- **Minimization:** A panel can be minimized to a compact item in the resident
  sidebar. Selecting the item restores the floating panel.
- **Closing:** The user can close a panel from its header. Closed panels are
  removed from the active UI but remain represented by the conversation history.
- **Positioning:** Floating panels are free-positioned and resizable desktop
  windows. The user can drag them, change their stacking order, or dock them
  to any desktop edge. Docking is panel-local and does not merge the panel into
  the sidebar.
- **Default layout:** The resident sidebar starts alone. No floating panel is
  created until Aios has a response and validated evidence/UI composition.
- **Independence:** Closing, moving, resizing, or docking a result panel does
  not close, resize, or repurpose the conversational sidebar.

### Tauri window configuration

- **Sidebar:** narrow, left-docked, persistent, chat-only
- **Floating panels:** created on demand, resizable, independently positioned
- **Position:** sidebar starts at the left edge; panels use a remembered or
  safe default position
- **Title:** "Aios" for the sidebar and an evidence-specific title for panels
- **Decorations:** Frameless window with custom title bar (sidebar-style)
- **Transparency:** Opaque background for v0.1. Slight background transparency
  supported if the compositor allows it.
- **Always on top:** Not enabled by default. Each canvas panel can have its
  own keep-on-top toggle.

### IPC protocol

Messages between the Tauri backend and frontend use JSON over Tauri's
built-in IPC. The following message types are defined:

- **Frontend → Backend:**
  - `chat:submit` — user sends a prompt
  - `approval:respond` — user approves or denies an item in the queue
  - `settings:update` — user changes a setting
  - `panel:close` — user closes a canvas panel
  - `panel:toggle-pin` — user toggles keep-on-top for a panel

- **Backend → Frontend:**
  - `chat:message` — a new chat message (user or AI)
  - `chat:stream` — a streaming token from the LLM
  - `approval:request` — a new approval request from the broker
  - `approval:resolved` — an approval was approved or denied
  - `widget:render` — the LLM generated a widget, render it in the canvas
  - `widget:error` — an error occurred during widget generation
  - `settings:changed` — a setting was updated
  - `system:status` — system status update (connectivity, health)

### "Most important" reasoning

When the user asks for "the most important" system data, the model uses
the specialist tool results to determine relevance. The model considers:
- Which subsystems are in a degraded or unknown state
- Which subsystems have the most impact on system health
- Which data points the user has asked about in the past (if available)

The model is not required to explain its reasoning. It must present the
data accurately and let the widget enum determine the visual representation.

### Settings panel

A settings panel is accessible from the sidebar. It provides access to:
- Provider configuration (which LLM providers are active)
- API key management (keys for each provider)
- Default model selection (which model is used for different task types)
- Specialist configuration (which specialists are active, their risk thresholds)

The settings panel is a dedicated canvas panel that opens from the sidebar.
It uses standard Dioxus form components, not generative widgets.
For v0.1, placeholder data is used until a full codebase scan is complete
to inventory all user-facing settings.

### Visual style (v0.1)

Opaque panels for v0.1. Panels with slight background transparency are
acceptable if the compositor supports it. Glassmorphic design is planned for
a later iteration. Styling is handled by the component templates (CSS classes
in Dioxus/Leptos rsx! macros), not by the model. The model controls widget
composition and data, not visual appearance.

The visual language uses a dark theme by default with a surface-based color
system. Components use consistent spacing (8px grid), rounded corners (8px),
and a subtle border system for panel separation.

### Screen viewing tools

Deferred to a later design session. The canvas will eventually support
screenshots and GPU frame capture, but this is not part of v0.1.

## Relationship to existing docs

- **System State panel (architecture §6, roadmap M8):** the panel is one *part*
  of the UI, not the whole. The full UI is presence + screen space + screen
  vision. The System State panel is replaced by the canvas in v0.1.
- **Conversational facade (human-interaction.md):** the facade remains a
  display/render layer; it does not authorize. This holds for the whole UI.
  The sidebar chat is the conversational facade; the canvas is the display
  layer for generative UI.
- **Interface Package (agent-packages.md):** the UI is an interface-layer
  concern. It is separate from the Graphics hardware specialist (docs/modules/
  graphics.md), which owns GPU/display/session *hardware*.

## Open design questions

These are deliberately not answered here; they need a dedicated UI design
session before being implemented:

- How does Aios reserve screen space and reflow other windows (compositor
  integration)?
- How is screen vision classified and consented (per region), and is any
  screen content ever sent to a model?
- How does an always-present, screen-seeing AI fit the trust and data-class
  boundaries in security-model.md?
- Which operations require approval when they touch the desktop (resizing,
  screen capture)?
- What is the complete widget enum for v0.1? (The examples above are a
  starting point; the full set needs to be defined.)
- How does the canvas handle partial or missing data when a widget's
  expected data is unavailable?
- What are all the user-facing settings that need to be exposed in the
  settings panel? (Requires a codebase scan; placeholder data used for v0.1.)

## Status

This is a scoping document with design decisions recorded as they are
resolved. The full design is its own workstream (roadmap M8 extends to cover
the whole UI, not just the panel). The canvas panel and sidebar are the
v0.1 target within a single Tauri window; separate overlapping windows and
docking are deferred. Screen vision and glassmorphic design are deferred.
The widget enum is defined for v0.1 but may grow. The settings panel
requires a codebase scan to inventory all user-facing options.