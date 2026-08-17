# Aios Session Notes

> Archived context snapshot from 2026-08-12. Use `PROJECT_GROUNDING.md` and its
> latest linked snapshot for current work.

> Durable context snapshot. Captures the full design-doc read (docs/) and the
> full source read (src/, ~20.5k lines Rust) as of 2026-08-12. Use this as the
> starting point for context instead of re-reading the whole tree.

---

## 1. What Aios is

Aios = **Artificially Intelligent Operating System**. One conversational
interface over a coordinated system of specialized agents and deterministic
services. The central safety principle:

> **No component should both make an autonomous decision and possess
> unrestricted authority to execute it.**

Aios is a user-space Rust prototype running above Linux (ADR-0001). It does
not modify the kernel, boot chain, or firmware in v0.1.

### Three planes

- **Agent plane** — reasoning/coordination: conversational facade, Planner,
  Verification Agent, domain/hardware specialists. Propose, analyze, explain.
  No automatic OS authority.
- **Enforcement plane** — execution/safety (the TCB): Policy Broker,
  Guardian, typed tools, Staged Transaction Executor, health checks, audit
  log. Deterministic; never trusts a model's output as proof of safety.
- **Trust plane** — lowest-level recovery/integrity (firmware/boot verify,
  watchdogs, known-good images, recovery supervisor). Must survive agent/bus
  failure. Largely deferred past v0.1.

### Dual-agent bridge

- **Planner Agent** — understands intent, produces a structured plan. No
  direct execution authority by default.
- **Verification Agent** — independently challenges intent/plan/assumptions.
  No direct execution authority.
- Agreement between them is **not** proof of correctness (shared blind
  spots). Output is advisory until accepted by the enforcement plane.

### Key principles

- **Fail-closed** (REQ-SAF-002): on ambiguity/missing data → deny.
- **Fail-fast, no silent fallbacks** (ADR-0003): dev-time errors surface
  immediately; no hidden fallbacks.
- **Two-dimensional authorization** (ADR-0004): capability (resource ×
  operation) **and** clearance (tool risk level 0–4).
- **Context never grants capability** (REQ-SAF-005): all external data
  untrusted; prompt injection must not elevate authority.
- **Secrets never leave the local trust boundary** (REQ-SAF-006).
- **Token cost is not a design constraint** for safety systems.
- **Lose intelligence before losing the ability to recover** (REQ-SAF-007).

---

## 2. Tool risk levels & clearance

| Level | Name | Gates | Examples |
|---|---|---|---|
| 0 | Read-only | Capability only | observe, diagnose, query |
| 1 | Routine | Capability + broker | restart, configure |
| 2 | Staged mutation | + Guardian + staging | stage, commit |
| 3 | Critical mutation | + user approval + staging | firmware_write, boot_config, kernel_module |
| 4 | Recovery | + user approval (staging may be skipped if Guardian authorizes; checkpoint still created) | reset, quarantine, rollback |

- `RiskLevel::requires_guardian()` = level >= 2; `requires_approval()` = level >= 3.
- Clearance is **static** (set at instantiation from package manifest). Higher
  clearance = package revision, not runtime request.
- Broker resolves authoritative risk level from the **ToolRegistry by
  tool_id** — never from the request (agent cannot lower its own risk).

---

## 3. Policy Broker decision algorithm (capability-model §5.2)

```
0.   Resolve tool from ToolRegistry by tool_id (authoritative risk_level)
0.5  Check deadline (envelope.deadline) — missing/expired → DENY
0.6  Check nonce (anti-replay) — duplicate (principal, nonce) → DENY
1.   Validate principal identity → DENY if unknown
2.   Validate capability (from tool's required_capabilities) + resource state
     (Discovered→read-only only; Quarantined→level 4 only; Removed→DENY)
2.5  Validate token (expiration, revocation)
3.   Validate clearance (agent clearance >= tool risk_level)
4.   Risk >= 2 → Guardian review (unavailable → DENY; Escalate collapsed to Deny)
5.   Risk >= 3 → user approval (plan hash, action_id, scope all checked)
6.   Risk >= 2 → authorize staged execution (executor does checkpoint/stage/health/commit)
7.   Audit log entry (always, incl. denials) — write failure → DENY
```

