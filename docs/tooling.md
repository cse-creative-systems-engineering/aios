# Aios Developer & Agent Tooling

**Status:** Living document.
**Purpose:** Document the tools that both humans and coding agents use to
navigate, understand, and modify the Aios codebase. If a tool is expected
first-line for a class of question, it belongs here — not in an editor-specific
config file.

## Principle

If a tool is important enough that agents should reach for it before `grep`,
it is important enough to be documented here. Editor-specific instructions
(`.github/copilot-instructions.md`, `.cursor/rules`, `AGENTS.md`) may
*reference* this document, but this document is the source of truth.

## Graphify (`graphify`)

**What it is.** A code-relationship knowledge-graph builder that ingests the
repository and produces a queryable graph of files, symbols, imports, calls,
and semantic relationships. Its output lives under `graphify-out/` with dated
snapshots (`graphify-out/YYYY-MM-DD/`) and a top-level current view
(`graphify-out/graph.json`, `graphify-out/GRAPH_REPORT.md`,
`graphify-out/wiki/index.md`).

**When it is the first-line tool.**

| Question shape | First action |
|---|---|
| "How is X connected to Y?" | `graphify path "X" "Y"` |
| "Where is the code that does Z?" / "How do I add/modify a Z?" | `graphify query "..."` |
| "Explain concept C in this codebase." | `graphify explain "C"` |
| Broad architecture review, unfamiliar territory | `graphify-out/wiki/index.md` for navigation, then `graphify-out/GRAPH_REPORT.md` for the report |

Only fall back to raw `grep` / `find_path` / reading source when:

1. The graph is missing or stale.
2. You are actively modifying or debugging specific code that the graph does
   not resolve to sufficient detail.
3. The question is textual (find a specific string), not structural.

**Refreshing the graph.**

- In Copilot Chat: `/graphify` builds or updates the graph.
- On the command line: `graphify update .` from the repo root.
- The dated bucket (`graphify-out/YYYY-MM-DD/`) is written per calendar day;
  the un-dated top-level files are the current view.

**Trust boundary.** `graphify-out/` is generated. Treat it as advisory —
useful for orientation, not authoritative for capability or ownership
decisions. The Broker and the type system are the sources of truth for those.

## Cargo (Rust build and test)

- `cargo build` — library build.
- `cargo build --manifest-path src-tauri/Cargo.toml` — Tauri desktop build.
- `cargo test --lib` — the primary regression gate. Current baseline
  documented in [`test-ledger.md`](test-ledger.md).
- `cargo test --test harness_drive -- --nocapture` — end-to-end coordinator
  harness.
- `cargo fix --lib -p aios --tests` — applies non-behavioral lints. Never run
  this on a dirty working tree without confirming the diff.

## Frontend

- `npm run build --prefix frontend` — production build gate. Must succeed
  before any PR merges.
- `npm run dev --prefix frontend` — Vite dev server. Ignored by the desktop
  shell; use `bash scripts/dev.sh` for the full Tauri development loop.

## Desktop

- `bash scripts/dev.sh` — full dev loop; sanitizes `GTK_PATH` from snap-editor
  environments (see grounding snapshot 2026-08-22_00-15-00).
- `bash scripts/ui-e2e.sh` — Selenium/WebDriver end-to-end that drives two
  coexisting surfaces and the close path.

## Regression Gate (all together)

Any merge to `main` runs at minimum:

```bash
cargo test --lib
cargo build --manifest-path src-tauri/Cargo.toml
npm run build --prefix frontend
cargo test --test harness_drive -- --nocapture
```

Milestone-specific gates (session restore, sandbox tiers, etc.) are declared
in each milestone doc and additive to the above.

## Editor-Specific Rules

These reference this document; they do not replace it:

- `.github/copilot-instructions.md` — Copilot Chat rules; points here for
  graph tooling and gates.
- `AGENTS.md` (repo root, if present) — cross-editor agent rules; points
  here for the same.

Adding a new editor rule file? Reference this document rather than duplicating
tool descriptions. Duplication drifts.

## References

- [`test-ledger.md`](test-ledger.md) — authoritative test-count ledger.
- [`doc-progress.md`](doc-progress.md) — doc status.
- [`decisions/0003-fail-fast-no-silent-fallbacks.md`](decisions/0003-fail-fast-no-silent-fallbacks.md)
  — no silent tool failures; if a tool the agent expected is missing,
  install it (or ask), do not silently degrade.
