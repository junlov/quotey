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

5. `deny`
   - Command: `cargo deny check`
   - Pass condition: dependency and policy checks pass
   - Failure action: remediate denied crate/license/advisory and rerun

6. `doc`
   - Command: `cargo doc --workspace --no-deps`
   - Pass condition: docs build successfully for workspace crates
   - Failure action: fix rustdoc/build issues and rerun

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
