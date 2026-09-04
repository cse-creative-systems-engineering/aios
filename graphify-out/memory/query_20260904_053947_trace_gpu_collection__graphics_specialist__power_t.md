---
type: "query"
date: "2026-09-04T05:39:47.810818+00:00"
question: "Trace GPU collection, graphics specialist, power/thermal collection, process GPU activity, network traffic, and SystemStateStore correlation findings."
contributor: "graphify"
outcome: "useful"
source_nodes: ["SystemStateStore", "GraphicsSpecialist"]
---

# Q: Trace GPU collection, graphics specialist, power/thermal collection, process GPU activity, network traffic, and SystemStateStore correlation findings.

## Answer

SystemStateStore owned the live collector path; graphics and GPU modules were specialist contracts only, with no existing runtime GPU adapter. A capability-gated nvidia-smi adapter belongs beside state collection, while correlations must remain non-causal.

## Outcome

- Signal: useful

## Source Nodes

- SystemStateStore
- GraphicsSpecialist