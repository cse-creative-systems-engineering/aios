# Repository Working Notes

Before exploring or editing this repository, read [`PROJECT_GROUNDING.md`](PROJECT_GROUNDING.md)
and the latest dated grounding snapshot linked there.

The snapshot is the fast context index for both people and coding agents. It
lists the current architecture, implementation paths, test conditions, known
gaps, and the smallest useful next reading set. After that, inspect the source
and focused documents for the task instead of relying on the snapshot alone.

Check `git status --short` before editing and preserve unrelated worktree
changes.

## graphify

This repo carries a knowledge graph built with `graphify` (a code-knowledge-graph tool) in `graphify-out/`. It has god nodes, community structure, and cross-file relationships, and is kept fresh automatically. Prefer it over grepping raw files.

### What's there
- `graphify-out/graph.json` — the graph (nodes, links, communities). For this repo ~2770 nodes / 7600 edges / 118 communities.
- `graphify-out/GRAPH_REPORT.md` — broad architecture overview with named communities. Read this to orient, not for specifics.
- `graphify-out/graph.html` — interactive visualization (gitignored; regenerate via `graphify extract` if wanted).
- `graphify-out/.graphify_analysis.json` and `.graphify_labels.json` — internal state, ignore them.

### How to query it
Two ways; use whichever the agent has wired:

1. **MCP server (preferred when available).** Named `graphify-mcp-server` (global in VS Code / VS Code Insiders) and `graphify` in opencode. Tools:
   - `query_graph` — BFS/DFS scoped subgraph for a question.
   - `get_node` / `get_neighbors` — a node's details and its direct neighbors.
   - `get_community` — all nodes in a community (cluster).
   - `god_nodes` — the most-connected core abstractions.
   - `shortest_path` — path between two concepts.
   - `graph_stats` — node/edge/community counts.
   - `list_prs` / `get_pr_impact` / `triage_prs` — which communities a PR touches (merge-risk / blast radius).
2. **CLI fallback** (fresh agent without MCP):
   - `graphify query "<question>"` — scoped subgraph.
   - `graphify explain "<concept>"` — plain-language explanation of a node + neighbors.
   - `graphify path "<A>" "<B>"` — shortest path between two nodes.
   - `graphify affected "<X>"` — reverse traversal: what is impacted if X changes. Run this before editing.
   - `graphify god-nodes` — architectural hubs.
   - `graphify tree` — D3 tree HTML. `graphify diagnose multigraph` — edge-collapse risk check. `graphify benchmark` — token-reduction vs naive search.

When the user types `/graphify`, use the installed graphify skill/instructions first.

### Keeping it fresh (hooks)
- `graphify hook install` installs post-commit/post-checkout hooks that rebuild the AST/structure layer for free (no LLM) on every commit. New clones should run it once.
- The *meaning* (semantic) layer is refreshed incrementally by `scripts/graphify-refresh.sh`: a debounced, lock-guarded background job that runs `graphify extract .` without `--force` (only re-extracts changed files, reuses cached semantic for the rest). It does not auto-commit. Wire it in with `bash scripts/install-graphify-hooks.sh` (adds it to post-commit beside graphify's hook), or just run the script by hand / from cron.
- Dirty `graphify-out/` files are expected after hooks or incremental updates; that is not a reason to skip graphify. Only skip if the task is about stale/incorrect graph output, or the user says not to.

### Teaching the graph (work-memory loop)
When a graph query clearly helped (or was wrong), record it so future queries improve:
- `graphify save-result --question "..." --answer "..." --outcome useful|dead_end|corrected --nodes <labels>...`
  - For `corrected`, also pass `--correction "..."`.
- Periodically `graphify reflect` aggregates these into `graphify-out/reflections/LESSONS.md`.

### Rules
- Reach for graphify (query / path / affected) before grepping the whole repo.
- Use `GRAPH_REPORT.md` for broad architecture, not for specific symbols.
- If `graphify-out/wiki/index.md` exists, use it for navigation.
