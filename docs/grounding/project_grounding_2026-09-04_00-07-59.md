# Grounding Snapshot: Propose live SystemStateStore and durable generative A2UI runtime

## Current State

`feature/live-system-state` is a documentation-first branch from `main`
(`876535c`). ADR-0012 proposes a new observational architecture: deterministic
Rust collectors publish into `SystemStateStore`; the SystemGraph remains the
topology/provenance map; the Policy Broker remains the authority boundary; and
the A2UI model remains free to generate and revise arbitrary presentation.

No implementation code has changed. `main` remains runnable in its separate
worktree.

## Relevant Paths

- `docs/decisions/0012-live-system-state-and-a2ui-runtime.md` — proposed
  architecture and non-negotiable authority/presentation boundaries.
- `docs/milestones/0006-live-system-state-a2ui-runtime.md` — staged migration,
  A2UI-runtime work streams, non-goals, and acceptance gates.
- `docs/ui.md` — corrects the active surface lifecycle description and records
  the proposed hardening path without imposing a widget vocabulary.
- `docs/system-graph.md` — records the proposed graph/store separation.
- `docs/architecture.md`, `docs/implementation-roadmap.md`, and
  `docs/doc-progress.md` — link the proposal into the architecture and roadmap.

## Open Work

1. Review and accept, revise, or reject ADR-0012 before implementation.
2. Define concrete Rust state schemas and collector contracts, beginning with
   process, GPU, network, and thermal correlation.
3. Specify the wire contract for context projections, deltas, explicit refresh,
   and A2UI bindings in the message protocol.
4. Design the proactive-insight policy separately before Aios may interrupt the
   user with observations.
