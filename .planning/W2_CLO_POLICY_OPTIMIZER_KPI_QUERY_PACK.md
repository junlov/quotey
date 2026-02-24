# W2 CLO Policy Optimizer KPI Query Pack

Version: `1.0.0`  
SQL artifact: `.planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.sql`  
Bead scope: `bd-lmuc.8.1`

This query pack is the canonical SQL source for CLO KPI dashboards and alerting.

## Data Contracts

- Time window and alert thresholds are configured per query in the `params` CTE.
- Realized outcomes are read from `policy_lifecycle_audit` rows where:
  - `event_type = 'monitoring_started'`
  - `event_payload_json` contains:
    - `$.realized_margin_delta_bps`
    - `$.realized_win_rate_delta_bps`
    - `$.realized_approval_latency_delta_seconds`
- Rollback MTTR reads `$.rollback_triggered_at` from `policy_rollback_record.rollback_metadata_json` when present.

## Query Catalog

| Query ID | Version | Purpose |
|---|---|---|
| `clo_kpi_summary_v1` | `v1` | One-row KPI snapshot for CLO lifecycle metrics, Wave-2 KPI contract metrics, and alert flags/reasons. |
| `clo_kpi_projected_vs_realized_detail_v1` | `v1` | Candidate-level projected vs realized outcome comparison for drill-down workflows. |

## Output Schema: `clo_kpi_summary_v1`

| Column | Type | Description |
|---|---|---|
| `window_start` | `TEXT` | Inclusive reporting window start (`UTC`). |
| `window_end` | `TEXT` | Exclusive reporting window end (`UTC`). |
| `candidate_throughput_count` | `INTEGER` | Count of `candidate_created` lifecycle events. |
| `review_decision_count` | `INTEGER` | Count of `approved/rejected/changes_requested` lifecycle events. |
| `approved_count` | `INTEGER` | Count of `approved` lifecycle events. |
| `adoption_rate_bps` | `INTEGER` | `approved_count / review_decision_count` in basis points. |
| `applied_candidate_count` | `INTEGER` | Distinct applied candidates in window. |
| `rolled_back_candidate_count` | `INTEGER` | Distinct rolled-back candidates in window. |
| `rollback_rate_bps` | `INTEGER` | `rolled_back_candidate_count / applied_candidate_count` in basis points. |
| `false_positive_candidate_count` | `INTEGER` | Distinct candidates both applied and rolled back. |
| `false_positive_rate_bps` | `INTEGER` | `false_positive_candidate_count / applied_candidate_count` in basis points. |
| `avg_approval_latency_seconds` | `INTEGER` | Avg first decision latency from first review packet per candidate. |
| `projected_outcome_sample_count` | `INTEGER` | Distinct candidates with replay evidence in window. |
| `realized_outcome_sample_count` | `INTEGER` | Distinct candidates with realized outcomes in window. |
| `projected_vs_realized_sample_size` | `INTEGER` | Overlap of projected and realized candidate sets. |
| `avg_projected_margin_delta_bps` | `INTEGER` | Avg projected margin delta (latest replay per candidate). |
| `avg_realized_margin_delta_bps` | `INTEGER` | Avg realized margin delta (latest monitoring observation per candidate). |
| `avg_projected_vs_realized_margin_gap_bps` | `INTEGER` | Avg `projected - realized` margin delta on overlap set. |
| `replayed_candidate_count` | `INTEGER` | Distinct candidates with replay evidence in window. |
| `approved_replayed_candidate_count` | `INTEGER` | Distinct replayed candidates that have approved decisions. |
| `candidate_acceptance_rate_bps` | `INTEGER` | `approved_replayed_candidate_count / replayed_candidate_count` in basis points. |
| `hard_violation_candidate_count` | `INTEGER` | Distinct replayed candidates with `hard_violation_count > 0`. |
| `hard_violation_blocked_count` | `INTEGER` | Hard-violation candidates with no approval decision. |
| `unsafe_candidate_block_rate_bps` | `INTEGER` | `hard_violation_blocked_count / hard_violation_candidate_count` in basis points. |
| `projected_realized_margin_delta_error_bps` | `INTEGER` | `abs(avg_projected_vs_realized_margin_gap_bps)`. |
| `rollback_mttr_seconds` | `INTEGER` | Avg rollback MTTR from `rollback_triggered_at` to `rolled_back_at` (when trigger timestamp exists). |
| `rollback_mttr_proxy_seconds` | `INTEGER` | Proxy MTTR using apply-to-rollback duration. |
| `rollback_count` | `INTEGER` | Rollback records in reporting window. |
| `replay_total_count` | `INTEGER` | Replay evaluations in reporting window. |
| `deterministic_pass_count` | `INTEGER` | Replay evaluations with `deterministic_pass = 1`. |
| `replay_determinism_rate_bps` | `INTEGER` | `deterministic_pass_count / replay_total_count` in basis points. |
| `realized_candidate_count` | `INTEGER` | Candidates contributing to realized outcome score. |
| `avg_realized_margin_delta_for_outcome_bps` | `INTEGER` | Avg realized margin delta used in outcome blend. |
| `avg_realized_win_rate_delta_bps` | `INTEGER` | Avg realized win-rate delta used in outcome blend. |
| `avg_realized_approval_latency_delta_seconds` | `INTEGER` | Avg realized approval latency delta used in outcome blend. |
| `weighted_realized_outcome_score` | `INTEGER` | Canonical blend: `0.40*margin + 0.40*win_rate - 0.20*latency_seconds`. |
| `max_rollback_rate_bps` | `INTEGER` | Alert threshold from `params`. |
| `max_false_positive_rate_bps` | `INTEGER` | Alert threshold from `params`. |
| `max_projected_realized_margin_gap_bps` | `INTEGER` | Alert threshold from `params`. |
| `rollback_spike_alert` | `INTEGER` | `1` when rollback rate threshold is exceeded. |
| `false_positive_alert` | `INTEGER` | `1` when false-positive threshold is exceeded. |
| `margin_gap_alert` | `INTEGER` | `1` when projected-vs-realized gap threshold is exceeded. |
| `alert_reason_codes` | `TEXT` | Comma-separated reason codes: `rollback_spike`, `false_positive_spike`, `margin_gap_drift`. |

