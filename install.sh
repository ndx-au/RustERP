#!/usr/bin/env bash
# Copyright 2026 NDX Pty Ltd and contributors
# SPDX-License-Identifier: Apache-2.0
#
# Bootstrap the thin `rusterp` helper CLI only (not the whole ERP).
# Linux + macOS. Idempotent: safe to re-run.
#
# Usage:
#   ./install.sh
#   curl -fsSL https://raw.githubusercontent.com/ndx-au/RustERP/main/install.sh | bash
#
# Then:
#   rusterp install core [--branch dist]
#
# Env:
#   RUSTERP_HOME              install prefix for CLI files (default user-local or /opt)
#   RUSTERP_BIN_DIR           where to place `rusterp` on PATH (default ~/.local/bin)
#   RUSTERP_BOOTSTRAP_REF     git ref for remote fetch of dist/* (default: main)
#   RUSTERP_REPO_RAW_BASE     raw content base (default GitHub raw URL)

set -euo pipefail

DEFAULT_REPO_RAW="https://raw.githubusercontent.com/ndx-au/RustERP"
BOOTSTRAP_REF="${RUSTERP_BOOTSTRAP_REF:-main}"
REPO_RAW_BASE="${RUSTERP_REPO_RAW_BASE:-$DEFAULT_REPO_RAW}"

die() {
  printf 'install.sh: error: %s\n' "$*" >&2
  exit 1
}

info() {
  printf 'install.sh: %s\n' "$*" >&2
}

default_home() {
  if [ -n "${RUSTERP_HOME:-}" ]; then
    printf '%s\n' "$RUSTERP_HOME"
    return 0
  fi
  if [ "$(id -u)" -eq 0 ]; then
    printf '%s\n' "/opt/rusterp"
  else
    printf '%s\n' "${HOME}/.local/share/rusterp"
  fi
}

default_bin_dir() {
  if [ -n "${RUSTERP_BIN_DIR:-}" ]; then
    printf '%s\n' "$RUSTERP_BIN_DIR"
    return 0
  fi
  if [ "$(id -u)" -eq 0 ]; then
    printf '%s\n' "/usr/local/bin"
  else
    printf '%s\n' "${HOME}/.local/bin"
  fi
}

SCRIPT_PATH="${BASH_SOURCE[0]:-$0}"
SCRIPT_DIR=""
if [ -f "$SCRIPT_PATH" ]; then
  SCRIPT_DIR="$(cd "$(dirname "$SCRIPT_PATH")" 2>/dev/null && pwd || true)"
fi

HOME_DIR="$(default_home)"
BIN_DIR="$(default_bin_dir)"
CLI_DIR="${HOME_DIR}/cli"

mkdir -p "$CLI_DIR" "$BIN_DIR"

fetch_file() {
  local rel="$1"
  local dest="$2"
  local url="${REPO_RAW_BASE}/${BOOTSTRAP_REF}/${rel}"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$dest" || return 1
  elif command -v wget >/dev/null 2>&1; then
    wget -qO "$dest" "$url" || return 1
  else
    die "need curl or wget to download ${rel} (or run install.sh from a git checkout)"
  fi
}

copy_or_fetch() {
  local rel="$1"
  local dest="$2"
  local local_path=""
  if [ -n "$SCRIPT_DIR" ] && [ -f "${SCRIPT_DIR}/${rel}" ]; then
    local_path="${SCRIPT_DIR}/${rel}"
  fi
  if [ -n "$local_path" ]; then
    info "installing ${rel} from checkout"
    cp "$local_path" "$dest"
  else
    info "downloading ${rel} (${BOOTSTRAP_REF})"
    fetch_file "$rel" "$dest" || die "failed to download ${rel} from ${REPO_RAW_BASE}/${BOOTSTRAP_REF}/"
  fi
  chmod +x "$dest" 2>/dev/null || true
}

# Only the helper CLI — not cargo build of the ERP.
copy_or_fetch "dist/rusterp-lib.sh" "${CLI_DIR}/rusterp-lib.sh"
copy_or_fetch "dist/rusterp" "${CLI_DIR}/rusterp"
chmod +x "${CLI_DIR}/rusterp"
# lib is sourced; ensure readable
chmod 644 "${CLI_DIR}/rusterp-lib.sh" 2>/dev/null || true

# Example unit (optional; not enabled)
if [ -n "$SCRIPT_DIR" ] && [ -f "${SCRIPT_DIR}/dist/rusterp-server.service.example" ]; then
  cp "${SCRIPT_DIR}/dist/rusterp-server.service.example" \
    "${HOME_DIR}/rusterp-server.service.example" 2>/dev/null || true
fi

# Wrapper: find lib via real CLI path; default RUSTERP_HOME to this bootstrap prefix.
cat >"${BIN_DIR}/rusterp" <<EOF
#!/usr/bin/env bash
export RUSTERP_HOME="\${RUSTERP_HOME:-${HOME_DIR}}"
exec "${CLI_DIR}/rusterp" "\$@"
EOF
chmod +x "${BIN_DIR}/rusterp"

info "installed rusterp → ${BIN_DIR}/rusterp"
info "CLI files under ${CLI_DIR}"
if ! echo ":$PATH:" | grep -q ":${BIN_DIR}:"; then
  info "note: ${BIN_DIR} is not on PATH; add it, e.g.:"
  info "  export PATH=\"${BIN_DIR}:\$PATH\""
fi
info "next: rusterp install core"
info "      (default git branch: dist; override with --branch <name>)"
info "this script does not install the ERP server — only the helper CLI"

echo "ok ${BIN_DIR}/rusterp"
