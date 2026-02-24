#!/usr/bin/env bash
#
# Build a deterministic E2E environment used by QA scripts and local reproduction.
#
# Usage:
#   ./scripts/e2e_bootstrap.sh
#   CLEAN_BEFORE_BOOTSTRAP=0 ./scripts/e2e_bootstrap.sh
#   QUOTEY_E2E_DB_PATH=target/test/e2e.db ./scripts/e2e_bootstrap.sh

set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

DB_PATH="${QUOTEY_E2E_DB_PATH:-target/quotey-e2e.db}"
CLEAN_BEFORE_BOOTSTRAP="${CLEAN_BEFORE_BOOTSTRAP:-1}"
SLACK_APP_TOKEN="${QUOTEY_E2E_APP_TOKEN:-xapp-test}"
SLACK_BOT_TOKEN="${QUOTEY_E2E_BOT_TOKEN:-xoxb-test}"
LOCAL_TARGET_DIR="${ROOT_DIR}/.tmp-target"
LOCAL_TMP_DIR="${LOCAL_TARGET_DIR}/tmp"

if command -v realpath >/dev/null 2>&1; then
  ABS_DB_PATH="$(realpath "$DB_PATH" 2>/dev/null || true)"
  if [[ -z "${ABS_DB_PATH}" ]]; then
    mkdir -p "$(dirname "$DB_PATH")"
    ABS_DB_PATH="$(realpath -m "$DB_PATH")"
  fi
else
  mkdir -p "$(dirname "$DB_PATH")"
  ABS_DB_PATH="$DB_PATH"
fi

DB_URL="sqlite://$ABS_DB_PATH"
if [[ "$DB_URL" == *"?"* ]]; then
  DB_URL="${DB_URL}&mode=rwc"
else
  DB_URL="${DB_URL}?mode=rwc"
fi
E2E_SEED_FILE="config/fixtures/e2e_seed_data.sql"

if [[ ! -f "$E2E_SEED_FILE" ]]; then
  echo "[e2e] expected seed file missing: $E2E_SEED_FILE" >&2
  exit 1
fi

if [[ "$CLEAN_BEFORE_BOOTSTRAP" == "1" ]]; then
  rm -f "$ABS_DB_PATH" "$ABS_DB_PATH-shm" "$ABS_DB_PATH-wal"
fi

mkdir -p "$(dirname "$ABS_DB_PATH")"

mkdir -p "$LOCAL_TARGET_DIR" "$LOCAL_TMP_DIR"
export CARGO_TARGET_DIR="$LOCAL_TARGET_DIR"
export TMPDIR="$LOCAL_TMP_DIR"
export QUOTEY_DATABASE_URL="$DB_URL"
export QUOTEY_SLACK_APP_TOKEN="$SLACK_APP_TOKEN"
export QUOTEY_SLACK_BOT_TOKEN="$SLACK_BOT_TOKEN"

echo "[e2e] bootstrapping database: $DB_URL"
echo "[e2e] seed fixture: $E2E_SEED_FILE"
cargo run -p quotey-cli -- migrate
cargo run -p quotey-cli -- seed

if command -v sqlite3 >/dev/null 2>&1; then
  echo "[e2e] seed summary:"
  sqlite3 "$ABS_DB_PATH" <<'SQL'
SELECT
  'quotes=' || COUNT(*) FROM quote
UNION ALL
SELECT 'quote_lines=' || COUNT(*) FROM quote_line
UNION ALL
SELECT 'flow_states=' || COUNT(*) FROM flow_state
UNION ALL
SELECT 'audit_events=' || COUNT(*) FROM audit_event;
SQL
else
  echo "[e2e] sqlite3 not installed; skipping local seed summary"
fi

echo "[e2e] ready"
