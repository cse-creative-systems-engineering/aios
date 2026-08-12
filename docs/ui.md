# Aios UI

**Status:** Draft — v0.1 module specification
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

- What is the v0.1 UI form — terminal/TUI, desktop GUI, or both?
- How does Aios reserve screen space and reflow other windows (compositor
  integration)?
- How is screen vision classified and consented (per region), and is any
  screen content ever sent to a model?
- How does an always-present, screen-seeing AI fit the trust and data-class
  boundaries in security-model.md?
- Which operations require approval when they touch the desktop (resizing,
  screen capture)?

## Status

This is a scoping placeholder, not a finished design. It records the gap so
the UI is not silently treated as merely a dashboard. The full design is its
own workstream (roadmap M8 extends to cover the whole UI, not just the panel).
