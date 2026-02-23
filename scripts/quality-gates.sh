#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

export TMPDIR="${TMPDIR:-$ROOT_DIR/.tmp}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
mkdir -p "$TMPDIR" "$CARGO_TARGET_DIR"

log() {
  printf '[quality-gates] %s\n' "$*"
}

run_gate() {
  local gate_name="$1"
  shift

  log "START ${gate_name}: $*"
  if ! "$@"; then
    log "FAIL ${gate_name}: fix the reported issue and rerun scripts/quality-gates.sh"
    return 1
  fi
  log "PASS ${gate_name}"
}

if ! command -v cargo >/dev/null 2>&1; then
  log "ERROR cargo is not installed or not in PATH"
  exit 2
fi

if ! cargo deny --version >/dev/null 2>&1; then
  log "ERROR cargo-deny is required. Install with: cargo install cargo-deny"
  exit 2
fi

run_gate "fmt" cargo fmt --all -- --check

if cargo lint --help >/dev/null 2>&1; then
  run_gate "lint" cargo lint
else
  run_gate "lint" cargo clippy --workspace --all-targets -- -D warnings
fi

run_gate "tests" cargo test --workspace
run_gate "deny" cargo deny check

log "ALL QUALITY GATES PASSED"
