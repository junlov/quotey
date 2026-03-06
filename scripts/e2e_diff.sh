#!/usr/bin/env bash
#
# E2E replay-diff and summary reporting.
# Compares two E2E runner artifacts to highlight decision deltas,
# timing regressions, and assertion drift between runs.
#
# Usage:
#   ./scripts/e2e_diff.sh                          # Compare last 2 runs
#   ./scripts/e2e_diff.sh RUN_A RUN_B              # Compare specific runs (IDs or paths)
#   ./scripts/e2e_diff.sh --runs                   # List available runs
#   ./scripts/e2e_diff.sh --report                 # Generate markdown report for last 2 runs
#   ./scripts/e2e_diff.sh --report RUN_A RUN_B     # Generate markdown report for specific runs
#
# Environment:
#   QUOTEY_E2E_ARTIFACT_DIR   Override artifact directory (default: target/e2e-artifacts)
#   QUOTEY_TIMING_THRESHOLD   Seconds delta to flag as regression (default: 5)

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

ARTIFACT_DIR="${QUOTEY_E2E_ARTIFACT_DIR:-$ROOT_DIR/target/e2e-artifacts}"
TIMING_THRESHOLD="${QUOTEY_TIMING_THRESHOLD:-5}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ── Parse arguments ──────────────────────────────────────────────────────

MODE="diff"
RUN_A=""
RUN_B=""
REPORT=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs)
      MODE="list"; shift ;;
    --report)
      REPORT=true; shift ;;
    --help|-h)
      sed -n '2,/^$/s/^# \?//p' "$0"
      exit 0 ;;
    *)
      if [[ -z "$RUN_A" ]]; then
        RUN_A="$1"
      elif [[ -z "$RUN_B" ]]; then
        RUN_B="$1"
      else
        echo "Error: too many arguments"; exit 1
      fi
      shift ;;
  esac
done

# ── Helpers ──────────────────────────────────────────────────────────────

resolve_run_dir() {
  local input="$1"
  # Accept full path, directory name, or run ID
  if [[ -d "$input" ]]; then
    echo "$input"
  elif [[ -d "$ARTIFACT_DIR/$input" ]]; then
    echo "$ARTIFACT_DIR/$input"
  elif [[ -d "$ARTIFACT_DIR/run-$input" ]]; then
    echo "$ARTIFACT_DIR/run-$input"
  else
    echo ""
  fi
}

# Extract test names and results from cargo output log
extract_test_results() {
  local log_file="$1"
  # Parse lines like: "test module::test_name ... ok" or "... FAILED"
  grep -E '^test .+ \.\.\. (ok|FAILED|ignored)' "$log_file" 2>/dev/null | \
    sed -E 's/^test (.+) \.\.\. (ok|FAILED|ignored)$/\1 \2/' | \
    sort
}

# ── Mode: list ───────────────────────────────────────────────────────────

if [[ "$MODE" == "list" ]]; then
  if [[ ! -d "$ARTIFACT_DIR" ]]; then
    echo "No artifact directory found at $ARTIFACT_DIR"
    exit 0
  fi

  printf '\n%bAvailable runs:%b\n\n' "$CYAN" "$NC"
  printf '  %-24s %-8s %7s %7s %7s\n' "Run ID" "Status" "Passed" "Failed" "Suites"
  printf '  %-24s %-8s %7s %7s %7s\n' "------" "------" "------" "------" "------"

  for run_dir in $(find "$ARTIFACT_DIR" -maxdepth 1 -mindepth 1 -type d -name 'run-*' | sort); do
    summary="$run_dir/SUMMARY.json"
    run_name="$(basename "$run_dir")"
    if [[ -f "$summary" ]] && command -v jq >/dev/null 2>&1; then
      status="$(jq -r '.overall_status' "$summary")"
      passed="$(jq -r '.total_passed' "$summary")"
      failed="$(jq -r '.total_failed' "$summary")"
      suite_count="$(jq -r '.suites | length' "$summary")"
      if [[ "$status" == "PASS" ]]; then
        color="$GREEN"
      else
        color="$RED"
      fi
      printf "  %-24s ${color}%-8s${NC} %7s %7s %7s\n" "$run_name" "$status" "$passed" "$failed" "$suite_count"
    else
      printf '  %-24s %-8s\n' "$run_name" "no-summary"
    fi
  done
  echo ""
  exit 0
fi

# ── Resolve runs ─────────────────────────────────────────────────────────

