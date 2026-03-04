# Foundation Quality Gate Matrix

This runbook defines the automated quality-gate matrix for foundation closure work.

Primary command:

```bash
scripts/quality-gates.sh
```

The script is deterministic and executes from repo root with:

- `TMPDIR` defaulting to `.tmp`
- `CARGO_TARGET_DIR` defaulting to `.tmp-target`

By default, the script forces workspace-local temp/target paths so host-level `/tmp` pressure
does not break gate execution.

Explicit overrides are supported when needed:

- `QUOTEY_TMPDIR_OVERRIDE`
- `QUOTEY_CARGO_TARGET_DIR_OVERRIDE`
- `QUOTEY_FAIL_FAST` (default `1`)

## Gate Matrix

1. `build`
   - Command: `cargo build --workspace`
   - Pass condition: workspace compiles
   - Failure action: fix compile errors and rerun

2. `fmt`
   - Command: `cargo fmt --all -- --check`
   - Pass condition: no formatting diffs
   - Failure action: run `cargo fmt --all` and recommit

3. `clippy`
   - Preferred command: `cargo lint`
   - Fallback command: `cargo clippy --workspace --all-targets -- -D warnings`
   - Pass condition: zero warnings/errors
   - Failure action: fix lint findings and rerun

4. `tests`
   - Command: `cargo test --workspace`
   - Pass condition: all unit/integration/doc tests pass
   - Failure action: fix regressions, rerun targeted tests, then rerun full matrix

5. `qa`
   - Command: `scripts/quality-gates.sh qa` (invokes QA threshold checks)
   - Pass condition:
     - real-DB coverage ratio meets threshold
     - open P0/P1 critical-path gap count is within threshold
     - E2E scenario suite meets pass-rate threshold
     - log-validator case count meets threshold
   - Failure action: fix threshold violation source (tests/policy/gaps), then rerun

6. `deny`
   - Command: `cargo deny check`
   - Pass condition: dependency and policy checks pass
   - Failure action: remediate denied crate/license/advisory and rerun

7. `doc`
   - Command: `cargo doc --workspace --no-deps`
   - Pass condition: docs build successfully for workspace crates
   - Failure action: fix rustdoc/build issues and rerun

## QA Threshold Contract

Default threshold environment variables:

| Variable | Default | Meaning |
|---|---:|---|
| `QUOTEY_THRESHOLD_REAL_DB_PCT` | `20` | Minimum real-DB test ratio from `scripts/test_inventory.sh --json` |
| `QUOTEY_THRESHOLD_CRITICAL_PATH_GAPS_MAX` | `0` | Max allowed open P0/P1 gaps in `.planning/qa/CRITICAL_PATH_MATRIX.md` |
| `QUOTEY_THRESHOLD_E2E_PASS_PCT` | `100` | Required pass rate for `cargo test -p quotey-db --test e2e_scenarios` |
| `QUOTEY_THRESHOLD_LOG_VALIDATOR_CASES_MIN` | `5` | Minimum count of `validate_e2e_log_records(&...)` call sites |

## Actionable Failure Output Contract

The script prints explicit gate lifecycle markers:

- `START <gate>`
- `PASS <gate>`
- `FAIL <gate>: fix the reported issue and rerun scripts/quality-gates.sh`

With fail-fast enabled (`QUOTEY_FAIL_FAST=1`, default), execution stops at the first
gate failure so remediation can happen immediately.

## Migration Reversibility Workflow (FND-10d)

Migration reversibility is verified with deterministic up/down/up tests in:

- `crates/db/src/migrations.rs`
- test: `migrations_up_down_up_preserves_schema_signature`

Execution command:

```bash
TMPDIR=/data/projects/quotey/.tmp CARGO_TARGET_DIR=target cargo test -p quotey-db migrations_up_down_up_preserves_schema_signature
```

Pass contract:

1. Initial `up` creates all managed schema objects.
2. Full `down` removes all managed objects.
3. Second `up` recreates an identical schema signature.