- Broker keeps its **own `resource_states` registry** (ADR-0005 P1-5), updated
  only by the owning specialist via `apply_resource_event` (non-owner claims
  rejected). System Graph is advisory, not authoritative.
- `GuardianVerdict` has only `Allow` / `Block(String)` in v0.1 — no `Escalate`.

---

## 4. Action state machine (action-state-machine.md)

States: `Proposed → ImpactAnalyzed → Reviewed → PolicyValidated →
GuardianChecked → Approved → Staged → HealthVerified → Committed` (or
`RollingBack → RolledBack`), plus `Rejected` and `Failed`.

- Terminal: `Committed`, `RolledBack`, `Rejected`, `Failed`.
- Risk 0–1 fast path: `PolicyValidated → Committed` (skip Guardian/staging).
- Risk 4 fast path: `Approved → Committed` (skip staging if Guardian
  authorizes; checkpoint still created first).
- **Write-ahead journaling**: pending transition persisted before executing;
  cleared after durable state update. Crash recovery reads state + journal.
- Checkpoints: created before staging, verified, deleted after commit/rollback,
  **retained on `Failed`** for manual recovery.
- Recovery is deterministic, no AI required (REQ-REL-002).

---

## 5. Security model (security-model.md)

**TCB (v0.1, in-process):** Policy Broker, Guardian (read-only veto), Staged
Executor, Capability Token Verification, Audit Log, Agent Package Loader +
Signature Verifier. Non-TCB: agents, specialists, graph, models.

**Trust boundaries:**
- A: External → Agent (all untrusted, prompt-injection defense)
- B: Agent → Enforcement (broker validates; no agent output trusted as safety)
- C: Enforcement → OS (only broker-approved, capability-validated, staged)
- D: OS → Trust plane (recovery independent of agent plane)

**Secrets:** v0.1 uses Linux keyring; only broker accesses it; secrets never
in prompts, messages, logs, or tool results visible to agents. Redaction layer
is part of the TCB.

**Audit log:** append-only, forward-chained SHA-256 hashes
(`entry.hash = SHA256(contents ++ previous_entry_hash)`). Tampering
detectable. Audit write failure → fail-stop (no unaudited actions).

---

## 6. System Graph (system-graph.md)

Five layers: physical, OS, agent, model/gateway, trust/recovery. Typed edges:
`owns`, `depends_on`, `communicates_with`, `observes`, `controls`, `affects`,
`hosted_on`, `fallback_to`.

- **One owner per resource** (`owns` edge enforced single).
- Edge provenance: Declared / Attested / Observed (trusted differently).
- Advisory for routing/analysis — **never** the authority for permissions.
- Staleness: nodes carry `expires_at`/TTL; stale → `STALE`, never silently
  healthy. Fail-closed on missing/conflicting data.
- v0.1 in-memory; SQLite in v0.2+.

---

## 7. Agent Packages (agent-packages.md)

A package is more than a prompt: manifest, tools, capabilities, invariants,
health checks, recovery, model/data policy, resource budgets, tests. Signed
and versioned. Capabilities requested, **granted by broker** — context never
grants authority. Unknown hardware → read-only inspector or quarantine, never
an invented privileged package.

---

## 8. Model routing (model-routing.md)

- Tiers: Local (Qwen baseline) → LAN gateway → Internet provider.
- Connectivity states: `Offline` / `LanOnly` / `Internet`; routing priority by
  tier for the current state.
- **Tasks are pinned** to provider+model while active; new task after
  connectivity change or health failure.
