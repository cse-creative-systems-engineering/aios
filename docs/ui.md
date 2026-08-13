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

- **Engineering:** a persistent process with a stable presence.
- **Safety:** low. A resident agent is mostly a long-running process.

### 2. Screen space

Aios occupies a sidebar of the desktop, and the other windows resize to the
new screen space when it is visible.

- **Engineering:** window/compositor integration (Wayland/X11) so Aios can
  reserve a region and other windows reflow around it.
- **Safety:** moderate. Reserving and managing screen space touches the window
  manager and compositor.

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

The v0.1 UI is a desktop GUI with two layers:

1. **Sidebar** — a persistent panel on the left side of the screen, default
   width 15% of the screen, user-configurable. It contains the chat interface
   and access to Aios settings (providers, API keys, default models, specialist
   configuration). All other windows resize to accommodate the sidebar when it
   is visible. The compositor handles the resize behavior.

2. **Canvas overlay** — a separate window that overlays the desktop. The
   canvas is the display layer for model-generated UI. It renders whatever the
   model designs using `egui` (immediate mode), which allows dynamic layouts,
   custom painting, and glassmorphic effects in future iterations. The canvas
   supports multiple overlapping overlays, each free-floating and dockable
   (snapping to edges). Each overlay has its own keep-on-top control. Overlays
   can be minimized to the sidebar as clickable list items.

### Approval flow

The sidebar has a consolidated approval queue. The model's UI generation is
unhindered — it renders data that has already passed through the broker and
security workflows. But when the user initiates a mutating action, the
approval flow applies. The sidebar shows a single queue of pending approvals
(not individual popups), and the user can approve/deny in batch or set
auto-approve preferences for low-risk tool categories. Read-only operations
(observe/diagnose) never trigger approval.

### UI generation

UI generation runs as a separate flow, unhindered by the broker or any
restrictions on layout and design. The data returned to the UI has already
gone through all Aios security and broker workflows. The model determines
the best way to present that data — the "how" is up to the model and the
GUI framework. The canvas must present true and correct data, but the layout,
styling, and interaction design are the model's responsibility.

### Visual style (v0.1)

Opaque panels for v0.1. Panels with slight background transparency are
acceptable if the compositor supports it. Glassmorphic design is planned for
a later iteration.

### Screen viewing tools

Deferred to a later design session. The canvas will eventually support
screenshots and GPU frame capture, but this is not part of v0.1.

## Relationship to existing docs

- **System State panel (architecture §6, roadmap M8):** the panel is one *part*
  of the UI, not the whole. The full UI is presence + screen space + screen
  vision.
- **Conversational facade (human-interaction.md):** the facade remains a
  display/render layer; it does not authorize. This holds for the whole UI.
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

## Status

This is a scoping document with design decisions recorded as they are
resolved. The full design is its own workstream (roadmap M8 extends to cover
the whole UI, not just the panel). The canvas overlay and sidebar are the
v0.1 target; screen vision and glassmorphic design are deferred.