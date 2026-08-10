# ADR-0003: Fail-fast, no silent fallbacks during development

**Status:** Accepted  
**Date:** 2026-08-09  

## Context

Aios is a safety-critical system. The architecture specifies fail-closed
behavior for production: the Policy Broker denies on ambiguity, the Guardian
blocks on insufficient evidence, and the System Graph shows `UNKNOWN`/`STALE`
instead of pretending things are healthy.

During development, the same principle must apply to error handling, missing
data, unexpected states, and unimplemented paths. Silent fallbacks — catching
an error and returning a default, retrying without logging, or continuing with
reduced functionality when a component is missing — hide bugs. In a
safety-critical system, a hidden bug is a latent safety failure.

## Decision

**During development, every error, ambiguity, missing capability, stale state,
or unexpected condition must cause an immediate and visible failure.**

- No fallback paths exist unless explicitly designed, discussed, and documented.
- Errors panic or return immediately with full context — they are never
  swallowed, defaulted, or silently retried.
- Unimplemented functions panic with `todo!()` or `unimplemented!()`, never
  return a plausible-looking default.
- Missing telemetry, missing graph nodes, or missing capabilities cause hard
  failures, not degraded-but-silent operation.
- Every fallback that is added must be the result of an explicit design
  decision, documented in an ADR, and protected by tests that verify the
  fallback triggers only under the intended condition.

## Consequences

**Positive:**

- Bugs surface at the point of introduction, not downstream where they are
  harder to trace.
- The system's failure modes are visible during development, which informs
  the production failure matrix.
- No silent safety degradation — if a capability check is missing, the system
  stops, rather than quietly proceeding without it.
- Aligns development behavior with the production fail-closed principle.

**Negative:**

- The system will crash frequently during early development. This is
  intentional and productive.
- Some convenience fallbacks that a developer might want (e.g., "use a default
  config if the file is missing") must be explicitly designed rather than
  added casually.

**Neutral:**

- This principle applies to development. Production behavior may include
  designed fallbacks (e.g., model provider fallback, recovery paths), but
  each must be explicitly designed, tested, and documented.
- The transition from "fail fast" to "fail safe" for a given component is
  itself a design milestone — it means the failure mode is understood and
  the safe state is defined.

## Related

- Architecture section 1 (core safety principle)
- Architecture section 13 (risks: alert fatigue, message-bus failure)
- REQ-SAF-002 (fail-closed by default)
