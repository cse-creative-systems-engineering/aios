# Project Grounding

Start here before making changes or answering questions about the repository.

## Latest Snapshot

Read the latest dated grounding snapshot:

- [`project_grounding_2026-08-22_00-15-00.md`](docs/grounding/project_grounding_2026-08-22_00-15-00.md) — multi-surface canvas shipped, provider-health fix for empty answers, snap env sanitizers
- [`project_grounding_2026-08-21_20-35-47.md`](docs/grounding/project_grounding_2026-08-21_20-35-47.md) — groundless surfaces validated live on a real desktop, provider teardown fix, fidelity gate unit tolerance
- [`project_grounding_2026-08-21_17-21-55.md`](docs/grounding/project_grounding_2026-08-21_17-21-55.md) — coordinator modularization, surface harness, Tauri desktop shell, and the graphify knowledge graph
- [`project_grounding_2026-08-18_00-33-45.md`](docs/grounding/project_grounding_2026-08-18_00-33-45.md) — restored bespoke graph renderer with real node health and backend activity wiring
- [`project_grounding_2026-08-17_17-19-56.md`](docs/grounding/project_grounding_2026-08-17_17-19-56.md) — surface-lifecycle foundation context

That snapshot records the current architecture, source layout, implementation
status, known gaps, test conditions, UI paths, and the recommended restart
order. It is a context index, not a replacement for the focused contracts in
`docs/`.

## Sidebar Layout Design (2026-08-17)

The sidebar uses a three-zone layout plus a slide-out panel:

- **Icon rail** (56px, always visible): permanent navigation skeleton with
  grouped section icons (Chat, Providers, Roles, Surfaces, Audit, Settings)
  and feedback indicator dots (backend readiness, connectivity, specialist
  activity, pending alerts).

- **System feedback block** (top half of the sidebar, ≈50%): substantial area
   showing Aios's full system state, plus controls. Every system, sub-system,
   specialist, and model has reserved real estate. The user sees Aios working
   while waiting for a response. The chat interface owns the bottom half.

- **Chat interface** (always visible): messages, composer, evidence. The
  primary control interface. Sits below the system feedback block.

- **Slide-out panel** (right edge, separate Tauri window): appears at x=420,
  same z-level as sidebar (always on top). Shows detailed admin views for
  Providers, Roles, Surfaces, Audit, Settings. Overlays the canvas. Nothing
  shifts. Click icon again or click outside to close.

Design principles:
- Screen space is precious. The rail is fixed at 56px. The system feedback
  block takes what it needs. The chat fills remaining space. The slide-out
  panel appears on demand.
- The UI must convey that Aios touches every part of the system down to
  kernel and hardware level. It is not a generic chat interface.
- Neither comprehensiveness nor complexity restricts design decisions. The
  right design is the one that serves Aios, regardless of complexity.
- Chat is always visible. It is never replaced by another view.

Full layout contract: `docs/ui.md`

## Re-grounding Order

1. Read the latest `project_grounding_*.md` file.
2. Read `docs/doc-progress.md`.
3. Read the focused document for the area being changed.
4. Inspect the relevant source files and tests.
5. Check `git status --short` before editing.
6. Run verification with the test environment described in the snapshot.

## Snapshot Maintenance

When the architecture, implementation status, active UI path, test baseline, or
known risks change materially, create a new timestamped
`project_grounding_YYYY-MM-DD_HH-MM-SS.md` file and update the Latest Snapshot
link above. Keep older snapshots for history unless they contain sensitive or
misleading information.

Humans and coding agents should not begin by rereading the entire repository.
Use the latest snapshot to choose the smallest relevant reading set, then check
the source before relying on any historical statement.
