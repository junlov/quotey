# W1 SIM Deal Flight Simulator - Operator Runbook

## Overview

The Deal Flight Simulator (SIM) provides deterministic quote scenario exploration in Slack.
It allows reps to test bounded what-if variants, compare price/policy/approval deltas,
and explicitly promote a selected variant.

This runbook covers SIM operational checks, incident triage, and rollout gate evidence.

---

## Architecture Summary

### Deterministic Path

```
/quote simulate -> command parser -> DealFlightSimulator
               -> deterministic CPQ runtime (constraints/pricing/policy)
               -> ranked scenario comparison
               -> telemetry event emission + scenario persistence
               -> Slack comparison card / promotion actions
```

### Key Components

| Component | Purpose | Location |
|-----------|---------|----------|
| `DealFlightSimulator` | deterministic variant generation/comparison | `crates/core/src/cpq/simulator.rs` |
| `Scenario*` domain types | run/variant/delta/audit + telemetry contracts | `crates/core/src/domain/simulation.rs` |
| `SqlScenarioRepository` | SIM persistence (run/variant/delta/audit/promotion) | `crates/db/src/repositories/simulation.rs` |
| SIM migration | scenario tables/indexes | `migrations/0014_deal_flight_simulator.*.sql` |
| Slack SIM command/actions | parser + comparison card + promote action payloads | `crates/slack/src/commands.rs`, `crates/slack/src/blocks.rs` |

---

## Telemetry Contract

### Event Taxonomy

- `request_received`
- `comparison_rendered`
- `error_occurred`
- `promotion_requested`
- `promotion_applied`

### Required Fields

- `quote_id`
- `correlation_id`
- `scenario_run_id` (nullable)
- `variant_key` (nullable for run-level events)
- `variant_count`
- `approval_required_variant_count`
- `latency_ms`
- `outcome`
- `error_code` (nullable)
- `occurred_at`

### Counters

- `sim_requests_total`
- `sim_success_total`
- `sim_failures_total`
- `sim_variants_generated_total`
- `sim_approval_required_variants_total`
- `sim_promotions_requested_total`
- `sim_promotions_applied_total`

---

## KPIs and Alert Triggers

| Metric | Target | Trigger |
|--------|--------|---------|
| Scenario success rate | >= 98% | < 95% over 1h |
| P95 SIM latency | <= 800ms | > 1000ms over 30m |
| Deterministic parity mismatch | 0% | any non-zero mismatch |
| Promotion failure rate | <= 2% | > 5% over 1h |

---

## Operational Queries (SQLite)

```sql
-- Scenario run outcomes in last 24h
SELECT status, COUNT(*) AS count
FROM deal_flight_scenario_run
WHERE created_at > datetime('now', '-1 day')
GROUP BY status;

-- Scenario variants requiring approvals
SELECT
  r.quote_id,
  v.variant_key,
  v.rank_order,
  v.policy_result_json
FROM deal_flight_scenario_variant v
JOIN deal_flight_scenario_run r ON r.id = v.scenario_run_id
WHERE v.policy_result_json LIKE '%approval_required%'
ORDER BY r.created_at DESC, v.rank_order ASC;

-- Promotion activity
SELECT
  r.quote_id,
  COUNT(CASE WHEN v.selected_for_promotion = 1 THEN 1 END) AS promoted_variants,
  COUNT(*) AS total_variants
FROM deal_flight_scenario_variant v
JOIN deal_flight_scenario_run r ON r.id = v.scenario_run_id
GROUP BY r.quote_id
ORDER BY promoted_variants DESC;

-- Scenario audit event distribution
SELECT event_type, COUNT(*) AS count
FROM deal_flight_scenario_audit
WHERE occurred_at > datetime('now', '-1 day')
GROUP BY event_type
ORDER BY count DESC;
```

---

## Common Scenarios

### 1) Guardrail Rejections Spike

Symptoms:
- Increased `error_occurred` events
- More failed scenario runs

Checks:
1. Inspect run status distribution query.
2. Inspect SIM audit payloads for `error_code` trends.
3. Verify command payload grammar changes in Slack command parser.

Response:
1. If payload regressions are present, roll back recent parser changes.
2. If valid user behavior exceeds bounds, review guardrail thresholds.
3. Capture examples and add deterministic test cases before redeploy.

### 2) Latency Regression

Symptoms:
- P95 SIM latency above 800ms target

Checks:
1. Verify scenario count per request is bounded (`<= 3` default).
2. Confirm no expensive non-deterministic downstream calls were introduced.
3. Re-run simulator targeted tests with timing capture locally.

Response:
1. Re-enable strict variant bound and fail-fast path.
2. Defer expensive enrichments to non-blocking post-response pathways.
3. Add regression test to cover the slow path.

### 3) Promotion Inconsistency

Symptoms:
- Multiple variants appear promoted for same run
- Retry duplicates during promotion actions

Checks:
1. Query `selected_for_promotion` by `scenario_run_id`.
2. Verify promote action payload idempotency parse path.
3. Confirm repository `promote_variant` transaction behavior.

Response:
1. Re-run `promote_variant` path tests.
2. Backfill promotion correction where needed.
3. Add missing idempotency key checks if regression is confirmed.

---

## Validation Matrix

Run for release confidence:

```bash
cargo test -p quotey-core cpq::simulator::tests::
cargo test -p quotey-core domain::simulation::tests::
cargo test -p quotey-db repositories::simulation::tests::
cargo test -p quotey-slack --all-targets
```

Expected:
- All tests pass.
- SIM command + comparison + promotion action tests pass in slack crate.
- SIM repository lifecycle/promotion tests pass in db crate.

---

## Rollback Notes

If SIM behavior regresses after deploy:
1. Disable SIM command route at orchestration layer.
2. Keep historical scenario tables intact for auditability.
3. Roll back to last known good commit where the validation matrix is green.
4. Re-run full SIM validation matrix before re-enabling route.

---

## References

- Spec: `.planning/W1_SIM_DEAL_FLIGHT_SIMULATOR_SPEC.md`
- Demo checklist: `.planning/W1_SIM_DEAL_FLIGHT_SIMULATOR_DEMO_CHECKLIST.md`
- Core simulator: `crates/core/src/cpq/simulator.rs`
- Domain contracts: `crates/core/src/domain/simulation.rs`
- Repository: `crates/db/src/repositories/simulation.rs`

---

*Last Updated: 2026-02-24*
*Version: 1.0*
