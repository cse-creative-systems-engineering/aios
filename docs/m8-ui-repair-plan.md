# M8 UI Repair and Completion Plan

**Status:** Repair plan / implementation handoff  
**Created:** 2026-08-13  
**Scope:** Restore a visible, functional Aios desktop UI and connect it to the existing M0–M7 backend without creating a privileged bypass.  
**Primary references:** `docs/ui.md`, `docs/human-interaction.md`, `docs/security-model.md`, `docs/capability-model.md`, `docs/message-protocol.md`, `docs/observability.md`, `docs/architecture.md`

### Implementation progress

- [x] Phase 0 inventory completed; existing dirty work preserved.
- [x] Phase 1 web bootstrap: Vite builds the resident sidebar shell from `frontend/src/main.ts`.
- [x] Phase 1 generated output: `frontend/dist/index.html` imports hashed JavaScript and CSS.
- [x] Phase 1 Tauri CLI installed; `tauri dev` compiles and reaches GTK startup.
- [x] Phase 2 worker-backed real `Facade` boot path added in `src-tauri/src/main.rs`.
- [x] Phase 3 typed `backend_status` and `submit_prompt` IPC commands added.
- [x] Phase 4 initial evidence/UI pipeline now carries real brokered specialist `ToolResult` evidence to the frontend.
- [x] Sidebar and floating panel now receive the same typed evidence; sidebar renders it in a collapsible evidence section.
- [x] Processes specialist now reports measured `cpu_utilization_percent` from `/proc/stat`.
- [x] Independent floating evidence window and left/right/top/bottom dock controls added.
- [ ] Phase 4 model-selected validated widget composition (rather than the temporary evidence list) remains.
- [ ] Phase 4 live desktop verification; current sandbox has no usable GTK display.
- [x] Provider configuration switched to free OpenRouter model routes and independently tested with HTTP 200 plus a usable assistant response.

#### Current implementation notes

