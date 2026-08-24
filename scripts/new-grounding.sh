#!/usr/bin/env bash
# Create a new dated grounding snapshot and link it at the top of PROJECT_GROUNDING.md.
#
# Usage:
#   scripts/new-grounding.sh                 # opens template in $EDITOR
#   scripts/new-grounding.sh "one-line summary of what changed"
#
# The summary becomes the bullet text next to the snapshot link. Fill in the
# sections in the generated file before committing.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

summary="${1:-}"
stamp=$(date +%Y-%m-%d_%H-%M-%S)
file="docs/grounding/project_grounding_${stamp}.md"

cat > "$file" <<EOF
# Grounding Snapshot: ${summary:-TODO one-line summary}

## Current State

TODO — what is true now that was not true before. Name the commits.

## Relevant Paths

- TODO — files that changed and why they matter

## Open Work

- TODO — next steps, deferred decisions
EOF

# Link it at the top of the Latest Snapshot list in PROJECT_GROUNDING.md.
python3 - "$file" "$summary" <<'PY'
import re, sys, datetime
path, fname, summary = sys.argv[1], sys.argv[1], sys.argv[2]
link = fname
text = open("PROJECT_GROUNDING.md").read()
today = datetime.date.today().isoformat()
bullet = f"- [`{link.split('/')[-1]}`]({link}) — {summary or 'TODO summary'}\n"
marker = "Read the latest dated grounding snapshot:\n\n"
assert marker in text, "PROJECT_GROUNDING.md missing snapshot list marker"
text = text.replace(marker, marker + bullet, 1)
open("PROJECT_GROUNDING.md", "w").write(text)
print(f"linked {bullet.strip()!r} into PROJECT_GROUNDING.md")
PY

echo "created $file"
if [ -t 0 ] && [ -z "$summary" ]; then
  "${EDITOR:-${VISUAL:-nano}}" "$file"
fi
