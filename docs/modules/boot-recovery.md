# Boot and Recovery Specialist

**Status:** Draft — v0.1 module specification
**Depends on:** agent-packages.md, capability-model.md, action-state-machine.md,
system-graph.md, message-protocol.md, security-model.md

## Scope

The boot and recovery specialist owns the boot and recovery domain: boot
state, recovery images, snapshots, and recovery paths. It reports boot and
recovery state through bounded tools. It does not expose shell execution or
unrestricted file access.

In v0.1, Aios operates above Linux and does not manage boot images or
watchdogs (ADR-0001). Boot-level rollback (A/B images, watchdog) is deferred
to v0.2+ (architecture §12). The trust plane remains the source of truth for
protected recovery boundaries (architecture §trust plane).

## Matching

The package matches `BootImage` nodes and recovery resources. A resource
without an unambiguous match remains read-only and is not assigned a
privileged specialist.

## Tools

| Tool | Risk | Purpose |
|---|---:|---|
| `observe_boot` | 0 | Read boot state and recovery-image availability |
| `diagnose_fault` | 0 | Compare observations with boot invariants |

v0.1 is read-only. Boot-level mutating operations (A/B image management,
watchdogs) are deferred to v0.2+ per ADR-0001. Read-only observation is the
default (architecture §5).

## Invariants

- `BOOT-001`: a known-good recovery image is available.
- `BOOT-002`: the boot chain is not modified (ADR-0001).

The specialist reports an invariant as unknown when its evidence is missing or
stale. Unknown evidence cannot authorize a change.

## Graph relationships

The specialist is linked to its boot and recovery resources with `owns`. Each
resource retains `depends_on` edges to the components it protects when
discovery can verify them. Observed relationships are advisory and are never
used as a substitute for broker capability checks.

## Recovery

v0.1 is read-only; the boot chain is never modified, so the system remains
bootable regardless of other outcome (architecture §12, ADR-0001). When
boot-level operations are added in v0.2+, checkpoints and A/B images will
capture the state needed to restore a staged change; a rollback failure
leaves the action in `Failed` and retains the checkpoint for manual recovery.