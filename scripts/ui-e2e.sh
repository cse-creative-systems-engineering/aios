#!/usr/bin/env bash
# End-to-end UI test for the Aios desktop app.
#
# Builds the app and frontend, then drives the real binary over WebDriver the
# way a user would: add provider -> enter credential -> discover and assign
# role models -> chat -> canvas surface -> close, repeated across themes. Any
# fallback render, leaked credential, or missing user-visible step fails.
#
# Requires: a Wayland/X11 desktop session, cargo, npm, and the dependencies
# installed by `npm install`. The WebDriver server is embedded in the test
# build of Aios; no tauri-driver or WebKitWebDriver process is involved.
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

echo "[ui-e2e] building app binary and local model stub"
cargo build --manifest-path src-tauri/Cargo.toml --features webdriver
cargo build --bin stub_provider

test_dir="$(mktemp -d)"
stub_log="$test_dir/stub-provider.log"
config="$test_dir/aios.toml"
cleanup() {
    if [[ -n "${stub_pid:-}" ]]; then kill "$stub_pid" 2>/dev/null || true; fi
    rm -rf -- "$test_dir"
}
trap cleanup EXIT

"$repo/target/debug/stub_provider" >"$stub_log" 2>&1 &
stub_pid=$!
for _ in $(seq 1 100); do
    if [[ -s "$stub_log" ]]; then break; fi
    sleep 0.1
done
stub_port="$(sed -n 's/.*:\([0-9][0-9]*\)$/\1/p' "$stub_log" | head -n 1)"
if [[ -z "$stub_port" ]]; then
    echo "[ui-e2e] stub provider did not announce a port" >&2
    exit 1
fi
cat >"$config" <<EOF
[shell]
max_tokens = 1024
history_len = 3
EOF

echo "[ui-e2e] running embedded WebDriver desktop suite"
AIOS_APP_BIN="$repo/src-tauri/target/debug/aios-tauri" AIOS_CONFIG="$config" AIOS_TEST_PROVIDER_ENDPOINT="http://127.0.0.1:$stub_port" npm run test:ui

echo "[ui-e2e] ok"
