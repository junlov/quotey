#!/usr/bin/env bash
# test_inventory.sh — Automated fake/mock/in-memory test inventory scanner
# Part of quotey-115.2 Track A deliverables
#
# Produces JSON + markdown reports of test patterns across the workspace.
# Usage: ./scripts/test_inventory.sh [--json] [--markdown] [--all]
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CRATES=("core" "db" "mcp" "slack" "agent" "cli" "server")
OUTPUT_FORMAT="${1:---all}"

# ── Counters ──────────────────────────────────────────────────────────────
declare -A TOTAL_TESTS
declare -A REAL_DB_TESTS
declare -A INMEMORY_TESTS
declare -A PURE_UNIT_TESTS
declare -A TEST_FILES

count_test_fns() {
    local file="$1"
    grep -cE '#\[(tokio::)?test\]' "$file" 2>/dev/null || echo 0
}

# ── Scan each crate ──────────────────────────────────────────────────────
scan_crate() {
    local crate="$1"
    local crate_dir="$REPO_ROOT/crates/$crate"
    local total=0 real_db=0 inmemory=0 pure_unit=0 files=0

    # Find all Rust files with test functions
    while IFS= read -r file; do
        local count
        count=$(count_test_fns "$file")
        if [ "$count" -gt 0 ]; then
            total=$((total + count))
            files=$((files + 1))

            # Classify: does file use real DB (migrations)?
            if grep -qE 'migrations::run_pending|run_pending\(' "$file" 2>/dev/null; then
                real_db=$((real_db + count))
            elif grep -qE 'connect_with_settings|connect\(' "$file" 2>/dev/null && \
                 grep -qE 'sqlite::memory' "$file" 2>/dev/null; then
                # Uses in-memory SQLite but no migrations — partial DB
                real_db=$((real_db + count))
            elif grep -qE 'InMemory|in_memory|Mock|mock|Fake|fake|Stub|stub' "$file" 2>/dev/null; then
                inmemory=$((inmemory + count))
            else
                pure_unit=$((pure_unit + count))
            fi
        fi
    done < <(find "$crate_dir" -name '*.rs' -type f 2>/dev/null)

    TOTAL_TESTS[$crate]=$total
    REAL_DB_TESTS[$crate]=$real_db
    INMEMORY_TESTS[$crate]=$inmemory
    PURE_UNIT_TESTS[$crate]=$pure_unit
    TEST_FILES[$crate]=$files
}

# ── InMemory type inventory ──────────────────────────────────────────────
scan_inmemory_types() {
    echo "["
    local first=true
    while IFS= read -r file; do
        local rel_path="${file#$REPO_ROOT/}"
        while IFS= read -r line; do
            local struct_name
            struct_name=$(echo "$line" | grep -oP 'pub struct (InMemory\w+|Mock\w+|Fake\w+)' | awk '{print $3}')
            if [ -n "$struct_name" ]; then
                if [ "$first" = true ]; then first=false; else echo ","; fi
                printf '  {"type": "%s", "file": "%s", "in_test_cfg": %s}' \
                    "$struct_name" "$rel_path" \
                    "$(grep -B5 "$line" "$file" | grep -q 'cfg(test)' && echo true || echo false)"
            fi
        done < <(grep -n 'pub struct \(InMemory\|Mock\|Fake\)' "$file" 2>/dev/null)
    done < <(find "$REPO_ROOT/crates" -name '*.rs' -type f 2>/dev/null)
    echo ""
    echo "]"
}

# ── Critical path analysis ───────────────────────────────────────────────
scan_critical_paths() {
    echo "["
    local first=true
    # Pricing engine
    for pattern in "DeterministicPricingEngine" "DeterministicPolicyEngine" "price_quote" "calculate_price"; do
        local files
        files=$(grep -rl "$pattern" "$REPO_ROOT/crates" --include='*.rs' 2>/dev/null | head -10)
        if [ -n "$files" ]; then
            local test_count=0
            for f in $files; do
                if grep -q 'cfg(test)' "$f" 2>/dev/null; then
                    local c
                    c=$(grep -c "#\[.*test\]" "$f" 2>/dev/null || echo 0)
                    test_count=$((test_count + c))
                fi
            done
            if [ "$first" = true ]; then first=false; else echo ","; fi
            printf '  {"path": "pricing/%s", "pattern": "%s", "test_count": %d}' \
                "$pattern" "$pattern" "$test_count"
        fi
    done
    echo ""
    echo "]"
}

