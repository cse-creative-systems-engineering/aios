# Surface Harness

Headless test harness for the generative surface pipeline. Drives the real
backend without a display: prompt -> specialist tool loop -> composer model
call -> validation -> text/HTML surface preview, and writes a monitoring
report for every prompt.

Binary: `src/bin/surface_harness.rs`.

## Usage

```
cargo run --bin surface_harness -- [--stub] [--config PATH] [--out DIR] [prompts...]
```

| Flag | Meaning |
| --- | --- |
| `--stub` | Deterministic stub model server, no network. |
| `--config PATH` | Alternate config file (defaults to `AIOS_CONFIG` / `~/.aios/config.toml`). |
| `--out DIR` | Output directory, default `harness-out`. |
| `prompts...` | Positional prompts. A 7-prompt canned suite runs when none are given. |

Exit code is non-zero if any probe failed.

### Stub run (no network, deterministic)

```
cargo run --bin surface_harness -- --stub --out harness-out
```

7/7 canned prompts run against the stub server. The stub captures every
request body and the harness asserts the composer request never carried tool
definitions (`composer request carried tool definitions: never`). This is the
structural guarantee that the surface call is tool-less.

### Live run (real system data)

```
cargo run --bin surface_harness -- --out harness-out
```

Uses the configured providers. Each prompt runs the real planner tool loop
(specialists read live system data), then the composer model call, then
validation. Any subset of the canned suite can be selected by passing prompts:

```
cargo run --bin surface_harness -- --out harness-out \
  "How much of the disk is used and is the drive healthy?"
```

## Output

Per prompt (`surface-N.*`):

- `surface-N.json` - the composed `Surface` (surface/v1).
- `surface-N.txt`  - text preview of regions/widgets.
- `surface-N.html` - self-contained HTML preview with evidence chips.

`report.json` - one `probes[]` entry per prompt with:

- `chat` - the grounded answer and the `ToolResult`s (evidence `tool-0`, ...).
- `compose` - ok/error, the routing decision (provider, model, connectivity,
  classification) that served the call, and the full surface JSON.
- `validation` - ok/error (hard) plus soft `diagnostics[]`.
- `ok` - true only when chat, compose, and validation all succeeded.

## Monitoring semantics

Hard failures (probe FAIL, exit non-zero):

- chat error (no answer produced),
- compose error (gateway, empty reply, or a reply that is not a usable
  surface after the one correction retry),
- validation error (schema/evidence/layout violation: zero columns, span
  overflow, missing evidence key, value absent from evidence, dangling or
  duplicate widget ids).

Soft findings (reported, do not fail the probe):

- status list `detail` text not verbatim in evidence,
- chart points not present as standalone numbers,
- notice bodies not verbatim in evidence.

## What the live runs found (2026-08-16)

- Free-tier models sometimes reply with prose instead of JSON. Mitigated by a
  dedicated composition token budget and a one-turn correction retry in
  `src/surface/composer.rs`.
- JSON with trailing commas was falling through `extract_json` to a nested
  fragment. `src/planner.rs` now repairs trailing commas before `}`/`]`.
- The validator caught invented values (e.g. a computed "used memory" figure
  that exists nowhere in evidence) and the composer prompt now forbids
  deriving values outright.
- Transient free-tier timeouts occur; the harness reports them without
  aggressive retries so the rate limit is not hammered.

## Related

- `docs/generative-surface-roadmap.md` Phase D (validator) and Phase I
  (harness) for design and done notes.
- `src/surface/validator.rs`, `src/surface/render.rs`, `src/surface/stub.rs`.
