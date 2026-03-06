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
#   QUOTEY_QA_REPORT_DIR=.planning/qa/reports       # QA dashboard/report artifact output dir

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
RUN_ID="$(date -u +%Y%m%dT%H%M%SZ)"
RUN_STARTED_AT_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
REPORT_DIR="${QUOTEY_QA_REPORT_DIR:-$ROOT_DIR/.planning/qa/reports}"
REPORT_ENABLED=0
REPORT_WRITTEN=0

# Per-gate execution results for dashboard/report artifacts
declare -a GATE_RESULT_NAME=()
declare -a GATE_RESULT_STATUS=()
declare -a GATE_RESULT_DURATION_S=()
declare -a GATE_RESULT_NOTES=()

# QA threshold metrics captured from qa gate execution
QA_TOTAL_TESTS=""
QA_REAL_DB_TESTS=""
QA_REAL_DB_PCT=""
QA_REAL_DB_THRESHOLD=""
QA_CRITICAL_GAP_COUNT=""
QA_CRITICAL_GAP_THRESHOLD=""
QA_E2E_PASSED=""
QA_E2E_FAILED=""
QA_E2E_TOTAL=""
QA_E2E_PASS_PCT=""
QA_E2E_THRESHOLD=""
QA_E2E_REASON="qa gate not executed"
QA_LOG_VALIDATOR_CASES=""
QA_LOG_VALIDATOR_THRESHOLD=""

# Derived report fields populated before artifact write
COVERAGE_CURRENT_TOTAL=0
COVERAGE_CURRENT_REAL_DB=0
COVERAGE_CURRENT_INMEMORY=0
COVERAGE_CURRENT_PURE_UNIT=0
COVERAGE_BASELINE_TOTAL=0
COVERAGE_BASELINE_REAL_DB=0
COVERAGE_BASELINE_INMEMORY=0
COVERAGE_BASELINE_PURE_UNIT=0
COVERAGE_DELTA_TOTAL=0
COVERAGE_DELTA_REAL_DB=0
COVERAGE_DELTA_INMEMORY=0
COVERAGE_DELTA_PURE_UNIT=0
FLAKY_SIGNAL_COUNT=0
FLAKY_SIGNAL_PREVIEW="none"
EXCEPTION_ACTIVE_COUNT=0
EXCEPTION_EXPIRED_COUNT=0
declare -a EXCEPTION_ID=()
declare -a EXCEPTION_PATH=()
declare -a EXCEPTION_OWNER=()
declare -a EXCEPTION_EXPIRY=()
declare -a EXCEPTION_EXPIRED=()
declare -a EXCEPTION_FOLLOWUP=()

append_gate_result() {
  local name="$1"
  local status="$2"
  local duration_s="$3"
  local notes="$4"

  GATE_RESULT_NAME+=("$name")
  GATE_RESULT_STATUS+=("$status")
  GATE_RESULT_DURATION_S+=("$duration_s")
  GATE_RESULT_NOTES+=("$notes")
}

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

json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/\\r}"
  value="${value//$'\t'/\\t}"
  printf '%s' "$value"
}

