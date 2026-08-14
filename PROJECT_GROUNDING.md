# Project Grounding

Start here before making changes or answering questions about the repository.

## Latest Snapshot

Read the latest dated grounding snapshot:

- [`project_grounding_2026-08-14_10-22-59.md`](project_grounding_2026-08-14_10-22-59.md)

That snapshot records the current architecture, source layout, implementation
status, known gaps, test conditions, UI paths, and the recommended restart
order. It is a context index, not a replacement for the focused contracts in
`docs/`.

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
