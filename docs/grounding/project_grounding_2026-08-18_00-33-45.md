# Grounding Snapshot: Bespoke Graph With Real Backend Wiring

## Current State

The restored bespoke sidebar graph is the active visual renderer. It uses fixed
positions, compact labels, health classes, edge animation, and hover details.
The visual renderer was deliberately kept instead of replacing it with a raw
mermaid auto-layout.

The backend now supplies real graph data and activity:

- Control-plane nodes include `facade`, `coordinator`, `planner`, `verifier`,
  `broker`, `gateway`, `composer`, `evidence`, `validator`, `staged`, `audit`,
  `tools`, and `graph`.
- Stable `Specialist` nodes exist for all eleven domains, plus `guardian`.
  Missing runtime resources remain `Unknown`; they are not silently green.
- Declared edges connect the control plane, specialists, Guardian, graph,
  composer, evidence, and validator after boot instantiation.
- `graph_activity` is emitted from planner, verifier, tool gathering, compose,
  and idle seams. The frontend maps those events into the existing bespoke
  renderer. The old frontend phase timers are removed.

## Relevant Paths

- `src/graph.rs`: real node types, including `SystemGraph`.
- `src/coordinator.rs`: stable node creation, declared topology edges, and
  progress reports.
- `src/progress.rs`: `GraphPhase`, `GraphActivity`, and reporter trait.
- `src-tauri/src/main.rs`: real snapshot health, event forwarding, and bespoke
  snapshot projection.
- `frontend/src/sidebar.ts`: fixed-position renderer and activity highlighting.
- `frontend/src/main.ts`: graph activity listener and prompt lifecycle.
- `docs/ui.md`: full visual contract and bespoke renderer decision.

## Verification

- `npm run build` passes.
- `cargo build --manifest-path src-tauri/Cargo.toml` passes.
- `cargo test --lib` passes: 438 tests.

## Open Work

- Verify the runtime visually after restarting the rebuilt binary.
- Exercise a risk level that requires Guardian review and confirm the Broker
  and Guardian nodes pulse together.
- Keep the bespoke renderer; do not replace it with a generic graph layout.