run_gate() {
  local gate_name="$1"
  shift

  local started_at duration_s
  started_at="$(date +%s)"
  log_start "${gate_name}"
  if ! "$@"; then
    duration_s=$(( $(date +%s) - started_at ))
    append_gate_result \
      "$gate_name" \
      "FAIL" \
      "$duration_s" \
      "See gate output for failure details"
    log_fail "${gate_name}: fix the reported issue and rerun scripts/quality-gates.sh"
    if [[ "$FAIL_FAST" == "1" ]]; then
      log_error "Fail-fast enabled; stopping at first gate failure"
      exit 1
    fi
    return 1
  fi
  duration_s=$(( $(date +%s) - started_at ))
  append_gate_result "$gate_name" "PASS" "$duration_s" ""
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
  local qa_failed=0
  local qa_reason_parts=()

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
  QA_TOTAL_TESTS="$total_tests"
  QA_REAL_DB_TESTS="$real_db_tests"

  if [[ "$total_tests" -le 0 ]]; then
    echo "qa: total_tests must be > 0 (got $total_tests)"
    rm -f "$inventory_json"
    return 1
  fi

  real_db_pct=$((real_db_tests * 100 / total_tests))
  local min_real_db_pct
  min_real_db_pct="${QUOTEY_THRESHOLD_REAL_DB_PCT:-20}"
  QA_REAL_DB_PCT="$real_db_pct"
  QA_REAL_DB_THRESHOLD="$min_real_db_pct"
  echo "qa: real_db_ratio=${real_db_pct}% (threshold >= ${min_real_db_pct}%)"
  if [[ "$real_db_pct" -lt "$min_real_db_pct" ]]; then
    echo "qa: real_db_ratio threshold violated"
    qa_reason_parts+=("real_db_ratio")
    qa_failed=1
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
  QA_CRITICAL_GAP_COUNT="$critical_gap_count"
  QA_CRITICAL_GAP_THRESHOLD="$max_critical_gaps"
  echo "qa: open_p0_p1_critical_gaps=${critical_gap_count} (threshold <= ${max_critical_gaps})"
  if [[ "$critical_gap_count" -gt "$max_critical_gaps" ]]; then
    echo "qa: critical-path gap threshold violated"
    qa_reason_parts+=("critical_path_gaps")
    qa_failed=1
  fi

  local e2e_log e2e_summary e2e_passed e2e_failed e2e_total e2e_pass_pct min_e2e_pass_pct
  e2e_log="$(mktemp "$TMPDIR/qa-thresholds.e2e.XXXXXX.log")"
  if ! cargo test -p quotey-db --test e2e_scenarios -- --nocapture 2>&1 | tee "$e2e_log"; then
    echo "qa: e2e_scenarios suite failed"
    qa_reason_parts+=("e2e_suite_failed")
    qa_failed=1
  fi
  e2e_summary="$(grep -E 'test result: (ok|FAILED)\.' "$e2e_log" | tail -n 1 || true)"
  if [[ -z "$e2e_summary" ]]; then
    echo "qa: unable to parse e2e summary line"
    qa_reason_parts+=("e2e_summary_missing")
    qa_failed=1
    e2e_passed=0
    e2e_failed=0
    e2e_total=0
    e2e_pass_pct=0
    QA_E2E_REASON="Unable to parse e2e_scenarios summary line"
  else
    e2e_passed="$(printf '%s\n' "$e2e_summary" | sed -nE 's/.* ([0-9]+) passed.*/\1/p')"
    e2e_failed="$(printf '%s\n' "$e2e_summary" | sed -nE 's/.*; ([0-9]+) failed.*/\1/p')"
    e2e_passed="${e2e_passed:-0}"
    e2e_failed="${e2e_failed:-0}"
    e2e_total=$((e2e_passed + e2e_failed))

    if [[ "$e2e_total" -le 0 ]]; then
      echo "qa: e2e test total must be > 0"
      qa_reason_parts+=("e2e_total_zero")
      qa_failed=1
      e2e_pass_pct=0
    else
      e2e_pass_pct=$((e2e_passed * 100 / e2e_total))
    fi

    local e2e_failed_preview
    e2e_failed_preview="$(
      grep -E '^test .+ \.\.\. FAILED$' "$e2e_log" \
        | sed -E 's/^test (.+) \.\.\. FAILED$/\1/' \
        | head -n 5 \
        | paste -sd ', ' -
    )"
    if [[ "$e2e_failed" -gt 0 ]]; then
      QA_E2E_REASON="${e2e_failed_preview:-One or more scenarios failed}"
    else
      QA_E2E_REASON="All e2e_scenarios cases passed"
    fi
  fi
  rm -f "$e2e_log"

  min_e2e_pass_pct="${QUOTEY_THRESHOLD_E2E_PASS_PCT:-100}"
  QA_E2E_PASSED="$e2e_passed"
  QA_E2E_FAILED="$e2e_failed"
  QA_E2E_TOTAL="$e2e_total"
  QA_E2E_PASS_PCT="$e2e_pass_pct"
  QA_E2E_THRESHOLD="$min_e2e_pass_pct"
  echo "qa: e2e_pass_rate=${e2e_pass_pct}% (threshold >= ${min_e2e_pass_pct}%)"
  if [[ "$e2e_pass_pct" -lt "$min_e2e_pass_pct" ]]; then
    echo "qa: e2e pass-rate threshold violated"
    qa_reason_parts+=("e2e_pass_rate")
    qa_failed=1
  fi

  local log_validator_cases min_log_validator_cases
  log_validator_cases="$(grep -c 'validate_e2e_log_records(&' crates/db/tests/e2e_scenarios.rs 2>/dev/null || true)"
  log_validator_cases="${log_validator_cases:-0}"
  min_log_validator_cases="${QUOTEY_THRESHOLD_LOG_VALIDATOR_CASES_MIN:-5}"
  QA_LOG_VALIDATOR_CASES="$log_validator_cases"
  QA_LOG_VALIDATOR_THRESHOLD="$min_log_validator_cases"
  echo "qa: log_validator_cases=${log_validator_cases} (threshold >= ${min_log_validator_cases})"
  if [[ "$log_validator_cases" -lt "$min_log_validator_cases" ]]; then
    echo "qa: log-validator coverage threshold violated"
    qa_reason_parts+=("log_validator_cases")
    qa_failed=1
  fi

  if [[ "${#qa_reason_parts[@]}" -gt 0 ]]; then
    QA_E2E_REASON="${QA_E2E_REASON}; threshold_violations=$(IFS=,; echo "${qa_reason_parts[*]}")"
  fi

  return "$qa_failed"
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

