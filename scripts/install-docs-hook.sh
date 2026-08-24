#!/usr/bin/env bash
# Local backstop for the grounding-snapshot freshness rule (AGENTS.md).
# Warns on every code-touching commit; hard-fails only when the newest
# snapshot predates the last code change AND this commit also touches code.
# Bypass with: git commit --no-verify
echo "[check-docs] bash scripts/check-docs.sh" > .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
echo "installed pre-commit hook (bypass with --no-verify)"
