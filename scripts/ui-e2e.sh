#!/usr/bin/env bash
# End-to-end UI test for the Aios desktop app.
#
# Builds the app and frontend, then drives the real binary over WebDriver the
# way a user would: sidebar prompt -> canvas window opens -> surface verified
# -> window closed, repeated across system metric themes. Any fallback render
# or missing surface fails the run.
#
# Requires: a display, cargo, npm, `tauri-driver`, and `WebKitWebDriver`.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

# Snap-packaged editors (VS Code / VS Code Insiders) leak their sandbox
# runtime into child shells. GTK_PATH pointing at the snap's gtk-3.0 module
# dir makes every GTK/WebKit process load immodules built against the snap's
# core20 glibc, which dies on contact with the system one:
#   libpthread.so.0: undefined symbol: __libc_pthread_init, version GLIBC_PRIVATE
# tauri-driver, WebKitWebDriver, MiniBrowser and the app under test all hit
# this. Strip the snap runtime vars so they resolve system modules instead.
for var in GTK_PATH GTK_EXE_PREFIX GIO_MODULE_DIR GSETTINGS_SCHEMA_DIR LOCPATH; do
    if [[ -n "${!var:-}" ]]; then
        echo "[ui-e2e] stripping snap-inherited $var=${!var}"
        unset "$var"
    fi
done

echo "[ui-e2e] building frontend (embedded into the app binary)"
npm run build

echo "[ui-e2e] building app binary"
cargo build --manifest-path src-tauri/Cargo.toml

echo "[ui-e2e] running WebDriver suite (this takes a while)"
AIOS_APP_BIN="$repo/src-tauri/target/debug/aios-tauri" \
    cargo test --test ui_e2e -- --ignored --nocapture

echo "[ui-e2e] ok"
