# Processes and Resources Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md

## Scope

The processes and resources specialist owns the process and resource domain:
running processes, namespaces, and their resource usage (CPU, memory). It
reports process and resource state through bounded tools. It does not expose
shell execution or unrestricted file access.

## Matching

The package matches discovered `Process` and `Namespace` nodes. A resource
without an unambiguous match remains read-only and is not assigned a
privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_process` | 0 | Read process, namespace, and resource-usage state |
| `diagnose_fault` | 0 | Compare observations with process invariants |

v0.1 is read-only. Resource budget enforcement is deferred to v0.2+; budgets
are declared in packages but advisory in v0.1, and enforcement requires
process isolation (architecture §review). Mutating operations (stopping a
process, adjusting a resource limit) are deferred to a later iteration and
will require a specific operation with a defined risk level, passing through
the action state machine and the Guardian.

## Invariants

- `PROC-001`: processes are present and report resource usage.
- `PROC-002`: resource usage stays within defined budgets.

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its process and namespace nodes with `owns`. Each
process retains `depends_on` edges to the resources it consumes when discovery
can verify them. Observed relationships are advisory and are never used as a
substitute for broker capability checks.

## Recovery

v0.1 is read-only. When mutating operations are added, checkpoints capture
the state needed to restore a staged change; a rollback failure leaves the
action in `Failed` and retains the checkpoint for manual recovery.