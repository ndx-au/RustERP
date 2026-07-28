#!/usr/bin/env bash
# Copyright 2026 NDX Pty Ltd and contributors
# SPDX-License-Identifier: Apache-2.0
#
# Offline checks for Phase 3 distribution scripts (no network git install).

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="${ROOT}/dist"
LIB="${DIST}/rusterp-lib.sh"
PASS=0
FAIL=0

assert_eq() {
  local name="$1" got="$2" want="$3"
  if [ "$got" = "$want" ]; then
    echo "ok - ${name}"
    PASS=$((PASS + 1))
  else
    echo "not ok - ${name}: got '${got}' want '${want}'" >&2
    FAIL=$((FAIL + 1))
  fi
}

assert_ok() {
  local name="$1"
  shift
  if "$@"; then
    echo "ok - ${name}"
    PASS=$((PASS + 1))
  else
    echo "not ok - ${name}" >&2
    FAIL=$((FAIL + 1))
  fi
}

assert_fail() {
  local name="$1"
  shift
  if "$@"; then
    echo "not ok - ${name} (expected failure)" >&2
    FAIL=$((FAIL + 1))
  else
    echo "ok - ${name}"
    PASS=$((PASS + 1))
  fi
}

echo "# syntax"
assert_ok "bash -n install.sh" bash -n "${ROOT}/install.sh"
assert_ok "bash -n dist/rusterp" bash -n "${DIST}/rusterp"
assert_ok "bash -n dist/rusterp-lib.sh" bash -n "${LIB}"
assert_ok "bash -n dist/test-dist.sh" bash -n "${DIST}/test-dist.sh"

# shellcheck source=rusterp-lib.sh
. "$LIB"

echo "# defaults"
assert_eq "default core branch" "$RUSTERP_DEFAULT_CORE_BRANCH" "dist"

echo "# parse install core args"
rusterp_parse_install_core_args
assert_eq "omit --branch → dist" "$RUSTERP_PARSE_BRANCH" "dist"

rusterp_parse_install_core_args --branch release/1.0
assert_eq "--branch value" "$RUSTERP_PARSE_BRANCH" "release/1.0"

rusterp_parse_install_core_args --branch=feature/x
assert_eq "--branch= value" "$RUSTERP_PARSE_BRANCH" "feature/x"

assert_fail "missing --branch value" rusterp_parse_install_core_args --branch
assert_fail "unknown flag" rusterp_parse_install_core_args --nope

echo "# require_cmd"
assert_ok "require_cmd finds bash" rusterp_require_cmd bash "hint"
assert_fail "require_cmd missing tool" rusterp_require_cmd definitely-not-a-real-cmd-xyz "install it"

echo "# ui-wasm rejected by CLI help path (grep)"
assert_ok "rusterp mentions core only in help" \
  bash -c "grep -q 'Phase 3 supports: core' '${DIST}/rusterp'"

echo "# install.sh is CLI-only (does not cargo build)"
assert_ok "install.sh does not invoke cargo build" \
  bash -c "! grep -E '^[^#]*cargo +build' '${ROOT}/install.sh'"

echo
echo "1..$((PASS + FAIL))"
echo "# passed=${PASS} failed=${FAIL}"
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
exit 0
