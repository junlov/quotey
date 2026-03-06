#!/usr/bin/env bash
#
# Operator-grade E2E test runner with log capture and artifact management.
#
# Usage:
#   ./scripts/e2e_runner.sh                       # Run all E2E suites
#   ./scripts/e2e_runner.sh --suite e2e           # Run only e2e_scenarios
#   ./scripts/e2e_runner.sh --suite critical       # Run only critical_path_coverage
#   ./scripts/e2e_runner.sh --suite regression     # Run only portal regression tests
#   ./scripts/e2e_runner.sh --suite e2e --filter s001  # Run matching scenario
#   ./scripts/e2e_runner.sh --list                # List available suites
#   ./scripts/e2e_runner.sh --cleanup             # Remove old artifacts
#   ./scripts/e2e_runner.sh --cleanup --keep 5    # Keep last N runs
#   ./scripts/e2e_runner.sh --summary             # Show summary of last run
#
# Environment:
#   QUOTEY_E2E_ARTIFACT_DIR   Override artifact directory (default: target/e2e-artifacts)
#   QUOTEY_E2E_KEEP_RUNS      Number of runs to retain (default: 10)
#   QUOTEY_E2E_VERBOSE        Show cargo test output live (default: 0)
#   CARGO_TARGET_DIR           Override cargo target dir

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
ARTIFACT_DIR="${QUOTEY_E2E_ARTIFACT_DIR:-$ROOT_DIR/target/e2e-artifacts}"
KEEP_RUNS="${QUOTEY_E2E_KEEP_RUNS:-10}"
VERBOSE="${QUOTEY_E2E_VERBOSE:-0}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ── Suite definitions ────────────────────────────────────────────────────

declare -A SUITE_PKG=(
  [e2e]="quotey-db"
  [critical]="quotey-db"
  [regression]="quotey-server"
)
declare -A SUITE_TEST=(
  [e2e]="--test e2e_scenarios"
  [critical]="--test critical_path_coverage"
  [regression]=""
)
# Filters applied after -- for specific suites
declare -A SUITE_BUILTIN_FILTER=(
  [e2e]=""
  [critical]=""
  [regression]="regression"
)
declare -A SUITE_DESC=(
  [e2e]="E2E integration scenarios (s001-s017)"
  [critical]="Critical path coverage (G-001 through G-004)"
  [regression]="Portal regression tests (R-001 through R-006)"
)
ALL_SUITES=(e2e critical regression)

# ── Parse arguments ──────────────────────────────────────────────────────

