# Aios Testing Strategy

**Status:** Draft — frozen for M1  
**Depends on:** architecture.md, requirements.md, security-model.md, capability-model.md, message-protocol.md, action-state-machine.md, implementation-roadmap.md, decisions/0003-fail-fast-no-silent-fallbacks.md

## Purpose

Define how correctness and safety are verified across Aios. Covers unit tests,
integration tests, simulation, fault injection, hardware-in-the-loop, and
Aios-specific evaluations.

### Design principles

1. **Tests are written alongside code, not after.** No code lands without
   tests. Per ADR-0003, fail-fast means tests must surface failures
   immediately.
2. **Safety properties are tested explicitly.** Every safety requirement
   (REQ-SAF-*) has a corresponding test that verifies it and a negative test
   that verifies it cannot be bypassed.
3. **The broker is the most tested component.** It is the TCB. It has the
   highest test coverage requirement — every decision path, every fail-closed
   branch, every edge case.
4. **Fault injection is mandatory.** Normal-path tests are necessary but not
   sufficient. The system must be tested under failure conditions: crashes,
   timeouts, missing components, corrupted state.
5. **AI output is never trusted as proof of safety.** Evaluations test agent
   behavior, but deterministic tests verify safety properties. A model
   passing an evaluation does not replace a broker test.

---

## 1. Test Layers

### 1.1 Layer overview

```text
Layer 1: Unit Tests
  └── Individual components in isolation
      (broker, guardian, graph, protocol types, state machine)

Layer 2: Integration Tests
  └── Multi-component flows
      (Planner → broker → specialist, staging → health → commit)

Layer 3: Simulation Tests
  └── Full action lifecycle in-process
      (discovery → instantiation → plan → execute → rollback)

Layer 4: Fault Injection Tests
  └── Deliberate failures
      (crash, timeout, bus failure, model failure, corrupted state)

Layer 5: Hardware-in-the-Loop Tests
  └── Real device interaction
      (Wi-Fi diagnosis, driver staging, sensor reading)

Layer 6: Aios Evaluations
  └── Agent behavior quality
      (tool selection, refusal, diagnosis accuracy, recovery planning)
```

### 1.2 Unit tests

Test individual components in isolation with mocked dependencies.

| Component | What to test | Coverage target |
|---|---|---|
| `PolicyBroker` | Every decision path, every `DenyReason`, fail-closed branches, capability validation, clearance validation, approval scope checking | 100% of decision paths |
| `Guardian` | Every invariant check, allow/escalate/block decisions, block type classification | 100% of invariant checks |
| `ActionStateMachine` | Every state transition, terminal states, invalid transitions rejected | 100% of transitions |
| `SystemGraph` | Node/edge CRUD, query API, subgraph extraction, staleness detection, conflict resolution | 90% |
| `Protocol` | Message construction, validation, envelope fields, unknown message rejection | 90% |
| `ModelRouter` | Provider selection, connectivity states, data classification filtering, fallback | 90% |
| `PackageRegistry` | Matching, dependency resolution, version management, circular dependency detection | 90% |

### 1.3 Integration tests

Test multi-component flows with real (non-mocked) components where possible.

| Flow | Components | What to verify |
|---|---|---|
| Tool request flow | Planner → broker → specialist → broker → Planner | Message types correct, capabilities validated, result returned |
| Staged execution | broker → executor → specialist → health check → commit | Checkpoint created, health verified, commit or rollback |
| Guardian review | broker → Guardian → broker | Guardian decision enforced by broker, block cannot be bypassed |
| Approval flow | Planner → broker → user → broker → executor | Approval scope checked, hash verified, expiry enforced |
| Crash recovery | executor → crash → restart → recovery | Interrupted actions detected, rolled back or committed, no limbo |
| Agent instantiation | discovery → registry → broker → graph | Package matched, capabilities granted, graph updated |
| Model routing | task → router → provider → response | Correct provider selected, data classification enforced, task pinned |

### 1.4 Simulation tests

Full lifecycle simulation in-process with mock hardware and mock models.

| Simulation | What it exercises |
|---|---|
| Wi-Fi diagnosis | Discovery → specialist instantiation → user query → diagnosis → response |
| Driver staging | Plan → Guardian review → checkpoint → stage → health check → commit |
| Driver rollback | Plan → stage → health check fails → rollback → verify restoration |
| Guardian block | Plan → Guardian blocks → user notified → action rejected |
| Offline operation | Connectivity → Offline → local model → plan → execute |
| Provider failure | Task in progress → provider fails → task fails → retry on fallback |
| Unknown device | Discovery → no matching package → quarantine → user notified |

### 1.5 Fault injection tests

