# Aios Project Grounding

**Snapshot:** 2026-08-17 18:05:00 EDT
**Purpose:** Restart context for the sidebar "Live System Graph" redesign.
**Supersedes:** `project_grounding_2026-08-17_17-19-56.md` for sidebar-graph
work (keep the prior snapshot for the surface-lifecycle foundation context).

## What This Work Is

The sidebar "System Feedback Block" (see `docs/ui.md`, "Live System Graph")
must become a **live, honest** visualization of Aios's runtime activity. The
user's hard rules, confirmed 2026-08-17:

1. **No trickery.** Every node reflects real backend state. No synthetic
   timers, no faked phase, no hardcoded "Healthy".
2. **Every component that can show activity during operation must be a real,
   honest node.** If a node is expected to fire in a phase but does not, that
   is a *possible bug signal* — a desired diagnostic property, not something
   to paper over.
3. **Event-driven, not polling.** The diagram lights up from real backend
   events, not a frontend timer.
4. **Renderer is mermaid** (user's explicit choice), but the data model is
   decoupled from the renderer so it can be swapped later with zero data risk.

## The Problem We Found (why prior work was wrong)

The prior "homemade SVG graph" agent built a decorative diagram:

- `src-tauri/src/main.rs:718` — `broker` node health hardcoded `"Healthy"`.
- `src-tauri/src/main.rs:867` — `graph` node health hardcoded `"Healthy"`.
- `src-tauri/src/main.rs:882` — `composer` node health hardcoded `"Healthy"`.
- `frontend/src/main.ts:333` — `flightProgress` is a fake timer (0ms planning,
  1s gathering, 2.6s composing) with **no connection to the backend**.
- `frontend/src/main.ts:216` and `:576` — `refreshGraph` polls every 8s.

So several nodes were never real, and "activity" was invented in the frontend.
This violates the no-trickery rule and is being replaced.

Root cause: `SystemGraph` (`src/graph.rs:16` `NodeType`) was scoped to
**discovered system resources** (Cpu, Device, Driver, Service, Specialist,
etc.). Aios's own control-plane components were never modeled as nodes, so the
prior agent faked them in the UI projection instead of modeling them for real.

We considered two lesser options and **rejected both**:

- (A) keep structural nodes as "derived real state" — still not a real graph
  node, and a node that should fire but can't is exactly the bug detector the
  user wants, so faking the *node* defeats that.
- (B) render only nodes that map 1:1 to a real graph node — would hide the
  Facade/Broker/Composer path the user most wants to watch.

Decision: **extend `SystemGraph` itself** so every `ui.md` component is a real
node with real health. Then the UI is a faithful projection — no A, no B.

## Backend Schema Change (the core of this work)

### New `NodeType` variants to add in `src/graph.rs:16`

- `Facade` — top-level entry (`src/facade.rs`)
- `Broker` — policy enforcement + specialist bus (`src/broker.rs`)
- `ModelGateway` — `src/model.rs` aggregate (LocalModel/LanGateway/InternetProvider)
- `SurfaceComposer` — `src/surface/composer.rs`
- `EvidenceIndex` — `src/surface/evidence.rs` (value-presence verification)
- `SurfaceValidator` — `src/surface/validator.rs` (value fidelity check)
- `StagedExecutor` — `src/executor.rs`
- `AuditLog` — `src/audit.rs`
- `ToolRegistry` — `src/tools.rs`

Already real (do NOT duplicate): `Coordinator`, `PlannerAgent`,
`VerificationAgent`, `Guardian`, `Specialist`, `LocalModel`, `LanGateway`,
`InternetProvider`, plus all discovered-resource types.

Deliberately NOT a node: `SystemGraph` itself (it is the container). The UI
"graph" node will instead reflect **overall graph health distribution** derived
from `PanelSnapshot.health_counts` — real, not a self-reference.

### Required follow-on edits when adding variants

`NodeType::all()` (`src/graph.rs:48`) must list the new variants, and **every
exhaustive `match node_type` / `match ... NodeType` in `graph.rs` and elsewhere
must be extended** (label formatting, health/serialization, any per-type logic).
`cargo build` will surface each missing arm; fix them all before proceeding.

### Instantiate at boot

In `Coordinator::boot` / `Facade::boot`, create the new nodes with
`ProvenanceSource::Declared` (or `Attested` for internally-attested components)
and a real initial `HealthState`. They should appear in the graph immediately
after backend boot (the worker already calls `refresh_graph_snapshot` on boot
at `main.rs:399`).

## Real Health Signal Mapping (missing signal => Unknown, never silent green)

| Node | Real signal source |
|---|---|
| Facade | `BackendStatus.ready` (set at boot `main.rs:393`) |
| Coordinator | existing `Coordinator` node health |
| PlannerAgent / VerificationAgent | existing graph nodes |
| Broker | broker liveness + last policy outcome from `audit` |
| Guardian | existing `Guardian` node health |
| ModelGateway | aggregate of `LocalModel`/`LanGateway`/`InternetProvider` health (computed at `main.rs:750`) |
| Specialist (×11) | existing `Specialist` nodes; `None` specialist => `Unknown` |
| AuditLog | audit reachable + no recent error entries (`PanelSnapshot.recent_audits`) |
| StagedExecutor | `PanelSnapshot.failed_actions` / `active_actions` |
| ToolRegistry | registered tool count > 0 (`coordinator.tools_help()` / registry) |
| SurfaceComposer | last compose success/failure (compose result / `write_surface_trace`) |
| EvidenceIndex | last evidence presence-check result |
| SurfaceValidator | last fidelity-check pass/fail (`verify_value_fidelity`) |

Rule: a node whose signal is absent or unobservable renders `Unknown` (gray),
**never** a silent green. This is the no-trickery guard.

## Real Activity Events (replaces the fake timer)

### Event model
A `GraphActivity` payload, serialized to the frontend:
- `phase`: `Idle | Planning | Verifying | Gathering | Composing | PolicyCheck`
- `active_node_ids: Vec<String>` — the real node ids involved
- `timestamp_ms: u64`

Emitted over Tauri as event name `"graph_activity"`.

### Plumbing
- New `src/progress.rs` (lib crate): `GraphActivity` (+ `GraphPhase` enum) and
  trait `ProgressReporter: Send + Sync { fn report(&self, GraphActivity); }`.
  `Coordinator` gets `progress: Option<Arc<dyn ProgressReporter>>` plus
  `set_progress_reporter(&mut self, ...)` and a private `report(...)` helper
  that no-ops when unset.
- Binary (`src-tauri/src/main.rs`) implements `TauriProgressReporter` using
  `AppHandle::emit("graph_activity", payload)` (needs `use tauri::Emitter;`).
- The worker thread currently has **no `AppHandle`** (`main.rs:389`
  `thread::spawn` captures only the three Arcs). Capture `app.handle().clone()`
  before spawn (mirror `AppState.app` at `main.rs:533`) and pass it into the
  reporter after `Facade::boot()`.

### Emit seams (all real)
- **Planning** — `Coordinator::chat_with_tools_outcome` (`src/coordinator.rs:1244`)
  emits `Planning` before each `self.planner.chat_with` (`coordinator.rs:1285`
  and `:1324`). Active node id: `planner`.
- **Verifying** — `Coordinator::plan_and_review` (`coordinator.rs:1594`) emits
  `Verifying` around `self.verifier.review(&plan)` (`coordinator.rs:1609`).
  Active node id: `verifier`. Only fires on that code path; a missing expected
  emission is the intended bug signal.
- **Gathering** — `Coordinator::run_tool_as` (`coordinator.rs:1361`) emits
  `Gathering` before `client.request_tool` (`coordinator.rs:1474`). Active node
  id derived from the already-computed `resource` (`coordinator.rs:1417-1447`):
  `wifi:domain` -> `wifi`, `storage:domain` -> `storage`, `system:graph` ->
  `graph`, etc. Covers every real specialist invocation and read-only discovery.
- **Composing** — worker (`main.rs:443` `compose_unconstrained_html`,
  `main.rs:474` `compose_surface`) emits `Composing` before the call. Active
  node id: `composer`.
- **PolicyCheck** — emit when the broker actually consults Guardian (risk >= 3).
  Under M4 most steps are read-only, so this is conditional. Locate the Guardian
  invocation seam during implementation and emit `PolicyCheck` with active node
  `broker`. (Open item: confirm exact Guardian call site.)
- **Idle** — worker emits `Idle` after each prompt iteration completes and on
  error (clears active set). The "return to rest" signal.

Frontend rule (per user): default is **light only while genuinely active**
(true activity). Optional latching (keep a node lit after activation) is a
future discussion, not part of v1.

## Frontend Plan (renderer-independent; mermaid chosen)

- Add `mermaid` to root `package.json` (currently only `@tauri-apps/api`,
  `@tauri-apps/cli`, vite, tailwind, postcss, autoprefixer).
- Define a frontend `GraphState` (nodes/edges/health/active/phase) that is a
  pure function of the backend `SystemGraphSnapshot` + `GraphActivity` events.
  **Renderer is a pure function of `GraphState`** — swap to custom SVG later
  without touching data plumbing.
- `system_graph` IPC (`main.rs:245`) returns the rewritten snapshot (real health
  now that nodes are real). Frontend renders mermaid from `GraphState`, applies
  health colors + active pulse via CSS classes on mermaid output.
- `listen("graph_activity")` toggles active classes and updates the phase text.
- **Delete** the fake `flightProgress` timer (`main.ts:333`) and the 8s
  `refreshGraph` poll (`main.ts:216`, `:576`). The bespoke SVG engine in
  `frontend/src/sidebar.ts` (`renderGraph` / `layoutGraphNodes` /
  `computeActiveNodeIds`) and the `.graph-*` CSS in `frontend/index.css` are
  replaced by the mermaid renderer.
- Text readout (phase, active route, provider health, node/health totals,
  surface status) is fed from the snapshot + events.

## Regression Firewall (do not cross)

Only `src/graph.rs`, `src/coordinator.rs`, `src/facade.rs` (reporter pass-through),
`src/panel.rs` (reuse), `src-tauri/src/main.rs` (worker + IPC), and the
frontend sidebar module change. **No** changes to the generated-surface
renderer, canvas geometry, native input-region handling, or surface-generation
protocol (per `docs/ui.md` "Regression Firewall").

## Verification

```bash
HOME=/tmp/opencode/aios-test-home \
RUSTUP_HOME=/home/shane/.rustup \
CARGO_HOME=/home/shane/.cargo \
cargo test --lib

cargo build --manifest-path src-tauri/Cargo.toml
npm run build --prefix frontend
AIOS_UNCONSTRAINED_SURFACE=1 ./src-tauri/target/debug/aios-tauri
```

## Recommended Restart Order

1. Read this snapshot + `docs/ui.md` (Live System Graph + the appended
   Implementation Decisions section).
2. Read `src/graph.rs` (`NodeType`, `NodeMetadata`, `add_node`) and confirm
   every exhaustive `match` on `NodeType`.
3. Add the 9 new `NodeType` variants + extend `NodeType::all()` + fix all match
   arms; `cargo build` until clean.
4. Instantiate the new nodes at boot with real initial health.
5. Add `src/progress.rs` (`GraphActivity` + `ProgressReporter`); wire
   `Coordinator.progress`; capture `AppHandle` in the worker; implement
   `TauriProgressReporter`.
6. Emit events at the six seams above; verify with a manual prompt that the
   right nodes pulse per phase (and note any phase that *should* fire but does
   not — that is the bug signal).
7. Rewrite `build_graph_snapshot` as a true projection of the now-real graph.
8. Frontend: add mermaid, rebuild `GraphState`, `listen("graph_activity")`,
   delete fake timer + poll.
9. Run the verification block above.