if [[ -z "$RUN_A" ]] || [[ -z "$RUN_B" ]]; then
  # Auto-select last 2 runs
  mapfile -t RUNS < <(find "$ARTIFACT_DIR" -maxdepth 1 -mindepth 1 -type d -name 'run-*' | sort)
  if [[ "${#RUNS[@]}" -lt 2 ]]; then
    echo "Need at least 2 runs to compare. Found: ${#RUNS[@]}"
    echo "Run ./scripts/e2e_runner.sh twice first."
    exit 1
  fi
  DIR_A="${RUNS[-2]}"
  DIR_B="${RUNS[-1]}"
else
  DIR_A="$(resolve_run_dir "$RUN_A")"
  DIR_B="$(resolve_run_dir "$RUN_B")"
  if [[ -z "$DIR_A" ]]; then echo "Run not found: $RUN_A"; exit 1; fi
  if [[ -z "$DIR_B" ]]; then echo "Run not found: $RUN_B"; exit 1; fi
fi

NAME_A="$(basename "$DIR_A")"
NAME_B="$(basename "$DIR_B")"

# ── Load summaries ───────────────────────────────────────────────────────

if [[ ! -f "$DIR_A/SUMMARY.json" ]] || [[ ! -f "$DIR_B/SUMMARY.json" ]]; then
  echo "One or both runs missing SUMMARY.json"
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for diff analysis"
  exit 1
fi

STATUS_A="$(jq -r '.overall_status' "$DIR_A/SUMMARY.json")"
STATUS_B="$(jq -r '.overall_status' "$DIR_B/SUMMARY.json")"
PASSED_A="$(jq -r '.total_passed' "$DIR_A/SUMMARY.json")"
PASSED_B="$(jq -r '.total_passed' "$DIR_B/SUMMARY.json")"
FAILED_A="$(jq -r '.total_failed' "$DIR_A/SUMMARY.json")"
FAILED_B="$(jq -r '.total_failed' "$DIR_B/SUMMARY.json")"
TOTAL_A="$(jq -r '.total_tests' "$DIR_A/SUMMARY.json")"
TOTAL_B="$(jq -r '.total_tests' "$DIR_B/SUMMARY.json")"

# ── Collect per-test results from all suite logs ─────────────────────────

TESTS_A="$(mktemp)"
TESTS_B="$(mktemp)"
trap 'rm -f "$TESTS_A" "$TESTS_B"' EXIT

for suite_dir in "$DIR_A"/*/; do
  [[ -f "$suite_dir/output.log" ]] && extract_test_results "$suite_dir/output.log" >> "$TESTS_A"
done
for suite_dir in "$DIR_B"/*/; do
  [[ -f "$suite_dir/output.log" ]] && extract_test_results "$suite_dir/output.log" >> "$TESTS_B"
done

sort -o "$TESTS_A" "$TESTS_A"
sort -o "$TESTS_B" "$TESTS_B"

# ── Compute diffs ────────────────────────────────────────────────────────

# Tests that changed status
declare -a FLIPPED_TO_FAIL=()
declare -a FLIPPED_TO_PASS=()
declare -a NEW_TESTS=()
declare -a REMOVED_TESTS=()

# Find tests in A but not in B (removed)
while IFS=' ' read -r test_name result; do
  if ! grep -q "^${test_name} " "$TESTS_B"; then
    REMOVED_TESTS+=("$test_name ($result)")
  fi
done < "$TESTS_A"

# Find tests in B but not in A (new), and status flips
while IFS=' ' read -r test_name result_b; do
  result_a="$(grep "^${test_name} " "$TESTS_A" | awk '{print $NF}' || true)"
  if [[ -z "$result_a" ]]; then
    NEW_TESTS+=("$test_name ($result_b)")
  elif [[ "$result_a" != "$result_b" ]]; then
    if [[ "$result_b" == "FAILED" ]]; then
      FLIPPED_TO_FAIL+=("$test_name: $result_a -> $result_b")
    elif [[ "$result_b" == "ok" ]]; then
      FLIPPED_TO_PASS+=("$test_name: $result_a -> $result_b")
    fi
  fi
done < "$TESTS_B"

# ── Compute timing deltas per suite ──────────────────────────────────────

declare -a TIMING_REGRESSIONS=()
declare -a TIMING_IMPROVEMENTS=()

