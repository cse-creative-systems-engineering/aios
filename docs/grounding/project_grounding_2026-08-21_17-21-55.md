# Grounding Snapshot: Coordinator Modularization, Surface Harness, and Graphify

## Current State

The orchestration core that lived in `src/coordinator.rs` is now a module
directory, `src/coordinator/`, split along capability lines: routing, providers,
chat, planning, consent, and surface. The public `aios::coordinator::*` API is
unchanged, so callers were not touched.

The generative surface has real plumbing now, not just a design doc:

- `src/surface/` holds the widget schema (`schema.rs`), the composer that emits
  surface-composition instructions (`composer.rs`), the evidence/value validator
  (`validator.rs` plus `evidence.rs`), a renderer (`render.rs`), and a stub
  server for harnessing. Note: this is the typed surface/v1 path — see the
  correction below before treating it as the intended architecture.
- `src/bin/surface_harness.rs` boots surfaces standalone.
- `src/harness.rs` is a deterministic campaign harness: it replays prompt plans,
  enforces capability and clearance at each step, quarantines steps whose
  approval is denied, and records the run. This is the "groundless generation,
  second-opinion verification" loop made testable.
- `tests/ui_e2e.rs` drives the real app: spawns the binary, opens a surface,
  loops prompts, and asserts on the resulting surface.

A Tauri desktop shell wraps the core. `src-tauri/src/main.rs` bridges the Rust
core to a webview: chat prompts, model discovery, provider and role management,
live System Graph snapshots, and the resident sidebar configured as an X11
layer-shell dock. `frontend/` renders the surface canvas, draggable widgets, and
input regions, plus the sidebar (icon rail, inspector, composer, alerts) and the
settings and chat panels.

The repo now ships a code knowledge graph (graphify) under `graphify-out/`. It is
rebuilt structurally on every commit by a hook, and refreshed semantically in the
background on changed files only. Agents use it through an MCP server named
`graphify` (opencode) or `graphify-mcp-server` (VS Code), or the `graphify` CLI.
The full capability list and rules are in `AGENTS.md`.

Test baseline: `cargo test --lib` is 432 passing tests (was 438). One real-model
test stays ignored.

## Surface Path Correction (2026-08-21)

The owner restated the architecture rule: every surface is generated on the fly
by a groundless model call working from gathered specialist evidence, then
checked for value fidelity before anything renders. Nothing is predetermined —
no CPU, RAM, or dashboard widget types. This matches milestone 0001 ("a
separate groundless generation call produces a surface from the verified
evidence") and the 0002 target model (`SurfaceRecord.generated_html`).

The code drifted from that rule. The free-form generator,
`Coordinator::compose_unconstrained_html`, works but is gated behind
`AIOS_UNCONSTRAINED_SURFACE=1` (`src-tauri/src/main.rs:890`). The default path
is instead a typed surface/v1 IR — fixed metric/gauge/status/chart/notice
widget renderers in `src/surface/schema.rs` and `frontend/src/main.ts` — added
in commit `6bcefc6`, one commit after the working groundless foundation
`8afcded`. That typed path contradicts the stated design. Restoring
generated-on-the-fly surfaces as the only path is the top open item; until
then, treat the IR as legacy rather than architecture.

## Relevant Paths

- `src/coordinator/` — `mod.rs`, `routing.rs`, `providers.rs`, `chat.rs`,
  `planning.rs`, `consent.rs`, `surface.rs`, `tests.rs`.
- `src/surface/` — `schema.rs`, `composer.rs`, `validator.rs`, `evidence.rs`,
  `render.rs`, `stub.rs`, `mod.rs`.
- `src/bin/surface_harness.rs` — standalone surface boot.
- `src/harness.rs` — deterministic campaign harness (quarantine, deny, record).
- `tests/ui_e2e.rs` — end-to-end UI driver.
- `src-tauri/src/main.rs` — chat, model discovery, provider/role management,
  graph snapshots, X11 layer-shell sidebar dock.
- `frontend/src/main.ts`, `frontend/src/sidebar.ts`, `frontend/src/components/`
  (`sidebar.rs`, `chat.rs`, `settings.rs`) — surface canvas, sidebar, panels.
- `graphify-out/` — `graph.json` (~2.8k nodes) and `GRAPH_REPORT.md`.
- `.opencode/opencode.json` — `mcp.graphify` local server; `plugin` points at the
  graphify opencode plugin by absolute path.
- `scripts/graphify-refresh.sh`, `scripts/install-graphify-hooks.sh` — semantic
  refresh and hook wiring.

## Verification

- `cargo test --lib` — 432 passing, 1 ignored.
- `cargo build` and `cargo build --manifest-path src-tauri/Cargo.toml` pass.
- `npm run build` (frontend) passes.
- `graphify graph_stats` (or the MCP `graphify_graph_stats`) returns node/edge/
  community counts; `graphify diagnose multigraph` reports no collapsed edges.
- The post-commit hook rebuilds the graph structurally and refreshes semantics in
  the background (logs: `~/.cache/graphify-rebuild.log`,
  `~/.cache/graphify-semantic.log`). It is detached, so `git commit` returns
  immediately.

## Open Work

- Restore on-the-fly surface generation as the only surface path: make
  `compose_unconstrained_html` the default and retire the typed surface/v1 IR
  (see the correction above).
- M8 lifecycle stages: a real surface manager, multiple surfaces at once, and
  persistent editing. There is no manager yet and the frontend holds a single
  slot, so a second request replaces the first surface. Plans:
  `docs/milestones/0002-multi-surface-lifecycle-plan.md`. Multi-specialist
  composition already works, and the sidebar administration backend
  (providers, model role assignment, status snapshots) exists; the remaining
  sidebar work is visual polish and the full chat experience
  (`docs/milestones/0003-sidebar-administration-panel.md`).
- Verify the desktop UI visually after a rebuild; the bespoke sidebar graph
  renderer should show real node health and backend activity.
- The semantic-refresh hook depends on an OpenRouter key in `~/.aios/config.toml`
  and the `nvidia/nemotron-nano-12b-v2-vl:free` model; if that model is retired,
  update `scripts/graphify-refresh.sh`.
