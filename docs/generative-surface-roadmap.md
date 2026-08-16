# Generative Surface Repair Roadmap

**Status:** Repair roadmap / implementation handoff
**Created:** 2026-08-16
**Scope:** Add the missing surface composition layer so Aios renders a true
dynamic generative surface (layout + widgets + evidence bindings) in the
floating canvas window instead of a single hard-coded `StatusList`. Does not
touch the M0-M7 backend contracts, broker, or specialists.
**Primary references:** `aios_issues.md` (research writeup), `docs/ui.md`,
`docs/m8-ui-repair-plan.md`, `docs/model-routing.md`, `docs/human-interaction.md`
**Working branch:** `feature/dynamic-generative-surfaces` (the current
`slint_ui_experiment` HEAD is a separate, abandoned experiment; see §7).

### Implementation progress

- [x] Phase A: Surface IR types in `src/surface/` (no model call yet).
- [x] Phase B: evidence index and value checks in `src/surface/evidence.rs`.
- [ ] Phase C: `AgentRole::SurfaceComposition` + composer model call.
- [ ] Phase D: Surface validator (schema + evidence + layout).
- [ ] Phase E: IPC contract change (`PromptResponse` carries a `Surface`).
- [ ] Phase F: frontend surface renderer + layout engine (replaces fixed grid).
- [ ] Phase G: placement hints (edge/width class) mapped to window geometry.
- [ ] Phase H: live desktop verification and doc updates.

---

## 1. Why this document exists

`aios_issues.md` records a research pass that concluded the project's
generative surface feature is broken for an architectural reason: everything
the specialists gather is collapsed into one `StatusList` widget, and there is
no model-driven composition step between evidence and rendered UI. This
document records that diagnosis as verified against the actual source (with
file references), corrects the parts of the research writeup that were off,
captures the insights that should drive the fix, and lays out a phased plan a
future session can execute from cold.

The product goal being repaired: the user talks to the always-on sidebar in
natural language, Aios investigates through its specialists, and the result is
a panel whose widgets, layout, density, and placement are composed for that
specific question - not a fixed dashboard and not a wall of evidence text.
Example from the project's own docs: "show me the important system specs"
produces a different surface than "diagnose anything unusual", given the same
underlying evidence.

## 2. Verified findings

Everything below was checked against the source on 2026-08-16.

### 2.1 What works

The backend investigation pipeline is real, well separated, and tested:

```
Facade::run_line            src/facade.rs:45
  -> Coordinator::chat_with_tools_outcome   src/coordinator.rs:1184
       -> Planner::chat_with (tool loop, capped at 4 turns)
            -> Broker -> owning specialist -> ToolResult
```

- `ToolResult` is `{ tool: &'static str, text: String }` (`src/tools.rs:30`).
  It is plain rendered text, not structured data.
- The tool loop appends a hard instruction after each tool result: "The tool
  result above is complete and authoritative. Do not call another tool."
  (`src/coordinator.rs:1240`). This makes a clean seam for a composer call.
- The Tauri shell runs the real `Facade` on a worker thread behind an mpsc
  channel (`src-tauri/src/main.rs:199`), exposing `submit_prompt` and
  `backend_status` commands.
- The sidebar window (chat) and the floating canvas window are separate Tauri
  windows (`src-tauri/tauri.conf.json`). The canvas is created on demand via a
  `canvas_response` event and can be docked left/right/top/bottom by buttons
  in the current canvas header (`frontend/src/main.ts:51`, `dockPanel` at
  `:56`).
- Linux windowing work (GTK Layer Shell, X11 EWMH dock struts, XWayland
  preference) is complete and independent of the composition problem. Leave it
  alone.

### 2.2 Where the pipeline collapses

The final stage bypasses the model entirely:

```
ToolResult[]
   -> compile_widgets()        src-tauri/src/main.rs:273
        -> ONE StatusList
             title = "Specialist evidence"
             items = all evidence formatted "tool: text"
```

- `compile_widgets()` literally returns a single `StatusList` containing every
  tool result as a bullet (`src-tauri/src/main.rs:273-284`). No widget
  selection, no layout, no composition. This is the break the research
  writeup identified, confirmed line-for-line.
