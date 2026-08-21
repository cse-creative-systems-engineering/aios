# Grounding Snapshot: Coordinator Modularization, Surface Harness, and Graphify

## Current State

The orchestration core that lived in `src/coordinator.rs` is now a module
directory, `src/coordinator/`, split along capability lines: routing, providers,
chat, planning, consent, and surface. The public `aios::coordinator::*` API is
unchanged, so callers were not touched.

The generative surface is groundless-only now, not just a design doc:

- `src/surface/` holds the relay call to the separate surface model
  (`composer.rs`), and the evidence index (`evidence.rs`). The typed
  surface/v1 widget IR (`schema.rs`, `validator.rs`, `render.rs`, `stub.rs`)
  was removed this same day after the owner confirmed it contradicted the
  architecture: Aios never designs surfaces, it relays request plus specialist
  data to a groundless model and verifies value fidelity on the result.
- `src/bin/stub_provider.rs` is a standalone stub that plays both the planner
  and the surface model for end-to-end runs, serving themed HTML whose values
  are marked `data-aios`.
- `src/harness.rs` is a deterministic campaign harness: it replays prompt plans,
  enforces capability and clearance at each step, quarantines steps whose
  approval is denied, and records the run.
- `tests/ui_e2e.rs` drives the real app: spawns the binary, opens a surface,
  loops prompts, and asserts each theme's generated HTML appears with marked
  values.

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

Test baseline: `cargo test --lib` is 391 passing tests (was 432 before the
typed surface IR and its tests were removed). One real-model test stays
ignored.

## Surface Path Correction (2026-08-21, resolved)

The owner restated the architecture rule: every surface is generated on the fly
by a groundless model call working from gathered specialist evidence, then
checked for value fidelity before anything renders. Nothing is predetermined —
no CPU, RAM, or dashboard widget types, and no widget vocabulary at all.

The code had drifted from that rule: a typed surface/v1 IR (fixed
metric/gauge/status/chart/notice renderers) added in commit `6bcefc6` had taken
over as the default, pushing the free-form generator behind
`AIOS_UNCONSTRAINED_SURFACE=1`. Resolved the same day: the typed IR was
deleted (`schema.rs`, `validator.rs`, `render.rs`, `stub.rs`,
`surface_harness`), the groundless relay is now the only path, and `src/tools.rs`
health roll-ups carry a machine-parsable summary line so the fidelity gate can
bind roll-up numbers.

## Relevant Paths

- `src/coordinator/` — `mod.rs`, `routing.rs`, `providers.rs`, `chat.rs`,
  `planning.rs`, `consent.rs`, `surface.rs`, `tests.rs`.
- `src/surface/` — `composer.rs` (relay + coverage + fidelity gates),
  `evidence.rs`, `mod.rs`.
- `src/bin/stub_provider.rs` — standalone stub planner + surface model.
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
