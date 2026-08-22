# Surface Harness

> Superseded: the `surface_harness` binary and the typed surface pipeline it
> drove were removed when groundless generation became the only path. The
> campaign-replay role lives on in `src/harness.rs`, and end-to-end runs use
> the stub surface model (`src/bin/stub_provider.rs`) with
> `tests/ui_e2e.rs`. Kept for implementation history.

Headless test harness for the generative surface pipeline, driven as a
conversation. It answers the two product questions directly:

1. Can Aios converse naturally with the user and return the details needed to
   generate a panel?
2. Can the generative UI layer turn that conversation into a surface that
   displays the data?

Binary: `src/bin/surface_harness.rs`.

## Usage

```
cargo run --bin surface_harness -- [--stub] [--config PATH] [--out DIR] [--conv NAME] [prompts...]
```

| Flag | Meaning |
| --- | --- |
| `--stub` | Deterministic stub model server, no network. |
| `--config PATH` | Alternate config file (defaults to `AIOS_CONFIG` / `~/.aios/config.toml`). |
| `--out DIR` | Output directory, default `harness-out`. |
| `--conv NAME` | Run one canned conversation (`overview`, `storage`, `memory`). |
| `prompts...` | Positional prompts, each run as a single-turn conversation. |

Exit code is non-zero if any conversation failed.

### Canned conversations

The default suite is three natural 2-turn dialogs:

- `overview`: "Hi, can you check on my system?" then
  "Show me a panel with the overall health and the biggest problems."
- `storage`: "How much space is left on my disk?" then
  "And is the drive itself healthy?"
- `memory`: "How much memory is in use?" then "Is there any memory pressure?"

Each turn goes through the real planner tool loop with the growing message
history, so the model can call specialists, answer a follow-up from context,
or both. At the end, a surface is composed from the last user question, the
final answer, and every tool result gathered across the conversation.

### Stub run (no network, deterministic)

```
cargo run --bin surface_harness -- --stub --out harness-out
```

Runs the canned suite against the stub server. The stub captures every request
body and the harness asserts the composer request never carried tool
definitions (`composer request carried tool definitions: never`). This is the
structural guarantee that the surface call is tool-less.

### Live run (real system data)

```
cargo run --bin surface_harness -- --out harness-out
cargo run --bin surface_harness -- --conv storage --out harness-out
```

Uses the configured providers. Each conversation runs the real planner tool
loop (specialists read live system data) with a growing history, then the
composer model call, then validation. Free-tier providers can be slow; a
2-turn conversation is typically 20-90 seconds.

## Output

Per conversation (`surface-N.*`):

- `surface-N.json` - the composed `Surface` (surface/v1).
- `surface-N.txt`  - text preview of regions/widgets.
- `surface-N.html` - self-contained HTML preview with evidence chips.

`report.json` - one `probes[]` entry per conversation with:

- `turns` - the transcript: each user message, ok/error, the Aios answer, and
  the `ToolResult`s gathered (evidence `tool-0`, ...).
- `used_tools` - whether any turn invoked a tool (follow-ups may legitimately
  answer from context without one).
- `compose` - ok/error, the routing decision (provider, model, connectivity,
  classification) that served the call, and the full surface JSON.
- `validation` - ok/error (hard) plus soft `diagnostics[]`.
- `ok` - true only when every turn, compose, and validation succeeded.

## Monitoring semantics

Hard failures (probe FAIL, exit non-zero):

- a turn produced no answer,
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

- A live `storage` conversation: turn 1 called `observe` tools and answered
  with real disk figures; the follow-up ("and is the drive healthy?") was
  answered naturally from context without a new tool call; compose produced a
  valid, evidence-bound surface with zero diagnostics. Both product questions
  confirmed on a real provider.
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

- `docs/decisions/0007-groundless-generation-model.md` and
  `docs/milestones/0002-multi-surface-lifecycle-plan.md`
  (harness) for design and done notes.
- `src/surface/validator.rs`, `src/surface/render.rs`, `src/surface/stub.rs`.