- The Tauri-side widget enum has three variants (`MetricCard`, `StatusList`,
  `Notice`, `src-tauri/src/main.rs:62-77`) but only `StatusList` is ever
  produced.
- `AgentRole` (`src/model.rs:167`) has `Planner`, `Verification`,
  `SpecialistReadOnly`, `SpecialistDiagnosis`. There is no role for surface
  composition and no model call in the widget path.
- The live frontend (`frontend/src/main.ts`) renders only `metricCard`,
  `statusList`, and `notice` (`:9-12`, `renderWidget` at `:87`), and places
  everything into one fixed CSS grid (`#root` canvas header + `.widget-grid`
  in `frontend/index.css:49`). Even if the backend produced ten composed
  widgets, this grid would flatten them into the same layout.

### 2.3 What the research writeup got right

- The final stage collapses evidence into a single `StatusList`. (Confirmed,
  §2.2.)
- The frontend and backend disagree about what a widget is. (Confirmed: the
  live TS frontend and the Tauri enum share three types, but there is no
  shared schema; the Dioxus tree uses a different enum entirely, see §2.4.)
- The missing layer is a surface compiler: evidence + intent should become a
  semantic surface, be validated, then laid out and rendered. (Correct, and
  this is the whole of the fix.)
- The model should describe the surface, not render it: closed widget
  vocabulary, structured JSON, no pixel coordinates, no emitted HTML/CSS/JS.
  (Matches `docs/ui.md` §Dynamic generative surface.)
- Every rendered value should be traceable to evidence, so the model cannot
  hallucinate a metric into a widget.
- Keep the windowing/window-management work out of this change.

### 2.4 What the research writeup got wrong or missed

- **The live frontend is plain TypeScript, not Dioxus.** `index.html` loads
  `frontend/src/main.ts`, built by Vite (`frontend/index.html:10`,
  `frontend/vite.config.js`). The Dioxus files under `frontend/src/`
  (`main.rs`, `app.rs`, `types.rs`, `ipc.rs`, `components/`) are a
  disconnected experiment: `ipc.rs:22` `submit_prompt` returns hard-coded
  widgets from keyword matching and is never called from the Tauri app.
  `docs/m8-ui-repair-plan.md` §7 already records the decision to restore the
  plain-TS path first and treat Dioxus as an optional later migration. The
  writeup's "five widget types in `app.rs`" therefore exist only in dead code.
- **The writeup implied the backend produces `MetricCard`/`Notice`; it does
  not.** It produces only `StatusList` (§2.2).
- **The writeup's proposal to route composition through the model router is
  right, but the seam is even cheaper than it appears.** The composer call can
  reuse `Planner::submit`/`chat_with` and the existing gateway/stub test
  harness; no new AI subsystem is needed.
- **Evidence is currently unstructured text.** The writeup's example binds
  `widget.value` to "evidence" assuming structured values exist. Today they
  do not; the plan must either add structured evidence or extract values from
  tool result text. See §6 decision 1.

## 3. Key insights

1. **A surface is not a collection of widgets.** A good generative surface has
   independent dimensions: content, widget selection, layout/regions, visual
   priority, density, interaction, responsive behavior. The current code only
   varies content (and only into a list). The others are constants.
2. **The compiler pipeline is the product.** `Evidence + Intent -> Semantic
   Surface -> Validation -> Deterministic Layout -> Rendered UI`. The first
   step can be the model; the rest must be deterministic Rust/TS. The LLM
   composes, it does not render.
3. **The composer should be a separate, groundless model call.** It needs only
   `{intent, answer, evidence, widget vocabulary, placement constraints}`.
   It gets no machine access, no tool execution, no secrets. This matches the
   existing "answer the question now, do not call another tool" boundary at
   `src/coordinator.rs:1240` and makes the composer trivially testable.
4. **Evidence binding is the anti-hallucination guarantee.** Every widget
   value must resolve to a value present in a referenced tool result. If it
   cannot, the validator rejects the surface and the frontend shows the plain
   answer instead. "Why am I seeing this?" becomes free.
5. **No pixels.** The model may express `span: 6`, `priority: primary`,
   `region: overview`, `width: narrow` - never `x/y/width/height`. A
   deterministic layout engine converts semantic constraints into geometry.
