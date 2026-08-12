# ADR-0006: Universal OpenAI-compatible gateway backend over per-provider backends

**Status:** Accepted  
**Date:** 2026-08-12  

## Context

M3 implements `aios::model` (registry, router, gateway, pinner) with a
`ModelBackend` trait as the seam between the gateway and any single provider.
Only one backend exists today: `LocalLlama` over `llama.cpp`. Aios also
routes to LAN gateways and internet providers (model-routing.md §1), and
nearly every such provider — Ollama, LM Studio, vLLM, llama.cpp server,
OpenAI, OpenRouter — exposes the same OpenAI-compatible
`POST /v1/chat/completions` API.

Two options were on the table:

1. **Per-provider backends.** One backend struct per provider, each with its
   own request/response codec. Fine for a handful of providers, but every new
   provider needs new code, and each provider drifts from the common API.
2. **One universal backend.** A single `HttpBackend` speaking the
   OpenAI-compatible chat API, with provider differences expressed as config
   (endpoint, headers, model name). Non-conforming providers get a thin
   adapter backend later, only when one actually appears.

## Decision

The gateway uses one universal `HttpBackend` for all remote providers
(LAN and internet), backed by the OpenAI-compatible chat API. Providers are
**config-driven**: they are declared in configuration (kind, endpoint,
tier, model), not hardcoded in Rust. At startup the gateway constructs the
backend for each provider entry and registers it against its `ProviderId`.
Routing, consent, health, and fallback then treat every provider uniformly —
the transport is invisible to routing (§3.1, §3.5 of model-routing.md).

The `ModelBackend` trait stays the only backend seam:

```rust
pub trait ModelBackend: Send + Sync {
    fn provider_id(&self) -> &ProviderId;
    fn is_healthy(&self) -> bool;
    fn generate(&self, request: &GenerationRequest)
        -> Result<GenerationResponse, GenerationError>;
}
```

`LocalLlama` implements it for local GGUF inference. `HttpBackend`
implements it for everything remote. A backend that does not speak the
OpenAI-compatible API is the exception, not the rule, and gets a dedicated
adapter only when Aios actually needs it.

## Consequences

- Adding a provider is a config change, not a code change.
- One wire protocol to audit for secrets: the chat API request/response
  path. `Secret` data never enters a `GenerationRequest` regardless of
  backend (model-routing.md §4.1).
- Config must hold credentials for paid providers. Credentials live in
  `~/.aios/` config, never in the registry or in messages, and stay local.
- A provider feature that falls outside the chat API (e.g. tool calling on a
  provider with a different shape) needs an adapter or a capability flag —
  decided when the need appears.

## Related

- `docs/model-routing.md` — section 6 (gateway architecture), §3 (routing),
  §4 (data classification and consent)
- `src/model.rs` — `ModelBackend`, `ModelGateway`, `ModelRouter`
- `src/local.rs` — `LocalLlama` (the first backend)
- ADR-0003 — fail-fast, no silent fallbacks (fallback issues a new task)