- Maintainer clarification on 2026-08-13: the resident sidebar is chat-only and has no canvas. Result panels are independent floating generative-UI canvas windows created on demand after specialist evidence returns. They may float or dock to any desktop edge without merging into or resizing the sidebar. `docs/ui.md` was updated to record this decision.
- The resident sidebar and independent floating panel shell are implemented as separate Tauri windows. The panel starts hidden and is opened by a typed frontend event only when brokered specialist evidence exists.
- The current widget payload is a temporary compiled `StatusList` of specialist evidence. It must still be replaced/extended with model-selected validated widget composition before M8 is considered complete. This composition is a dynamic generative surface, not a fixed dashboard: the evidence and user intent determine the widgets, arrangement, density, and emphasis for each panel instance.
- Dock buttons currently position the floating panel against the active monitor work area. The resident sidebar is a separate native dock path: GTK Layer Shell is feature-detected, configured before the hidden Tauri window is shown, and never mixed with ordinary absolute positioning. Wayland compositor-level behavior remains dependent on Layer Shell support.
- Research finding: normal Tauri/TAO windows cannot reliably reserve work area or choose global coordinates on Wayland. X11 requires native EWMH dock/strut properties for a true reserved dock. GNOME Wayland does not provide the required Layer Shell protocol, so startup now prefers XWayland when `DISPLAY` is available; only a pure-Wayland/no-Layer-Shell environment remains an explicitly best-effort fallback.
- Research sources: [Wayland Layer Shell](https://wayland.app/protocols/wlr-layer-shell-unstable-v1), [GTK Layer Shell](https://wmww.github.io/gtk-layer-shell/gtk-layer-shell.html), [Tauri window configuration](https://v2.tauri.app/reference/config/#windowconfig), and [TAO GTK mapping issue](https://github.com/tauri-apps/tao/issues/925).

- `Facade` cannot be stored directly in Tauri state because its existing backend trait objects are not `Send + Sync`; the implementation keeps it on a dedicated worker thread and exposes only message-channel requests.
- `frontend/src/main.ts` calls `submit_prompt` through Tauri IPC and renders the returned answer. Browser preview catches IPC failure visibly; it does not fabricate backend metrics.
- `@tauri-apps/cli` is installed and `npm run tauri:dev` reaches the compiled binary. The sandbox then fails at GTK initialization (`Failed to initialize GTK`) despite reporting `DISPLAY=:0` and `WAYLAND_DISPLAY=wayland-0`; no `xvfb-run` is available.
- The resident sidebar is configured for a 420px width, hidden startup, no decorations, and no resize handles. On a Layer Shell-capable Wayland compositor its left/top/bottom anchors and 420px exclusive zone are applied natively before it is shown; unsupported environments use an observable best-effort fallback.
- The first widget set is deliberately limited to compiled `MetricCard`, `StatusList`, and `Notice` variants. It maps only trusted `panel::snapshot()` evidence and does not fabricate CPU or temperature metrics.
- Greetings (`hi`, `hello`, `hey`) are handled locally and do not require a model provider. Failed model responses no longer cause evidence widgets to appear.

---

## 1. Purpose of this document

M0 through M7 produced a substantial Rust backend: discovery, specialists, model routing, the conversational facade, broker enforcement, approvals, transactions, rollback, audit, and a terminal System State panel. M8 attempted to add a Tauri/Dioxus graphical interface, but the result has no working end-to-end launch path.

This document is a detailed handoff for repairing M8. It is intentionally explicit enough for a less capable coding agent to follow without reinterpreting the architecture.

The immediate objective is not to create a visually perfect interface. The immediate objective is to produce a small, correct vertical slice:

1. A visible desktop window launches reliably.
2. The left sidebar accepts natural-language input.
3. Input reaches the real `aios::facade::Facade`, not a mock or echo handler.
4. Aios can use its existing read-only specialist tools to investigate the live machine.
5. The response is displayed in the conversation.
6. Real, structured system evidence is rendered as safe compiled widgets in the canvas.
7. No UI route bypasses the broker, Guardian, approval store, staging, rollback, audit, or secret boundaries.

Do not begin with docking, drag-and-drop panels, streaming tokens, settings, screen vision, or model-generated layouts. Those features come after the vertical slice works.

---

## 2. Authoritative architectural constraints

Any implementation that violates this section is incorrect even if it displays a polished UI.

### 2.1 Authority and execution

- The UI is an **Interface Package**, not an enforcement component.
- The UI may submit user intent and render results.
- The UI must not execute OS commands directly.
- The UI must not hold direct tool handles.
- The UI must not construct authority from model output or graph state.
- Every mutating action must follow the existing broker path.
- The System Graph is advisory. It is not an authorization source.
- Context never grants capability.

### 2.2 Approval

For risk level 3 and 4 actions:

- Approval must use the broker-owned approval channel.
- The UI may render an approval request and collect the user's direct response.
- The UI must not mint an `Approval` object.
- The UI must not modify an approval scope.
- The UI must display the full plan scope, not only a summary:
  - plan hash,
  - every action,
  - every resource,
  - every operation,
  - risk per action,
  - rollback state,
  - expiration,
  - Guardian verdict,
  - Verification Agent verdict.
- Approval cannot override the Guardian or a safety invariant.
- Rejection and timeout result in `Rejected`.
- There is no `Modified` approval decision in v0.1. Modification means rejection followed by a new plan.
- Automatic rollback does not require approval.
- Manual recovery is risk 4 and does require approval.

Existing backend integration points include `Coordinator::issue_reset_approval` and `Coordinator::submit_approval` in `src/coordinator.rs`. Use these or an equivalent broker-owned API. Do not preserve the current Tauri approval stub.

### 2.3 Secrets

- API keys, passwords, tokens, and credentials must never enter model prompts.
- Secret values must never be returned to the frontend.
- Secret values must never appear in Tauri IPC payloads.
- Secret values must never be logged.
- The frontend must not use plain textareas to load, display, or save API keys.
- Provider credentials must remain in the existing local configuration/secret path and be injected by trusted backend code.

### 2.4 Generative UI

- A model must never stream raw HTML, JavaScript, CSS, RSX, Rust, or executable UI code.
- The frontend renders only a predefined, compiled widget set.
- Model output must be deserialized into a closed enum and validated.
- Unknown widget types fail visibly. They are not ignored and do not fall back silently.
- In the preferred design, model-selected widgets reference trusted evidence IDs. The model does not invent displayed metric values.
- Missing evidence displays `UNKNOWN`, `STALE`, or `N/A`; it never displays a fabricated healthy value.

### 2.5 Audit and observability

- UI-originated user intents, model calls, tool calls, approvals, denials, and action results must remain auditable through existing backend paths.
- Do not create a second UI-only audit store.
- Do not log model chain-of-thought.
- Do not log full prompts when they may contain personal data.

---

## 3. Current codebase condition

### 3.1 Dirty working tree warning

At the time this plan was written, the repository already contained uncommitted M8 work. Do not run `git reset --hard`, `git checkout -- .`, broad restore commands, or delete the frontend tree.

Known modified source/config files included:

- `frontend/Cargo.toml`
- `frontend/src/app.rs`
- `frontend/src/components/mod.rs`
- `frontend/src/components/settings.rs`
- `frontend/src/components/sidebar.rs`
- `frontend/src/main.rs`
- `package.json`
- `src-tauri/Cargo.lock`
- `src-tauri/Cargo.toml`

Known untracked authored files included:

- `frontend/Cargo.lock`
- `frontend/src/ipc.rs`
- `frontend/src/types.rs`
- `src-tauri/src/bin.rs`
- `src-tauri/src/main_test.rs`

There were also many generated files under `frontend/target/`, `src-tauri/target/`, and `frontend/dist/`. Generated files are not source work and should eventually be ignored, but do not mix that cleanup with the first functional repair.

Before implementing:

```sh
git --no-optional-locks status --short
git --no-pager diff --stat
```

If possible, the human maintainer should make a safety commit or external backup before large rewrites. An agent must not commit unless explicitly asked.

### 3.2 Existing M8 file layout

```text
frontend/
├── Cargo.toml
├── index.html
├── index.css
├── dist/
│   ├── index.html
│   └── pkg/
└── src/
    ├── main.rs
    ├── app.rs
    ├── ipc.rs
    ├── types.rs
    └── components/
        ├── approval.rs
        ├── canvas.rs
        ├── chat.rs
        ├── settings.rs
        ├── sidebar.rs
        └── widgets.rs

src-tauri/
├── Cargo.toml
├── build.rs
├── tauri.conf.json
└── src/
    ├── main.rs
    ├── bin.rs
    └── main_test.rs
```

There is also a duplicate root `tauri.conf.json`.

---

## 4. Verified failures and root causes

### 4.1 `npm run build` fails

Observed command:

```sh
npm run build
```

Observed failure:

```text
Could not resolve entry module "index.html".
```

Cause: Vite runs from the repository root, but the current `index.html` is under `frontend/`.

Even after correcting the Vite root, `frontend/index.html` currently contains:

```html
<script type="module" src="/src/main.rs"></script>
```

Vite cannot compile Rust directly.

### 4.2 Packaged frontend does not load the generated application

`frontend/dist/index.html` currently contains only:

```html
<html><body>Loading...</body></html>
```

Although `frontend/dist/pkg/` contains generated JavaScript and WebAssembly artifacts, the HTML does not import them. Tauri can therefore display only `Loading...`.

### 4.3 Frontend architectures are mixed incorrectly

`frontend/src/main.rs` launches `dioxus-desktop`:

```rust
let cfg = dioxus_desktop::Config::default();
dioxus_desktop::launch::launch(app::app, vec![], cfg);
```

That creates a standalone native Dioxus window. It is not a web frontend for Tauri.

Tauri expects HTML/JavaScript/WebAssembly inside its webview. A Rust/Dioxus frontend used inside Tauri must be compiled for the web target. The current frontend crate is configured for desktop.

### 4.4 Frontend IPC is a mock

`frontend/src/ipc.rs`:

- sleeps for 500 milliseconds,
- checks for the words `system` or `info`,
- returns hard-coded CPU, memory, and disk values,
- never invokes a Tauri command,
- never calls the real Aios backend.

This file must not be used in the completed vertical slice.

### 4.5 Tauri backend is an echo server

`src-tauri/src/main.rs` currently implements:

```rust
async fn submit_prompt(prompt: String) -> Result<String, String> {
    Ok(format!("Echo: {}", prompt))
}
```

It imports several Aios types but does not instantiate or use `Facade` or `Coordinator`.

### 4.6 There are multiple competing Tauri entry points

- `src-tauri/src/main.rs` is the actual Cargo binary entry point.
- `src-tauri/src/bin.rs` contains an incomplete GTK layer-shell experiment.
- `src-tauri/src/main_test.rs` contains a more complete docking experiment but is not compiled by Cargo.

Do not continue editing all three. Select `src-tauri/src/main.rs` as the only application entry point. Preserve the docking experiment until its useful code has been deliberately migrated.

### 4.7 Duplicate/disconnected frontend components

`frontend/src/app.rs` contains complete inline implementations of the sidebar, chat, approval queue, canvas, and widgets. The files under `frontend/src/components/` contain separate partial or placeholder implementations.

Choose one component tree. Do not maintain two. The recommended target is a componentized frontend where `app.rs` owns state and delegates rendering to files under `components/`.

### 4.8 Configuration problems

- `package.json` has no Tauri CLI dependency and no `tauri` launch script.
- Root and `src-tauri/` both contain Tauri configuration.
- The root config's `frontendDist` is incorrect relative to its location.
- Bundle configuration references `.icns` and `.ico` icons that were not found; only PNG icons were present under `src-tauri/assets/`.
- `tailwind.config.js` contains a malformed content string and does not scan Rust/RSX files.
- `.gitignore` ignores only `/target`, not nested Rust targets.
- `package-lock.json` is ignored even though reproducible frontend dependencies are desirable.

### 4.9 Test result caveat

`cargo test` was run. The result was:

- 304 passed,
- 52 failed,
- 1 ignored.

The failures shared an environmental cause: coordinator/facade tests boot real discovery, which runs `systemctl`; in the test sandbox, `systemctl` exited with status 1. This does not prove the underlying assertions are wrong, but it exposes host-coupled tests.

Do not claim the full suite passes until those tests are made independent of host `systemctl` or are run successfully on a compatible host.

The following focused checks did compile their Rust crates:

```sh
cargo check --manifest-path frontend/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

The Tauri check had unused-import warnings because Aios is imported but not used.

---

## 5. Existing backend to reuse

Do not create a parallel assistant implementation. Reuse these components.

### 5.1 Conversational entry point

`src/facade.rs` defines:

```rust
pub struct Facade {
    pub coordinator: Coordinator,
    // conversation history...
}

impl Facade {
    pub fn boot() -> Result<Self, BootError>;
    pub fn run_line(&mut self, input: &str) -> String;
}
```

`Facade::run_line` already supports:

- natural-language chat,
- `status`,
- `scan`,
- `graph`,
- `panel`,
- `observe`,
- `diagnose`,
- `query`,
- `deps`,
- `impact`,
- `health`,
- `audit`,
- planner/verifier commands,
- consent commands.

Unknown commands are treated as natural-language chat and pass to `Facade::chat`.

### 5.2 Tool-using investigation

`Coordinator::chat_with_tools` in `src/coordinator.rs`:

- adds the approved tool instructions,
- attaches local machine context when routing policy permits,
- permits bounded model tool calls,
- runs calls through `Coordinator::run_tool_as`,
- limits the loop to four turns,
- returns an error if the model never reaches a grounded answer.

This is the correct path for requests such as:

> “My PC is suddenly sluggish. What is going on?”

The UI must call this through `Facade::run_line`, not bypass it.

### 5.3 System State data

`src/panel.rs` defines:

```rust
pub struct PanelSnapshot { ... }
pub fn snapshot(coordinator: &Coordinator) -> PanelSnapshot;
pub fn render(snapshot: &PanelSnapshot) -> String;
```

The snapshot already includes:

- connectivity,
- selected model route,
- graph size,
- health counts,
- subsystem rollups,
- warnings,
- active actions,
- failed actions,
- recent audits.

This is a strong deterministic source for initial canvas widgets.

### 5.4 Specialist data relevant to sluggishness

Existing read-only specialists include:

- `MemorySpecialist` in `src/memory.rs`
- `ProcessesSpecialist` in `src/processes.rs`
- `PowerSpecialist` in `src/power.rs`
- Storage, Network, Drivers, Graphics, Security, Packages, Boot, and Wi-Fi specialists

Current useful evidence includes:

- memory total/available nodes and `size_kb`,
- process nodes, command name, and `rss_kb`,
- thermal sensor values and units,
- health and ownership information,
- subsystem counts and warnings.

A live CPU utilization percentage was not found in the reviewed UI integration path. Add it deliberately in a later phase rather than displaying a mock percentage.

---

## 6. Target architecture

The completed vertical slice should use this flow:

```text
User
  |
  v
Tauri webview frontend
  |  invoke("submit_prompt", { prompt })
  v
Tauri command boundary
  |
  v
AppState owns the real Facade
  |
  +--> Facade::run_line(prompt)
  |      |
  |      +--> Coordinator::chat_with_tools
  |             |
  |             +--> broker --> owning specialist --> result
  |
  +--> panel::snapshot(&facade.coordinator)
  |
  v
UiResponse { answer, evidence, widgets, status }
  |
  v
Frontend validates tagged widget objects
  |
  v
Compiled widget renderer updates canvas
```

No arrow from the frontend may go directly to an OS command, specialist implementation, secret store, or model provider.

---

## 7. Framework decision

### 7.1 Recommended repair decision

For the first working vertical slice, use:

- Tauri v2 for the native shell and Rust command backend.
- Vite for serving/building a small web frontend.
- Tailwind or ordinary CSS for styling.
- Plain TypeScript/JavaScript or minimal framework code for the first slice.
- Rust on the trusted backend boundary.

Do **not** make Dioxus Web compilation a prerequisite for restoring a visible window. The current Dioxus Desktop code can be preserved temporarily while the working Tauri path is established.

Reason: the project currently has a Vite/Tauri pipeline and a Dioxus Desktop pipeline incorrectly merged. Repairing both simultaneously adds build-tool risk without advancing Aios behavior. Once the real vertical slice works, migrate the frontend renderer to Dioxus Web if the maintainer still wants a Rust frontend.

`docs/ui.md` is a draft workstream, not one of the frozen M1 contracts. Changing the frontend rendering framework does not alter the safety model, but the final choice should be recorded in `docs/ui.md` or a UI ADR.

### 7.2 Alternative if Rust frontend is mandatory immediately

Use Dioxus **Web**, not `dioxus-desktop`:

- Change frontend dependencies from desktop to web.
- Install and pin a compatible Dioxus build tool or Trunk.
- Compile to WebAssembly.
- Ensure generated HTML imports the JS/WASM bootstrap.
- Configure Tauri `devUrl` and `frontendDist` to those outputs.

Do not combine `dioxus-desktop::launch` with Tauri.

This alternative is valid but more likely to consume time on frontend toolchain issues. It should not be selected by an agent without explicit maintainer confirmation.

---

## 8. Repair phases

Complete phases in order. Do not start a later phase while an earlier phase fails its acceptance checks.

---

## Phase 0 — Preserve and inventory the current work

### Tasks

1. Run:

   ```sh
   git --no-optional-locks status --short
   git --no-pager diff --stat
   ```

2. Record which files are modified/untracked.
3. Do not remove source files merely because they are currently disconnected.
4. Do not touch M0–M7 behavior during the initial UI repair.
5. Update `.gitignore` in a standalone cleanup change:

   ```gitignore
   /target/
   /frontend/target/
   /src-tauri/target/
   /node_modules/
   /frontend/dist/
   .idea/
   *.iml
   .DS_Store
   ```

6. Decide whether `package-lock.json` is tracked. Recommendation: track it for reproducible Vite/Tauri frontend builds.
7. Do not delete already tracked generated files unless the maintainer explicitly approves repository cleanup.

### Acceptance

- Existing source work remains intact.
- Generated trees no longer produce new untracked noise.
- No broad git restore/reset command was used.

---

## Phase 1 — Create one reliable launch path

### Goal

Launching one documented command opens a visible Tauri window containing a styled sidebar and empty canvas.

### Files

- `package.json`
- `frontend/index.html`
- a new frontend JavaScript/TypeScript entry file, e.g. `frontend/src/main.ts`
- `frontend/index.css`
- `src-tauri/tauri.conf.json`
- root `tauri.conf.json`
- `src-tauri/src/main.rs`

### Required changes

1. Keep `src-tauri/tauri.conf.json` as the canonical Tauri config.
2. Remove or clearly retire the duplicate root `tauri.conf.json` after confirming no script uses it.
3. Remove the explicit window URL `dev://localhost:5173`. Tauri should use `build.devUrl` in development and `build.frontendDist` in production.
4. Set the correct paths for the command working directory. One acceptable approach is to make root scripts explicitly point Vite at `frontend/`:

   ```json
   {
     "scripts": {
       "dev": "vite --config frontend/vite.config.js",
       "build": "vite build --config frontend/vite.config.js",
       "tauri": "tauri",
       "tauri:dev": "tauri dev --config src-tauri/tauri.conf.json",
       "tauri:build": "tauri build --config src-tauri/tauri.conf.json"
     }
   }
   ```

   The exact script syntax may vary with the installed Tauri CLI. Verify rather than guessing.

5. Add the Tauri CLI as a pinned development dependency compatible with Tauri v2.
6. Add `frontend/vite.config.js` with:
   - `root` pointing to `frontend`,
   - fixed port `5173`,
   - `strictPort: true`,
   - output to `frontend/dist`,
   - `clearScreen: false` for useful Tauri logs.
7. Change `frontend/index.html` to load a JavaScript/TypeScript entry, not Rust directly.
8. Ensure `frontend/index.css` is imported by the frontend entry.
9. Correct `tailwind.config.js` syntax. If using Tailwind with TypeScript/HTML, include:

   ```js
   content: [
     "./frontend/index.html",
     "./frontend/src/**/*.{js,ts,jsx,tsx,html}",
   ]
   ```

10. Temporarily exclude missing `.icns` and `.ico` files from bundle config, or generate them deliberately. Do not leave references to nonexistent assets.
11. In this phase, `src-tauri/src/main.rs` can expose only a simple `ping` command. Do not wire the real backend until the window is proven.
12. Do not add docking yet.

### Required UI

The first static view should show:

- A left sidebar approximately 300–400 pixels wide.
- Header: `Aios`.
- Conversation area with `Aios is starting…`.
- Prompt input and Send button.
- Main canvas with `Ask Aios about this system.`.

### Validation

```sh
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri:dev
```

Manual acceptance:

- A window appears.
- It is not a blank white screen.
- Browser/webview console has no boot error.
- Closing the window exits the application.

Do not proceed until this works.

---

## Phase 2 — Boot the real Aios backend in Tauri

### Goal

The Tauri process owns one real `Facade` instance for the life of the application.

### Files

- `src-tauri/src/main.rs`
- optionally a new `src-tauri/src/state.rs`
- optionally a new `src-tauri/src/ui_types.rs`

### State design

Use one long-lived facade so conversation history, model routing state, graph state, audit state, and specialist instances are preserved across prompts.

Start with a structure similar to:

```rust
use aios::facade::Facade;
use std::sync::Mutex;

struct AppState {
    facade: Mutex<Option<Facade>>,
    boot_error: Mutex<Option<String>>,
}
```

Boot in Tauri setup, not separately on every request:

```rust
.setup(|app| {
    let state = match Facade::boot() {
        Ok(facade) => AppState::ready(facade),
        Err(error) => AppState::failed(error.to_string()),
    };
    app.manage(state);
    Ok(())
})
```

This is illustrative, not copy-paste authoritative. Confirm Tauri's `State` bounds and that `Facade` is `Send`. If `Facade` cannot safely live in shared Tauri state:

- create one dedicated backend worker thread,
- move `Facade` into that thread,
- communicate through typed `std::sync::mpsc` or Tokio channels,
- give Tauri commands only a worker client handle.

Do not use `unsafe` to force `Send` or `Sync`.

### Boot failure behavior

A boot failure must be visible and fail fast:

- Keep the window open if possible.
- Return a typed `BackendUnavailable` result from commands.
- Display the actual safe error summary in the sidebar.
- Do not silently substitute mock data.
- Do not silently switch to an echo assistant.

`Facade::boot()` currently performs real discovery and may fail if `systemctl` fails. Preserve this fail-fast behavior for now; improve test injection separately.

### Validation

Add a Tauri command:

```rust
#[tauri::command]
fn backend_status(state: tauri::State<'_, AppState>) -> BackendStatus;
```

The frontend should show one of:

- `Aios ready`
- `Aios unavailable: <safe reason>`

Acceptance:

- The facade boots only once.
- A failed boot produces no mock state.
- No prompt can be submitted before backend status is known.

---

## Phase 3 — Define a typed UI IPC contract

### Goal

Replace stringly typed and mock IPC with a small typed boundary.

### Canonical location

For the first vertical slice, define UI-specific serializable DTOs in `src-tauri/src/ui_types.rs`. They are interface-boundary types, not core authorization protocol types.

Do not duplicate core `Approval`, `ToolRequest`, or capability semantics in frontend-defined objects.

### Minimum response types

```rust
#[derive(Debug, Serialize)]
pub struct UiResponse {
    pub answer: String,
    pub widgets: Vec<UiWidget>,
    pub backend_status: BackendStatus,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "props")]
pub enum UiWidget {
    MetricCard(MetricCardProps),
    Gauge(GaugeProps),
    StatusList(StatusListProps),
    Table(TableProps),
    Notice(NoticeProps),
}

#[derive(Debug, Serialize)]
pub enum UiHealth {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
    Stale,
}
```

Keep the first enum small. Do not implement `ActionForm` until approval and mutation paths are complete.

Every displayed health value should include, where available:

- source,
- observation timestamp,
- freshness state,
- confidence or evidence quality.

### Submit command

Use one command:

```rust
#[tauri::command]
fn submit_prompt(
    state: tauri::State<'_, AppState>,
    prompt: String,
) -> Result<UiResponse, UiError>;
```

Behavior:

1. Reject an empty/whitespace-only prompt.
2. Obtain exclusive access to the one conversation facade.
3. Call `Facade::run_line(&prompt)`.
4. Obtain `panel::snapshot(&facade.coordinator)`.
5. Convert trusted snapshot/evidence into widgets.
6. Return the conversational answer plus widgets.
7. Never fall back to hard-coded values.

If model calls are blocking, avoid freezing Tauri's main event loop. Use a Tauri async command plus `spawn_blocking`, or route work through the dedicated facade worker described in Phase 2.

Do not hold an async mutex guard across unrelated `.await` points.

### Frontend invocation

Use Tauri v2 invoke APIs. The exact import depends on frontend format. Example:

```ts
import { invoke } from "@tauri-apps/api/core";

const response = await invoke<UiResponse>("submit_prompt", { prompt });
```

Do not call `frontend/src/ipc.rs` mock functions.

### Validation

- Submitting `status` returns the real facade status.
- Submitting `panel` returns the real terminal panel answer plus widgets.
- Submitting natural language reaches the configured model provider.
- A provider or boot failure is displayed as an error, not mock success.

---

## Phase 4 — Deterministic canvas widgets from real evidence

### Goal

Render useful widgets before introducing model-generated composition.

### Why deterministic first

This separates two concerns:

1. Can Aios collect correct machine evidence?
2. Can a model choose a good presentation?

If both are implemented simultaneously, invented metrics can be mistaken for real telemetry.

### Initial widget mapping

Convert `panel::PanelSnapshot` into compiled widgets:

- Connectivity → `MetricCard`
- Model route → `MetricCard`
- Total graph nodes → `MetricCard`
- Health counts → `StatusList`
- Subsystem attention counts → `StatusList` or `Table`
- Warnings → `Notice`/`StatusList`
- Active actions → `Table`
- Failed actions/recovery state → high-priority `Notice`

Health mapping must preserve all five states:

- Healthy
- Degraded
- Unhealthy
- Unknown
- Stale

Never map Unknown or Stale to Healthy.

### Evidence-focused response mapping

For natural-language investigation, include only relevant widgets where possible. A safe first implementation may return the full snapshot after every prompt. Optimize relevance later.

### Frontend renderer

The renderer should be a closed switch over `widget.type`:

```ts
switch (widget.type) {
  case "MetricCard":
  case "Gauge":
  case "StatusList":
  case "Table":
  case "Notice":
    // render compiled template
    break;
  default:
    renderWidgetError("Unsupported widget type");
}
```

Do not inject widget data with `innerHTML`. Use DOM text properties or framework escaping.

### Acceptance

- Canvas values come from `PanelSnapshot` or explicitly collected evidence.
- Removing a sensor causes N/A/Unknown, not a hard-coded value.
- Unknown widget types produce a visible error.
- Widget rendering does not execute arbitrary code.

---

## Phase 5 — Complete the “sluggish PC” evidence path

### Goal

A prompt such as:

> “My PC is suddenly sluggish. What is going on?”

causes Aios to inspect real CPU, memory, processes, and thermal state and render the findings.

### Existing usable evidence

- Memory capacity/availability from discovery and `MemorySpecialist`.
- Process name and RSS from discovery and `ProcessesSpecialist`.
- Temperature/fan/power readings from `PowerSpecialist`.
- Storage and general graph health.

### Missing/weak evidence to add

#### CPU utilization

Add a deterministic, read-only telemetry component. Recommended new file:

- `src/telemetry.rs`

Read `/proc/stat` and calculate utilization from deltas:

```text
idle = idle + iowait
total = user + nice + system + idle + iowait + irq + softirq + steal
busy_delta = total_delta - idle_delta
utilization_percent = busy_delta / total_delta * 100
```

Preferred design:

- Maintain the previous sample with its timestamp.
- On the first sample, report `Unknown` because no delta exists.
- On subsequent samples, calculate utilization.
- Reject counter rollback or zero total delta as invalid evidence.
- Do not sleep inside a UI command just to manufacture a second sample unless explicitly accepted as a temporary implementation.

Also consider `/proc/loadavg` for 1/5/15-minute load averages. Label load average correctly; do not call it CPU percentage.

#### Top CPU processes

Current process discovery captures RSS but may not capture CPU percentage. Add per-process CPU counters only if they can be measured correctly from `/proc/<pid>/stat` and normalized over an interval. Until then:

- Show top memory consumers.
- Do not label them as top CPU consumers.

#### Memory pressure

Use available memory and total memory to derive a clearly labeled percentage. Preserve raw values and units. If required fields are absent, report Unknown.

#### Temperature conversion

Sensors using `millidegree_c` must be converted to Celsius deterministically. Preserve source and timestamp.

### Tool instructions

Update the model tool instructions only if necessary so the Planner knows to inspect:

- health,
- memory specialist,
- processes specialist,
- power/thermal specialist,
- storage specialist when I/O pressure is plausible.

The model chooses tools, but displayed metric values must come from trusted tool results/evidence.

### Acceptance scenario

1. Launch Aios.
2. Submit: `My PC is suddenly sluggish. What is going on?`
3. Audit confirms bounded read-only tools were called.
4. Response identifies observed evidence and uncertainty.
5. Canvas displays real available evidence, for example:
   - CPU utilization or Unknown on first sample,
   - load average,
   - memory available/total,
   - top memory-consuming processes,
   - temperature sensors,
   - relevant warnings.
6. Missing sensor data is not fabricated.
7. No mutation or approval occurs for this read-only investigation.

---

## Phase 6 — Add constrained generative UI composition

### Goal

Allow a model to choose how verified evidence is presented without allowing it to invent values or code.

### Do not send raw unrestricted system state

Build an evidence catalog:

```rust
pub struct UiEvidence {
    pub evidence_id: String,
    pub label: String,
    pub value: UiValue,
    pub unit: Option<String>,
    pub health: UiHealth,
    pub source: String,
    pub observed_at: u64,
    pub freshness: UiFreshness,
}
```

Give the model only evidence appropriate for its routing/data classification.

### Preferred model output

The model selects evidence references:

```json
{
  "widgets": [
    {
      "type": "MetricCard",
      "props": {
        "title": "Memory available",
        "evidence_id": "memory.available.percent"
      }
    }
  ]
}
```

The trusted backend resolves `evidence_id` to the actual value. If the model references an unknown ID, reject that widget visibly.

Do not accept:

```json
{
  "type": "MetricCard",
  "props": {
    "value": "42%"
  }
}
```

unless `42%` is validated against trusted evidence.

### Validation rules

- Maximum number of widgets per response.
- Maximum title/label length.
- Maximum table rows and chart points.
- Closed widget enum.
- Evidence IDs must exist in the current response catalog.
- No URLs unless a later policy explicitly permits them.
- No HTML fields.
- No executable expressions.
- No style/CSS fields from the model.
- No action widget in this phase.

### Failure behavior

Invalid structured output:

- does not crash the UI,
- does not silently accept partial arbitrary JSON,
- produces a visible `Could not compose the canvas from the model response` notice,
- retains the conversational answer if safe,
- can fall back only to the **explicitly designed deterministic widget mapping** from Phase 4.

This deterministic mapping is an allowed fallback because it is documented and tested. It must be logged as a composition failure, not hidden.

---

## Phase 7 — Approval UI and mutations

Do not implement this phase until read-only chat and widgets are stable.

### Tasks

1. Define a frontend-safe `ApprovalView` derived from the broker's actual `ApprovalRequest` and plan.
2. Include the complete required scope.
3. Poll or subscribe to pending broker-owned requests.
4. User selection sends only:
   - approval request ID,
   - `Approved` or `Rejected(reason)`.
5. Tauri backend calls `Coordinator::submit_approval` or a stricter typed equivalent.
6. The backend/broker creates and stores the approval.
7. The frontend receives the resulting action state.
8. Keep approvals visually separate from ordinary chat input.
9. Expiration must be visible and enforced by backend time, not browser time alone.

### Remove/replace current stubs

Do not keep the current behavior in:

- `frontend/src/ipc.rs::respond_to_approval`
- `src-tauri/src/main.rs::respond_to_approval`

Those functions currently only format strings and do not use broker authority.

### Tests

- Agent/frontend cannot mint approval.
- Unknown request ID is denied.
- Expired request is rejected.
- Plan hash mismatch is denied.
- Scope mismatch is denied.
- Rejection transitions to `Rejected`.
- Approval does not bypass Guardian block.
- Automatic rollback proceeds without a new approval.
- Manual recovery requires risk-4 approval.

---

## Phase 8 — Docking and resident presence

### Goal

After the application is functional, implement the always-present Linux sidebar/window behavior.

### Current experimental code

`src-tauri/src/main_test.rs` contains GTK layer-shell experimentation:

- initialize layer shell,
- use top layer,
- anchor left/top/bottom,
- reserve an exclusive zone,
- set width to 400.

`src-tauri/src/bin.rs` contains an incomplete version.

### Required decision

There is a design discrepancy that the maintainer must resolve:

- The user's current intent is a sidebar docked to the left edge of the screen.
- `docs/ui.md` currently describes a single 1200×800 movable/resizable Tauri window containing both a 15% sidebar and an 85% canvas, and says it does not reflow other windows.

Do not silently choose one. Update `docs/ui.md` or add an ADR after confirmation.

### Suggested behavior if docked mode is selected

A practical model is:

- Collapsed/resident mode: narrow left sidebar anchored to the screen.
- Expanded response mode: open or expand a canvas window when results are ready.

However, multiple windows conflict with the present `docs/ui.md` single-window decision. This requires an explicit design update.

### Linux compatibility

- `gtk-layer-shell` is compositor-dependent and primarily Wayland/wlroots-oriented.
- Test Ubuntu GNOME Wayland and X11 behavior separately.
- If exclusive-zone docking is unavailable, fail visibly or use an explicitly documented non-docking mode.
- Do not silently claim screen space is reserved when it is not.

### Safety

- The user controls window size/position where the compositor permits.
- The UI must not obscure approval details.
- Screen capture/vision remains out of scope.

---

## Phase 9 — Settings and secret-safe provider configuration

### Current problem

`frontend/src/components/settings.rs` includes a plain `API Keys` textarea. Do not wire this component as written.

### Safe minimum settings

Read-only settings may display:

- provider ID,
- provider tier,
- model ID,
- provider health,
- endpoint hostname if not sensitive,
- whether a credential is configured: `Configured`/`Missing`, never the value,
- connectivity state,
- consent scope.

Credential updates require a separate secret-entry flow:

- input is sent directly to a trusted backend secret-storage command,
- value is never echoed back,
- value is never placed in application logs,
- response only confirms success/failure,
- frontend clears the field immediately,
- model never sees the value.

Do not return the current configuration file if it contains inline API keys.

---

## Phase 10 — Testing and completion criteria

### 10.1 Build tests

```sh
npm run build
cargo check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

If retaining a Rust frontend:

```sh
cargo check --manifest-path frontend/Cargo.toml
```

### 10.2 Backend unit tests

Add tests for:

- `PanelSnapshot` to `UiWidget` conversion.
- Unknown/Stale preservation.
- Unit conversion.
- CPU sampler delta calculation.
- Empty prompt rejection.
- Boot failure response.
- Unsupported widget rejection.
- Evidence-reference validation.
- No secret fields in serialized IPC responses.

### 10.3 Tauri command tests

Keep command logic in ordinary Rust functions so it can be tested without launching a webview.

Test:

- one facade is reused across turns,
- natural-language prompt invokes real facade,
- status command returns live state,
- backend failure returns typed error,
- approval response calls broker-owned path,
- concurrent prompt behavior is defined.

For v0.1, serialize prompt processing per session. Do not run two mutable conversation turns concurrently against the same facade.

### 10.4 Frontend tests

At minimum test:

- prompt submission state,
- error display,
- loading/disabled input behavior,
- every widget enum variant,
- Unknown and Stale styling,
- unsupported widget error,
- escaping of text content,
- approval detail completeness when Phase 7 lands.

### 10.5 End-to-end manual checklist

- [ ] One documented command launches the app.
- [ ] Window is visible and styled.
- [ ] Sidebar accepts input.
- [ ] `status` shows actual backend state.
- [ ] `panel` produces real widgets.
- [ ] Natural-language chat reaches the configured model.
- [ ] Read-only investigation can call brokered tools.
- [ ] Canvas contains no hard-coded CPU/memory/disk values.
- [ ] Missing telemetry shows Unknown/Stale/N/A.
- [ ] Model unavailable produces recovery-only/unavailable state.
- [ ] Audit contains the interaction/tool events.
- [ ] No secrets appear in frontend payloads or logs.
- [ ] No UI command can directly execute an OS mutation.
- [ ] Approval, when implemented, is broker-owned and full-scope.
- [ ] Closing the application exits cleanly.

### 10.6 M8 is not complete until

1. The UI is demonstrably launchable.
2. It uses real Aios state.
3. It has no mock metrics in the production path.
4. Natural-language investigation works.
5. The canvas renders trusted evidence through compiled widgets.
6. Failure states are visible and fail closed.
7. Security and approval boundaries are preserved.
8. Focused UI/backend tests pass.
9. The launch procedure is documented.

---

## 9. File-by-file disposition guide

### Root

#### `package.json`

Repair scripts and add/pin Tauri CLI. Ensure Vite uses `frontend/` as root.

#### `tailwind.config.js`

Fix malformed syntax. Scan actual frontend sources. If the initial implementation uses plain CSS, Tailwind may be removed later, but do not leave a broken configuration.

#### `postcss.config.js`

Keep only if Tailwind/PostCSS remains in the build.

#### `tauri.conf.json`

Retire after confirming `src-tauri/tauri.conf.json` is canonical. Do not maintain two diverging configs.

#### `.gitignore`

Ignore nested targets, `node_modules`, and generated frontend output. Decide to track `package-lock.json`.

### Frontend

#### `frontend/index.html`

Replace direct Rust source import with the real web entry. Ensure root mount element exists.

#### `frontend/index.css`

Keep/reset global layout. Ensure full-height body/root. Add accessible focus states and readable error colors.

#### `frontend/dist/`

Generated output only. Never hand-edit `dist/index.html` as the source fix.

#### `frontend/Cargo.toml`

If using the recommended initial TypeScript/JavaScript frontend, this crate is temporarily unused. Preserve it until the maintainer decides whether to migrate to Dioxus Web or remove it. Do not continue adding desktop dependencies.

If moving to Dioxus Web, remove `dioxus-desktop` and desktop features.

#### `frontend/src/main.rs`

Current standalone desktop launcher. Not part of Tauri web path. Preserve or convert only after framework decision.

#### `frontend/src/app.rs`

Current Dioxus UI with inline duplicated components and mock IPC. It is prototype/reference code, not a production path.

#### `frontend/src/ipc.rs`

Remove all hard-coded responses from production. Replace with actual Tauri IPC only if retaining Dioxus Web.

#### `frontend/src/types.rs`

Do not allow this file to become an independent authority for risk or approval. UI widget DTOs may mirror the backend schema, but backend serialization is authoritative.

#### `frontend/src/components/*`

Several files are placeholders or disconnected. Consolidate only after the real launch/IPC path works.

#### `frontend/src/components/settings.rs`

Do not expose API keys in a general textarea.

### Tauri

#### `src-tauri/Cargo.toml`

Keep the `aios` path dependency. Remove unused GTK layer-shell dependencies until Phase 8, or gate them behind a feature such as `linux-docked` so ordinary builds are not forced to compile compositor-specific code.

#### `src-tauri/src/main.rs`

Make this the only entry point. It should:

- manage backend state,
- boot the real facade,
- expose typed commands,
- register command handlers,
- never contain hard-coded system responses.

#### `src-tauri/src/bin.rs`

Not a Cargo bin target by filename alone in this layout and currently incomplete. Migrate useful docking code during Phase 8, then remove it after maintainer approval.

#### `src-tauri/src/main_test.rs`

Not an automated Rust test; it is an alternate executable experiment. Rename/move it if retained. Migrate useful docking code only after Phase 7.

#### `src-tauri/tauri.conf.json`

Canonical config. Correct dev/build commands, URL behavior, frontend output, and icon references.

### Core Aios

#### `src/facade.rs`

Reuse `Facade::boot` and `Facade::run_line`. Do not duplicate chat logic in Tauri.

#### `src/coordinator.rs`

Reuse tool-using chat, specialist routing, panel access, and broker-owned approval APIs. Avoid unrelated refactoring during UI vertical slice.

#### `src/panel.rs`

Use `PanelSnapshot` as the deterministic initial canvas source. Consider deriving/adding serialization only if that does not expose unsuitable internal fields; otherwise map it to UI DTOs in Tauri.

#### `src/discovery.rs`

Add telemetry only when necessary and keep it deterministic/read-only. For CPU utilization, a separate telemetry module may be cleaner than bloating one-time discovery.

#### `src/lib.rs`

Export a new telemetry module only when Phase 5 is implemented.

---

## 10. Known non-UI issue to address separately

The current coordinator/facade tests are host-coupled because boot invokes real service discovery. In environments without a functional systemd instance, `systemctl` failure causes dozens of tests to fail.

Recommended separate repair:

1. Introduce an injectable discovery/service-discovery trait.
2. Production boot uses real sysfs/procfs/systemctl discovery.
3. Unit tests use deterministic fake discovery.
4. Keep at least one explicitly marked host integration test for real Linux discovery.
5. Do not silently ignore `systemctl` failure in production merely to make tests pass; that would conflict with fail-fast behavior.

Do not mix this refactor into Phase 1 unless it blocks launching on the maintainer's actual machine.

---

## 11. Anti-patterns: instructions for future agents

A future agent must not:

- Replace real backend calls with mocks to make the UI look functional.
- Display hard-coded CPU, memory, disk, or temperature values.
- Make Vite import `.rs` source directly.
- Launch `dioxus-desktop` inside a Tauri architecture.
- Maintain multiple Tauri `main` implementations.
- Hand-edit `frontend/dist/` as the source of truth.
- Put API keys in frontend state returned by the backend.
- Let the model generate HTML/JavaScript/CSS/RSX.
- Let model output supply unverified metric values.
- Use `innerHTML` for model or tool content.
- Let the UI call shell commands.
- Let the UI mint approvals.
- Show only an approval summary for risk 3+ actions.
- Treat graph health as authorization.
- Turn Unknown or Stale into Healthy.
- catch and hide backend boot errors.
- add an undocumented fallback.
- use `unsafe` to force backend state across threads.
- delete existing user work or reset the dirty working tree.
- declare M8 complete merely because `cargo check` passes.

---

## 12. Suggested first implementation session

A competent next agent should limit its first change set to the following:

1. Fix the Vite root/build.
2. Consolidate Tauri config.
3. Create a visible static sidebar/canvas.
4. Boot and store one real `Facade`.
5. Add `backend_status` and `submit_prompt` commands.
6. Make `submit_prompt` call `Facade::run_line`.
7. Render the returned answer in chat.
8. Convert `panel::snapshot` into a small deterministic widget list.
9. Add focused tests for widget conversion and command behavior.
10. Run focused build/check commands.

Explicitly defer:

- docking,
- Dioxus migration,
- generative widget selection,
- settings,
- approval UI,
- screen vision,
- drag/drop panels,
- streaming tokens.

That first session should end with a real, useful Aios window even if the visuals are basic.

---

## 13. Handoff summary

The M8 failure is not evidence that the Aios backend is absent. The backend already has a real conversational facade, live discovery, brokered read-only specialist tools, model routing, audit, approvals, transactions, and a structured terminal panel.

The failure is an integration failure:

- Vite is pointed at the wrong place.
- Vite is asked to compile Rust directly.
- Dioxus Desktop and Tauri WebView architectures are mixed.
- Built WASM is not loaded by the packaged HTML.
- Frontend IPC is mocked.
- Tauri backend only echoes.
- Generative widgets are disconnected from trusted system evidence.

Repair the integration in narrow, testable phases. First make the real backend visible. Then make it useful. Then make it generative. Finally add high-risk interactions and docking without weakening the security model.