for suite_dir_b in "$DIR_B"/*/; do
  suite_name="$(basename "$suite_dir_b")"
  timing_a="$DIR_A/$suite_name/timing.txt"
  timing_b="$suite_dir_b/timing.txt"
  if [[ -f "$timing_a" ]] && [[ -f "$timing_b" ]]; then
    time_a="$(cat "$timing_a")"
    time_b="$(cat "$timing_b")"
    delta=$((time_b - time_a))
    if [[ "$delta" -ge "$TIMING_THRESHOLD" ]]; then
      TIMING_REGRESSIONS+=("$suite_name: ${time_a}s -> ${time_b}s (+${delta}s)")
    elif [[ "$delta" -le "-$TIMING_THRESHOLD" ]]; then
      abs_delta=$((-delta))
      TIMING_IMPROVEMENTS+=("$suite_name: ${time_a}s -> ${time_b}s (-${abs_delta}s)")
    fi
  fi
done

# ── Output ───────────────────────────────────────────────────────────────

output_diff() {
  printf '\n%b== E2E Replay Diff ==%b\n' "$CYAN" "$NC"
  printf 'Baseline:  %s (%s, %s tests)\n' "$NAME_A" "$STATUS_A" "$TOTAL_A"
  printf 'Current:   %s (%s, %s tests)\n' "$NAME_B" "$STATUS_B" "$TOTAL_B"

  # Overall status change
  if [[ "$STATUS_A" != "$STATUS_B" ]]; then
    if [[ "$STATUS_B" == "FAIL" ]]; then
      printf '\n%bOverall status: %s -> %s%b\n' "$RED" "$STATUS_A" "$STATUS_B" "$NC"
    else
      printf '\n%bOverall status: %s -> %s%b\n' "$GREEN" "$STATUS_A" "$STATUS_B" "$NC"
    fi
  else
    printf '\nOverall status: unchanged (%s)\n' "$STATUS_B"
  fi

  # Test count delta
  delta_total=$((TOTAL_B - TOTAL_A))
  delta_passed=$((PASSED_B - PASSED_A))
  delta_failed=$((FAILED_B - FAILED_A))
  printf 'Test count: %s (%+d)\n' "$TOTAL_B" "$delta_total"
  printf 'Passed:     %s (%+d)\n' "$PASSED_B" "$delta_passed"
  printf 'Failed:     %s (%+d)\n' "$FAILED_B" "$delta_failed"

  # Decision deltas (status flips)
  printf '\n%b--- Decision Deltas ---%b\n' "$YELLOW" "$NC"
  if [[ "${#FLIPPED_TO_FAIL[@]}" -gt 0 ]]; then
    printf '%bNew failures:%b\n' "$RED" "$NC"
    for entry in "${FLIPPED_TO_FAIL[@]}"; do
      printf '  - %s\n' "$entry"
    done
  fi
  if [[ "${#FLIPPED_TO_PASS[@]}" -gt 0 ]]; then
    printf '%bNew passes (fixed):%b\n' "$GREEN" "$NC"
    for entry in "${FLIPPED_TO_PASS[@]}"; do
      printf '  - %s\n' "$entry"
    done
  fi
  if [[ "${#FLIPPED_TO_FAIL[@]}" -eq 0 ]] && [[ "${#FLIPPED_TO_PASS[@]}" -eq 0 ]]; then
    printf '  No decision changes.\n'
  fi

  # Assertion drift (new/removed tests)
  printf '\n%b--- Assertion Drift ---%b\n' "$YELLOW" "$NC"
  if [[ "${#NEW_TESTS[@]}" -gt 0 ]]; then
    printf 'New tests (+%d):\n' "${#NEW_TESTS[@]}"
    for entry in "${NEW_TESTS[@]}"; do
      printf '  + %s\n' "$entry"
    done
  fi
  if [[ "${#REMOVED_TESTS[@]}" -gt 0 ]]; then
    printf '%bRemoved tests (-%d):%b\n' "$RED" "${#REMOVED_TESTS[@]}" "$NC"
    for entry in "${REMOVED_TESTS[@]}"; do
      printf '  - %s\n' "$entry"
    done
  fi
  if [[ "${#NEW_TESTS[@]}" -eq 0 ]] && [[ "${#REMOVED_TESTS[@]}" -eq 0 ]]; then
    printf '  No assertion drift.\n'
  fi

  # Timing regressions
  printf '\n%b--- Timing Analysis (threshold: %ds) ---%b\n' "$YELLOW" "$TIMING_THRESHOLD" "$NC"
  if [[ "${#TIMING_REGRESSIONS[@]}" -gt 0 ]]; then
    printf '%bRegressions:%b\n' "$RED" "$NC"
    for entry in "${TIMING_REGRESSIONS[@]}"; do
      printf '  - %s\n' "$entry"
    done
  fi
  if [[ "${#TIMING_IMPROVEMENTS[@]}" -gt 0 ]]; then
    printf '%bImprovements:%b\n' "$GREEN" "$NC"
    for entry in "${TIMING_IMPROVEMENTS[@]}"; do
      printf '  - %s\n' "$entry"
    done
  fi
  if [[ "${#TIMING_REGRESSIONS[@]}" -eq 0 ]] && [[ "${#TIMING_IMPROVEMENTS[@]}" -eq 0 ]]; then
    printf '  No significant timing changes.\n'
  fi
}

output_report() {
  local report_file="$DIR_B/DIFF_REPORT.md"
  local delta_total=$((TOTAL_B - TOTAL_A))
  local delta_passed=$((PASSED_B - PASSED_A))
  local delta_failed=$((FAILED_B - FAILED_A))

  cat > "$report_file" <<EOF
# E2E Replay Diff Report

**Generated:** $(date -u +"%Y-%m-%d %H:%M:%S UTC")

## Comparison

| | Baseline | Current | Delta |
|---|---|---|---|
| Run | $NAME_A | $NAME_B | |
| Status | $STATUS_A | $STATUS_B | $(if [[ "$STATUS_A" == "$STATUS_B" ]]; then echo "unchanged"; else echo "CHANGED"; fi) |
| Total tests | $TOTAL_A | $TOTAL_B | $( printf '%+d' "$delta_total") |
| Passed | $PASSED_A | $PASSED_B | $(printf '%+d' "$delta_passed") |
| Failed | $FAILED_A | $FAILED_B | $(printf '%+d' "$delta_failed") |

## Decision Deltas

EOF

  if [[ "${#FLIPPED_TO_FAIL[@]}" -gt 0 ]]; then
    echo "### New Failures" >> "$report_file"
    for entry in "${FLIPPED_TO_FAIL[@]}"; do
      echo "- $entry" >> "$report_file"
    done
    echo "" >> "$report_file"
  fi

  if [[ "${#FLIPPED_TO_PASS[@]}" -gt 0 ]]; then
    echo "### Fixed (Now Passing)" >> "$report_file"
    for entry in "${FLIPPED_TO_PASS[@]}"; do
      echo "- $entry" >> "$report_file"
    done
    echo "" >> "$report_file"
  fi

  if [[ "${#FLIPPED_TO_FAIL[@]}" -eq 0 ]] && [[ "${#FLIPPED_TO_PASS[@]}" -eq 0 ]]; then
    echo "No decision changes between runs." >> "$report_file"
    echo "" >> "$report_file"
  fi

  echo "## Assertion Drift" >> "$report_file"
  echo "" >> "$report_file"

  if [[ "${#NEW_TESTS[@]}" -gt 0 ]]; then
    echo "### New Tests (+${#NEW_TESTS[@]})" >> "$report_file"
    for entry in "${NEW_TESTS[@]}"; do
      echo "- $entry" >> "$report_file"
    done
    echo "" >> "$report_file"
  fi

  if [[ "${#REMOVED_TESTS[@]}" -gt 0 ]]; then
    echo "### Removed Tests (-${#REMOVED_TESTS[@]})" >> "$report_file"
    for entry in "${REMOVED_TESTS[@]}"; do
      echo "- $entry" >> "$report_file"
    done
    echo "" >> "$report_file"
  fi

  if [[ "${#NEW_TESTS[@]}" -eq 0 ]] && [[ "${#REMOVED_TESTS[@]}" -eq 0 ]]; then
    echo "No test additions or removals." >> "$report_file"
    echo "" >> "$report_file"
  fi

  echo "## Timing Analysis" >> "$report_file"
  echo "" >> "$report_file"
  echo "Threshold: ${TIMING_THRESHOLD}s" >> "$report_file"
  echo "" >> "$report_file"

  if [[ "${#TIMING_REGRESSIONS[@]}" -gt 0 ]]; then
    echo "### Regressions" >> "$report_file"
    for entry in "${TIMING_REGRESSIONS[@]}"; do
      echo "- $entry" >> "$report_file"
    done
    echo "" >> "$report_file"
  fi

  if [[ "${#TIMING_IMPROVEMENTS[@]}" -gt 0 ]]; then
    echo "### Improvements" >> "$report_file"
    for entry in "${TIMING_IMPROVEMENTS[@]}"; do
      echo "- $entry" >> "$report_file"
    done
    echo "" >> "$report_file"
  fi

  if [[ "${#TIMING_REGRESSIONS[@]}" -eq 0 ]] && [[ "${#TIMING_IMPROVEMENTS[@]}" -eq 0 ]]; then
    echo "No significant timing changes." >> "$report_file"
    echo "" >> "$report_file"
  fi

  printf 'Report written to: %s\n' "$report_file"
}

# ── Execute ──────────────────────────────────────────────────────────────

output_diff

if [[ "$REPORT" == true ]]; then
  output_report
fi