load_current_coverage_metrics() {
  local inventory_json
  inventory_json="$(mktemp "$TMPDIR/qa-report.inventory.XXXXXX.json")"

  if ! command -v jq >/dev/null 2>&1; then
    COVERAGE_CURRENT_TOTAL=0
    COVERAGE_CURRENT_REAL_DB=0
    COVERAGE_CURRENT_INMEMORY=0
    COVERAGE_CURRENT_PURE_UNIT=0
    rm -f "$inventory_json"
    return
  fi

  if ! bash scripts/test_inventory.sh --json >"$inventory_json" 2>/dev/null; then
    COVERAGE_CURRENT_TOTAL=0
    COVERAGE_CURRENT_REAL_DB=0
    COVERAGE_CURRENT_INMEMORY=0
    COVERAGE_CURRENT_PURE_UNIT=0
    rm -f "$inventory_json"
    return
  fi

  COVERAGE_CURRENT_TOTAL="$(jq -r '.summary.total_tests // 0' "$inventory_json")"
  COVERAGE_CURRENT_REAL_DB="$(jq -r '.summary.real_db_tests // 0' "$inventory_json")"
  COVERAGE_CURRENT_INMEMORY="$(jq -r '.summary.inmemory_tests // 0' "$inventory_json")"
  COVERAGE_CURRENT_PURE_UNIT="$(jq -r '.summary.pure_unit_tests // 0' "$inventory_json")"
  rm -f "$inventory_json"
}

load_baseline_coverage_metrics() {
  local baseline_file="$ROOT_DIR/.planning/qa/COVERAGE_BASELINE.md"

  COVERAGE_BASELINE_REAL_DB=0
  COVERAGE_BASELINE_INMEMORY=0
  COVERAGE_BASELINE_PURE_UNIT=0
  COVERAGE_BASELINE_TOTAL=0

  if [[ ! -f "$baseline_file" ]]; then
    return
  fi

  COVERAGE_BASELINE_REAL_DB="$(
    awk -F'|' '
      /^\| Real-DB \(SQLite \+ migrations\)/ {
        gsub(/[^0-9]/, "", $3);
        if ($3 != "") { print $3; exit }
      }
    ' "$baseline_file"
  )"
  COVERAGE_BASELINE_INMEMORY="$(
    awk -F'|' '
      /^\| InMemory\/Fake implementations/ {
        gsub(/[^0-9]/, "", $3);
        if ($3 != "") { print $3; exit }
      }
    ' "$baseline_file"
  )"
  COVERAGE_BASELINE_PURE_UNIT="$(
    awk -F'|' '
      /^\| Pure unit tests/ {
        gsub(/[^0-9]/, "", $3);
        if ($3 != "") { print $3; exit }
      }
    ' "$baseline_file"
  )"

  COVERAGE_BASELINE_REAL_DB="${COVERAGE_BASELINE_REAL_DB:-0}"
  COVERAGE_BASELINE_INMEMORY="${COVERAGE_BASELINE_INMEMORY:-0}"
  COVERAGE_BASELINE_PURE_UNIT="${COVERAGE_BASELINE_PURE_UNIT:-0}"
  COVERAGE_BASELINE_TOTAL=$((COVERAGE_BASELINE_REAL_DB + COVERAGE_BASELINE_INMEMORY + COVERAGE_BASELINE_PURE_UNIT))
}