6. **Placement is part of the surface, not window plumbing.** "A very narrow
   panel docked to the right side" is a bounded placement hint
   (`edge: right, width: narrow`) that the Tauri layer maps onto its existing
   dock/strut machinery. Do not hardcode it; let the composer emit it.
7. **The existing Dioxus widget components are reusable reference
   implementations.** The gauge and chart renderers already exist in
   `frontend/src/app.rs` and `frontend/src/components/widgets.rs`; port their
   markup/styles to the TS renderer rather than writing from scratch.

## 4. Target architecture

```
Facade
  |-- Planner (unchanged)
  |     \-- Broker -> Specialists -> ToolResult[]
  |
  \-- SurfaceComposer (new model role, no tools)
        |-- input: intent + answer + evidence + widget vocabulary + placement
        |-- output: Surface IR (JSON, closed vocabulary)
        |
        +-- Validator (deterministic)
        |     |-- schema check (typed deserialize)
        |     |-- evidence check (every value traceable to a ToolResult)
        |     \-- layout check (regions reference real widget ids, spans valid)
        |
        +-- fallback: plain answer + Notice widget on any failure
        |
        v
   PromptResponse.surface (versioned, surface/v1)
        v
   Tauri IPC -> canvas_response event
        v
   frontend (TS) Surface renderer
        |-- Layout engine (columns, regions, priority order)
        \-- Widget renderers (metricCard, sensorGauge, statusList, chart, notice)
        v
   placement hint -> window size/dock (reuses existing dock code)
        v
   Rendered generative surface
```

The Rust `Surface` struct is the contract between the AI system and the
frontend. It should be versioned (`surface/v1`) because the widget vocabulary
will grow.

## 5. Fix plan

Phases in order. Do not start a later phase while an earlier phase fails its
acceptance checks (ADR-0003 fail-fast applies to the plan too).

---

### Phase A: Surface IR types (Rust, no model call yet)

New module `src/surface/` with `mod.rs` and `schema.rs`.

Minimal v0.1 schema (aligned with what `frontend/src/types.rs` already shapes
for Dioxus, minus `ActionForm` which is deferred to the mutation pass):

```rust
// surface/v1
pub struct Surface {
    pub intent: String,          // echoed user intent
    pub title: String,
    pub subtitle: Option<String>,
    pub placement: SurfacePlacement,   // Phase G
    pub layout: SurfaceLayout,
    pub regions: Vec<SurfaceRegion>,
    pub widgets: Vec<SurfaceWidget>,
}

pub struct SurfaceLayout {
    pub mode: LayoutMode,        // Grid | Stack | Row
    pub columns: u32,            // grid only; default 12
}

pub struct SurfaceRegion {
    pub id: String,
    pub span: u32,               // grid columns
    pub priority: RegionPriority, // Primary | Secondary | Tertiary
    pub widgets: Vec<String>,    // widget ids
}

pub enum SurfaceWidget {
    MetricCard { id, title, value: String, unit: Option<String>, status: Option<String>, evidence: Vec<String> },
    SensorGauge { id, title, value: f64, min, max, unit: Option<String>, evidence: Vec<String> },
    StatusList { id, title, items: Vec<StatusItem>, evidence: Vec<String> },
    Chart { id, title, data: Vec<ChartPoint>, evidence: Vec<String> },
    Notice { id, title, body, evidence: Vec<String> },
}

pub struct SurfacePlacement {
    pub edge: Option<DockEdge>, // Left | Right | Top | Bottom
    pub width: Option<WidthClass>, // Narrow | Medium | Wide
    pub float: bool,            // default true; dock only when edge set
}
```

`evidence: Vec<String>` holds evidence keys (see Phase B). Widget `id`s are
referenced by regions; the validator resolves them.

Deliverables:
- `src/surface/schema.rs` with the types above, serde derive, `PartialEq` for
  tests, `camelCase` tags matching the TS side.
- `LayoutMode`, `RegionPriority`, `DockEdge`, `WidthClass` enums.
- `mod.rs` declaring the module; wire `pub mod surface;` in `src/lib.rs`.
- Unit tests: serde round-trip for every widget variant, a valid example
  surface, and an invalid one (region referencing a missing widget id).

Acceptance:
- `cargo test surface::` passes.
- No behavioral change to existing modules.

---

### Phase B: Evidence keys and value extraction

