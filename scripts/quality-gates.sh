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
#   QUOTEY_SKIP_QA=1         # Skip QA threshold gate
#   QUOTEY_SKIP_DENY=1       # Skip deny gate
#   QUOTEY_SKIP_DOC=1        # Skip doc gate
#   QUOTEY_PARALLEL=1        # Run independent gates in parallel
#   QUOTEY_VERBOSE=1         # Show verbose output
#   QUOTEY_FAIL_FAST=1       # Stop at first failing gate (default: enabled)
#   QUOTEY_THRESHOLD_REAL_DB_PCT=20                 # Minimum real-DB test ratio
#   QUOTEY_THRESHOLD_CRITICAL_PATH_GAPS_MAX=0       # Maximum open P0/P1 gaps
#   QUOTEY_THRESHOLD_E2E_PASS_PCT=100               # Minimum E2E pass rate
#   QUOTEY_THRESHOLD_LOG_VALIDATOR_CASES_MIN=5      # Minimum log-validator cases

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
FAIL_FAST="${QUOTEY_FAIL_FAST:-1}"

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
      echo "  qa      Enforce QA threshold checks"
      echo "  deny    Run cargo-deny security checks"
      echo "  doc     Build documentation"
      echo ""
      echo "Environment Variables:"
      echo "  QUOTEY_SKIP_*=1    Skip specific gate (e.g., QUOTEY_SKIP_CLIPPY=1)"
      echo "  QUOTEY_PARALLEL=1  Run independent gates in parallel"
      echo "  QUOTEY_VERBOSE=1   Show verbose output"
      echo "  QUOTEY_FAIL_FAST=1 Stop at first failure (default: 1)"
      echo "  QUOTEY_THRESHOLD_REAL_DB_PCT=20"
      echo "  QUOTEY_THRESHOLD_CRITICAL_PATH_GAPS_MAX=0"
      echo "  QUOTEY_THRESHOLD_E2E_PASS_PCT=100"
      echo "  QUOTEY_THRESHOLD_LOG_VALIDATOR_CASES_MIN=5"
      echo ""
      echo "Examples:"
      echo "  $0                  # Run all gates"
      echo "  $0 build fmt        # Run only build and fmt"
      echo "  QUOTEY_SKIP_CLIPPY=1 $0  # Run all except clippy"
      exit 0
      ;;
    --list)
      echo "Available gates: build fmt clippy tests qa deny doc"
      exit 0
      ;;
    --verbose|-v)
      export QUOTEY_VERBOSE=1
      shift
      ;;
    build|fmt|clippy|tests|qa|deny|doc)
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
    if [[ "$FAIL_FAST" == "1" ]]; then
      log_error "Fail-fast enabled; stopping at first gate failure"
      exit 1
    fi
    return 1
  fi
  log_pass "${gate_name}"
}

# Check for required tools
if ! command -v cargo >/dev/null 2>&1; then
  log_error "cargo is not installed or not in PATH"
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

