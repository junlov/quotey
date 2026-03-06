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
#   ./scripts/e2e_runner.sh --cleanup --dry-run   # Preview cleanup without deleting
#   ./scripts/e2e_runner.sh --summary             # Show summary of last run
#   ./scripts/e2e_runner.sh --compare-last        # Generate replay-diff vs previous run
#   ./scripts/e2e_runner.sh --compare-last --strict-diff  # Fail run if diff finds regressions
#
# Environment:
#   QUOTEY_E2E_ARTIFACT_DIR   Override artifact directory (default: target/e2e-artifacts)
#   QUOTEY_E2E_KEEP_RUNS      Number of runs to retain (default: 10)
#   QUOTEY_E2E_VERBOSE        Show cargo test output live (default: 0)
#   QUOTEY_E2E_LOCK_TIMEOUT   Seconds to wait for runner lock (default: 30)
#   CARGO_TARGET_DIR           Override cargo target dir

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"
ARTIFACT_DIR="${QUOTEY_E2E_ARTIFACT_DIR:-$ROOT_DIR/target/e2e-artifacts}"
KEEP_RUNS="${QUOTEY_E2E_KEEP_RUNS:-10}"
VERBOSE="${QUOTEY_E2E_VERBOSE:-0}"
LOCK_TIMEOUT="${QUOTEY_E2E_LOCK_TIMEOUT:-30}"

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
declare -A SUITE_TEST_BIN=(
  [e2e]="e2e_scenarios"
  [critical]="critical_path_coverage"
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
DRY_RUN=0
COMPARE_LAST=0
STRICT_DIFF=0

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
    --dry-run)
      DRY_RUN=1; shift ;;
    --compare-last)
      COMPARE_LAST=1; shift ;;
    --strict-diff)
      STRICT_DIFF=1; shift ;;
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

if [[ "$STRICT_DIFF" == "1" ]] && [[ "$COMPARE_LAST" != "1" ]]; then
  printf '%bError:%b --strict-diff requires --compare-last\n' "$RED" "$NC" >&2
  exit 1
fi

# ── Helpers ──────────────────────────────────────────────────────────────

timestamp() {
  date -u +%Y%m%dT%H%M%SZ
}

