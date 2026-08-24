#!/usr/bin/env bash
# Local backstop for the grounding-snapshot freshness rule (AGENTS.md).
# Warns on every code-touching commit; hard-fails only when the newest
# snapshot predates the last code change AND this commit also touches code.
# Bypass with: git commit --no-verify
echo '#!/usr/bin/env bash' > .git/hooks/pre-commit
echo '# Grounding-snapshot freshness backstop (AGENTS.md). Bypass: git commit --no-verify' >> .git/hooks/pre-commit
echo 'set -euo pipefail' >> .git/hooks/pre-commit
echo 'cd "$(git rev-parse --show-toplevel)"' >> .git/hooks/pre-commit
echo 'exec bash scripts/check-docs.sh' >> .git/hooks/pre-commit
chmod +x .git/hooks/pre-commit
echo "installed pre-commit hook (bypass with --no-verify)"
