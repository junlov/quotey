# W2 CLO Closed-Loop Policy Optimizer Demo Checklist

Version: `1.0.0`  
Scope beads: `bd-lmuc.10`, `bd-lmuc.10.1`

## Purpose

Provide a reproducible end-to-end CLO demonstration and rollout gate checklist
covering candidate generation, replay evidence, approval actions, signed
apply/rollback, monitoring telemetry, and rollback drill safety.

## Preconditions

1. Run from repo root: `/data/projects/quotey`.
2. Rust toolchain is installed and working.
3. CLO migrations and code are present on current branch.
4. No destructive filesystem/git actions are required.

## Scripted Demo Flow

Run in order.

### Step 1: Candidate Generation + Replay Evidence

```bash
cargo test -p quotey-core policy::optimizer::tests::candidate_generator_is_deterministic_and_links_replay_evidence -- --nocapture
```

Expected:
- candidate generation is deterministic for identical inputs.
- generated candidate references replay checksum evidence.

Failure handling:
1. Inspect replay checksum linkage and canonical JSON paths in `crates/core/src/policy/optimizer.rs`.
2. Re-run with `-- --nocapture` and capture failing assertion payload.

### Step 2: Safety/Red-Team Blocking

```bash
cargo test -p quotey-core policy::optimizer::tests::candidate_generator_rejects_red_team_policy_bypass -- --nocapture
```

Expected:
- control-surface bypass candidate is blocked with explicit reason.

Failure handling:
1. Check policy-bypass scenario evaluator and risky-surface detection.
2. Re-validate candidate diff schema and red-team artifact linkage.

### Step 3: Approval Packet Contract

```bash
cargo test -p quotey-core policy::optimizer::tests::approval_packet_validation_enforces_required_sections -- --nocapture
cargo test -p quotey-cli commands::policy_packet::tests::run_action_requires_reason_for_reject -- --nocapture
```

Expected:
- packet contract enforces required sections/versioning.
- reject/request-changes action path requires reason.

Failure handling:
1. Verify packet/action schema versions and required section guards.
2. Confirm CLI action parser rejects missing reason for reject/request-changes.

### Step 4: Signed Apply + Rollback Lifecycle

```bash
cargo test -p quotey-core policy::optimizer::tests::apply_and_rollback_are_idempotent_with_queryable_audit_history -- --nocapture
```

Expected:
- apply and rollback are idempotent.
- lifecycle events remain queryable and immutable.

Failure handling:
1. Check idempotency key normalization and conflict checks.
2. Verify apply/rollback audit event emission and record lookup paths.

### Step 5: Monitoring + KPI/Alert Determinism

```bash
cargo test -p quotey-core policy::optimizer::tests::lifecycle_kpis_compute_deterministic_rates_latency_and_alerts -- --nocapture
```

Expected:
- KPI rates/latency/margin calculations remain deterministic.
- rollback/false-positive/margin-gap alert flags evaluate correctly.

Failure handling:
1. Inspect candidate-set normalization and overlap calculations.
2. Compare thresholds and alert reason generation output.

### Step 6: Rollback Drill Safety

```bash
cargo test -p quotey-core policy::optimizer::tests::rollback_drill_report_contains_timing_checksum_and_safety_artifacts -- --nocapture
```

Expected:
- rollback drill report includes timing metrics, checksums, and safety pass bit.

Failure handling:
1. Verify apply -> rollback -> reapply sequence contract.
2. Confirm final policy version and checksum chain integrity.

### Step 7: Quality Gates (Targeted)

```bash
cargo clippy -p quotey-core -p quotey-cli --all-targets -- -D warnings
ubs crates/core
```

Expected:
- clippy returns zero warnings for targeted crates.
- UBS findings are triaged for changed core scope; legitimate critical/runtime findings must be remediated before rollout.

Failure handling:
1. Fix clippy warnings first (blocking gate).
2. Triage UBS findings; separate heuristics/false positives from real runtime defects.
3. Remediate legitimate critical/runtime-impacting issues and document accepted false positives.

### Step 8: Telemetry Query-Pack Sanity Check

```bash
python3 - <<'PY'
import sqlite3
from pathlib import Path
conn = sqlite3.connect(':memory:')
conn.executescript(Path('migrations/0015_clo_policy_optimizer.up.sql').read_text())
conn.executescript(Path('.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.sql').read_text())
print('clo_query_pack_ok')
PY
```

Expected:
- query pack executes without SQL errors on CLO schema.

Failure handling:
1. Fix SQL syntax/column mismatches in query pack.
2. Re-run parser sanity check and update output schema docs if needed.

## Acceptance Mapping

| CLO acceptance criterion | Evidence artifact |
|---|---|
| Candidate generation -> replay -> approval -> apply -> monitor -> rollback drill flow is reproducible | Steps 1-6 in this checklist |
| Quality gates include targeted tests, clippy, UBS on changed scopes | Step 7 |
| KPI/telemetry and projected-vs-realized analysis are concretely queryable | Step 8 + `.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.sql` |
| Safety controls and kill-switch behavior are documented for operators | `.planning/W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_RUNBOOK.md` |

## Rollout Gate (Go/No-Go)

Go only if all are true:
1. Steps 1-8 pass in current release candidate.
2. No open CLO P0/P1 defects in `br`.
3. `rollback_spike_alert = 0`, `false_positive_alert = 0`, and `margin_gap_alert = 0` in latest summary window.
4. Safety Owner and Runtime Owner both sign off.

No-Go triggers:
1. Any failing deterministic lifecycle/replay/rollback drill test.
2. Any unresolved critical UBS finding in changed runtime paths.
   Heuristic false positives are allowed only with explicit triage notes.
3. Any active safety-pause incident without documented closure.

Sign-off roles:
- Runtime Owner: implementation and rollback readiness.
- Safety Owner: control-surface and incident-response readiness.
- Revenue Ops Owner: KPI and business-impact readiness.

## Known Gaps / Deferred Work

1. Dedicated `policy-opt` CLI commands for apply/rollback/control toggles are still pending; current demo exercises engine-level contracts and packet CLI pathways.
2. `rollback_triggered_at` metadata coverage is not universal yet, so MTTR reporting may fall back to proxy measurements.
3. External dashboard automation is still follow-up work; current evidence is SQL query-pack level.

## References

- Spec: `.planning/W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_SPEC.md`
- Runbook: `.planning/W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_RUNBOOK.md`
- KPI query pack SQL: `.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.sql`
- KPI query pack docs: `.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.md`
- Core lifecycle/optimizer module: `crates/core/src/policy/optimizer.rs`
- CLI packet controls: `crates/cli/src/commands/policy_packet.rs`
