#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Force workspace-local temp/target paths by default to avoid host-level /tmp pressure.
# Operators can override explicitly with QUOTEY_*_OVERRIDE vars when needed.
export TMPDIR="${QUOTEY_TMPDIR_OVERRIDE:-$ROOT_DIR/.tmp}"
export CARGO_TARGET_DIR="${QUOTEY_CARGO_TARGET_DIR_OVERRIDE:-$ROOT_DIR/.tmp-target}"
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

run_gate "build" cargo build --workspace
run_gate "fmt" cargo fmt --all -- --check

if cargo lint --help >/dev/null 2>&1; then
  run_gate "clippy" cargo lint
else
  run_gate "clippy" cargo clippy --workspace --all-targets -- -D warnings
fi

run_gate "tests" cargo test --workspace
run_gate "deny" cargo deny check
run_gate "doc" cargo doc --workspace --no-deps

log "ALL QUALITY GATES PASSED"
