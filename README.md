# Aios

Aios stands for **Artificially Intelligent Operating System**.

Aios is an AI-native operating environment that presents a single
conversational interface to the user, but internally coordinates
specialized agents and deterministic services to manage the OS and
hardware safely.

## Core safety principle

> No component should both make an autonomous decision and possess
> unrestricted authority to execute it.

Agents propose. The broker decides. The Guardian vetoes. Staged
execution tests before committing. Everything is reversible.

## Current state

**Design phase — frozen for M1 implementation.**

The design doc set is complete and frozen. 18 documents + 5 ADRs covering
security model, capability-based authorization, typed message protocol,
action state machine with crash recovery, system graph, agent packages,
model routing, and human interaction.

The codebase is a minimal Rust 2024 binary scaffold. Implementation
starts with Milestone 1: an in-process simulation of the broker,
Guardian, executor, graph, and mock agents.

## Key architectural decisions

- **v0.1 runs above Linux** in user space — no kernel modifications
  ([ADR-0001](docs/decisions/0001-v01-runs-above-linux.md))
- **Rust** as the implementation language — type system enforces
  capability boundaries
  ([ADR-0002](docs/decisions/0002-rust-as-implementation-language.md))
- **Fail-fast, no silent fallbacks** during development — every error
  surfaces immediately
  ([ADR-0003](docs/decisions/0003-fail-fast-no-silent-fallbacks.md))
- **Two-dimensional authorization** — capability (resource + operation)
  × tool risk level (0–4)
  ([ADR-0004](docs/decisions/0004-two-dimensional-authorization.md))

## Documentation

Start with the [architecture overview](docs/architecture.md), then read
the contract docs in dependency order:

```
glossary → requirements → security-model → capability-model →
message-protocol → action-state-machine → system-graph →
agent-packages → model-routing → human-interaction →
testing-strategy → observability → implementation-roadmap
```

See [doc-progress.md](docs/doc-progress.md) for the full status tracker.

## Build

```bash
cargo check    # verify the scaffold compiles
cargo run      # run the minimal binary
```

## License

PolyForm Noncommercial — see [LICENSE](LICENSE). Use, study, modify,
and contribute freely. No commercial use without permission.
