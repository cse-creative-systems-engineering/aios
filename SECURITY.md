# Security Policy

Aios is an AI-native operating system design with safety as its core
principle. We take security and safety reports seriously.

## Reporting a vulnerability

Please report security vulnerabilities privately, not in a public
issue:

- **GitHub:** use the
  [private vulnerability reporting](https://github.com/cse-creative-systems-engineering/aios/security/advisories/new)
  form.

Include as much of the following as you can:

- Component affected (broker, guardian, executor, state machine,
  message protocol, agent package, model routing, human interaction)
- A minimal description of the flaw and its impact
- Whether it affects the design docs (pre-implementation) or the code
- Any relevant section references from the docs

## Response

- We aim to acknowledge reports within 5 business days.
- Pre-implementation security issues in the design docs are tracked as
  ADRs or design issues.
- Code vulnerabilities are prioritized by severity and impact.

## Scope

This policy covers the `aios` repository. Note that the project is in
the design phase — most of the codebase is a scaffold, and the security
relevant material lives in the design doc set under
[`docs/`](docs/).