Deliberately break things and verify the system fails safely.

| Fault | What to inject | Expected behavior |
|---|---|---|
| Process crash during staging | Kill process after checkpoint, before staging | On restart: action detected, rolled back |
| Process crash during commit | Kill process after health check, before commit | On restart: action detected, commit attempted or rolled back |
| Power loss during staging | Simulate by not persisting final state | On restart: action in `Staged` state, health check run, commit or rollback |
| Guardian unavailable | Guardian process/channel fails | Broker denies level 2+ requests (fail-closed) |
| Model provider timeout | Provider does not respond within deadline | Task fails, no silent degradation |
| Model provider returns garbage | Provider returns malformed response | Error handled, task fails, no crash |
| Specialist crash | Specialist process/channel fails | Broker detects, action fails fast |
| Audit log write failure | Disk full or permission denied | Broker denies all actions (no unaudited operations) |
| Graph corruption | Graph state inconsistent | Fail-closed: affected resources denied |
| Capability token expired | Token past expiration | Broker denies request |
| Capability token revoked | Token revoked but agent still tries | Broker denies request |
| Package signature invalid | Modified package loaded | Package rejected, fail-fast |
| Circular package dependency | Two packages depend on each other | Detected at registration, rejected |

### 1.6 Hardware-in-the-loop tests

Real device interaction for specialists that have been implemented.

| Test | Hardware | What to verify |
|---|---|---|
| Wi-Fi discovery | Real Wi-Fi adapter | Device appears in graph with correct attributes |
| Wi-Fi diagnosis | Real Wi-Fi adapter | Specialist can observe and diagnose |
| Wi-Fi driver staging | Real Wi-Fi adapter + test driver | Driver staged, health checked, rolled back if unhealthy |
| NVMe health | Real NVMe drive | Health metrics read correctly |
| Sensor reading | Real temperature sensor | Temperature values read and graph updated |

Hardware-in-the-loop tests require specific hardware and are run manually or
in a dedicated CI environment, not in the standard CI pipeline.

### 1.7 Aios evaluations

Agent behavior quality tests using model providers.

| Evaluation | What it measures | How |
|---|---|---|
| Tool selection accuracy | Does the Planner choose the right tool for the task? | Present scenarios, verify correct tool is requested |
| Refusal correctness | Does the agent refuse unsafe requests? | Present unsafe requests, verify refusal |
| Diagnosis accuracy | Does the specialist produce correct diagnoses? | Present symptoms, verify diagnosis matches known issue |
| Recovery planning | Does the Planner produce valid recovery plans? | Present failure scenario, verify plan is safe and correct |
| Prompt injection resistance | Does external data elevate authority? | Inject malicious content in tool results, verify no authority change |
| Safe command generation | Does the agent generate only safe operations? | Present tasks, verify all operations are within capabilities |

Evaluations use a set of scenarios with known correct outcomes. They are run
against each model provider to track quality across providers.

---

## 2. Safety-Specific Tests

Every safety requirement has explicit tests:

### 2.1 Capability escalation (REQ-SAF-001)

```rust
#[test]
fn agent_cannot_execute_without_capability() {
    let mut broker = PolicyBroker::new();
    let agent = create_agent_with_no_capabilities();
    let request = ToolRequest {
        operation: Operation::Stage,
        resource: ResourceId::from("device:wifi0"),
        tool_id: ToolId::from("wifi.stage_driver"),
        ..
    };
    let decision = broker.evaluate(&request);
    assert!(matches!(decision, PolicyVerdict::Deny(DenyReason::MissingCapability)));
}

#[test]
fn agent_cannot_access_resource_without_ownership() {
    // Agent has capability for wifi0 but tries wifi1
    let mut broker = PolicyBroker::new();
    let agent = create_agent_with_capability("device:wifi0", Operation::Stage);
    let request = ToolRequest {
        resource: ResourceId::from("device:wifi1"),  // Different device
        operation: Operation::Stage,
        tool_id: ToolId::from("wifi.stage_driver"),
        ..
    };
    let decision = broker.evaluate(&request);
    assert!(matches!(decision, PolicyVerdict::Deny(DenyReason::MissingCapability)));
}
```

### 2.2 Clearance enforcement (ADR-0004)

```rust
#[test]
fn agent_with_clearance_1_cannot_use_level_2_tools() {
    let mut broker = PolicyBroker::new();
    let agent = create_agent_with_clearance(Clearance(RiskLevel::Routine));  // Level 1
    let request = ToolRequest {
        operation: Operation::Stage,
        tool_id: ToolId::from("wifi.stage_driver"),
        // Broker resolves risk_level 2 from registry by tool_id
        ..default_request()
    };
    let decision = broker.evaluate(&request);
    assert!(matches!(decision, PolicyVerdict::Deny(DenyReason::InsufficientClearance)));
}
```