human_ts() {
  date -u +"%Y-%m-%d %H:%M:%S UTC"
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

duration_fmt() {
  local secs="$1"
  if [[ "$secs" -ge 60 ]]; then
    printf '%dm%02ds' $((secs / 60)) $((secs % 60))
  else
    printf '%ds' "$secs"
  fi
}

require_positive_int() {
  local label="$1"
  local value="$2"
  if [[ ! "$value" =~ ^[0-9]+$ ]]; then
    printf '%bError:%b %s must be a positive integer, got: %s\n' "$RED" "$NC" "$label" "$value" >&2
    exit 1
  fi
  if [[ "$value" -lt 1 ]]; then
    printf '%bError:%b %s must be >= 1, got: %s\n' "$RED" "$NC" "$label" "$value" >&2
    exit 1
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

acquire_lock() {
  mkdir -p "$ARTIFACT_DIR"
  local lock_file="$ARTIFACT_DIR/.runner.lock"
  exec 9>"$lock_file"
  if ! flock -w "$LOCK_TIMEOUT" 9; then
    printf '%bError:%b could not acquire E2E runner lock within %ss (%s)\n' \
      "$RED" "$NC" "$LOCK_TIMEOUT" "$lock_file" >&2
    exit 1
  fi
}

# ── Mode: list ───────────────────────────────────────────────────────────

if [[ "$MODE" == "list" ]]; then
  printf '\n%bAvailable E2E suites:%b\n\n' "$CYAN" "$NC"
  for suite in "${ALL_SUITES[@]}"; do
    printf '  %-12s  %s\n' "$suite" "${SUITE_DESC[$suite]}"
    printf '                Package: %s  Test: %s\n\n' "${SUITE_PKG[$suite]}" "${SUITE_TEST_BIN[$suite]:-(all)}"
  done
  exit 0
fi

# ── Mode: cleanup ────────────────────────────────────────────────────────

if [[ "$MODE" == "cleanup" ]]; then
  acquire_lock
  require_positive_int "KEEP_RUNS" "$KEEP_RUNS"

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
    if [[ "$DRY_RUN" == "1" ]]; then
      printf '  [dry-run] Would remove: %s\n' "$(basename "$run_dir")"
    else
      printf '  Removing: %s\n' "$(basename "$run_dir")"
      rm -rf "$run_dir"
    fi
  done

  if [[ "$DRY_RUN" == "1" ]]; then
    printf '%bDry-run cleanup complete.%b\n' "$GREEN" "$NC"
  else
    printf '%bCleanup complete.%b\n' "$GREEN" "$NC"
  fi
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

acquire_lock
require_positive_int "KEEP_RUNS" "$KEEP_RUNS"

RUN_TS="$(timestamp)"
RUN_DIR="$ARTIFACT_DIR/run-$RUN_TS"
mkdir -p "$RUN_DIR"
RUN_STARTED_AT="$(human_ts)"

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
  test_bin="${SUITE_TEST_BIN[$suite]}"

  # Build cargo command array
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

  cmd=(cargo test -p "$pkg")
  if [[ -n "$test_bin" ]]; then
    cmd+=(--test "$test_bin")
  fi

  cmd+=(--)
  if [[ -n "$all_filters" ]]; then
    cmd+=("$all_filters")
  fi
  cmd+=(--nocapture)

  {
    printf '#!/usr/bin/env bash\n'
    printf 'set -euo pipefail\n'
    printf 'cd %q\n' "$ROOT_DIR"
    printf 'exec '
    printf '%q ' "${cmd[@]}"
    printf '\n'
  } > "$suite_dir/replay.sh"
  chmod +x "$suite_dir/replay.sh"

  printf '%b[%s]%b Running:' "$YELLOW" "$suite" "$NC"
  printf ' %q' "${cmd[@]}"
  printf '\n'

  start_epoch="$(date +%s)"

  suite_exit=0
  if [[ "$VERBOSE" == "1" ]]; then
    "${cmd[@]}" | tee "$log_file" || suite_exit=$?
  else
    "${cmd[@]}" > "$log_file" 2>&1 || suite_exit=$?
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
RUN_FINISHED_AT="$(human_ts)"

cat > "$RUN_DIR/SUMMARY.json" <<ENDJSON
{
  "run_id": "$RUN_TS",
  "started_at": "$RUN_STARTED_AT",
  "finished_at": "$RUN_FINISHED_AT",
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

{
  suites_json=""
  for suite in "${SELECTED_SUITES[@]}"; do
    if [[ -n "$suites_json" ]]; then
      suites_json+=", "
    fi
    suites_json+="\"$suite\""
  done

  cat > "$RUN_DIR/RUN_METADATA.json" <<ENDJSON
{
  "run_id": "$RUN_TS",
  "started_at": "$RUN_STARTED_AT",
  "finished_at": "$RUN_FINISHED_AT",
  "root_dir": "$(json_escape "$ROOT_DIR")",
  "artifact_dir": "$(json_escape "$ARTIFACT_DIR")",
  "keep_runs": $KEEP_RUNS,
  "verbose": $VERBOSE,
  "filter": "$(json_escape "$FILTER")",
  "suites": [$suites_json],
  "git": {
    "branch": "$(json_escape "$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)")",
    "commit": "$(json_escape "$(git rev-parse HEAD 2>/dev/null || echo unknown)")"
  },
  "host": "$(json_escape "$(hostname 2>/dev/null || echo unknown)")"
}
ENDJSON
}

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
ln -sfn "$(basename "$RUN_DIR")" "$ARTIFACT_DIR/latest"

# ── Optional replay-diff against previous run ────────────────────────────

summary_status_override=""
if [[ "$COMPARE_LAST" == "1" ]]; then
  mapfile -t ALL_RUNS_FOR_DIFF < <(find "$ARTIFACT_DIR" -maxdepth 1 -mindepth 1 -type d -name 'run-*' | sort)
  if [[ "${#ALL_RUNS_FOR_DIFF[@]}" -ge 2 ]]; then
    prev_run="${ALL_RUNS_FOR_DIFF[-2]}"
    printf '\n%b== Replay Diff (previous vs current) ==%b\n' "$CYAN" "$NC"
    printf 'Baseline: %s\n' "$(basename "$prev_run")"
    printf 'Current:  %s\n' "$(basename "$RUN_DIR")"

    diff_cmd=(./scripts/e2e_diff.sh --report "$prev_run" "$RUN_DIR")
    if [[ "$STRICT_DIFF" == "1" ]]; then
      diff_cmd+=(--strict)
    fi

    diff_exit=0
    "${diff_cmd[@]}" | tee "$RUN_DIR/DIFF_OUTPUT.txt" || diff_exit=$?

    if [[ "$diff_exit" -ne 0 ]]; then
      if [[ "$STRICT_DIFF" == "1" ]]; then
        OVERALL_EXIT=1
        summary_status_override="FAIL"
        printf '%bReplay diff strict check failed (exit=%d); marking run as failed.%b\n' \
          "$RED" "$diff_exit" "$NC"
      else
        printf '%bReplay diff command exited non-zero (exit=%d), continuing.%b\n' \
          "$YELLOW" "$diff_exit" "$NC"
      fi
    else
      printf '%bReplay diff complete.%b\n' "$GREEN" "$NC"
      if [[ -f "$RUN_DIR/DIFF_REPORT.md" ]]; then
        printf 'Replay report: %s\n' "$RUN_DIR/DIFF_REPORT.md"
      fi
      if [[ -f "$RUN_DIR/DIFF_SUMMARY.json" ]]; then
        printf 'Replay summary: %s\n' "$RUN_DIR/DIFF_SUMMARY.json"
      fi
    fi
  else
    printf '\n%bReplay diff skipped:%b need at least 2 runs\n' "$YELLOW" "$NC"
  fi
fi

if [[ "$summary_status_override" == "FAIL" ]]; then
  sed -i 's/"overall_status": "PASS"/"overall_status": "FAIL"/' "$RUN_DIR/SUMMARY.json"
fi

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