`ToolResult` carries only `tool` and `text`. The composer must be able to
reference evidence and the validator must be able to prove a widget value came
from evidence.

v0.1 approach (recommended, see §6 decision 1):
- Build an evidence index when composing: each result gets a key like
  `tool-0`, `tool-1`, ... in the order they were returned, alongside
  `tool` and `text`.
- The composer prompt receives the evidence list as `[key, tool, text]`
  triples and is instructed that widget values must be copied verbatim from
  one of those texts (numbers may be extracted from a text line, e.g. the
  "63" in a thermal observation).
- The validator checks each widget's `value` against the referenced
  evidence's `text` (verbatim substring match for strings; for `f64` values,
  the number must appear in the text). Failure to match = rejected surface.

Deliverables:
- `src/surface/evidence.rs`: `EvidenceIndex` builder from `&[ToolResult]`,
  key assignment, and the `value_present_in_evidence()` check used by the
  validator.
- Tests: exact copy passes, invented value fails, numeric extraction from a
  text line passes, cross-tool reference fails.

Acceptance:
- `cargo test surface::evidence` passes.
- The checks are pure functions over plain strings (no specialists touched).

---

### Phase C: SurfaceComposition role and composer call

1. Add `AgentRole::SurfaceComposition` to `src/model.rs:167` with required
   capabilities (TextGeneration is sufficient; no ToolUse, no code emission).
   It will route through the same `ModelGateway`/`ModelRouter` machinery.
   Keep `required_capabilities()` exhaustive over all roles.
2. Add a composer entry point on `Coordinator` (next to
   `chat_with_tools_outcome`, `src/coordinator.rs:1184`), e.g.
   `compose_surface(intent: &str, answer: &str, evidence: &[ToolResult]) ->
   Result<Surface, AgentError>`:
   - Build the system prompt from a new `surface_composition_instructions()`
     in `src/surface/mod.rs` (closed vocabulary, evidence list, placement
     rules, JSON-only output, no tool calls).
   - Call the gateway through the existing planner submit path
     (`Planner::submit` or a small `Composer` in `src/planner.rs`) with a
     `ModelTask` of role `SurfaceComposition`.
   - Parse with the existing `extract_json` helper (`src/planner.rs`).
   - Deserialize into `Surface`, then run the Phase D validator.
3. Do not emit `SurfaceComposition` instructions into the normal chat loop's
   system prompt; it is a separate call with its own prompt.
4. Update `docs/model-routing.md` role/capability table when the enum changes.

Deliverables:
- Role + capability wiring in `src/model.rs`.
- `surface_composition_instructions()` prompt text (plain prose, lists the
  vocabulary and constraints explicitly).
- `Coordinator::compose_surface` using the gateway with a stub-friendly path.
- Tests with `testutil::spawn_json_server`: valid JSON surface returned;
  malformed JSON returns a structured error; the composer never emits a tool
  call (stub asserts no `tool_calls` in the request body).

Acceptance:
- `cargo test` (full suite) passes with the new role registered.
- A stub model can drive `compose_surface` end to end without a display.

---

### Phase D: Validator

`src/surface/validator.rs` with three checks, run in order:

1. Schema: deserialization already enforces the enum; additionally reject a
   surface whose `layout.columns` is 0 or whose regions exceed it.
2. Evidence: every widget's `evidence` list is non-empty, every key exists in
   the `EvidenceIndex`, and every value passes the Phase B
   `value_present_in_evidence()` check.
3. Layout: every region's `widgets` ids exist in the surface's widget map;
   no widget id is referenced twice across regions unless it is duplicated
   deliberately (v0.1: reject duplicates); span values are >= 1 and
   <= `columns`.

On any failure return `Err` with a message describing the first failure.
The IPC layer maps that to a `Notice` widget + the plain answer (Phase E).

Deliverables:
- `src/surface/validator.rs` + tests (valid surface, invented value, missing
  evidence key, unknown widget id in a region, span overflow).

Acceptance:
- `cargo test surface::validator` passes.

---

### Phase E: IPC contract change

In `src-tauri/src/main.rs`:

1. Replace the `compile_widgets(&evidence)` call (`:238`, `:273`) with
   `compose_surface(&intent, &answer, &evidence)` on the worker thread. The
   facade owns `take_tool_results()`; add a facade passthrough for
   `compose_surface` (or call the coordinator method via the existing worker
   once `Facade` exposes it).