### 2.3 Guardian block enforcement (REQ-SAF-003)

```rust
#[test]
fn guardian_block_cannot_be_bypassed() {
    let mut broker = PolicyBroker::new();
    broker.set_guardian(Guardian::blocking(InvariantId::from("BOOT-001")));
    let agent = create_agent_with_all_capabilities();
    let request = ToolRequest {
        operation: Operation::BootConfig,
        tool_id: ToolId::from("boot.boot_config"),
        // Broker resolves risk_level 3 from registry by tool_id
        ..default_request()
    };
    let decision = broker.evaluate(&request);
    assert!(matches!(decision, PolicyVerdict::Deny(DenyReason::GuardianBlocked(_))));
}
```

### 2.4 Fail-closed behavior (REQ-SAF-002, ADR-0003)

```rust
#[test]
fn broker_fails_closed_on_unknown_principal() { ... }

#[test]
fn broker_fails_closed_on_missing_capability() { ... }

#[test]
fn broker_fails_closed_on_ambiguous_capability() { ... }

#[test]
fn broker_fails_closed_on_guardian_unavailable() { ... }

#[test]
fn broker_fails_closed_on_audit_log_failure() { ... }
```

### 2.5 Rollback correctness (REQ-REL-001)

```rust
#[test]
fn health_check_failure_triggers_rollback() { ... }

#[test]
fn rollback_restores_previous_state() { ... }

#[test]
fn rollback_failure_enters_failed_state() { ... }
```

### 2.6 Secret leakage prevention (REQ-SAF-006)

```rust
#[test]
fn secrets_never_appear_in_audit_log() { ... }

#[test]
fn secrets_never_appear_in_model_prompts() { ... }

#[test]
fn secrets_never_appear_in_tool_results() { ... }

#[test]
fn redaction_replaces_secrets_in_logs() { ... }
```

### 2.7 Prompt injection resistance (REQ-SAF-005)

```rust
#[test]
fn injected_authority_in_tool_result_does_not_grant_capability() { ... }

#[test]
fn injected_authority_in_file_content_does_not_grant_capability() { ... }

#[test]
fn context_does_not_grant_authority() { ... }
```

---

## 3. Test Infrastructure

### 3.1 Test harness

```rust
pub struct TestHarness {
    pub broker: PolicyBroker,
    pub guardian: Guardian,
    pub executor: ActionExecutor,
    pub graph: SystemGraph,
    pub registry: PackageRegistry,
    pub model_router: ModelRouter,
    pub audit_log: AuditLog,
}

impl TestHarness {
    pub fn new() -> Self { ... }
    pub fn with_mock_specialist(mut self, specialist: MockSpecialist) -> Self { ... }
    pub fn with_mock_model(mut self, model: MockModel) -> Self { ... }
    pub fn with_guardian_block(mut self, invariant: InvariantId) -> Self { ... }
    pub fn submit_tool_request(&mut self, request: ToolRequest) -> PolicyVerdict { ... }
    pub fn crash_and_recover(&mut self) -> Vec<ActionId> { ... }
}
```

### 3.2 Mock components

| Mock | Purpose |
|---|---|
| `MockSpecialist` | Returns canned tool results, can be programmed to fail |
| `MockModel` | Returns canned model responses, can be programmed to timeout |
| `MockGuardian` | Returns allow/block/escalate on demand |
| `MockDiscovery` | Returns canned device lists for graph population |
| `MockAuditLog` | Records entries, can be programmed to fail writes |

### 3.3 Property-based testing

Use `proptest` for the capability model and protocol:

```rust
proptest! {
    #[test]
    fn broker_never_allows_without_capability(
        principal in arbitrary_principal(),
        resource in arbitrary_resource(),
        operation in arbitrary_operation(),
    ) {
        let broker = PolicyBroker::new();  // No capabilities granted
        let request = ToolRequest { principal, resource, operation, .. };
        let decision = broker.evaluate(&request);
        prop_assert!(matches!(decision, PolicyVerdict::Deny(_)));
    }

    #[test]
    fn protocol_rejects_unknown_message_types(
        message_type in arbitrary_unknown_type(),
    ) {
        let result = parse_message(message_type, arbitrary_payload());
        prop_assert!(result.is_err());
    }
}
```

### 3.4 CI pipeline

