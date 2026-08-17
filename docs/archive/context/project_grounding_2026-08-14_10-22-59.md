# Aios Project Grounding

> Archived grounding snapshot. The current snapshot is linked from
> `PROJECT_GROUNDING.md`.

**Snapshot:** 2026-08-14 10:22:59 EDT  
**Purpose:** Durable context for restarting work after a context reset.  
**Scope:** Documentation and source review completed in this session.

This is a working context snapshot, not a new architecture contract. The focused
documents under `docs/` remain authoritative where they define contracts. Code
wins when it differs from an older design statement.

## Project In One Paragraph

Aios is a Rust user-space operating-system layer that runs above Linux. It gives
the user one conversational interface over a planner, verifier, bounded domain
specialists, deterministic policy enforcement, staged transactions, health
checks, audit logging, and recovery. The central rule is:

> No component should both make an autonomous decision and possess unrestricted
> authority to execute it.

Models propose and explain. They do not directly control the machine. The broker
decides whether a typed request is allowed, the Guardian can veto higher-risk
work, and the executor provides checkpointing, health verification, commit, and
rollback.

## Source Documents Read

Root Markdown files:

- `README.md`
- `CONTRIBUTING.md`
- `SECURITY.md`
- `session-notes.md`

The complete Markdown tree under `docs/` was read, including:

- Foundation and contracts: `architecture.md`, `glossary.md`, `requirements.md`,
  `security-model.md`, `capability-model.md`, `message-protocol.md`,
  `action-state-machine.md`, `system-graph.md`, `agent-packages.md`,
  `model-routing.md`, `human-interaction.md`, `observability.md`,
  `testing-strategy.md`
- Progress and planning: `doc-progress.md`, `implementation-roadmap.md`,
  `specialist-depth-plan.md`, `ui.md`, `m8-ui-repair-plan.md`
- ADRs: `decisions/0001` through `decisions/0006`
- All 19 module specifications under `docs/modules/`

## Non-Negotiable Design Rules

- v0.1 runs above Linux. It does not modify the kernel, boot chain, or firmware.
- Agents have no unrestricted OS authority.
- The Policy Broker is the authority for capabilities and action gating.
- The System Graph is advisory. It is never the permissions database.
- The broker maintains its own resource-state registry.
- Resource state claims must come from the owning specialist.
- Context never grants capability.
- External data, files, device state, and model output are untrusted.
- Authorization is two-dimensional: capability `(resource, operation)` plus
  static clearance for the tool risk level.
- Risk levels are 0 read-only, 1 routine, 2 staged mutation, 3 critical
  mutation, and 4 recovery.
- Risk 2+ requires Guardian review.
- Risk 3+ requires scoped, expiring user approval.
- Approval is bound to the exact plan hash and action/resource/operation/tool
  scope.
- The facade renders approval requests but cannot create or relay authority.
- Automatic rollback needs no new approval. Manual recovery is risk 4.
- Unknown, stale, ambiguous, missing, or unaudited state fails closed.
- Audit-log write failure stops mutations and leaves the system read-only.
- No secrets enter prompts, tool results, audit records, or frontend payloads.
- No model is placed in a real-time safety loop.
- Recovery must work without models, agents, or the message bus.
- Development errors must fail fast. No undocumented fallback behavior.

## Core Runtime Shape

The main runtime is in `src/` and is one Rust crate named `aios`.

### Protocol and authority

- `src/protocol.rs` owns message-bearing types: envelopes, action plans,
  verification reports, tool requests/results, events, approvals, health
  reports, policy decisions, user responses, and errors.
- `src/capability.rs` owns principals, resources, operations, risk levels,
  clearance, capability tokens, deny reasons, tool definitions, and the tool
  registry.
- `src/broker.rs` contains the pure `PolicyBroker` and the runtime `Broker`/
  `LocalBroker` wrapper. The pure decision order is tool lookup, deadline,
  nonce, principal, required capabilities, resource state, token, clearance,
  Guardian, approval, then allow/deny. The runtime forwards low-risk calls to
  specialists and sends higher-risk calls through the executor.
- `src/guardian.rs` contains deterministic invariant checks for firmware,
  kernel modules, and boot configuration.

### Transactions and audit

- `src/action.rs` defines action states, valid transitions, checkpoints,
  durable action records, and `FileActionStore`.
- `src/executor.rs` implements write-ahead transition journaling, fsync-backed
  action/checkpoint storage, staging, health checks, commit, rollback, reset,
  crash recovery, and manual recovery.
- `src/audit.rs` implements an in-memory buffer plus optional append-only file
  output and forward-chained SHA-256 entries.