- Data classification gates routing: `Public` any; `PersonalMemory` local or
  trusted gateway (consent); `SystemConfig` local by default; `Protected`
  local/tightly trusted; `Secret` never sent to any model.
- Provider failure → mark unhealthy (30s cooldown), task fails, retry on
  fallback with a **new task id** (fail-fast, ADR-0003).
- ADR-0006: one universal OpenAI-compatible `HttpBackend` for all remote
  providers; providers are config-driven (`~/.aios/config.toml`).

---

## 9. Human interaction (human-interaction.md)

- **Facade renders, does not authorize.** Approvals flow through a
  broker-owned channel; facade cannot mint/modify/relay approvals.
- UI must show **full scope** (every action/resource/operation/risk), not a
  summary. `plan_summary` is a title, not the binding content.
- Denial and timeout both → `Rejected`. No auto-approval ever.
- Approval is scoped (plan hash + actions + resources + operations) and
  expiring (risk 3: 10 min; risk 4: 5 min defaults).
- No `Modified` decision in v0.1 — user rejects, Planner re-plans.
- Automatic rollback needs no approval (safety mechanism); manual recovery is
  a risk-4 operation requiring approval.

---

## 10. Implementation status (roadmap)

- **M0** Design Foundation — ✅
- **M1** In-Process Simulation — ✅ (mock planner/verifier/specialists, demo in main.rs)
- **M2** Read-Only Linux Discovery — ✅ (sysfs/procfs/systemctl, reconcile)
- **M3** Local Model Runtime — ✅ (llama.cpp Qwen, gateway/router/pinner)
- **M4** Dual-Agent Orchestration — ✅ (config + HttpBackend, shell, read-only tools)
- **M5** Transactions and Staging — ✅ (checkpoints, approval, crash recovery)
- **M6** First Hardware Specialist (Wi-Fi) — ✅ (vertical slice, driver control)
- **M7** Additional Specialists — ✅ (Storage, Network, Drivers, Graphics, Memory, Power/thermal, Security/identity, Processes, Packages, Boot/Recovery, all wired read-only through the broker)
- **M8** System State Panel — ✅ terminal panel + resident docked UI (sidebar/canvas, layer-shell dock); dynamic generative surface pending (../superseded/m8-ui-repair-plan.md)

---

## 11. Codebase map (all src/ files read in full)

~20.5k lines Rust, 32 files in `src/`.

### Foundational
- `lib.rs` — module declarations.
- `protocol.rs` — all message types, `MessageEnvelope`, `DataClassification`,
  `ToolRequest`/`ToolResult`, `Approval`/`ApprovalScope`/`UserResponse`,
  `GuardianVerdict`, `PolicyVerdict`/`PolicyDecision`, `Message` enum.
- `capability.rs` — `PrincipalId` (user/agent/system), `ResourceId`,
  `Operation` (with `default_risk_level`), `RiskLevel`, `Clearance` newtype,
  `CapabilityToken` (no `revoked` field; revocation is broker-side),
  `ToolRegistry` (panics on duplicate tool), `DenyReason`, `GuardianClient`.
- `testutil.rs` — `spawn_json_server` + `openai_response` for tests.

### Enforcement plane
- `broker.rs` — `PolicyBroker` (pure, clock-injectable, `evaluate()` per §5.2)
  + async `Broker`/`LocalBroker` (tokio, `spawn_specialist` via mpsc,
  `set_executor`, per-resource locks). `LocalBroker.request_tool`: risk ≤1 →
  forward to specialist; else `run_staged` (walks action states then
  `stage_and_commit` or `reset_and_commit`).
- `guardian.rs` — 3 default invariants (FIRMWARE-001, DRIVER-001 tested-driver,
  BOOT-001 fallback image), `InvariantCheck`, `mark_driver_tested` etc.
- `action.rs` — `ActionState` + `can_transition`, `FileActionStore` with
  write-ahead pending journal + fsync-atomic writes + checkpoint files,
  `PendingTransition`, `ActionRecord` with `state_history`.
