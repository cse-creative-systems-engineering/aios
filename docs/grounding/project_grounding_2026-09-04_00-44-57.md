# Grounding Snapshot: Prefer native Wayland with explicit XWayland dock fallback

## Current State

Commit `10d5c52` continuously refreshes the deterministic live-state store.
The desktop shell now prefers native Wayland. It uses GTK Layer Shell when
available, reports an ordinary window when the compositor does not support it,
and uses XWayland/EWMH only when `AIOS_DISPLAY_BACKEND=x11` is explicitly set.

## Relevant Paths

- `src-tauri/src/main.rs` — selects native Wayland by default and makes the
  XWayland dock path opt-in.
- `docs/ui.md` — documents the compositor behavior and compatibility escape
  hatch.

## Open Work

- Replace the obsolete external `tauri-driver`/`WebKitWebDriver` test
  transport: it opens its own WebKit page rather than attaching to Aios on
  this host. Prefer Tauri's embedded WebDriver service for desktop tests.
- Add versioned live-state deltas and surface bindings.
- Add GPU collection through a runtime capability adapter.