2. Extend `PromptResponse` (`src-tauri/src/main.rs:46`) with
   `surface: Option<Surface>` (serde-camel-cased). Keep `widgets` during the
   transition for the fallback path (an empty widgets vec + a Notice, or a
   minimal flattened surface). Record the decision to drop `widgets` once the
   frontend renders surfaces only.
3. `canvas_response` emission (`:142`) sends the full surface payload.
4. On compose failure, emit `surface: None` with the plain answer; the
   frontend shows the chat answer and a notice in the canvas instead of a
   blank window.

Deliverables:
- `Surface` shared type visible to `src-tauri` (re-export from `aios::surface`).
- Worker path rewired, tests in `src-tauri/src/main_test.rs` updated.

Acceptance:
- `cargo test` in `src-tauri` passes.
- With a stub provider, `submit_prompt` returns a populated `surface`.

---

### Phase F: Frontend surface renderer

In `frontend/src/main.ts` (and `frontend/index.css`):

1. Add `Surface`, region, and the five widget types to the TS type layer
   (mirror `src/surface/schema.rs`; keep `type` tags and camelCase).
2. Replace `widgets.map(renderWidget)` inside `renderCanvas()` (`:51`) with a
   `renderSurface(surface)` that:
   - Renders layout: grid with `layout.columns`; each `region` spans
     `span` columns; regions render in priority order (primary first);
   - Calls a per-widget renderer inside each region.
3. Port the `sensorGauge` (progress-bar style) and `chart` (bar chart)
   renderers from `frontend/src/app.rs:279` / `:327` (the Dioxus reference
   implementations) into `renderWidget`.
4. Add `notice` renderer behavior for the compose-failure fallback.
5. Add CSS for `.widget-grid` to honor region spans (grid-column: span N) and
   gauge/chart classes; keep the existing dark card look.
6. Keep the sidebar rendering unchanged (chat-only, collapsible evidence).

Deliverables:
- `frontend/src/main.ts` renders a `Surface` with regions/priority.
- `frontend/index.css` gains span/gauge/chart styles.

Acceptance:
- `npm run build` succeeds.
- Browser preview with a hand-injected sample surface renders regions and
  widgets in the intended arrangement (test by pasting a sample surface in the
  devtools `canvas_response` listener or a temporary fixture).

---

### Phase G: Placement hints

The canvas currently docks via the header buttons and `dockPanel()`
(`frontend/src/main.ts:56`), which moves but does not resize the window.

1. The composer may emit `placement` (§5 Phase A). Map `WidthClass` to a pixel
   width (e.g. narrow 320, medium 520, wide 780) in the frontend or Tauri
   layer; do not let the model emit pixels.
2. On receiving a surface with `placement.edge`, size then dock the canvas
   window against the active monitor work area (reuse the geometry math in
   `dockPanel`, adding a `setSize` before `setPosition`).
3. Keep the manual dock buttons working for float/dock overrides.

Deliverables:
- `setSize` + dock on surface placement in `main.ts` (or a small Tauri
  command if the window must be touched from Rust; prefer frontend for v0.1).
- Optional `src-tauri` strut reservation when docked right (the X11 strut
  code in `src-tauri/src/main.rs` currently reserves the left edge only;
  generalize edge selection if reserved dock space is wanted).

Acceptance:
- Asking for "a narrow panel docked to the right" positions a narrow canvas
  window on the right edge with the composed widgets inside.

---

### Phase H: Live verification and doc updates

1. `npm run tauri:dev` on a machine with a working display. Test the example
   prompts: "give me a quick overview", "diagnose anything unusual",
   "show me everything", and the placement example from Phase G.
2. Confirm: sidebar chat always works; canvas only appears when there is
   evidence + a valid surface; compose failure shows a notice, never a blank
   window, never fabricated metrics.
3. Update docs to match reality:
   - `docs/ui.md` §Dynamic generative surface / widget enum: replace the
     v0.1 widget list and data flow with the surface IR shape; note the TS
     renderer decision (Dioxus stays optional per m8-ui-repair-plan.md §7).
   - `docs/model-routing.md`: add `SurfaceComposition` to the role table.
   - `docs/doc-progress.md`: mark this roadmap's status.