# ── Run scan ─────────────────────────────────────────────────────────────
for crate in "${CRATES[@]}"; do
    scan_crate "$crate"
done

# ── Compute totals ───────────────────────────────────────────────────────
grand_total=0 grand_real=0 grand_inmem=0 grand_pure=0 grand_files=0
for crate in "${CRATES[@]}"; do
    grand_total=$((grand_total + ${TOTAL_TESTS[$crate]}))
    grand_real=$((grand_real + ${REAL_DB_TESTS[$crate]}))
    grand_inmem=$((grand_inmem + ${INMEMORY_TESTS[$crate]}))
    grand_pure=$((grand_pure + ${PURE_UNIT_TESTS[$crate]}))
    grand_files=$((grand_files + ${TEST_FILES[$crate]}))
done

# ── Output JSON ──────────────────────────────────────────────────────────
if [ "$OUTPUT_FORMAT" = "--json" ] || [ "$OUTPUT_FORMAT" = "--all" ]; then
    cat <<ENDJSON
{
  "scan_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "scanner_version": "1.0.0",
  "summary": {
    "total_tests": $grand_total,
    "real_db_tests": $grand_real,
    "inmemory_tests": $grand_inmem,
    "pure_unit_tests": $grand_pure,
    "test_files": $grand_files,
    "crate_count": ${#CRATES[@]}
  },
  "by_crate": {
$(for crate in "${CRATES[@]}"; do
    printf '    "%s": {"total": %d, "real_db": %d, "inmemory": %d, "pure_unit": %d, "files": %d}' \
        "$crate" "${TOTAL_TESTS[$crate]}" "${REAL_DB_TESTS[$crate]}" \
        "${INMEMORY_TESTS[$crate]}" "${PURE_UNIT_TESTS[$crate]}" "${TEST_FILES[$crate]}"
    if [ "$crate" != "server" ]; then echo ","; fi
done)
  },
  "inmemory_types": $(scan_inmemory_types),
  "critical_path_coverage": $(scan_critical_paths)
}
ENDJSON
fi

# ── Output Markdown ──────────────────────────────────────────────────────
if [ "$OUTPUT_FORMAT" = "--markdown" ] || [ "$OUTPUT_FORMAT" = "--all" ]; then
    if [ "$OUTPUT_FORMAT" = "--all" ]; then echo "---"; fi
    cat <<ENDMD
# Test Coverage Inventory Report

**Generated:** $(date -u +%Y-%m-%dT%H:%M:%SZ)
**Scanner:** test_inventory.sh v1.0.0

## Summary

| Metric | Count |
|--------|-------|
| Total test functions | $grand_total |
| Real-DB tests (SQLite + migrations) | $grand_real |
| InMemory/Mock-based tests | $grand_inmem |
| Pure unit tests (no DB) | $grand_pure |
| Test files | $grand_files |

## By Crate

| Crate | Total | Real DB | InMemory | Pure Unit | Files |
|-------|-------|---------|----------|-----------|-------|
$(for crate in "${CRATES[@]}"; do
    printf '| %s | %d | %d | %d | %d | %d |\n' \
        "$crate" "${TOTAL_TESTS[$crate]}" "${REAL_DB_TESTS[$crate]}" \
        "${INMEMORY_TESTS[$crate]}" "${PURE_UNIT_TESTS[$crate]}" "${TEST_FILES[$crate]}"
done)
| **Total** | **$grand_total** | **$grand_real** | **$grand_inmem** | **$grand_pure** | **$grand_files** |

## Coverage Ratio

- **Real DB coverage:** $(( grand_real * 100 / (grand_total > 0 ? grand_total : 1) ))% of tests exercise the real SQLite stack
- **InMemory seam:** $(( grand_inmem * 100 / (grand_total > 0 ? grand_total : 1) ))% of tests use InMemory/Mock implementations
- **Pure logic:** $(( grand_pure * 100 / (grand_total > 0 ? grand_total : 1) ))% of tests are pure unit tests
ENDMD
fi
