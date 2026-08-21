#!/bin/bash
# Debounced, lock-guarded background refresh of the graphify SEMANTIC layer.
#
# Runs `graphify extract .` WITHOUT --force, so it only re-extracts files that
# actually changed and reuses cached semantic for the rest (cheap, incremental).
# It does NOT auto-commit, so there is no commit loop; the refreshed graph is
# picked up by the next normal commit.
#
# Safe to call from a git hook: it serializes via a flock and waits for
# graphify's AST hook (which imports graphify.watch) to finish before writing
# graph.json, so the two never write concurrently. The wait pattern is specific
# enough that it does not match this script or the opencode process.

set -u

REPO=$(git rev-parse --show-toplevel 2>/dev/null) || { echo "[graphify-refresh] not in a git repo"; exit 0; }
cd "$REPO" || exit 0

export PATH="$HOME/.local/bin:$PATH"
export OPENAI_BASE_URL="https://openrouter.ai/api/v1"
KEY=$(awk -F'"' '/^api_key/{print $2; exit}' ~/.aios/config.toml 2>/dev/null)
export OPENAI_API_KEY="$KEY"
export GRAPHIFY_OPENAI_MODEL="nvidia/nemotron-nano-12b-v2-vl:free"

if [ -z "$OPENAI_API_KEY" ]; then
  echo "[graphify-refresh] no API key in ~/.aios/config.toml; skipping semantic refresh" >> "$HOME/.cache/graphify-semantic.log"
  exit 0
fi

LOCK="$REPO/graphify-out/.semantic_refresh.lock"
LOG="$HOME/.cache/graphify-semantic.log"
mkdir -p "$(dirname "$LOG")"

# Serialize: only one refresh at a time.
exec 9>"$LOCK"
if ! flock -n 9; then
  echo "[graphify-refresh] another refresh running; skipping ($(date -Is))" >> "$LOG"
  exit 0
fi

# Wait for graphify's AST hook (imports graphify.watch) to finish so we don't
# write graph.json at the same time it does.
for _ in $(seq 1 60); do
  if pgrep -f 'graphify\.watch' >/dev/null 2>&1; then
    sleep 2
  else
    break
  fi
done

echo "[graphify-refresh] incremental extract (no --force) starting ($(date -Is))" >> "$LOG"
graphify extract . >> "$LOG" 2>&1
echo "[graphify-refresh] done ($(date -Is), exit $?)" >> "$LOG"