load_flaky_signals() {
  local latest_run diff_summary
  latest_run="$(find "$ROOT_DIR/target/e2e-artifacts" -maxdepth 1 -mindepth 1 -type d -name 'run-*' 2>/dev/null | sort | tail -n 1)"
  if [[ -z "$latest_run" ]]; then
    FLAKY_SIGNAL_COUNT=0
    FLAKY_SIGNAL_PREVIEW="no e2e artifact runs found"
    return
  fi

  diff_summary="$latest_run/DIFF_SUMMARY.json"
  if [[ ! -f "$diff_summary" ]]; then
    FLAKY_SIGNAL_COUNT=0
    FLAKY_SIGNAL_PREVIEW="no replay diff found for latest run $(basename "$latest_run")"
    return
  fi

  if ! command -v jq >/dev/null 2>&1; then
    FLAKY_SIGNAL_COUNT=0
    FLAKY_SIGNAL_PREVIEW="jq missing; replay diff summary not parsed"
    return
  fi

  local flipped_to_fail flipped_to_pass timing_regressions
  flipped_to_fail="$(jq -r '.counts.flipped_to_fail // 0' "$diff_summary")"
  flipped_to_pass="$(jq -r '.counts.flipped_to_pass // 0' "$diff_summary")"
  timing_regressions="$(jq -r '.counts.timing_regressions // 0' "$diff_summary")"

  FLAKY_SIGNAL_COUNT=$((flipped_to_fail + flipped_to_pass))
  FLAKY_SIGNAL_PREVIEW="run=$(basename "$latest_run"), flipped_to_fail=${flipped_to_fail}, flipped_to_pass=${flipped_to_pass}, timing_regressions=${timing_regressions}"
}