## Output Schema: `clo_kpi_projected_vs_realized_detail_v1`

| Column | Type | Description |
|---|---|---|
| `candidate_id` | `TEXT` | Policy candidate identifier. |
| `replayed_at` | `TEXT` | Latest replay timestamp for candidate in window. |
| `monitored_at` | `TEXT` | Latest monitoring observation timestamp in window. |
| `projected_margin_delta_bps` | `INTEGER` | Latest projected margin delta from replay. |
| `realized_margin_delta_bps` | `INTEGER` | Latest realized margin delta from monitoring payload. |
| `margin_gap_bps` | `INTEGER` | `projected_margin_delta_bps - realized_margin_delta_bps`. |
| `projected_win_rate_delta_bps` | `INTEGER` | Latest projected win-rate delta from replay. |
| `realized_win_rate_delta_bps` | `INTEGER` | Latest realized win-rate delta from monitoring payload. |
| `projected_approval_latency_delta_seconds` | `INTEGER` | Latest projected approval-latency delta from replay. |
| `realized_approval_latency_delta_seconds` | `INTEGER` | Latest realized approval-latency delta from monitoring payload. |

## Alert Mapping (Direct Query Output Binding)

| Alert | Direct Expression on `clo_kpi_summary_v1` Output |
|---|---|
| Rollback spike | `rollback_spike_alert = 1` (`rollback_rate_bps > max_rollback_rate_bps`) |
| False-positive spike | `false_positive_alert = 1` (`false_positive_rate_bps > max_false_positive_rate_bps`) |
| Margin drift spike | `margin_gap_alert = 1` (`abs(avg_projected_vs_realized_margin_gap_bps) > max_projected_realized_margin_gap_bps`) |

## Operational Usage

Run the SQL from SQLite shell:

```sql
.read migrations/0015_clo_policy_optimizer.up.sql
.read .planning/W2_CLO_POLICY_OPTIMIZER_KPI_QUERY_PACK.sql
```

Use `clo_kpi_summary_v1` for daily/weekly rollups and alert checks.  
Use `clo_kpi_projected_vs_realized_detail_v1` for candidate-level drill-down and postmortems.
