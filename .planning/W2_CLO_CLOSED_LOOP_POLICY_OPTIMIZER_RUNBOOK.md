# W2 CLO Closed-Loop Policy Optimizer - Operator Runbook

Version: `1.0.0`  
Scope bead: `bd-lmuc.9`

This runbook defines operator controls, audit expectations, incident procedures,
and safety-pause behavior for the CLO lifecycle.

---

## 1) Operational Control Matrix

| Control ID | Control | Interface | Allowed Roles | Audit Artifact | Status |
|---|---|---|---|---|---|
| `CLO-CTRL-001` | Build review packet from deterministic contracts | `quotey policy-packet build` | Runtime Owner, Revenue Ops | packet JSON (`packet_id`, replay checksum, candidate id/version) | Implemented |
| `CLO-CTRL-002` | Submit review decision (`approve/reject/request_changes`) | `quotey policy-packet action` | Runtime Owner, Revenue Ops, Safety Owner | action JSON + deterministic idempotency key + audit event payload | Implemented |
| `CLO-CTRL-003` | Execute signed apply pipeline | Core lifecycle engine (`InMemoryPolicyLifecycleEngine::apply`) | Runtime Owner | `policy_apply_record` + `policy_lifecycle_audit(event_type='applied')` | Implemented (engine), CLI surface deferred |
| `CLO-CTRL-004` | Execute signed rollback pipeline | Core lifecycle engine (`InMemoryPolicyLifecycleEngine::rollback`) | Runtime Owner, Safety Owner | `policy_rollback_record` + `policy_lifecycle_audit(event_type='rolled_back')` | Implemented (engine), CLI surface deferred |
| `CLO-CTRL-005` | Run rollback drill automation | Core lifecycle engine (`run_rollback_drill`) | Runtime Owner | `RollbackDrillReport` artifact (timings/checksums/safety bit) | Implemented |
| `CLO-CTRL-006` | Safety pause / kill switch (freeze apply path) | Operational procedure (Section 4) | Safety Owner, Runtime Owner | incident ticket + candidate decision logs + rollback audit chain | Implemented procedurally |

Role policy:
- Runtime Owner is primary authority for apply/rollback operations.
- Safety Owner can trigger/maintain safety pause and require rollback.
- Revenue Ops can prepare/review packets but should not execute apply/rollback directly.

---

## 2) KPI Monitoring and Alert Checks

Primary telemetry source:
- `.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.sql`
  - `clo_kpi_summary_v1`
  - `clo_kpi_projected_vs_realized_detail_v1`

Alert fields from `clo_kpi_summary_v1`:
- `rollback_spike_alert`
- `false_positive_alert`
- `margin_gap_alert`
- `alert_reason_codes`

Daily operator check:
1. Run query pack summary.
2. Record the one-row snapshot in ops notes/ticket.
3. If any alert column is `1`, follow Section 3 incident flow.

Weekly operator check:
1. Run rollback drill (Section 5 validation command).
2. Confirm:
   - non-empty verification checksums,
   - `safety_passed = true`,
   - final policy version equals expected reapplied version.
3. Attach drill output to the weekly CLO governance record.

---

## 3) Incident Triage Playbook

### 3.1 Rollback Spike

Trigger:
- `rollback_spike_alert = 1`

Immediate actions:
1. Engage safety pause (Section 4).
2. Inspect `clo_kpi_projected_vs_realized_detail_v1` for largest absolute `margin_gap_bps`.
3. Identify impacted candidates and verify replay checksums and approval decision lineage.
4. For active harmful candidates, execute rollback pipeline.

Exit criteria:
- rollback spike clears for one full reporting window.
- safety owner signs off to release pause.

### 3.2 False-Positive Spike

Trigger:
- `false_positive_alert = 1`

Immediate actions:
1. Engage safety pause.
2. Compare applied vs rolled-back candidate sets from summary metrics.
3. Audit approval rationales and red-team safety artifacts for affected candidates.
4. Tighten candidate generation thresholds before resuming apply path.

Exit criteria:
- false-positive rate below threshold across one reporting window.

### 3.3 Projected vs Realized Margin Drift

Trigger:
- `margin_gap_alert = 1`

Immediate actions:
1. Pause new applies until drift is explained.
2. Use detail query to identify largest drift candidates.
3. Validate that monitoring payloads include expected realized fields.
4. Run replay re-evaluation for affected candidates and compare checksums/artifacts.

Exit criteria:
- drift alert clears and root cause is documented.

---

## 4) Safety Pause / Kill Switch Procedure

Objective:
- stop policy applies quickly while preserving full auditability.

Procedure:
1. Open incident ticket tagged `CLO-SAFETY-PAUSE`.
2. Safety Owner declares pause in incident channel and references trigger metric(s).
3. Runtime Owner blocks all new apply operations operationally:
   - no `approve` actions may be promoted to apply during pause,
   - only `reject`/`request_changes` actions proceed.
4. Run targeted validation (Section 5) to verify control-surface protections still pass.
5. If damage exists, execute signed rollback for affected candidates.
6. Release pause only after:
   - trigger metrics clear,
   - remediation is documented,
   - Safety Owner and Runtime Owner both approve release.

Audit requirements:
- incident ticket link,
- command/test outputs,
- affected candidate IDs,
- rollback audit IDs (if any),
- release approval decision record.

---

## 5) Deterministic Validation Matrix

Run during weekly checks and all safety-pause incidents:

```bash
cargo test -p quotey-core policy::optimizer::tests::rollback_drill_report_contains_timing_checksum_and_safety_artifacts -- --nocapture
cargo test -p quotey-core policy::optimizer::tests::candidate_generator_rejects_red_team_policy_bypass -- --nocapture
cargo test -p quotey-core policy::optimizer::tests::lifecycle_kpis_compute_deterministic_rates_latency_and_alerts -- --nocapture
cargo test -p quotey-cli commands::policy_packet::tests::run_action_requires_reason_for_reject -- --nocapture
```

What these prove:
- rollback drill remains deterministic and auditable.
- control-surface bypass attempts are blocked.
- KPI/alert formulas remain deterministic.
- operator decision path enforces reason requirement for reject/request-changes.

---

## 6) Known Gaps and Deferred Work

| Gap | Impact | Owner | Follow-up |
|---|---|---|---|
| No dedicated `quotey policy-opt apply/rollback/control` CLI commands yet | Operators rely on library/runtime integration and procedural controls | Runtime Owner | Add explicit CLI control surface in CLO follow-up slice |
| `rollback_triggered_at` is not universally emitted in rollback metadata payloads | `rollback_mttr_seconds` may be sparse; proxy metric needed | Core Owner | Emit trigger timestamp in rollback metadata contract |
| Role-gate enforcement is procedural in this runbook, not a centralized runtime authorizer | Policy depends on process discipline | Security Owner + Runtime Owner | Add explicit role-authorizer contract and integration tests |

---

## 7) References

- CLO spec: `.planning/W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_SPEC.md`
- CLO KPI query pack SQL: `.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.sql`
- CLO KPI query pack schema docs: `.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.md`
- Core lifecycle engine: `crates/core/src/policy/optimizer.rs`
- CLI policy packet controls: `crates/cli/src/commands/policy_packet.rs`
