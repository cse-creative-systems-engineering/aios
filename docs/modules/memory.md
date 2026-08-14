# Memory Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The memory specialist owns the system's memory domain: physical memory,
swap, pressure, and ECC state. It reports the full meminfo, PSI, and page/
swap/oom counters the host exposes plus memory health through bounded
tools. It does not expose shell execution or unrestricted file access.

## Matching

The package matches the memory resources the system exposes (the
`memory:total` and `memory:available` nodes discovered from `/proc/meminfo`,
plus the `memory:pressure` and `memory:vmstat` evidence nodes, and any
per-bank ECC sensors). A system without an unambiguous memory resource
remains read-only and is not assigned a privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_memory` | 0 | Read total, used, free, cached, swap, pressure, and page/oom counters |
| `diagnose_fault` | 0 | Compare observations with memory invariants |
| `stage_policy` | 2 | Checkpoint current memory policy and stage a candidate |
| `request_reset` | 4 | Request a memory controller reset to known-good state |

`observe_memory` reports typed metrics derived from the graph: total,
available, used, free, and swap in kB (`total_kb`, `available_kb`,
`used_kb`, `free_kb`, `swap_total_kb`, `swap_free_kb`, `swap_used_kb`),
every meminfo key as `meminfo_*`, the pressure averages as `pressure_*`,
and the page/swap/oom counters as `vmstat_*`. Health is cross-layer:
`memory:available` is degraded when available memory is below a tenth of
total, and the domain reports `degraded` plus `nodes_reporting_capacity`
counts.

`stage_policy` uses the action state machine: checkpoint → stage → health
check → commit or rollback. A failed health check always rolls back. A reset
requires a scoped approval and never bypasses the Guardian.

## Invariants

- `MEMORY-001`: the memory subsystem is present and reports usable capacity.
- `MEMORY-002`: ECC errors are within the tolerated threshold after a staged
  change.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist owns its memory resources (`memory:total`,
`memory:available`, `memory:pressure`, `memory:vmstat`, and any ECC
sensors) with `owns` edges. The resources retain `depends_on` edges to
their controller, bus, and ECC sensors when discovery can verify them.
Observed relationships are advisory and are never used as a substitute for
broker capability checks.

## Recovery

v0.1 is policy-level only. Checkpoints contain the current memory policy and
configuration needed to restore it. The package does not modify the boot chain
or firmware. A rollback failure leaves the action in `Failed` and retains the
checkpoint for manual recovery.