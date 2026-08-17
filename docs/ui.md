# Aios UI

**Status:** Current M8 workstream
**Active renderer:** Tauri v2 plus Vite/TypeScript
**Current checkpoint:** `docs/milestones/0001-generative-surface-desktop-foundation.md`
**Next plan:** `docs/milestones/0002-multi-surface-lifecycle-plan.md`

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

The sidebar is intentionally basic at the checkpoint. Its redesign is a
separate workstream after the surface lifecycle is stable. The redesign should
improve hierarchy, conversation states, prompt composition, evidence display,
surface management, settings, and desktop polish without moving backend
authority into the frontend.

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