4. Record this roadmap's completion and close out the Phase E `widgets`
   compatibility field.

Acceptance:
- The three example prompts produce visibly different surfaces from the same
  evidence.
- No model output is ever rendered as raw HTML/CSS/JS.

---

## 6. Open decisions (maintainer input)

1. **Structured evidence now or later?** The plan extracts values from tool
   result text (v0.1) and defers structured, typed evidence
   (`ToolResult` gains a `fields: Vec<EvidenceField>` or specialist-provided
   JSON) to a later pass. Structured evidence is richer (units, ranges,
   timestamps) and makes the validator stronger, but touches every
   specialist. Recommendation: text extraction first, structured evidence as
   a follow-up.
2. **Widget vocabulary.** Proposed v0.1 set: `MetricCard`, `SensorGauge`,
   `StatusList`, `Chart`, `Notice`. `ActionForm` is deferred until the
   mutation pass exists (the backend is read-only today). Confirm this list.
3. **Type sync strategy.** Rust `Surface` and TS types are mirrored by hand.
   For v0.1 that is acceptable; if the vocabulary churns, consider generating
   the TS types from the Rust schema. Confirm hand-mirrored types are fine.
4. **Placement scope.** Whether placement hints ship in the first composer
   version or after the surface renderer is proven. Recommendation: ship the
   schema field now, wire the behavior in Phase G once surfaces render.
5. **Reserved dock space on the right.** The X11 strut logic currently
   reserves the left edge for the sidebar. If a docked-right canvas should
   reserve space too, generalize the strut edge. Confirm whether reserved
   space (vs. simple positioned window) is required for v0.1.

## 7. Session handoff

State as of 2026-08-16 (all verification reads done, no code changed):

- **Branch:** working tree is on `slint_ui_experiment`
  (`git log -1` = 6dbac68 "Start Slint UI experiment"), which is `main` plus
  an abandoned Slint experiment. The relevant work branch is
  `feature/dynamic-generative-surfaces` (has the deeper specialist reports and
  the Dioxus-era files). Recommendation for the next session: switch to
  `feature/dynamic-generative-surfaces` before starting Phase A, and merge or
  drop the Slint experiment deliberately rather than leaving both live.
- **Untracked files at repo root:** `aios_issues.md` (the research writeup
  this roadmap references) and `docs/slint-ui-handoff.md`. Nothing was
  committed.
- **Live frontend:** `frontend/src/main.ts` (vanilla TS, Vite build). The
  Dioxus tree under `frontend/src/` is dead code and must not be wired into
  Tauri (see §2.4).
- **Key line references** (in case files move):
  - Collapse point: `src-tauri/src/main.rs:273` (`compile_widgets`),
    `:238` (call site), `:62` (`UiWidget` enum).
  - Tool loop + answer boundary: `src/coordinator.rs:1184`,
    `:1240` (authoritative-answer instruction), `:1253` (`ChatOutcome`).
  - Facade passthrough: `src/facade.rs:45`, `:260` (`take_tool_results`).
  - Roles: `src/model.rs:167` (`AgentRole`), `:175`
    (`required_capabilities`).
  - JSON parsing reuse: `src/planner.rs` (`extract_json`, `strip_think`,
    `submit`).
  - Evidence type: `src/tools.rs:30` (`ToolResult`).
  - Test harness: `src/testutil.rs` (`spawn_json_server`, `openai_response`).
  - Frontend renderer: `frontend/src/main.ts:51` (canvas render), `:87`
    (`renderWidget`), `:56` (`dockPanel`).
  - CSS grid: `frontend/index.css:49` (`.widget-grid`).
  - Reference widget implementations: `frontend/src/app.rs:279` (gauge),
    `:327` (chart), `frontend/src/components/widgets.rs`.
  - Panel snapshot (terminal, untouched): `src/panel.rs:83`.
- **Commands:**
  - Backend tests: `cargo test`
  - Surface module tests only: `cargo test surface::`
  - Frontend build: `npm run build`
  - Full desktop run: `npm run tauri:dev` (needs a real display; the sandbox
    fails at GTK init, see m8-ui-repair-plan.md current notes)
  - Shell with stub provider: existing `testutil` JSON server covers
    `Coordinator`/`Facade` tests without a display.
