#!/usr/bin/env bash
# Fails if source code changed more recently than the newest grounding snapshot.
#
# Heuristic: compare the commit date of the newest commit touching src/,
# src-tauri/, frontend/src/, or Cargo.toml against the filename date of the
# newest docs/grounding/project_grounding_*.md snapshot. Docs-only and
# graphify-only commits never trigger failure.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

latest_code_commit=$(git --no-pager log -1 --format=%ct -- src src-tauri frontend/src Cargo.toml tests || true)
if [ -z "$latest_code_commit" ]; then
  echo "check-docs: no code commits found; nothing to check"
  exit 0
fi

latest_snapshot=$(ls -1 docs/grounding/project_grounding_*.md 2>/dev/null | sort | tail -1 || true)
if [ -z "$latest_snapshot" ]; then
  echo "check-docs: FAIL — no grounding snapshot exists in docs/grounding/"
  exit 1
fi

# Snapshot filenames are project_grounding_YYYY-MM-DD_HH-MM-SS.md — parse the
# name, not mtime, so the comparison is stable across clones.
snap_name=$(basename "$latest_snapshot")
snap_iso=$(echo "$snap_name" | sed -E 's/^project_grounding_([0-9]{4}-[0-9]{2}-[0-9]{2})_([0-9]{2})-([0-9]{2})-([0-9]{2})\.md$/\1 \2:\3:\4/')
snap_epoch=$(date -d "$snap_iso" +%s 2>/dev/null || echo 0)

code_date=$(date -u -d "@$latest_code_commit" '+%Y-%m-%d %H:%M')
snap_date=$(date -u -d "@$snap_epoch" '+%Y-%m-%d %H:%M')

echo "check-docs: last code change : $code_date UTC"
echo "check-docs: latest snapshot  : $snap_date UTC ($snap_name)"

if [ "$latest_code_commit" -gt "$snap_epoch" ]; then
  echo ""
  echo "check-docs: FAIL — code changed after the newest grounding snapshot."
  echo "Run scripts/new-grounding.sh to write one, then commit it."
  exit 1
fi

echo "check-docs: OK"