qa_threshold_checks() {
  local inventory_json
  inventory_json="$(mktemp "$TMPDIR/qa-thresholds.inventory.XXXXXX.json")"

  if ! bash scripts/test_inventory.sh --json >"$inventory_json"; then
    echo "qa: test inventory generation failed"
    rm -f "$inventory_json"
    return 1
  fi

  if ! command -v jq >/dev/null 2>&1; then
    echo "qa: jq is required for threshold checks"
    rm -f "$inventory_json"
    return 1
  fi

  local total_tests real_db_tests real_db_pct
  total_tests="$(jq -r '.summary.total_tests // 0' "$inventory_json")"
  real_db_tests="$(jq -r '.summary.real_db_tests // 0' "$inventory_json")"

  if [[ "$total_tests" -le 0 ]]; then
    echo "qa: total_tests must be > 0 (got $total_tests)"
    rm -f "$inventory_json"
    return 1
  fi

  real_db_pct=$((real_db_tests * 100 / total_tests))
  local min_real_db_pct
  min_real_db_pct="${QUOTEY_THRESHOLD_REAL_DB_PCT:-20}"
  echo "qa: real_db_ratio=${real_db_pct}% (threshold >= ${min_real_db_pct}%)"
  if [[ "$real_db_pct" -lt "$min_real_db_pct" ]]; then
    echo "qa: real_db_ratio threshold violated"
    rm -f "$inventory_json"
    return 1
  fi
  rm -f "$inventory_json"

  if [[ ! -f ".planning/qa/CRITICAL_PATH_MATRIX.md" ]]; then
    echo "qa: missing .planning/qa/CRITICAL_PATH_MATRIX.md"
    return 1
  fi

  local critical_gap_count max_critical_gaps
  critical_gap_count="$(
    awk -F'|' '
      function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
      /^\| G-[0-9]+/ {
        pri=toupper(trim($4))
        status=toupper(trim($6))
        if ((pri == "P0" || pri == "P1") && status !~ /CLOSED/) {
          c++
        }
      }
      END { print c+0 }
    ' .planning/qa/CRITICAL_PATH_MATRIX.md
  )"
  max_critical_gaps="${QUOTEY_THRESHOLD_CRITICAL_PATH_GAPS_MAX:-0}"
  echo "qa: open_p0_p1_critical_gaps=${critical_gap_count} (threshold <= ${max_critical_gaps})"
  if [[ "$critical_gap_count" -gt "$max_critical_gaps" ]]; then
    echo "qa: critical-path gap threshold violated"
    return 1
  fi

  local e2e_log e2e_summary e2e_passed e2e_failed e2e_total e2e_pass_pct min_e2e_pass_pct
  e2e_log="$(mktemp "$TMPDIR/qa-thresholds.e2e.XXXXXX.log")"
  if ! cargo test -p quotey-db --test e2e_scenarios -- --nocapture 2>&1 | tee "$e2e_log"; then
    echo "qa: e2e_scenarios suite failed"
    rm -f "$e2e_log"
    return 1
  fi
  e2e_summary="$(grep -E 'test result: (ok|FAILED)\.' "$e2e_log" | tail -n 1 || true)"
  rm -f "$e2e_log"
  if [[ -z "$e2e_summary" ]]; then
    echo "qa: unable to parse e2e summary line"
    return 1
  fi

  e2e_passed="$(printf '%s\n' "$e2e_summary" | sed -nE 's/.* ([0-9]+) passed.*/\1/p')"
  e2e_failed="$(printf '%s\n' "$e2e_summary" | sed -nE 's/.*; ([0-9]+) failed.*/\1/p')"
  e2e_passed="${e2e_passed:-0}"
  e2e_failed="${e2e_failed:-0}"
  e2e_total=$((e2e_passed + e2e_failed))

  if [[ "$e2e_total" -le 0 ]]; then
    echo "qa: e2e test total must be > 0"
    return 1
  fi

  e2e_pass_pct=$((e2e_passed * 100 / e2e_total))
  min_e2e_pass_pct="${QUOTEY_THRESHOLD_E2E_PASS_PCT:-100}"
  echo "qa: e2e_pass_rate=${e2e_pass_pct}% (threshold >= ${min_e2e_pass_pct}%)"
  if [[ "$e2e_pass_pct" -lt "$min_e2e_pass_pct" ]]; then
    echo "qa: e2e pass-rate threshold violated"
    return 1
  fi

  local log_validator_cases min_log_validator_cases
  log_validator_cases="$(grep -c 'validate_e2e_log_records(&' crates/db/tests/e2e_scenarios.rs 2>/dev/null || true)"
  log_validator_cases="${log_validator_cases:-0}"
  min_log_validator_cases="${QUOTEY_THRESHOLD_LOG_VALIDATOR_CASES_MIN:-5}"
  echo "qa: log_validator_cases=${log_validator_cases} (threshold >= ${min_log_validator_cases})"
  if [[ "$log_validator_cases" -lt "$min_log_validator_cases" ]]; then
    echo "qa: log-validator coverage threshold violated"
    return 1
  fi

  return 0
}

run_qa() {
  run_gate "qa" qa_threshold_checks
}

run_deny() {
  if ! cargo deny --version >/dev/null 2>&1; then
    log_error "cargo-deny is required for deny gate. Install with: cargo install cargo-deny"
    return 1
  fi
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

# QA thresholds (depends on test inventory + E2E suite)
if should_run_gate "qa"; then
  run_qa || FAILED=1
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
