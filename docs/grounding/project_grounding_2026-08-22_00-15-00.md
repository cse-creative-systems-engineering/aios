# Grounding Snapshot: Multi-Surface Canvas Shipped, Provider Health Fixed

## Current State

Surfaces accumulate now. Every generation lands as its own card on the
canvas — the owner stacked a cpu widget and a ram widget side by side,
dragged each around, closed them one at a time. The single-slot renderer
from the last snapshot is gone. Two commits carry it all: `003f70a`
(multi-surface) and `4ff3d63` (adaptive budgets), both validated end to end
in the UI by the owner with no issues found.

What the multi-surface commit covers:

- Backend worker keeps a `Vec<SurfaceCard>` instead of one html string;
  each card gets an id (`surface-N`) and `BackendRequest::CloseSurface`
  removes exactly that one. The canvas window input region unions one rect
  per card, so clicks between cards fall through to the desktop.
- Dragging had two symptoms worth writing down: grabbing one card moved all
  of them, and after a drop the next grab jumped half a screen from the
  cursor. One root cause — generated headers carried
  `data-tauri-drag-region`, which Tauri's injected script treats as a
  whole-window drag handle, so the native window drag fought the per-card
  JS drag. The prompt now asks for `data-aios-drag-region`, the renderer
  renames the legacy attribute on every render (the stub provider keeps
  emitting the old one on purpose so the e2e exercises this path), and the
  JS drag re-anchors from the pointer on each grab.
- The fidelity prompt tells models to keep full raw values inside spans and
  truncate visually with CSS ellipsis; dots-3 used to leave long composite
  fields empty rather than squeeze in a whole cmdline.

The adaptive-budgets commit fixes why the provider kept going dark:

- Empty-content errors were flagged recoverable, so after the gateway's one
  identical retry failed too, `mark_provider_unhealthy` cooled the whole
  provider down. Chat then died for every role until reboot. A thinking
  model out of budget looked like an outage. Empty answers are their own
  failure kind now: same prompt asked once more with double the tokens,
  provider health untouched.
- Surface composition sends OpenRouter's normalized reasoning-off switch
  (`reasoning.enabled=false`); providers without support ignore the field.
  This is meant as a stopgap until the thinking toggle lands (see Open
  Work). Deliberation adds nothing to markup today but will matter once
  Aios generates more than widgets, which is exactly why the toggle is
  user control, not a hardcoded default.
- No model types are restricted or recommended anywhere in the product or
  its messages; whatever the user assigns gets more room.

Also fixed while testing: snap-packaged editors leak `GTK_PATH` into child
shells, which crashes WebKitGTK processes against core20 glibc
(`__libc_pthread_init`). Both entry scripts sanitize it, and the webdriver
suite strips it defensively too. The suite itself now drives two coexisting
surfaces plus the close path.

Test baseline: `cargo test --lib` is 398 passing tests (394 before), one
ignored real-model test.

## Relevant Paths

- `src-tauri/src/main.rs` — `SurfaceCard`, `CloseSurface`, `close_surface`
  command, `set_input_region(Vec<InputRect>)`, worker surface list
- `frontend/src/main.ts` — surfaces array, `adoptSurfaceHtml`,
  grab-anchored drag, per-card close + resize observation
- `src/model.rs` — `GenerationError::empty_content`,
  `GatewayError::Generation { empty_content }`,
  `submit_with_budget_retry`, `GenerationRequest.reasoning_disabled`
- `src/http.rs` — reasoning switch in request body, neutral empty-content
  error wording
- `src/surface/composer.rs` — compose prompt (drag marker, ellipsis rule)
- `scripts/dev.sh`, `scripts/ui-e2e.sh` — snap env sanitizers

## Open Work

- Thinking toggle beside each role's model picker in Settings (UI +
  backend), replacing the hardcoded compose-side reasoning-off. Planned
  with the owner; deferred as its own feature.
- Surface editing/iteration: feed a chosen card's html back through the
  relay's previous-design parameter and update it in place. Relay plumbing
  exists; selection UX not designed yet (click-to-select vs naming the
  card in the prompt).
- Sidebar polish and full chat experience per milestone 0003.