### Discovery and graph

- `src/graph.rs` implements the in-memory typed System Graph, ownership
  enforcement, dependency traversal, subgraphs, health, staleness, and impact
  reports.
- `src/discovery.rs` scans `/proc`, `/sys`, sysfs, firmware class entries,
  filesystems, sensors, processes, and Linux network interfaces. It links
  interfaces to underlying PCI/USB devices and reconciles additions/removals.
- `ServiceDiscovery` invokes `systemctl`; failures are visible rather than
  silently converted into an empty service set.

### Models and agents

- `src/model.rs` implements connectivity states, provider tiers, model registry,
  provider health cooldowns, consent, routing, task pinning, model backends,
  and the model gateway.
- `src/local.rs` runs local GGUF models through `llama.cpp`.
- `src/http.rs` is the universal OpenAI-compatible remote backend.
- `src/hub.rs` verifies model files by SHA-256 and can resolve/download catalog
  models through an injected HTTP client.
- `src/config.rs` loads `~/.aios/config.toml` or `AIOS_CONFIG`, including local
  and OpenAI-compatible providers.
- `src/planner.rs` handles model submission, `<think>` removal, plan parsing,
  balanced JSON extraction, and native/simple tool-call parsing.
- `src/verifier.rs` parses independent review results and defaults malformed
  review output to insufficient information.
- `src/coordinator.rs` boots the provider gateway, discovery, graph, broker,
  all current specialists, session capabilities, handlers, and executor.
- `src/facade.rs` is the terminal interface and conversational entry point.
- `src/tools.rs` contains generic graph tools and the model tool instructions.
- `src/panel.rs` produces the read-only System State snapshot.
- `src/harness.rs` is a deterministic read-only policy campaign with virtual
  resources and optional enforcement mode.
- `src/main.rs` runs either the in-process demonstration or `shell`.

## Specialist Implementation Status

The coordinator currently wires these specialists when their resources exist:

- `wifi.rs`: Wi-Fi discovery, ownership, observe/diagnose, staged driver, and
  risk-4 reset definitions.
- `wifi_driver.rs`: bounded driver control. Linux mutation is dry-run by
  default; `AIOS_LIVE_DRIVER_CONTROL` selects live control.
- `storage.rs`: block devices and filesystems. Deep structured disk and
  filesystem evidence is implemented. Mutations remain deferred.
- `network.rs`: wired interfaces and Bluetooth controllers. Read-only observe
  and diagnose are implemented, but output still contains the older resource
  summary shape.
- `drivers.rs`: unclaimed PCI/USB hardware, firmware, and loaded drivers.
  Read-only only; mutation tools are deferred.
- `graphics.rs`: GPU, display-service, and session resources. Read-only only.
- `memory.rs`: full discovered meminfo fields, pressure, vmstat, swap, and
  capacity evidence. Read-only only.
- `power.rs`: thermal, fan, voltage, power, and current sensor grouping.
  Read-only only; unit conversion and richer evidence remain unfinished.
- `security.rs`: seeded enforcement-plane domain. Read-only observe/diagnose;
  quarantine is deferred.
- `processes.rs`: process discovery plus windowed system and per-process CPU
  sampling, RSS, state, and command-line evidence.
- `packages.rs`: package-node observation/diagnosis. Actual package discovery
  is not implemented in `discovery.rs` yet.
- `boot.rs`: seeded boot image, snapshot, and watchdog nodes. Read-only only;
  real boot/recovery grounding is deferred.

The module specifications also describe child specialists for storage
Block/Disk, Filesystem, and Files/Data; network Wired/LAN and Bluetooth; and
graphics GPU, Display, and Session. Those child implementations are not
separate source modules yet. Their current behavior is represented by umbrella
specialists and discovery data.

## Specialist Evidence Shape

The intended direction is complete structured evidence rather than a count plus
`resources` and `state:<id>` debug fields.

Already deep or mostly deep:

- Memory: typed memory totals, available/used/free, swap, meminfo, pressure,
  vmstat, and OOM/page counters.
- Storage: `disk_N` rows with capacity/I/O/queue attributes and `fs_N` rows with
  mount, options, read-only state, and statvfs usage.
- Processes: CPU utilization, core count, and `top_cpu_N` rows over a 100 ms
  sample window.

Still shallow or mixed:

- Network
- Drivers
- Graphics
- Power/thermal
- Security
- Packages
- Boot/recovery

These still emit legacy `resources` and/or `state:<id>` fields. The model
instruction claims are ahead of several implementations. The
`specialist-depth-plan.md` is the intended sequencing for closing this gap.

