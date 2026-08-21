#!/bin/bash
# Install graphify git hooks for this repo.
#
# 1. graphify hook install  -> post-commit/post-checkout that rebuild the AST
#    (structure) layer for free on every commit.
# 2. Appends a call to scripts/graphify-refresh.sh to post-commit so the
#    SEMANTIC (meaning) layer is refreshed incrementally in the background.
#
# Re-running this script is safe: it re-installs graphify's hook and only adds
# the semantic-refresh line once.

set -e
REPO=$(git rev-parse --show-toplevel)
cd "$REPO"

echo "[install] graphify hook install (AST auto-rebuild)..."
graphify hook install

HOOK="$REPO/.git/hooks/post-commit"
MARKER='# graphify-semantic-refresh'

if [ ! -f "$HOOK" ]; then
  echo "[install] post-commit hook missing after graphify hook install; aborting" >&2
  exit 1
fi

if grep -q "$MARKER" "$HOOK" 2>/dev/null; then
  echo "[install] semantic refresh already wired into post-commit"
else
  cat >> "$HOOK" <<EOF

$MARKER
# Incremental semantic refresh (debounced, no auto-commit). See scripts/graphify-refresh.sh
# Fully detached: redirect fds away from git's commit pipe so the hook returns
# immediately. The script logs to \$HOME/.cache/graphify-semantic.log.
bash "$REPO/scripts/graphify-refresh.sh" >> "\$HOME/.cache/graphify-semantic.log" 2>&1 </dev/null &
EOF
  echo "[install] added semantic refresh to post-commit"
fi

echo "[install] done. AST rebuilds on every commit; semantics refresh incrementally in the background."
