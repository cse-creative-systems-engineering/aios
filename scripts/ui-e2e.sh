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

echo "[ui-e2e] building frontend (embedded into the app binary)"
npm run build

echo "[ui-e2e] building app binary"
cargo build --manifest-path src-tauri/Cargo.toml

echo "[ui-e2e] running WebDriver suite (this takes a while)"
AIOS_APP_BIN="$repo/src-tauri/target/debug/aios-tauri" \
    cargo test --test ui_e2e -- --ignored --nocapture

echo "[ui-e2e] ok"
