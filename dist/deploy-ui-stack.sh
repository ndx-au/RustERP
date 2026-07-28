#!/usr/bin/env bash
# Build RustERP-UI-WASM, install static assets under dist/ui/, build & start rusterp-server.
#
# Usage (from RustERP repo root):
#   ./dist/deploy-ui-stack.sh          # build + foreground server
#   ./dist/deploy-ui-stack.sh --bg     # build + background server
#
# Env:
#   RUSTERP_UI_ROOT   path to RustERP-UI-WASM checkout (default: ../RustERP-UI-WASM)
#   RUSTERP_CONFIG    server config TOML (default: ./rusterp-server.toml)
#   RUSTERP_HOME      pidfile prefix (default: repo root)

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UI_ROOT="${RUSTERP_UI_ROOT:-$ROOT/../RustERP-UI-WASM}"
CONFIG="${RUSTERP_CONFIG:-$ROOT/rusterp-server.toml}"
BG=false

for arg in "$@"; do
  case "$arg" in
    --bg) BG=true ;;
    -h|--help)
      sed -n '1,20p' "$0"
      exit 0
      ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "error: required command '$1' not on PATH" >&2
    exit 1
  }
}

need cargo
need trunk
need protoc

if [[ ! -d "$UI_ROOT" ]]; then
  echo "error: UI repo not found at $UI_ROOT (set RUSTERP_UI_ROOT)" >&2
  exit 1
fi

if [[ ! -f "$CONFIG" ]]; then
  if [[ -f "$ROOT/rusterp-server.toml.example" ]]; then
    cp "$ROOT/rusterp-server.toml.example" "$CONFIG"
    echo "installed default config at $CONFIG"
  else
    echo "error: missing config $CONFIG" >&2
    exit 1
  fi
fi

echo "==> building WASM UI in $UI_ROOT"
(
  cd "$UI_ROOT"
  unset NO_COLOR FORCE_COLOR
  trunk build --release
)

echo "==> installing static assets to $ROOT/dist/ui"
mkdir -p "$ROOT/dist/ui"
rsync -a --delete "$UI_ROOT/dist/" "$ROOT/dist/ui/"

echo "==> generating zstd precompressed assets (.zst)"
for f in "$ROOT/dist/ui"/*.wasm "$ROOT/dist/ui"/*.js "$ROOT/dist/ui"/*.html; do
  [[ -f "$f" ]] || continue
  zstd -19 -f -o "${f}.zst" "$f" 2>/dev/null && echo "  $(basename "$f").zst ($(stat -c%s "${f}.zst") bytes)"
done

echo "==> building rusterp-server (release)"
# Agent shells may set CARGO_TARGET_DIR away from the repo; deploy uses ./target/.
(
  cd "$ROOT"
  unset CARGO_TARGET_DIR
  cargo build -p rusterp-server --release
)
BIN="$ROOT/target/release/rusterp-server"
if [[ ! -x "$BIN" ]]; then
  echo "error: expected binary at $BIN" >&2
  exit 1
fi

export RUSTERP_HOME="${RUSTERP_HOME:-$ROOT}"
export RUSTERP_CONFIG="$CONFIG"
mkdir -p "$ROOT/.local"

echo "==> starting rusterp-server (config: $CONFIG)"
if $BG; then
  nohup "$BIN" >>"$ROOT/.local/rusterp-server.log" 2>&1 &
  echo "background pid $! — log: $ROOT/.local/rusterp-server.log"
  sleep 1
  curl -sS -m 3 -o /dev/null -w "HTTP %{http_code}\n" "http://127.0.0.1:8123/" || true
else
  exec "$BIN"
fi
