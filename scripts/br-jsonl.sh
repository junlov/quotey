#!/usr/bin/env bash
set -euo pipefail

# Wrapper for Beads issue tracker operations.
# Current environment issue: br DB mode fails with a runtime SQL aggregate error.
# Use JSONL mode as a stable workaround until the upstream br db-mode bug is fixed.

exec br --no-db "$@"
