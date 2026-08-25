# Grounding Snapshot: Session-Day-Buckets Merged; exec.run Stub Flagged for Sandbox Work

## Current State

Branch `feature/workspace-co-partner/planner-loop` absorbed
`origin/feature/session-day-buckets` (the unmerged "lost evening" work) via a
merge commit. The branch now carries, on top of `main`:

- Today's earlier work: file/web tool claims, multi-turn tool loop,
  effort-based reasoning (`{"reasoning": {"effort": "minimal"}}` replaces the
  `enabled:false` that made Ox 400), read-only-only heuristics, files
  observe handler fix.
- From the merge: ADR-0009 + milestone 0005 (session day-buckets),
  `src/session.rs` SessionStore with Facade persistence and persisted
  surfaces, project scaffold triggers, dynamic workspace-tool advertisement,
  `docs/test-ledger.md`, ADR-0010 (typed execution primitive), and
  `src/exec.rs`.

**Known gap — exec.run is a stub that violates its own ADR.** ADR-0010
specifies bubblewrap sandboxing (primary), landlock fallback, Guardian
pattern checks EXEC-P-001..005, and Default/Auto/YOLO approval modes.
`src/exec.rs` currently runs `sh -c <command>` directly on the host with none
of those controls. `bwrap` is installed on this machine. Next work item:
implement the ADR for real before relying on exec.run for anything beyond
trivial commands.

Conflict resolutions made during the merge:
- `src/tools.rs` test: kept the loose `text.len() > 3676` assertion over the
  branch's pinned `== 4577`.
- `src/coordinator/chat.rs`: kept our read-only heuristic block (files.observe
  seed retained); both sides already agreed no auto file writes.
- Reasoning fix: ours (effort-based) supersedes the branch's global
  `reasoning_disabled: false` flip in composer.rs — merged cleanly because
  the branch changed the same lines we changed differently; verify at test
  time.

## Relevant Paths

- `src/session.rs`, `src/facade.rs`, `src-tauri/src/main.rs` — session persistence
- `src/exec.rs`, `docs/decisions/0010-execution.md` — execution primitive (stub vs spec)
- `src/http.rs` — effort-based reasoning switch
- `src/tools.rs` — FILES/WEB_TOOL_CLAIMS

## Open Work

1. Implement ADR-0010 for real: bwrap wrap of Command::new, EXEC-P checks,
   approval modes. Treat current exec.rs as dev-stub only.
2. Verify surface composition works against Ox end-to-end (reasoning fix).
3. Stage 4 remainder: batched plan-hash approvals, artifact card actions.
4. Milestone 0002 leftovers (surface editing v0.2), sidebar polish (0003).
5. type-level-tcb branch (Phase 1.1 AuthorizationProof) still unmerged —
   depends on this branch landing first.
