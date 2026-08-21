---
type: "query"
date: "2026-08-21T21:57:53.408051+00:00"
question: "Which surface generation path is Aios's intended architecture?"
contributor: "graphify"
outcome: "corrected"
correction: "Do not describe src/surface schema.rs/composer.rs as the intended surface architecture. The intended path is AIOS-gated compose_unconstrained_html; typed widget IR is legacy pending retirement."
source_nodes: ["Surface Composer", "Unconstrained HTML", "Surface Schema"]
---

# Q: Which surface generation path is Aios's intended architecture?

## Answer

Groundless on-the-fly generation only. compose_unconstrained_html + verify_value_fidelity is the design (milestone 0001); the typed surface/v1 IR (schema.rs metric/gauge/status/chart/notice) that became the default was drift added in 6bcefc6 and contradicts the owner's stated rule.

## Outcome

- Signal: corrected
- Correction: Do not describe src/surface schema.rs/composer.rs as the intended surface architecture. The intended path is AIOS-gated compose_unconstrained_html; typed widget IR is legacy pending retirement.

## Source Nodes

- Surface Composer
- Unconstrained HTML
- Surface Schema