# Foundation Quality Gate Matrix

This runbook defines the automated quality-gate matrix for foundation closure work.

Primary command:

```bash
scripts/quality-gates.sh
```

The script is deterministic and executes from repo root with:

- `TMPDIR` defaulting to `.tmp`
- `CARGO_TARGET_DIR` defaulting to `target`

This avoids failures caused by host-level `/tmp` pressure and keeps artifacts local to the workspace.

## Gate Matrix

1. `fmt`
   - Command: `cargo fmt --all -- --check`
   - Pass condition: no formatting diffs
   - Failure action: run `cargo fmt --all` and recommit

2. `lint`
   - Preferred command: `cargo lint`
   - Fallback command: `cargo clippy --workspace --all-targets -- -D warnings`
   - Pass condition: zero warnings/errors
   - Failure action: fix lint findings and rerun

3. `tests`
   - Command: `cargo test --workspace`
   - Pass condition: all unit/integration/doc tests pass
   - Failure action: fix regressions, rerun targeted tests, then rerun full matrix

4. `deny`
   - Command: `cargo deny check`
   - Pass condition: dependency and policy checks pass
   - Failure action: remediate denied crate/license/advisory and rerun

## Actionable Failure Output Contract

The script prints explicit gate lifecycle markers:

- `START <gate>`
- `PASS <gate>`
- `FAIL <gate>: fix the reported issue and rerun scripts/quality-gates.sh`

This ensures failures are immediately attributable to a single gate.

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
