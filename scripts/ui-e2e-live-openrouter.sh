#!/usr/bin/env bash
# Live provider E2E: runs the visible OpenRouter setup and chat journey with
# a real key from the operator's local key file. The key is passed only to the
# browser process, typed into the password field, and never printed or saved
# outside the disposable configuration file.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo"

key_file="${AIOS_KEYS_FILE:-/home/shane/Documents/API_KEYS.txt}"
if [[ ! -r "$key_file" ]]; then
    echo "[ui-e2e-live] cannot read the configured key file" >&2
    exit 1
fi
api_key="$(awk -F '[:=]' 'tolower($1) ~ /openrouter/ { value=$2; sub(/^[[:space:]]+/, "", value); sub(/[[:space:]]+$/, "", value); print value; exit }' "$key_file")"
if [[ -z "$api_key" ]]; then
    echo "[ui-e2e-live] no OpenRouter key entry found" >&2
    exit 1
fi

for var in GTK_PATH GTK_EXE_PREFIX GIO_MODULE_DIR GSETTINGS_SCHEMA_DIR LOCPATH; do
    [[ -z "${!var:-}" ]] || unset "$var"
done

npm run build
cargo build --manifest-path src-tauri/Cargo.toml --features webdriver
test_dir="$(mktemp -d)"
config="$test_dir/aios.toml"
cleanup() { rm -rf -- "$test_dir"; }
trap cleanup EXIT
cat >"$config" <<EOF
[shell]
max_tokens = 1024
history_len = 3
EOF

AIOS_APP_BIN="$repo/src-tauri/target/debug/aios-tauri" \
AIOS_CONFIG="$config" \
AIOS_LIVE_OPENROUTER_API_KEY="$api_key" \
AIOS_UI_SPEC="$repo/tests/wdio/live-provider.e2e.cjs" \
npm run test:ui
