#!/usr/bin/env bash
#
# Quotey Quality Gates Script
# Runs comprehensive quality checks on the codebase
#
# Usage:
#   ./scripts/quality-gates.sh           # Run all gates
#   ./scripts/quality-gates.sh --help    # Show help
#   ./scripts/quality-gates.sh build     # Run only build gate
#   ./scripts/quality-gates.sh build fmt # Run build and fmt gates
#   ./scripts/quality-gates.sh --list    # List available gates
#
# Environment:
#   QUOTEY_SKIP_BUILD=1      # Skip build gate
#   QUOTEY_SKIP_FMT=1        # Skip fmt gate
#   QUOTEY_SKIP_CLIPPY=1     # Skip clippy gate
#   QUOTEY_SKIP_TESTS=1      # Skip tests gate
#   QUOTEY_SKIP_DENY=1       # Skip deny gate
#   QUOTEY_SKIP_DOC=1        # Skip doc gate
#   QUOTEY_PARALLEL=1        # Run independent gates in parallel
#   QUOTEY_VERBOSE=1         # Show verbose output

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

# Force workspace-local temp/target paths by default to avoid host-level /tmp pressure.
# Operators can override explicitly with QUOTEY_*_OVERRIDE vars when needed.
export TMPDIR="${QUOTEY_TMPDIR_OVERRIDE:-$ROOT_DIR/.tmp}"
export CARGO_TARGET_DIR="${QUOTEY_CARGO_TARGET_DIR_OVERRIDE:-$ROOT_DIR/.tmp-target}"
mkdir -p "$TMPDIR" "$CARGO_TARGET_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default: run all gates
RUN_ALL=true
GATES_TO_RUN=()

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --help|-h)
      echo "Quotey Quality Gates Script"
      echo ""
      echo "Usage: $0 [options] [gate...]"
      echo ""
      echo "Options:"
      echo "  --help, -h       Show this help message"
      echo "  --list          List available gates"
      echo "  --verbose, -v    Show verbose output"
      echo ""
      echo "Gates:"
      echo "  build    Build the workspace"
      echo "  fmt     Check code formatting"
      echo "  clippy  Run clippy lints"
      echo "  tests   Run all tests"
      echo "  deny    Run cargo-deny security checks"
      echo "  doc     Build documentation"
      echo ""
      echo "Environment Variables:"
      echo "  QUOTEY_SKIP_*=1    Skip specific gate (e.g., QUOTEY_SKIP_CLIPPY=1)"
      echo "  QUOTEY_PARALLEL=1  Run independent gates in parallel"
      echo "  QUOTEY_VERBOSE=1   Show verbose output"
      echo ""
      echo "Examples:"
      echo "  $0                  # Run all gates"
      echo "  $0 build fmt        # Run only build and fmt"
      echo "  QUOTEY_SKIP_CLIPPY=1 $0  # Run all except clippy"
      exit 0
      ;;
    --list)
      echo "Available gates: build fmt clippy tests deny doc"
      exit 0
      ;;
    --verbose|-v)
      export QUOTEY_VERBOSE=1
      shift
      ;;
    build|fmt|clippy|tests|deny|doc)
      RUN_ALL=false
      GATES_TO_RUN+=("$1")
      shift
      ;;
    *)
      echo "Unknown option: $1"
      echo "Use --help for usage information"
      exit 1
      ;;
  esac
done

log() {
  if [[ "${QUOTEY_VERBOSE:-0}" == "1" ]]; then
    printf '[quality-gates] %s\n' "$*"
  fi
}

log_start() {
  printf '[quality-gates] %sSTART%s: %s\n' "$YELLOW" "$NC" "$1"
}

log_pass() {
  printf '[quality-gates] %sPASS%s: %s\n' "$GREEN" "$NC" "$1"
}

log_fail() {
  printf '[quality-gates] %sFAIL%s: %s\n' "$RED" "$NC" "$1"
}

log_error() {
  printf '[quality-gates] %sERROR%s: %s\n' "$RED" "$NC" "$1"
}

run_gate() {
  local gate_name="$1"
  shift

  log_start "${gate_name}"
  if ! "$@"; then
    log_fail "${gate_name}: fix the reported issue and rerun scripts/quality-gates.sh"
    return 1
  fi
  log_pass "${gate_name}"
}

# Check for required tools
if ! command -v cargo >/dev/null 2>&1; then
  log_error "cargo is not installed or not in PATH"
  exit 2
fi

if ! cargo deny --version >/dev/null 2>&1; then
  log_error "cargo-deny is required. Install with: cargo install cargo-deny"
  exit 2
fi

# Define gate functions
run_build() {
  run_gate "build" cargo build --workspace
}

run_fmt() {
  run_gate "fmt" cargo fmt --all -- --check
}

run_clippy() {
  if cargo lint --help >/dev/null 2>&1; then
    run_gate "clippy" cargo lint
  else
    run_gate "clippy" cargo clippy --workspace --all-targets -- -D warnings
  fi
}

run_tests() {
  run_gate "tests" cargo test --workspace
}

run_deny() {
  run_gate "deny" cargo deny check
}

run_doc() {
  run_gate "doc" cargo doc --workspace --no-deps
}

# Determine which gates to run
should_run_gate() {
  local gate="$1"
  local env_var="QUOTEY_SKIP_${gate^^}"
  
  # Check if explicitly skipped
  if [[ "${!env_var:-0}" == "1" ]]; then
    log "Skipping ${gate} (QUOTEY_SKIP_${gate^^}=1)"
    return 1
  fi
  
  # Check if in user-specified list
  if [[ "$RUN_ALL" == "false" ]]; then
    for g in "${GATES_TO_RUN[@]}"; do
      if [[ "$g" == "$gate" ]]; then
        return 0
      fi
    done
    return 1
  fi
  
  return 0
}

# Run gates
FAILED=0

# Build dependencies first (needed for most gates)
if should_run_gate "build" || should_run_gate "clippy" || should_run_gate "tests"; then
  run_build || FAILED=1
fi

# Format check (fast, no dependencies)
if should_run_gate "fmt"; then
  run_fmt || FAILED=1
fi

# Clippy (needs build)
if should_run_gate "clippy"; then
  run_clippy || FAILED=1
fi

# Tests (needs build)
if should_run_gate "tests"; then
  run_tests || FAILED=1
fi

# Deny (fast, standalone)
if should_run_gate "deny"; then
  run_deny || FAILED=1
fi

# Doc (fast, standalone)
if should_run_gate "doc"; then
  run_doc || FAILED=1
fi

if [[ $FAILED -eq 1 ]]; then
  log_error "One or more quality gates failed"
  exit 1
fi

log_pass "ALL QUALITY GATES PASSED"