## Broker and Security Findings

These are implementation facts to remember before changing behavior:

1. `ToolParameters` are not currently validated against `Operation` in the
   broker. A request can carry a mismatched parameter variant if it has the
   right tool/capability. Read-only handlers can panic when they receive a
   mutating parameter through `tool_arguments`.
2. The broker does not currently verify that `envelope.origin` matches the
   authenticated sender/channel identity. The protocol document says the
   broker should set or authenticate origin.
3. `CapabilityToken` is a clonable public struct and `BrokerClient` exposes
   token retrieval. The design calls for opaque broker-owned handles. The
   current session flow does issue tokens at session setup, but the type-level
   boundary is weaker than the document describes.
4. `PolicyBroker` keeps policy decisions in an in-memory vector. Coordinator
   audit calls go to `AuditLog`, but broker decisions are not directly written
   into the persistent hash-chained audit file.
5. Initial domain resource state is set to `Available` during coordinator boot.
   The broker's resource registry is not continuously reconciled from graph
   health/staleness or specialist events in the normal coordinator path.
6. The audit log startup parser intentionally fails on malformed current-format
   logs, but there is no migration path for the old space-separated format.
   The existing `/home/shane/.aios/audit.log` is legacy format and causes every
   coordinator/facade test to panic during boot.
7. `AuditLog::record` computes hash contents with one timestamp and stores the
   entry with a second timestamp. If the second changes between those calls,
   later in-memory verification can disagree with the stored hash.

## M8 Active UI

The active application path is Tauri plus Vite/TypeScript, not the Dioxus
desktop crate.

### Active path

```text
frontend/src/main.ts
  -> Tauri invoke("submit_prompt")
  -> src-tauri/src/main.rs
  -> dedicated worker thread owns one Facade
  -> Facade::run_line
  -> Coordinator::chat_with_tools_outcome
  -> broker -> specialist -> typed ToolResult
  -> PromptResponse
  -> frontend sidebar and canvas window
```

- `src-tauri/src/main.rs` boots one real `Facade` on a worker thread.
- `backend_status` exists, though the active TypeScript UI does not currently
  use it before submission.
- `submit_prompt` returns answer, evidence, widgets, and backend status.
- Evidence is sent to the sidebar and to the separate canvas window.
- `compile_widgets` currently creates one compiled `StatusList` from evidence.
- The canvas is a separate window and has left/right/top/bottom positioning.
- GTK Layer Shell is attempted first; X11 EWMH dock fallback is configured.
- Pure Wayland behavior remains compositor-dependent.
- The Tauri backend has a null CSP in `tauri.conf.json`.
- The Tauri config currently sets both sidebar and canvas `alwaysOnTop` true.

### Disconnected UI code

`frontend/src/main.rs`, `app.rs`, `ipc.rs`, and `components/` are an older
Dioxus desktop implementation. They are not the active Tauri webview path.
They still contain:

- mock prompt responses;
- hard-coded CPU, memory, and disk metrics;
- mock approval responses;
- a plain API-key textarea;
- duplicate sidebar/canvas/widget implementations.

Do not treat that code as production behavior. Either retire it or migrate it
deliberately after the active path is stable.

### M8 remaining work

- Replace temporary evidence `StatusList` output with closed-enum,
  model-selected composition.
- Prefer model-selected evidence IDs resolved by trusted backend data.
- Reject unknown widget types and unknown evidence IDs visibly.
- Preserve `UNKNOWN`, `STALE`, and `N/A`; never accept invented values.
- Keep action widgets out until the broker-owned approval UI is implemented.
- Add typed approval UI through `Coordinator::issue_reset_approval` and
  `submit_approval` rather than the old mock functions.
- Add focused UI/backend tests.
- Verify the desktop behavior on a real GTK display and supported compositors.

### Clarified Generative Surface Direction

The detached canvas is the next primary product surface. It should not be
treated as a fixed dashboard that always renders the same collection of cards.

There are two display modes:

1. An explicit request such as "show me the system dashboard" or "overall
   system status" opens a deliberate, deterministic overview surface. This is
   the one excellent standard surface. It should consistently cover the major
   system domains and make health, stale, unknown, and missing evidence clear.
2. Other requests should produce a dynamic surface whose layout depends on the
   actual evidence returned for that request. A storage question should not be
   forced into the system dashboard layout, and a Wi-Fi diagnosis should not
   reserve space for unrelated domains.

