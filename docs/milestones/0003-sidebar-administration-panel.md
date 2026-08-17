# Sidebar Administration Panel

**Status:** Planned work
**Created:** 2026-08-17
**Prerequisites:** `0002-multi-surface-lifecycle-plan.md`

## Purpose

Turn the current sidebar wireframe into Aios's resident system administration
panel. This is a control and observability surface, not a generic chatbot
shell.

## Work Order

### 1. State and IPC Contract

Define typed backend views for:

- backend readiness;
- connectivity;
- provider records without secrets;
- discovered model metadata;
- capability requirements;
- role and specialist assignments;
- active task pins;
- provider/model health;
- current operation phase;
- surface lifecycle summaries;
- warnings and recovery state.

The backend remains authoritative. The frontend cannot directly edit the config
file, route a request, or access a credential.

### 2. Provider Registry View

Add a settings view that can add providers, validate connectivity, discover
models, and show whether credentials are configured. Credentials enter through
a trusted backend command and are never returned to the frontend.

The view should make provider state legible:

- configured or missing credentials;
- reachable, unavailable, or cooling down;
- discovered model count;
- last health-check result;
- data classification and consent limitations.

### 3. Model Assignment View

Expose assignments for:

- Planner;
- Verifier;
- SurfaceComposition;
- general conversational answer generation;
- each read-only specialist role;
- future specialist roles as they are registered.

Support system defaults, role defaults, specialist overrides, and active task
pins. The Policy Broker is intentionally absent from this list because it is
deterministic enforcement code, not a model slot.

Before saving an assignment, the backend validates required capabilities such
as tool calling, structured output, context length, data classification, and
provider connectivity. Incompatible assignments fail with a typed reason.

### 4. Persistent Status Surface

Build a compact always-visible status strip plus an expandable detail panel.
The user should be able to tell whether Aios is idle, gathering evidence,
verifying, composing, waiting on a provider, blocked by policy, or recovering.

The status surface should include active surfaces and their lifecycle state,
but surface editing remains owned by the surface manager from milestone 0002.

### 5. Visual System

Develop the visual language after the state contract exists:

- strong hierarchy and readable typography;
- deliberate density for a narrow resident panel;
- quiet system-instrument styling rather than chatbot styling;
- clear healthy, degraded, unknown, stale, waiting, and failed states;
- restrained transitions that communicate state changes;
- accessible keyboard focus and navigation;
- no decorative glass effects that reduce status legibility.

Keep the native sidebar geometry and canvas overlay untouched during the first
visual passes.

## Safety Requirements

- API keys never enter frontend state, prompts, logs, or generated HTML.
- The sidebar cannot create capabilities, approvals, or policy decisions.
- The broker remains deterministic and is not model-assignable.
- A model assignment cannot be saved when required capabilities are missing.
- Provider discovery cannot silently replace an existing assignment.
- A failed health check is visible and does not become a healthy state.

## Acceptance

1. A user can add a provider without exposing its credential value.
2. Aios can discover and display that provider's models.
3. A user can assign models by role and specialist with visible validation.
4. An incompatible tool-calling or structured-output assignment is rejected.
5. The sidebar continuously shows backend, provider, operation, and surface
   status.
6. Existing CPU/RAM surface generation, dragging, and click-through behavior
   remains unchanged.