```yaml
# .github/workflows/ci.yml (or equivalent)
jobs:
  test:
    - cargo fmt --check
    - cargo clippy -- -D warnings
    - cargo test --lib          # Unit tests
    - cargo test --integration   # Integration tests
    - cargo test --simulation    # Simulation tests
    - cargo test --fault         # Fault injection tests
    - cargo test --safety        # Safety-specific tests
  coverage:
    - cargo tarpaulin --out html  # Coverage report
    # Broker coverage must be 100% of decision paths
```

---

## 4. Acceptance Criteria per Milestone

Each milestone in the implementation roadmap defines its own acceptance tests.
No milestone is complete until all its tests pass.

| Milestone | Key test categories | Coverage target |
|---|---|---|
| M1: In-Process Simulation | Unit, integration, simulation, safety | Broker: 100% decision paths, all others: 90% |
| M2: Linux Discovery | Unit (mock sysfs), integration, fault | Discovery: 90% |
| M3: Local Model | Unit, integration, fault (provider failure) | Router: 90% |
| M4: Dual-Agent | Integration, simulation, evaluations | E2E flows: 90% |
| M5: Transactions | Unit, integration, fault injection, safety | State machine: 100% transitions |
| M6: Wi-Fi Specialist | Integration, simulation, hardware-in-the-loop | Specialist: 90% |
| M7: Each specialist | Integration, hardware-in-the-loop | Specialist: 90% |
| M8: System State Panel | Unit, integration | Aggregator: 90% |

---

## 5. Test File Organization

```text
tests/
├── unit/
│   ├── broker_tests.rs
│   ├── guardian_tests.rs
│   ├── state_machine_tests.rs
│   ├── graph_tests.rs
│   ├── protocol_tests.rs
│   ├── router_tests.rs
│   └── registry_tests.rs
├── integration/
│   ├── tool_request_flow.rs
│   ├── staged_execution.rs
│   ├── guardian_review.rs
│   ├── approval_flow.rs
│   ├── crash_recovery.rs
│   ├── agent_instantiation.rs
│   └── model_routing.rs
├── simulation/
│   ├── wifi_diagnosis.rs
│   ├── driver_staging.rs
│   ├── driver_rollback.rs
│   ├── guardian_block.rs
│   ├── offline_operation.rs
│   ├── provider_failure.rs
│   └── unknown_device.rs
├── fault/
│   ├── crash_during_staging.rs
│   ├── crash_during_commit.rs
│   ├── guardian_unavailable.rs
│   ├── model_timeout.rs
│   ├── specialist_crash.rs
│   ├── audit_log_failure.rs
│   ├── graph_corruption.rs
│   └── package_signature_invalid.rs
├── safety/
│   ├── capability_escalation.rs
│   ├── clearance_enforcement.rs
│   ├── guardian_block.rs
│   ├── fail_closed.rs
│   ├── rollback_correctness.rs
│   ├── secret_leakage.rs
│   └── prompt_injection.rs
├── evaluations/
│   ├── tool_selection.rs
│   ├── refusal_correctness.rs
│   ├── diagnosis_accuracy.rs
│   ├── recovery_planning.rs
│   └── prompt_injection_resistance.rs
└── hardware/
    ├── wifi_discovery.rs
    ├── wifi_diagnosis.rs
    ├── wifi_driver_staging.rs
    └── nvme_health.rs
```

---

## 6. Open questions

1. **Coverage enforcement.** Should CI fail if broker coverage drops below
   100%? (Recommendation: yes — the broker is the TCB.)
2. **Evaluation automation.** Should evaluations run in CI or manually?
   (Recommendation: manually for v0.1 — they require model providers. CI
   for v0.2+ with mock models.)
3. **Hardware test CI.** Should hardware-in-the-loop tests run in CI?
   (Recommendation: no for v0.1 — they require physical hardware. Dedicated
   test machine for v0.2+.)
4. **Fuzzing.** Should the broker and protocol be fuzzed?
   (Recommendation: yes for v0.2 — use `cargo-fuzz` to find edge cases in
   message parsing and capability validation.)
5. **Benchmark tests.** Should there be performance benchmarks?
   (Recommendation: yes for the broker — capability checks must be
   sub-millisecond. Benchmark in CI for regressions.)

---

## References

- `docs/architecture.md` — section 15 (gaps: verification and evaluation)
- `docs/security-model.md` — all threat scenarios need corresponding tests
- `docs/capability-model.md` — section 5 (broker decision algorithm)
- `docs/action-state-machine.md` — section 6 (crash recovery)
- `docs/requirements.md` — all REQ-SAF-* requirements
- `docs/implementation-roadmap.md` — per-milestone acceptance criteria
- `docs/decisions/0003-fail-fast-no-silent-fallbacks.md` — tests must
  surface failures immediately