- `executor.rs` — `StagedExecutor` (create_action, transition w/ write-ahead,
  create/verify checkpoint, `stage_and_commit`, `reset_and_commit` from
  Approved, `do_rollback`, `recover()` crash recovery, `manual_recover`).
  `ResourceDriver` trait with `reset()` default unsupported. `validate_module()`
  for module-name safety (REQ-SAF-005).
- `audit.rs` — hash-chained `AuditLog` (previous_entry_hash/entry_hash),
  `load_last_hash` continues chain across sessions, `verify_chain`, fail-fast
  on read-only file.

### Discovery & graph
- `graph.rs` — `SystemGraph` (nodes/edges/adjacency), single-owner enforcement
  on `Owns`, `get_subgraph` BFS, `mark_stale`, `analyze_impact`,
  `get_owner`/`get_dependencies`/`get_dependents`/`get_affected`.
- `discovery.rs` — `SysfsDiscovery` scans /proc + /sys (kernel, cpu, memory,
  network, pci, usb, firmware, block, filesystems, sensors),
  `link_network_interfaces` post-pass, `reconcile` diffs → DeviceAdded/Removed,
  `ServiceDiscovery` via systemctl, `parse_systemctl_units`,
  `print_hardware_report`.
- `tools.rs` — read-only `ToolRegistry` (observe, diagnose, query, deps,
  impact, health), `resolve_one` (exact → label/attr → ambiguous),
  `tools_context`, `resource_index`, `model_tool_instructions` (advertises all
  specialist tools).

### Model layer
- `model.rs` — `ConnectivityState`, `combine()`, `has_default_route` v4/v6,
  `ConnectivityProbe`, `ProviderTier`, `tiers_for`, `ModelRegistry` (health,
  mark_unhealthy w/ 30s cooldown), `TaskPinner`, `ModelRouter` (route: filter
  tier/health/consent/capability, tier priority, reduced_confidence),
  `ModelGateway` (submit pins; submit_with_fallback marks unhealthy + new task
  + excludes failed), `ModelBackend` trait, `GenerationRequest`/`Response`.
- `local.rs` — `LocalLlama` via llama.cpp (chat template, tokenize, samplers);
  ignored real-model test w/ `AIOS_MODEL_PATH`.
- `hub.rs` — `ModelStore` verify/resolve by SHA-256, `HttpClient`/`UreqClient`,
  `CatalogModel` default qwen3-4b-q4-k-m.
- `http.rs` — `HttpBackend` OpenAI-compatible chat/completions; advertises
  tools when system message contains "Read-only machine tools"; parses content
  or tool_calls JSON; health via /models; 4xx non-recoverable, 5xx/429/
  transport recoverable.
- `config.rs` — `AiosConfig` TOML (model + [[provider]] + shell),
  `ProviderConfig` kinds local/openai-compatible with validation, tier parse,
  capabilities parse, `api_key_env`, `effective_api_key`.

### Agents
- `coordinator.rs` (largest) — `Coordinator::boot`/`boot_with_probe` wires
  providers → gateway, grants SystemConfig session consent, runs discovery,
  instantiates + wires wifi/storage/network/drivers/graphics/memory specialists
  (register tool, spawn handler, register principal, set resource state, grant
  session capability, static session_tokens). `BootError`. `chat_with_tools`
  (tool loop cap 4), `run_tool_as` (routes specialist tools via broker, static
  session token, nonce), `plan_and_review` (M4 rejects non-read-only steps),
  grant/revoke/consent, scan/graph_summary/state_panel,
  `status_text`/`providers_text`/`send_direct`. `configure_read_only_broker`
  registers observe/diagnose/query/deps/impact/health for `system:graph`.
  `issue_reset_approval` + `submit_approval` (broker-owned approval channel).
  Test wire helpers: `wire_storage`/`wire_network`/`wire_drivers`/`wire_graphics`/`wire_memory`.