load_exception_inventory() {
  local policy_file="$ROOT_DIR/.planning/qa/QA_POLICY.md"
  local today
  today="$(date -u +%Y-%m-%d)"

  EXCEPTION_ACTIVE_COUNT=0
  EXCEPTION_EXPIRED_COUNT=0
  EXCEPTION_ID=()
  EXCEPTION_PATH=()
  EXCEPTION_OWNER=()
  EXCEPTION_EXPIRY=()
  EXCEPTION_EXPIRED=()
  EXCEPTION_FOLLOWUP=()

  if [[ ! -f "$policy_file" ]]; then
    return
  fi

  while IFS='|' read -r _ raw_id raw_path _ raw_owner raw_expiry _ raw_followup _; do
    local id path owner expiry followup expired_flag
    id="$(echo "$raw_id" | xargs)"
    [[ ! "$id" =~ ^EX-[0-9]+$ ]] && continue

    path="$(echo "$raw_path" | xargs)"
    owner="$(echo "$raw_owner" | xargs)"
    expiry="$(echo "$raw_expiry" | xargs)"
    followup="$(echo "$raw_followup" | xargs)"
    expired_flag="false"

    if [[ "$expiry" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] && [[ "$expiry" < "$today" ]]; then
      expired_flag="true"
      EXCEPTION_EXPIRED_COUNT=$((EXCEPTION_EXPIRED_COUNT + 1))
    fi

    EXCEPTION_ACTIVE_COUNT=$((EXCEPTION_ACTIVE_COUNT + 1))
    EXCEPTION_ID+=("$id")
    EXCEPTION_PATH+=("$path")
    EXCEPTION_OWNER+=("${owner:---}")
    EXCEPTION_EXPIRY+=("$expiry")
    EXCEPTION_EXPIRED+=("$expired_flag")
    EXCEPTION_FOLLOWUP+=("${followup:---}")
  done < <(awk -F'|' '/^\| EX-[0-9]+/{print $0}' "$policy_file")
}

write_report_artifacts() {
  local exit_code="$1"
  local run_finished_at_utc overall_status
  local report_run_dir report_json report_md latest_pointer_file
  local qa_total qa_real_db qa_real_pct qa_real_threshold
  local qa_critical qa_critical_threshold qa_e2e_passed qa_e2e_failed qa_e2e_total qa_e2e_pct
  local qa_e2e_threshold qa_log_cases qa_log_threshold qa_reason
  local idx last_index gate_count exception_count

  if [[ "$REPORT_ENABLED" != "1" ]] || [[ "$REPORT_WRITTEN" == "1" ]]; then
    return
  fi
  REPORT_WRITTEN=1

  mkdir -p "$REPORT_DIR"
  load_current_coverage_metrics
  load_baseline_coverage_metrics
  load_flaky_signals
  load_exception_inventory

  COVERAGE_DELTA_TOTAL=$((COVERAGE_CURRENT_TOTAL - COVERAGE_BASELINE_TOTAL))
  COVERAGE_DELTA_REAL_DB=$((COVERAGE_CURRENT_REAL_DB - COVERAGE_BASELINE_REAL_DB))
  COVERAGE_DELTA_INMEMORY=$((COVERAGE_CURRENT_INMEMORY - COVERAGE_BASELINE_INMEMORY))
  COVERAGE_DELTA_PURE_UNIT=$((COVERAGE_CURRENT_PURE_UNIT - COVERAGE_BASELINE_PURE_UNIT))

  run_finished_at_utc="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  if [[ "$exit_code" -eq 0 ]]; then
    overall_status="PASS"
  else
    overall_status="FAIL"
  fi

  report_run_dir="$REPORT_DIR/run-$RUN_ID"
  report_json="$report_run_dir/QUALITY_GATE_SUMMARY.json"
  report_md="$report_run_dir/QUALITY_GATE_SUMMARY.md"
  latest_pointer_file="$REPORT_DIR/LATEST_RUN_ID"
  mkdir -p "$report_run_dir"

  qa_total="${QA_TOTAL_TESTS:-0}"
  qa_real_db="${QA_REAL_DB_TESTS:-0}"
  qa_real_pct="${QA_REAL_DB_PCT:-0}"
  qa_real_threshold="${QA_REAL_DB_THRESHOLD:-${QUOTEY_THRESHOLD_REAL_DB_PCT:-20}}"
  qa_critical="${QA_CRITICAL_GAP_COUNT:-0}"
  qa_critical_threshold="${QA_CRITICAL_GAP_THRESHOLD:-${QUOTEY_THRESHOLD_CRITICAL_PATH_GAPS_MAX:-0}}"
  qa_e2e_passed="${QA_E2E_PASSED:-0}"
  qa_e2e_failed="${QA_E2E_FAILED:-0}"
  qa_e2e_total="${QA_E2E_TOTAL:-0}"
  qa_e2e_pct="${QA_E2E_PASS_PCT:-0}"
  qa_e2e_threshold="${QA_E2E_THRESHOLD:-${QUOTEY_THRESHOLD_E2E_PASS_PCT:-100}}"
  qa_log_cases="${QA_LOG_VALIDATOR_CASES:-0}"
  qa_log_threshold="${QA_LOG_VALIDATOR_THRESHOLD:-${QUOTEY_THRESHOLD_LOG_VALIDATOR_CASES_MIN:-5}}"
  qa_reason="${QA_E2E_REASON:-qa gate not executed}"

  gate_count="${#GATE_RESULT_NAME[@]}"
  exception_count="${#EXCEPTION_ID[@]}"

  {
    echo "{"
    printf '  "run_id": "%s",\n' "$(json_escape "$RUN_ID")"
    printf '  "started_at_utc": "%s",\n' "$(json_escape "$RUN_STARTED_AT_UTC")"
    printf '  "finished_at_utc": "%s",\n' "$(json_escape "$run_finished_at_utc")"
    printf '  "overall_status": "%s",\n' "$(json_escape "$overall_status")"
    printf '  "exit_code": %d,\n' "$exit_code"
    printf '  "report_dir": "%s",\n' "$(json_escape "$report_run_dir")"
    echo '  "gates": ['
    if [[ "$gate_count" -gt 0 ]]; then
      last_index=$((gate_count - 1))
      for idx in "${!GATE_RESULT_NAME[@]}"; do
        printf '    {"name":"%s","status":"%s","duration_seconds":%s,"notes":"%s"}' \
          "$(json_escape "${GATE_RESULT_NAME[$idx]}")" \
          "$(json_escape "${GATE_RESULT_STATUS[$idx]}")" \
          "${GATE_RESULT_DURATION_S[$idx]:-0}" \
          "$(json_escape "${GATE_RESULT_NOTES[$idx]:-}")"
        if [[ "$idx" -lt "$last_index" ]]; then
          echo ","
        else
          echo
        fi
      done
    fi
    echo "  ],"
    echo '  "qa_thresholds": {'
    printf '    "total_tests": %s,\n' "$qa_total"
    printf '    "real_db_tests": %s,\n' "$qa_real_db"
    printf '    "real_db_pct": %s,\n' "$qa_real_pct"
    printf '    "real_db_threshold_pct": %s,\n' "$qa_real_threshold"
    printf '    "critical_gap_count": %s,\n' "$qa_critical"
    printf '    "critical_gap_threshold_max": %s,\n' "$qa_critical_threshold"
    printf '    "e2e_passed": %s,\n' "$qa_e2e_passed"
    printf '    "e2e_failed": %s,\n' "$qa_e2e_failed"
    printf '    "e2e_total": %s,\n' "$qa_e2e_total"
    printf '    "e2e_pass_pct": %s,\n' "$qa_e2e_pct"
    printf '    "e2e_threshold_pct": %s,\n' "$qa_e2e_threshold"
    printf '    "log_validator_cases": %s,\n' "$qa_log_cases"
    printf '    "log_validator_threshold_min": %s,\n' "$qa_log_threshold"
    printf '    "scenario_reason": "%s"\n' "$(json_escape "$qa_reason")"
    echo "  },"
    echo '  "coverage": {'
    printf '    "current": {"total": %s, "real_db": %s, "inmemory": %s, "pure_unit": %s},\n' \
      "$COVERAGE_CURRENT_TOTAL" "$COVERAGE_CURRENT_REAL_DB" "$COVERAGE_CURRENT_INMEMORY" "$COVERAGE_CURRENT_PURE_UNIT"
    printf '    "baseline": {"total": %s, "real_db": %s, "inmemory": %s, "pure_unit": %s},\n' \
      "$COVERAGE_BASELINE_TOTAL" "$COVERAGE_BASELINE_REAL_DB" "$COVERAGE_BASELINE_INMEMORY" "$COVERAGE_BASELINE_PURE_UNIT"
    printf '    "delta": {"total": %s, "real_db": %s, "inmemory": %s, "pure_unit": %s}\n' \
      "$COVERAGE_DELTA_TOTAL" "$COVERAGE_DELTA_REAL_DB" "$COVERAGE_DELTA_INMEMORY" "$COVERAGE_DELTA_PURE_UNIT"
    echo "  },"
    echo '  "flaky_signals": {'
    printf '    "count": %s,\n' "$FLAKY_SIGNAL_COUNT"
    printf '    "preview": "%s"\n' "$(json_escape "$FLAKY_SIGNAL_PREVIEW")"
    echo "  },"
    echo '  "exception_inventory": {'
    printf '    "active_count": %s,\n' "$EXCEPTION_ACTIVE_COUNT"
    printf '    "expired_count": %s,\n' "$EXCEPTION_EXPIRED_COUNT"
    echo '    "exceptions": ['
    if [[ "$exception_count" -gt 0 ]]; then
      last_index=$((exception_count - 1))
      for idx in "${!EXCEPTION_ID[@]}"; do
        printf '      {"id":"%s","path":"%s","owner":"%s","expiry":"%s","expired":%s,"follow_up":"%s"}' \
          "$(json_escape "${EXCEPTION_ID[$idx]}")" \
          "$(json_escape "${EXCEPTION_PATH[$idx]}")" \
          "$(json_escape "${EXCEPTION_OWNER[$idx]}")" \
          "$(json_escape "${EXCEPTION_EXPIRY[$idx]}")" \
          "${EXCEPTION_EXPIRED[$idx]}" \
          "$(json_escape "${EXCEPTION_FOLLOWUP[$idx]}")"
        if [[ "$idx" -lt "$last_index" ]]; then
          echo ","
        else
          echo
        fi
      done
    fi
    echo "    ]"
    echo "  }"
    echo "}"
  } > "$report_json"

  {
    echo "# Quality Gate Dashboard Summary"
    echo
    echo "- Run ID: \`$RUN_ID\`"
    echo "- Status: **$overall_status**"
    echo "- Started (UTC): $RUN_STARTED_AT_UTC"
    echo "- Finished (UTC): $run_finished_at_utc"
    echo "- Exit code: \`$exit_code\`"
    echo
    echo "## Gate Results"
    echo
    echo "| Gate | Status | Duration (s) | Notes |"
    echo "|---|---|---:|---|"
    if [[ "$gate_count" -eq 0 ]]; then
      echo "| (none) | n/a | 0 | no gates executed |"
    else
      for idx in "${!GATE_RESULT_NAME[@]}"; do
        echo "| ${GATE_RESULT_NAME[$idx]} | ${GATE_RESULT_STATUS[$idx]} | ${GATE_RESULT_DURATION_S[$idx]} | ${GATE_RESULT_NOTES[$idx]:---} |"
      done
    fi
    echo
    echo "## QA Threshold Snapshot"
    echo
    echo "| Metric | Value | Threshold |"
    echo "|---|---:|---:|"
    echo "| Total tests | $qa_total | n/a |"
    echo "| Real DB tests | $qa_real_db | n/a |"
    echo "| Real DB % | $qa_real_pct | >= $qa_real_threshold |"
    echo "| Open P0/P1 critical gaps | $qa_critical | <= $qa_critical_threshold |"
    echo "| E2E pass rate % | $qa_e2e_pct | >= $qa_e2e_threshold |"
    echo "| Log validator cases | $qa_log_cases | >= $qa_log_threshold |"
    echo
    echo "Scenario status: $qa_reason"
    echo
    echo "## Coverage Delta"
    echo
    echo "| Category | Baseline | Current | Delta |"
    echo "|---|---:|---:|---:|"
    echo "| Total tests | $COVERAGE_BASELINE_TOTAL | $COVERAGE_CURRENT_TOTAL | $COVERAGE_DELTA_TOTAL |"
    echo "| Real DB tests | $COVERAGE_BASELINE_REAL_DB | $COVERAGE_CURRENT_REAL_DB | $COVERAGE_DELTA_REAL_DB |"
    echo "| InMemory/Fake tests | $COVERAGE_BASELINE_INMEMORY | $COVERAGE_CURRENT_INMEMORY | $COVERAGE_DELTA_INMEMORY |"
    echo "| Pure unit tests | $COVERAGE_BASELINE_PURE_UNIT | $COVERAGE_CURRENT_PURE_UNIT | $COVERAGE_DELTA_PURE_UNIT |"
    echo
    echo "## Replay/Flaky Signal"
    echo
    echo "- Signal count: \`$FLAKY_SIGNAL_COUNT\`"
    echo "- Signal preview: $FLAKY_SIGNAL_PREVIEW"
    echo
    echo "## Residual Exception Inventory"
    echo
    echo "- Active exceptions: \`$EXCEPTION_ACTIVE_COUNT\`"
    echo "- Expired exceptions: \`$EXCEPTION_EXPIRED_COUNT\`"
    echo
    echo "| ID | Path | Owner | Expiry | Expired | Follow-up |"
    echo "|---|---|---|---|---|---|"
    if [[ "$exception_count" -eq 0 ]]; then
      echo "| (none) | n/a | n/a | n/a | n/a | n/a |"
    else
      for idx in "${!EXCEPTION_ID[@]}"; do
        echo "| ${EXCEPTION_ID[$idx]} | ${EXCEPTION_PATH[$idx]} | ${EXCEPTION_OWNER[$idx]} | ${EXCEPTION_EXPIRY[$idx]} | ${EXCEPTION_EXPIRED[$idx]} | ${EXCEPTION_FOLLOWUP[$idx]} |"
      done
    fi
  } > "$report_md"

  printf '%s\n' "run-$RUN_ID" > "$latest_pointer_file"
  log "Wrote QA report artifacts: $report_json, $report_md"
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
REPORT_ENABLED=1
trap 'exit_code=$?; write_report_artifacts "$exit_code"' EXIT

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
