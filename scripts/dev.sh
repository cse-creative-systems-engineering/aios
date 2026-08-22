#!/usr/bin/env bash
# Manual dev run of the desktop app with the snap-editor environment
# sanitized. Equivalent to the old inline incantation:
#   export PATH="$HOME/.cargo/bin:$PATH" && unset GTK_PATH ... && npm run tauri:dev
#
# Why: snap-packaged editors (VS Code Insiders) leak their sandbox runtime
# into terminals. GTK_PATH pointing at the snap's gtk-3.0 dir makes WebKitGTK
# load modules built against core20 glibc, which crashes against the system
# one ("undefined symbol: __libc_pthread_init, version GLIBC_PRIVATE").
# scripts/ui-e2e.sh carries the same sanitizer for test runs.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

export PATH="$HOME/.cargo/bin:$PATH"

for var in GTK_PATH GTK_EXE_PREFIX GIO_MODULE_DIR GSETTINGS_SCHEMA_DIR \
           LOCPATH GDK_PIXBUF_MODULE_FILE GTK_IM_MODULE_FILE GTK2_RC_FILES; do
    if [[ -n "${!var:-}" ]]; then
        echo "[dev] stripping snap-inherited $var"
        unset "$var"
    fi
done

exec npm run tauri:dev -- "$@"