- `planner.rs` — `Planner`, `submit` via gateway submit_with_fallback,
  `strip_think`, `plan` → JSON parse (`extract_json` balanced/string-aware),
  `parse_tool_calls` (native function shape + simple tool shape +
  `normalize_arguments`), `format_plan`.
- `verifier.rs` — `Verifier` review → JSON verdict parse, `loose_review`
  fallback.
- `facade.rs` — shell commands (help/status/providers/scan/graph/panel/state/
  consent/plan/model/route/tools/harness/audit/observe...), chat history,
  `harness_command`.
- `harness.rs` — deterministic read-only observation campaign (SplitMix64,
  plan from RESOURCE_POOL, `TestHarness` w/ broker/graph/vfs, quarantine,
  enforce stops early, `run_all`, `RunReport` render/json).
- `mocks.rs` — `MockWifiDriver`/`MockPlanner`/`MockVerificationAgent` +
  `wifi_specialist`/`storage_specialist` tool fns.

### Specialists (M7)
- `wifi.rs` — `WifiSpecialist` (discover single wireless device, instantiate
  with owns edge, tool_definitions incl. stage_driver(2)/request_reset(4),
  health via 2-hop subgraph walk, observe/diagnose).
- `wifi_driver.rs` — `DriverControl` trait; `MockDriverControl`;
  `LinuxDriverControl` (reads sysfs, **dry-runs mutations by default**,
  `AIOS_LIVE_DRIVER_CONTROL` opts into execute); `WifiDriverResourceDriver`
  (checkpoint DriverBackup, stage/health/commit/rollback/reset).
- `storage.rs` — `StorageSpecialist` umbrella (block devices + filesystems,
  owns edges, observe_storage/diagnose_fault, STORAGE-001/002).
- `network.rs` — `NetworkSpecialist` umbrella (wired interfaces + bluetooth,
  skips wireless owned by wifi, NETWORK-001/002).
- `drivers.rs` — `DriversSpecialist` peer (unclaimed PCI/USB + firmware +
  drivers, skips GPU/block/wireless, DRIVER-001).
- `graphics.rs` — `GraphicsSpecialist` umbrella (GPU/display/session, GFX-001).
- `memory.rs` — `MemorySpecialist` umbrella (memory nodes + ECC sensors, MEMORY-001).
- `power.rs` — `PowerSpecialist` umbrella (temperature and fan sensors, THERMAL-001).
- `security.rs` — `SecuritySpecialist` umbrella (enforcement plane, SEC-001).
- `processes.rs` — `ProcessesSpecialist` umbrella (process nodes, PROC-001).
- `packages.rs` — `PackagesSpecialist` umbrella (package domain, PKG-001).
- `boot.rs` — `BootRecoverySpecialist` umbrella (trust plane, BOOT-001).

### Entry
- `main.rs` — `run_demo` (M1 mock flow: observe/diagnose/smart/verifier/stage
  driver/health-fail/guardian block/approved kernel module/graph/discovery/
  reconcile) + `shell` subcommand → `facade::run_interactive`.

### Wi-Fi manifest (`modules/wifi/manifest.toml`)
- package `wifi.specialist`, clearance 4, tools observe(0)/diagnose(0)/
  stage_driver(2)/request_reset(4), capabilities `${device}:Observe/Diagnose/
  Stage/Reset`, invariants DRIVER-001/NETWORK-002.

---

## 12. Spec↔code alignment (verified against both docs/ and src/)

The implementation tracks the design closely. The following cross-checks were
verified line-by-line and are recorded so nothing is re-derived:

**In agreement**
- Broker `evaluate()` order matches capability-model §5.2 exactly (tool lookup
  → deadline → nonce → principal → capability+resource-state → token →
  clearance → Guardian → approval).
- `PolicyVerdict`/`GuardianVerdict` have **no `Escalate` variant** in v0.1
  (ADR-0003); Guardian escalation collapses to `Deny(GuardianEscalation)`.
