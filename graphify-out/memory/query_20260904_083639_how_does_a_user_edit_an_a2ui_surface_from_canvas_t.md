---
type: "query"
date: "2026-09-04T08:36:39.839362+00:00"
question: "How does a user edit an A2UI surface from canvas to sidebar through the backend revision protocol and preserve stable identity?"
contributor: "graphify"
outcome: "useful"
source_nodes: ["main.ts", "sidebar.ts", "src-tauri/src/main.rs", "SurfaceRecord"]
---

# Q: How does a user edit an A2UI surface from canvas to sidebar through the backend revision protocol and preserve stable identity?

## Answer

Canvas starts a revision handoff with the stable surface ID and observed revision. The sidebar holds that target while its normal composer submits revise_surface. The backend checks optimistic concurrency, updates only the matching SurfaceRecord, persists it, and emits surface_lifecycle so every webview converges on the same revision.

## Outcome

- Signal: useful

## Source Nodes

- main.ts
- sidebar.ts
- src-tauri/src/main.rs
- SurfaceRecord