# Specialist Depth Plan

**Status:** Working plan
**Depends on:** implementation-roadmap.md (M7 complete), docs/modules/*.md, src/discovery.rs, specialist modules

## Goal

Make every specialist's `observe` able to answer any question the host OS can
answer through world-readable `/proc` and `/sys` plus the read-only commands
Aios already runs (`systemctl`). Output returns as structured evidence the
model can actually use, not count metrics plus a `resources` blob.

The audit that produced this plan is grounded in the current source: discovery
reads a thin slice of the OS (`/proc/meminfo` two fields, `/proc/cpuinfo`
three, net interfaces by `operstate` only, no `/proc/net/*`, no
`/proc/pressure`, no `/sys/class/thermal`, no `/sys/class/power_supply`, no
`/proc/diskstats`). Only network-up, process, and service nodes get a health
set in discovery; everything else stays `Unknown`, which is why the memory
specialist reported two healthy nodes as `degraded=2`. Observe output is
counts plus `state:<id>` Debug strings and a few attributes capped at eight
nodes.

## Phase 0 - Shared infrastructure (touches every specialist)

1. **Health pass in discovery.** Set a real health for every node type
   (kernel, cpu, memory, pci/usb/block, filesystem, firmware, sensor, driver,
   bus currently default to `Unknown`). Healthy means discovered and reporting
   core evidence. This removes false `degraded=N` alarms and makes the
   `degraded` metric meaningful. Also fix the network operstate asymmetry:
   `up` -> Healthy, `down` -> Degraded, missing -> Unknown.
2. **Observe evidence standard.** Replace the `resources` blob and
   `state:<id>` Debug strings with structured rows (`top_<name>_N`, the
   processes pattern) whose values are quoted so they survive the `k=v k=v`
   flattening in `protocol_tool_result` (coordinator.rs).
3. **Flattener quoting.** Quote values containing spaces so command lines,
   mountpoints, and labels are not split into fake key/value pairs.
4. **Shared live-sampling util.** Lift the processes window sampling
   (`cpu_stats`) into a shared helper used when specialists need deltas
   (network counters, diskstats). Lands with storage/network.
5. **Claims-vs-implementation test.** Split `model_tool_instructions()` into
   per-domain constants and add a test that each claimed metric exists in the
   specialist's observe output. The current memory claim ("total, used, free,
   swap, and pressure state") overstates what observe returns.

## Per-specialist depth

- **Memory** (start here, transcript-proven). Parse all of `/proc/meminfo`
  (used, free, buffers, cached, slab, shmem, dirty, swap, hugepages),
  `/proc/pressure/memory`, `/proc/vmstat` (swap in/out, oom kills). Emit typed
  rows plus swap, pressure, and oom metrics. Health from MemAvailable.
- **Processes** (mostly done). Add `/proc/<pid>/status` (VmSize, VmData,
  VmSwap, Threads, Uid, Nice) and `/proc/<pid>/io`; add `top_mem_N` rows and
  `/proc/loadavg`, `/proc/uptime`.
- **Storage.** `/proc/diskstats`, `/sys/block/<n>/stat` and `queue/*`
  (rotational, scheduler, block sizes), the options field of `/proc/mounts`,
  per-filesystem usage through a bounded `statvfs` collector. Emit `disk_N`
  and `fs_N` rows. Read-only-mounted or error filesystems degrade.
- **Network.** `/proc/net/dev` (rx/tx bytes, errors, drops), `/proc/net/route`
  (gateway), `/proc/net/arp`, `/proc/net/wireless`, and the per-interface
  speed/duplex/carrier/statistics files. Emit `iface_N` rows.
- **Drivers.** Full `/proc/modules` columns (size, refcount, used-by, state),
  `/sys/module/*/version` and parameters, `/sys/class/dmi/id/*`. Emit
  `driver_N`, `device_N`, `system_N` rows. Driver health from module state.
- **Graphics.** `/sys/class/drm/*` (connector status, modes, edid, card
  driver), sessions from `/run/systemd/sessions` plus a loginctl collector.
  Emit `gpu_N`, `display_N`, `session_N` rows.
- **Power/thermal.** `/sys/class/thermal/thermal_zone*`, `/sys/class/power_supply/*`,
  `/sys/class/backlight/*`. Emit `thermal_N`, `power_N`, `backlight_N` rows in
  human units. Temperature at a trip point or low battery degrades.
- **Security.** Kernel knobs (`dmesg_restrict`, `kptr_restrict`,
  `perf_event_paranoid`, `yama`), `/sys/kernel/security/lsm`, AppArmor and
  SELinux state, `Seccomp`; plus broker quarantine and pending-op counts from
  the enforcement plane. Emit `lsm_N`, `knob_N`, `trust_N` rows.
- **Packages.** Implement the missing `discover_packages` (the module header
  cites a function that does not exist). Module-level per ADR-0001:
  `/etc/os-release`, kernel and module versions from `/proc/modules` and
  `/sys/module/*/version`. The distro package DB is deferred.
- **Boot.** `/proc/cmdline`, `/sys/firmware/efi/vars`, boot id, `/proc/uptime`,
  `/boot/loader/entries`, `/sys/class/watchdog/*`. Ground the seeded
  bootimage/watchdog/snapshot nodes in real files; make `label`/`kind` come
  from attributes so they actually appear.
- **Wifi.** `/proc/net/wireless` (link, signal, noise), plus the
  `firmware_version` metric the module doc claims but the code never emits.
  Signal below threshold or no carrier degrades.

## Cross-cutting

Rename ambiguous counts (`nodes_with_capacity`, `nodes_with_usage`,
`gpus_with_state`) to explicit names. Stay read-only; no mutating tools. The
structured rows are the typed evidence the Phase 6 dynamic surfaces consume.

## Sequencing and verification

Phase 0, then Memory, then Storage and Network, then Drivers and Graphics,
then Power, Security, Packages, Boot, then Wifi. Each lands with extended
`mock_root` fixtures, parser tests, observe-output assertions, a live
`/proc` smoke check, `cargo test` green, a roadmap progress note, and the
per-domain instruction sentence updated to match what actually ships.

## Out of scope

Mutating tools (`stage_driver`, `request_reset`, `stage_update`,
`request_rollback`, quarantine execution), anything needing root beyond
world-readable `/proc` and `/sys`, and distro package DBs.