- Session capability tokens are **static, issued at session start** via
  `configure_read_only_broker` + specialist wiring, and held for the session
  lifetime (resolves the M1 carry-forward — no on-demand `capability_tokens()`
  pull by the real planner).
- Broker keeps its **own `resource_states` registry**; only the owning
  specialist's `apply_resource_event` can change it (ADR-0005 P1-5,
  capability-model §10.2). The System Graph is advisory.
- Checkpoints: created before staging, verified, deleted after commit/rollback,
  **retained on `Failed`** for `manual_recover` (M5).
- Audit log: forward-chained SHA-256; `load_last_hash` continues the chain
  across sessions; malformed current-format log **panics** (fail-fast); a
  read-only audit file fails fast; `record()` returns an error so the broker
  can fail closed.
- `LinuxDriverControl` reads real sysfs (active module, module version,
  carrier/operstate) but **dry-runs mutations by default** (execute=false);
  `AIOS_LIVE_DRIVER_CONTROL` opts into real execution. Safety boundary kept.
- Risk-4 reset path (`executor::reset_and_commit`) only proceeds from
  `Approved`, creates a checkpoint first, then resets + health-checks + commits
  (or rolls back). Requires a broker-owned approval.
- M4 `plan_and_review` rejects any non-read-only plan step before the verifier
  runs (mutations belong to M5).
- Module-name safety in `executor::validate_module` (REQ-SAF-005): only
  `[A-Za-z0-9._-]`, ≤256 chars.

**Notable discrepancy (hold for a decision)**
- **`request_reset` risk level — RESOLVED (3d6daae).** `src/wifi.rs`
  `tool_definitions()` now declares `request_reset` = `RiskLevel::Recovery`
  (4), matching the manifest and docs. Test assertion and coordinator
  `wire_specialist` helper aligned to Recovery. Rationale: the broker only
  permits recovery-level ops on a quarantined resource, so a Critical reset
  would be wrongly denied with `ResourceQuarantined`. All 301 tests pass.

---

## 13. Notable implementation details (supplementary)

- Broker runtime methods use `expect()` on mutex locks — intentional fail-fast
  under ADR-0003, no silent fallbacks.
- Duplicate `ok`/`not_found` helpers are per-module (storage/network/drivers/
  graphics/memory), not shared.
- `chat_with_tools` tool loop is capped at 4 turns; exceeding it returns an
  error and records a `tool_loop` audit entry (no silent infinite loop).
- The generic read-only tools (observe/diagnose/query/deps/impact/health)
  route to the `system:graph` resource; specialist tools (wifi/storage/
  network/drivers/graphics/memory) route to their domain/device resource. The
  `run_tool_as` mapping is explicit and exhaustive.
- Discovery `link_network_interfaces` is a post-pass that links each
  `device:net-*` interface to its underlying PCI/USB device via
  `sys/class/net/<if>/device` (M6 acceptance criterion #6).
- `reconcile` emits `DeviceAdded`/`DeviceRemoved` and cleans dangling edges;
  real-time udev push is deferred (polling detects removals next cycle).
- M2 note: discovered nodes carry `expires_at` TTL and `mark_stale` flags them
  `STALE`; `Unhealthy` is never silently shown as healthy.

---

## 14. Build / run

```bash
cargo build
cargo test          # full suite (hundreds of tests)
cargo run          # M1 demo flow
cargo run -- shell # interactive aios shell
```

- Config: `~/.aios/config.toml` (or `AIOS_CONFIG`). Providers as
  `[[provider]]` entries (kind local / openai-compatible, tier, endpoint,
  model, api_key or api_key_env).
- Local model: put a `.gguf` in `~/.aios/models/` or set `[model] path`.
- Real-model test: `AIOS_MODEL_PATH=<path>` (ignored by default).
- Live driver control: `AIOS_LIVE_DRIVER_CONTROL` (default dry-run).