MODE="run"
SELECTED_SUITES=()
FILTER=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --list)
      MODE="list"; shift ;;
    --cleanup)
      MODE="cleanup"; shift ;;
    --summary)
      MODE="summary"; shift ;;
    --suite)
      shift
      if [[ $# -eq 0 ]]; then echo "Error: --suite requires a value"; exit 1; fi
      SELECTED_SUITES+=("$1"); shift ;;
    --filter)
      shift
      if [[ $# -eq 0 ]]; then echo "Error: --filter requires a value"; exit 1; fi
      FILTER="$1"; shift ;;
    --keep)
      shift
      if [[ $# -eq 0 ]]; then echo "Error: --keep requires a value"; exit 1; fi
      KEEP_RUNS="$1"; shift ;;
    --verbose|-v)
      VERBOSE=1; shift ;;
    --help|-h)
      sed -n '2,/^$/s/^# \?//p' "$0"
      exit 0 ;;
    *)
      echo "Unknown option: $1"; exit 1 ;;
  esac
done

# Default: run all suites
if [[ "${#SELECTED_SUITES[@]}" -eq 0 ]]; then
  SELECTED_SUITES=("${ALL_SUITES[@]}")
fi

# ── Helpers ──────────────────────────────────────────────────────────────

timestamp() {
  date -u +%Y%m%dT%H%M%SZ
}

human_ts() {
  date -u +"%Y-%m-%d %H:%M:%S UTC"
}

duration_fmt() {
  local secs="$1"
  if [[ "$secs" -ge 60 ]]; then
    printf '%dm%02ds' $((secs / 60)) $((secs % 60))
  else
    printf '%ds' "$secs"
  fi
}

parse_test_result() {
  local log_file="$1"
  local passed failed ignored
  local summary_line
  summary_line="$(grep -E 'test result: (ok|FAILED)\.' "$log_file" | tail -n 1 || true)"

  if [[ -z "$summary_line" ]]; then
    echo "0 0 0"
    return
  fi

  passed="$(printf '%s' "$summary_line" | sed -nE 's/.* ([0-9]+) passed.*/\1/p')"
  failed="$(printf '%s' "$summary_line" | sed -nE 's/.*; ([0-9]+) failed.*/\1/p')"
  ignored="$(printf '%s' "$summary_line" | sed -nE 's/.*; ([0-9]+) ignored.*/\1/p')"
  echo "${passed:-0} ${failed:-0} ${ignored:-0}"
}

# ── Mode: list ───────────────────────────────────────────────────────────

if [[ "$MODE" == "list" ]]; then
  printf '\n%bAvailable E2E suites:%b\n\n' "$CYAN" "$NC"
  for suite in "${ALL_SUITES[@]}"; do
    printf '  %-12s  %s\n' "$suite" "${SUITE_DESC[$suite]}"
    printf '                Package: %s  Test: %s\n\n' "${SUITE_PKG[$suite]}" "${SUITE_TEST[$suite]}"
  done
  exit 0
fi

# ── Mode: cleanup ────────────────────────────────────────────────────────

if [[ "$MODE" == "cleanup" ]]; then
  if [[ ! -d "$ARTIFACT_DIR" ]]; then
    echo "No artifact directory found at $ARTIFACT_DIR"
    exit 0
  fi

  # List run directories sorted by name (timestamp-based)
  mapfile -t RUNS < <(find "$ARTIFACT_DIR" -maxdepth 1 -mindepth 1 -type d -name 'run-*' | sort)
  total="${#RUNS[@]}"

  if [[ "$total" -le "$KEEP_RUNS" ]]; then
    printf 'Found %d runs, keeping %d. Nothing to clean.\n' "$total" "$KEEP_RUNS"
    exit 0
  fi

  to_remove=$((total - KEEP_RUNS))
  printf 'Found %d runs, keeping %d, removing %d oldest.\n' "$total" "$KEEP_RUNS" "$to_remove"

  for ((i = 0; i < to_remove; i++)); do
    run_dir="${RUNS[$i]}"
    printf '  Removing: %s\n' "$(basename "$run_dir")"
    rm -rf "$run_dir"
  done

  printf '%bCleanup complete.%b\n' "$GREEN" "$NC"
  exit 0
fi

# ── Mode: summary ────────────────────────────────────────────────────────

if [[ "$MODE" == "summary" ]]; then
  if [[ ! -d "$ARTIFACT_DIR" ]]; then
    echo "No artifact directory found at $ARTIFACT_DIR"
    exit 1
  fi

  latest="$(find "$ARTIFACT_DIR" -maxdepth 1 -mindepth 1 -type d -name 'run-*' | sort | tail -n 1)"
  if [[ -z "$latest" ]]; then
    echo "No runs found."
    exit 1
  fi

  summary_file="$latest/SUMMARY.json"
  if [[ ! -f "$summary_file" ]]; then
    echo "No summary found in $(basename "$latest")"
    exit 1
  fi

  if command -v jq >/dev/null 2>&1; then
    jq '.' "$summary_file"
  else
    cat "$summary_file"
  fi
  exit 0
fi

# ── Mode: run ────────────────────────────────────────────────────────────

RUN_TS="$(timestamp)"
RUN_DIR="$ARTIFACT_DIR/run-$RUN_TS"
mkdir -p "$RUN_DIR"

printf '\n%b== E2E Runner ==%b\n' "$CYAN" "$NC"
printf 'Run ID:     %s\n' "$RUN_TS"
printf 'Artifact:   %s\n' "$RUN_DIR"
printf 'Suites:     %s\n' "${SELECTED_SUITES[*]}"
if [[ -n "$FILTER" ]]; then
  printf 'Filter:     %s\n' "$FILTER"
fi
printf '\n'

# Track overall result
OVERALL_PASSED=0
OVERALL_FAILED=0
OVERALL_IGNORED=0
OVERALL_EXIT=0
declare -a SUITE_RESULTS=()

for suite in "${SELECTED_SUITES[@]}"; do
  if [[ -z "${SUITE_PKG[$suite]:-}" ]]; then
    printf '%bUnknown suite: %s%b\n' "$RED" "$suite" "$NC"
    OVERALL_EXIT=1
    continue
  fi

  suite_dir="$RUN_DIR/$suite"
  mkdir -p "$suite_dir"
  log_file="$suite_dir/output.log"
  timing_file="$suite_dir/timing.txt"
  result_file="$suite_dir/result.json"

  pkg="${SUITE_PKG[$suite]}"
  test_args="${SUITE_TEST[$suite]}"

  # Build cargo command
  builtin_filter="${SUITE_BUILTIN_FILTER[$suite]:-}"
  # Combine builtin filter and user filter
  all_filters=""
  if [[ -n "$builtin_filter" ]]; then
    all_filters="$builtin_filter"
  fi
  if [[ -n "$FILTER" ]]; then
    if [[ -n "$all_filters" ]]; then
      # Both filters: use the user filter (more specific)
      all_filters="$FILTER"
    else
      all_filters="$FILTER"
    fi
  fi

  if [[ -n "$all_filters" ]]; then
    cmd="cargo test -p $pkg $test_args -- $all_filters --nocapture 2>&1"
  else
    cmd="cargo test -p $pkg $test_args -- --nocapture 2>&1"
  fi

  printf '%b[%s]%b Running: %s\n' "$YELLOW" "$suite" "$NC" "$cmd"

  start_epoch="$(date +%s)"

  suite_exit=0
  if [[ "$VERBOSE" == "1" ]]; then
    eval "$cmd" | tee "$log_file" || suite_exit=$?
  else
    eval "$cmd" > "$log_file" 2>&1 || suite_exit=$?
  fi

  end_epoch="$(date +%s)"
  elapsed=$((end_epoch - start_epoch))
  echo "$elapsed" > "$timing_file"

  # Parse results
  read -r passed failed ignored <<< "$(parse_test_result "$log_file")"
  OVERALL_PASSED=$((OVERALL_PASSED + passed))
  OVERALL_FAILED=$((OVERALL_FAILED + failed))
  OVERALL_IGNORED=$((OVERALL_IGNORED + ignored))

  if [[ "$suite_exit" -ne 0 ]]; then
    OVERALL_EXIT=1
    status="FAIL"
    printf '%b[%s] FAIL%b — %d passed, %d failed (%s)\n' "$RED" "$suite" "$NC" "$passed" "$failed" "$(duration_fmt "$elapsed")"
  else
    status="PASS"
    printf '%b[%s] PASS%b — %d passed, %d ignored (%s)\n' "$GREEN" "$suite" "$NC" "$passed" "$ignored" "$(duration_fmt "$elapsed")"
  fi

  # Write per-suite result
  cat > "$result_file" <<ENDJSON
{
  "suite": "$suite",
  "package": "$pkg",
  "status": "$status",
  "passed": $passed,
  "failed": $failed,
  "ignored": $ignored,
  "duration_seconds": $elapsed,
  "exit_code": $suite_exit,
  "log_file": "$(basename "$log_file")"
}
ENDJSON

  SUITE_RESULTS+=("$suite:$status:$passed:$failed:$ignored:$elapsed")
done

# ── Write run summary ────────────────────────────────────────────────────

total_tests=$((OVERALL_PASSED + OVERALL_FAILED + OVERALL_IGNORED))
if [[ "$OVERALL_EXIT" -eq 0 ]]; then
  overall_status="PASS"
else
  overall_status="FAIL"
fi

cat > "$RUN_DIR/SUMMARY.json" <<ENDJSON
{
  "run_id": "$RUN_TS",
  "started_at": "$(human_ts)",
  "overall_status": "$overall_status",
  "total_tests": $total_tests,
  "total_passed": $OVERALL_PASSED,
  "total_failed": $OVERALL_FAILED,
  "total_ignored": $OVERALL_IGNORED,
  "suites": [
$(
  first=true
  for entry in "${SUITE_RESULTS[@]}"; do
    IFS=: read -r s_name s_status s_passed s_failed s_ignored s_elapsed <<< "$entry"
    if $first; then first=false; else printf ',\n'; fi
    printf '    {"suite": "%s", "status": "%s", "passed": %s, "failed": %s, "ignored": %s, "duration_seconds": %s}' \
      "$s_name" "$s_status" "$s_passed" "$s_failed" "$s_ignored" "$s_elapsed"
  done
)
  ]
}
ENDJSON

# ── Print summary ────────────────────────────────────────────────────────

printf '\n%b== Summary ==%b\n' "$CYAN" "$NC"
printf '%-12s %-6s %7s %7s %7s %8s\n' "Suite" "Status" "Passed" "Failed" "Ignored" "Duration"
printf '%-12s %-6s %7s %7s %7s %8s\n' "-----" "------" "------" "------" "-------" "--------"

for entry in "${SUITE_RESULTS[@]}"; do
  IFS=: read -r s_name s_status s_passed s_failed s_ignored s_elapsed <<< "$entry"
  if [[ "$s_status" == "PASS" ]]; then
    color="$GREEN"
  else
    color="$RED"
  fi
  printf "%-12s ${color}%-6s${NC} %7s %7s %7s %8s\n" \
    "$s_name" "$s_status" "$s_passed" "$s_failed" "$s_ignored" "$(duration_fmt "$s_elapsed")"
done

printf '%-12s ' "TOTAL"
if [[ "$overall_status" == "PASS" ]]; then
  printf '%b%-6s%b' "$GREEN" "$overall_status" "$NC"
else
  printf '%b%-6s%b' "$RED" "$overall_status" "$NC"
fi
printf ' %7s %7s %7s\n' "$OVERALL_PASSED" "$OVERALL_FAILED" "$OVERALL_IGNORED"

printf '\nArtifacts: %s\n' "$RUN_DIR"

# ── Auto-cleanup old runs ────────────────────────────────────────────────

mapfile -t OLD_RUNS < <(find "$ARTIFACT_DIR" -maxdepth 1 -mindepth 1 -type d -name 'run-*' | sort)
total_runs="${#OLD_RUNS[@]}"
if [[ "$total_runs" -gt "$KEEP_RUNS" ]]; then
  to_remove=$((total_runs - KEEP_RUNS))
  for ((i = 0; i < to_remove; i++)); do
    rm -rf "${OLD_RUNS[$i]}"
  done
  printf 'Auto-cleaned %d old run(s) (retention: %d)\n' "$to_remove" "$KEEP_RUNS"
fi

exit $OVERALL_EXIT