The generator is a presentation component, not a system agent. Its input is
only the trusted display payload/evidence that the backend has already selected
for display. It must not query the system, call specialists, invoke tools,
change graph state, request capabilities, approve actions, or execute commands.
The generator can be local or remote, but its only system interaction is the
display payload crossing into it.

The intended flow is:

```text
user request
  -> planner/coordinator retrieves typed specialist evidence
  -> backend builds a display payload with evidence ids, values, provenance,
     timestamps, health, and stale/unknown state
  -> UI generator chooses a layout and closed widget vocabulary from that payload
  -> validator rejects unknown widgets, evidence ids, or invented values
  -> detached canvas renders the validated composition
```

This must work on arbitrary supported Linux systems. The composition cannot
assume a particular GPU, disk count, network device, sensor set, screen size,
desktop environment, or specialist availability. The payload needs explicit
absence and uncertainty states so the generated UI can adapt instead of filling
gaps with made-up data.

GenUI/A2UI-style frameworks may be useful for the composition protocol or
renderer, but they do not change the authority boundary. They should be
evaluated as presentation tooling only. No framework should be allowed to turn
model-generated UI into a new path to system authority.

The implementation order should therefore be:

- define the typed display payload and evidence-reference contract;
- keep the deterministic overall-system dashboard as a special composition;
- implement dynamic evidence-bound compositions for non-dashboard requests;
- validate generated compositions against the backend payload and closed widget
  registry;
- add action widgets only after they are routed through the existing broker and
  approval channel.

The current `compile_widgets` function is only a temporary `StatusList` bridge.
It should evolve into the backend composition boundary, not into a collection
of hard-coded dashboard cards.

### Chat Sidebar Role

The current chat interface is only a skeleton. The intended sidebar is the
permanent Aios control surface, not merely a text input and message stream.

It should eventually contain:

- the conversational chat stream;
- Aios settings and provider/model configuration;
- Aios system controls and visible runtime state;
- session creation, switching, and history;
- minimized generated surfaces, so detached canvases can be collapsed back into
  the sidebar without losing their state;
- approval and recovery controls once those are connected to the broker-owned
  approval path.

The responsibilities should remain separate:

- the sidebar is the stable control and navigation surface;
- the detached canvas is the dynamic evidence presentation surface;
- the backend/facade remains the only path to system state and authority.

Minimizing a generated surface should preserve its validated composition and
evidence context. Restoring it should not require rerunning a system action or
silently refresh data without showing that the evidence changed.

## Test and Build State

Verified during this snapshot:

```text
HOME=/tmp/opencode/aios-test-home \
RUSTUP_HOME=/home/shane/.rustup \
CARGO_HOME=/home/shane/.cargo \
cargo test --lib

366 passed; 0 failed; 1 ignored
```

The ignored test is the real GGUF model test and requires `AIOS_MODEL_PATH`.

```text
npm run build
```

Vite build passes.

```text
cargo check --manifest-path src-tauri/Cargo.toml
```

Tauri check passes with dead-code warnings for `UiWidget::MetricCard` and
`UiWidget::Notice`, which are declared but not constructed by the temporary
compiler.

The normal workspace test command uses `~/.aios/audit.log` and currently fails
with 52 boot-related failures because that file ends in the pre-hash,
space-separated format. This is reproducible with both parallel and
single-threaded test execution.

## Current Worktree Note

At snapshot time, `git status --short` showed:

```text
 M frontend/dist/index.html
 M src-tauri/Cargo.lock
 M src-tauri/target/debug/build/aios-tauri-a27541ea8da0d947/out/capabilities.json
```

These are generated/build-related changes observed during the grounding pass.
Do not reset or discard them without checking ownership and intent first.

## Recommended Restart Order

1. Read this file.
2. Read `docs/doc-progress.md`, `docs/specialist-depth-plan.md`, and
   `docs/m8-ui-repair-plan.md`.
3. Check `git status --short` before editing.
4. Decide whether the next task is evidence depth, broker hardening, audit-log
   migration/isolation, or M8 generative UI.
5. If running tests, use an isolated `HOME` or deliberately handle the legacy
   audit file. Do not silently weaken the audit parser merely to make tests pass.
6. Preserve the existing broker path. Do not add UI-local authority, shell
   execution, mock metrics, or undocumented fallbacks.

## Short Status

The backend is real and broadly functional. The current project is not blocked
by the absence of an Aios core. The main unfinished work is integration and
hardening:

- complete structured specialist evidence;
- enforce protocol invariants at the broker boundary;
- connect broker decisions to durable audit behavior;
- make persisted audit-log upgrades safe and testable;
- finish evidence-grounded generative UI;
- remove or quarantine disconnected mock frontend code.
