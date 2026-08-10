# Contributing to Aios

Thanks for your interest in Aios. The project is in an early phase: the
design doc set is frozen for M1, and the codebase is a minimal scaffold.

## Project status and what to work on

- **The doc set is frozen.** Design docs change only when M1 surfaces a
  blocker, and changes are recorded as ADRs. Do not open PRs that
  re-draft the design docs.
- **M1 is the active milestone:** an in-process simulation of the Policy
  Broker, Infrastructure Guardian, Staged Executor, System Graph, and
  mock agents. See
  [implementation-roadmap.md](docs/implementation-roadmap.md) for scope.
- Open [issues](https://github.com/cse-creative-systems-engineering/aios/issues)
  for bugs, design blockers M1 surfaces, and ideas. Use the issue
  templates.

## Development setup

```bash
cargo check
cargo run
cargo test
```

## Pull request guidelines

- Keep changes scoped to M1. If a change touches the design docs,
  explain which M1 blocker forced it.
- Format with `cargo fmt` and keep `cargo clippy -- -D warnings` clean.
- Fail-fast applies to code too: no silent fallbacks, no `unwrap()` in
  library code, no ignored errors (ADR-0003).
- Run the full test suite before opening a PR. CI runs format, clippy,
  build, and tests on every push.

## Communication

- Use issues for questions and discussion. There is no chat channel yet.
- Security vulnerabilities: report privately per
  [SECURITY.md](SECURITY.md), not in public issues.
