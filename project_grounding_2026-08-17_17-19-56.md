# Aios Project Grounding

**Snapshot:** 2026-08-17 17:19:56 EDT
**Purpose:** Restart context for the current working foundation.

## Current Git State

- Current branch: `main`
- Foundation checkpoint: `8afcded`
- Current documentation checkpoint: `04fc4c6`
- `main` and `feature/dynamic-generative-surfaces` are pushed to GitHub.
- The former Slint branch and handoff were removed.

## Working Desktop Foundation

The active path is Tauri v2 plus Vite/TypeScript:

```text
frontend/src/main.ts
  -> submit_prompt
  -> src-tauri/src/main.rs
  -> Facade / Coordinator / Broker / specialists
  -> grounded evidence
  -> groundless surface generation
  -> value-fidelity check
  -> canvas_response
  -> transparent movable canvas host
```

The current checkpoint has been manually verified to:

- generate CPU and RAM widgets separately;
- display the complete generated widget without the old panel clipping;
- move the widget around the usable desktop area;
- pass clicks through outside the widget;
- keep the sidebar docked below the desktop top bar.

The current implementation intentionally supports one generated surface at a
time. Read `docs/milestones/0002-multi-surface-lifecycle-plan.md` before
changing that boundary.

## Next Implementation Order

1. Reproduce and repair combined CPU plus RAM evidence collection.
2. Add backend-owned surface IDs, revisions, evidence snapshots, and lifecycle
   state.
3. Render multiple surfaces in one transparent overlay with independent input
   regions and positions.
4. Add explicit surface create versus update intent handling.
5. Support one surface composed from multiple specialist domains.
6. Redesign the basic sidebar after the surface lifecycle is stable.

## Non-Negotiable Development Rules

- Fail fast. Do not hide broken delivery behind a fallback renderer.
- Return or log errors at native window, IPC, model, and validation boundaries.
- Never invent values or silently turn stale/unknown evidence into healthy data.
- Keep model output outside the authority path.
- Keep mutations behind the broker, Guardian, approvals, staging, and audit.

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

The ignored WebDriver suite is available at `tests/ui_e2e.rs` and is run by
`scripts/ui-e2e.sh` when the required desktop driver is installed.
