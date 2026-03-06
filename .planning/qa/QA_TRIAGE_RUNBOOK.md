# QA Triage Runbook (Local + CI)

**Bead:** `quotey-115.1.3`  
**Track:** `quotey-115.1` (Track D)  
**Status:** Active  
**Owner:** quotey engineering team

## 1. Purpose

Provide a deterministic triage workflow for quality-gate failures in local development and CI:

- reproduce failures with the same command surface used by gates
- classify failures consistently
- choose the right next action quickly
- capture evidence for handoff or escalation

This runbook complements:

- `.planning/FOUNDATION_QUALITY_GATES.md`
- `.planning/qa/QA_POLICY.md`
- `scripts/quality-gates.sh`

## 2. Deterministic Execution Baseline

Use repo-root execution and workspace-local temp/target directories:

```bash
scripts/quality-gates.sh
```

By default, the gate script sets:

- `TMPDIR=/data/projects/quotey/.tmp` (repo-local equivalent)
- `CARGO_TARGET_DIR=/data/projects/quotey/.tmp-target` (repo-local equivalent)

Override only when necessary:

- `QUOTEY_TMPDIR_OVERRIDE`
- `QUOTEY_CARGO_TARGET_DIR_OVERRIDE`

## 2.1 QA Dashboard Artifact Location

Each `scripts/quality-gates.sh` execution now publishes run-scoped artifacts under:

- `.planning/qa/reports/run-<RUN_ID>/QUALITY_GATE_SUMMARY.json`
- `.planning/qa/reports/run-<RUN_ID>/QUALITY_GATE_SUMMARY.md`

Latest-run pointer:

- `.planning/qa/reports/LATEST_RUN_ID`

Note: `.planning/qa/reports/` stores runtime artifacts and is intentionally git-ignored except for the directory-level `.gitignore` file.

Quick inspection commands:

```bash
cat .planning/qa/reports/LATEST_RUN_ID
latest="$(cat .planning/qa/reports/LATEST_RUN_ID)"
cat ".planning/qa/reports/${latest}/QUALITY_GATE_SUMMARY.md"
```

## 3. Fast Failure Intake

1. Capture failing gate name from the first `FAIL` line in gate output.
2. Record:
   - commit SHA
   - failing gate(s)
   - command used
   - first error line
3. Re-run only the failing gate to confirm reproducibility:

```bash
scripts/quality-gates.sh <gate>
```

If unreproducible, classify as potential flake/environment issue and continue with Section 6.

## 4. Gate-Specific Reproduction Commands

Use these commands exactly (matching gate behavior):

- `build`
```bash
cargo build --workspace
```

- `fmt`
```bash
cargo fmt --all -- --check
```

- `clippy`
```bash
cargo lint || cargo clippy --workspace --all-targets -- -D warnings
```

- `tests`
```bash
cargo test --workspace
```

- `deny`
```bash
cargo deny check
```

- `doc`
```bash
cargo doc --workspace --no-deps
```

When the failing target is broad, narrow to the affected crate/test first, then rerun full gate.

## 5. Failure Classification Taxonomy

Apply exactly one primary class:

1. `code_regression`
   - deterministic failure introduced by behavior/type/signature change
2. `test_defect`
   - expected behavior is correct, test expectation/setup is wrong
3. `env_or_toolchain`
   - missing tool/version mismatch/disk pressure/path issues
4. `data_fixture_drift`
   - seed/migration/fixture mismatch, stale assumptions
5. `flake_or_timing`
   - non-deterministic order/race/time-sensitive behavior
6. `policy_or_dependency`
   - license/advisory/policy denial from `cargo deny`

## 6. Next-Action Matrix

| Class | Immediate Action | Follow-up |
|---|---|---|
| `code_regression` | Fix root cause in code and add/adjust targeted tests | Rerun failing gate, then full `scripts/quality-gates.sh` |
| `test_defect` | Fix test setup/expectation (not production behavior) | Add note in test explaining invariant |
| `env_or_toolchain` | Normalize environment (tool install, paths, local temp/target) | Document in README/troubleshooting if recurring |
| `data_fixture_drift` | Regenerate/update deterministic fixtures or migration assumptions | Add fixture contract assertions |
| `flake_or_timing` | Stabilize with deterministic input ordering/time control | Add bead if fix is non-trivial or cross-cutting |
| `policy_or_dependency` | Remediate denied crate/advisory/license | Record dependency decision rationale |

## 7. CI Triage Workflow

1. Identify failing job and gate step from CI logs.
2. Extract the exact command from that step.
3. Reproduce locally with the same command and local temp/target defaults.
4. Compare local vs CI dimensions:
   - rust/cargo version
   - OS/runtime differences
   - dependency lockfile deltas
5. If CI-only:
   - capture failing job URL + step name + first failure block
   - open/update a bead with class from Section 5
   - add mitigation notes in the bead before handing off

## 8. Escalation Conditions

Escalate immediately when any condition is true:

- same gate fails 3+ times without new information
- reproducible deterministic failure affects pricing/policy/audit critical path
- `cargo deny` reports high-severity advisory with no approved exception
- CI-only failure blocks merge and cannot be reproduced within 30 minutes

Escalation target: active owner for the affected track + project thread in Agent Mail.

## 9. Closure Checklist

A triage incident is closed only when all are true:

- [ ] Failure class assigned (Section 5)
- [ ] Root cause recorded in bead update or commit message context
- [ ] Fix validated with failing gate command
- [ ] Full `scripts/quality-gates.sh` rerun succeeds (or explicit exception documented)
- [ ] Agent Mail update sent for team visibility when shared tracks are impacted

## 10. Useful Commands

```bash
# list gates
scripts/quality-gates.sh --list

# run only failing gate
scripts/quality-gates.sh tests

# generate test inventory evidence
bash scripts/test_inventory.sh --json
bash scripts/test_inventory.sh --markdown
```
