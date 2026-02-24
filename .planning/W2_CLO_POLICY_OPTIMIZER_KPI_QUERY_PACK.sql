-- W2 CLO Policy Optimizer KPI Query Pack
-- Version: 1.0.0
-- Scope: bd-lmuc.8.1 ([CLO] Subtask 8a KPI Query Pack)
--
-- This pack provides canonical SQLite queries for CLO telemetry snapshots and
-- projected-vs-realized analysis. The query outputs are designed to map
-- directly to the KPI formulas documented in:
--   .planning/W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_SPEC.md
--
-- Time-window and threshold values are intentionally explicit and centralized
-- in each query via a `params` CTE so operators can tune windows/thresholds
-- without rewriting formulas.

-- ============================================================================
-- Query ID: clo_kpi_summary_v1
-- Purpose:
--   One-row KPI summary for CLO lifecycle telemetry, Wave-2 KPI contract, and
--   alert booleans/reasons.
--
-- Notes:
-- - Realized metrics are sourced from `policy_lifecycle_audit` records with
--   `event_type = 'monitoring_started'` and payload fields:
--     $.realized_margin_delta_bps
--     $.realized_win_rate_delta_bps
--     $.realized_approval_latency_delta_seconds
-- - Rollback MTTR uses `rollback_metadata_json.rollback_triggered_at` when
--   present; otherwise `rollback_mttr_proxy_seconds` is provided from apply->rollback.
-- ============================================================================
WITH
params AS (
    SELECT
        datetime('now', '-30 days') AS window_start,
        datetime('now') AS window_end,
        1500 AS max_rollback_rate_bps,
        1000 AS max_false_positive_rate_bps,
        200 AS max_projected_realized_margin_gap_bps
),
audit_window AS (
    SELECT a.*
    FROM policy_lifecycle_audit a
    JOIN params p
      ON datetime(a.occurred_at) >= p.window_start
     AND datetime(a.occurred_at) < p.window_end
),
candidate_counts AS (
    SELECT
        COUNT(CASE WHEN event_type = 'candidate_created' THEN 1 END) AS candidate_throughput_count,
        COUNT(CASE WHEN event_type IN ('approved', 'rejected', 'changes_requested') THEN 1 END) AS review_decision_count,
        COUNT(CASE WHEN event_type = 'approved' THEN 1 END) AS approved_count
    FROM audit_window
),
review_started AS (
    SELECT
        candidate_id,
        MIN(datetime(occurred_at)) AS review_packet_built_at
    FROM audit_window
    WHERE event_type = 'review_packet_built'
    GROUP BY candidate_id
),
first_decision AS (
    SELECT
        candidate_id,
        MIN(datetime(occurred_at)) AS first_decision_at
    FROM audit_window
    WHERE event_type IN ('approved', 'rejected', 'changes_requested')
    GROUP BY candidate_id
),
approval_latency AS (
    SELECT
        AVG(
            CASE
                WHEN strftime('%s', d.first_decision_at) >= strftime('%s', r.review_packet_built_at)
                    THEN strftime('%s', d.first_decision_at) - strftime('%s', r.review_packet_built_at)
                ELSE 0
            END
        ) AS avg_approval_latency_seconds
    FROM review_started r
    JOIN first_decision d
      ON d.candidate_id = r.candidate_id
),
applied_candidates AS (
    SELECT DISTINCT candidate_id
    FROM audit_window
    WHERE event_type = 'applied'
),
rolled_back_candidates AS (
    SELECT DISTINCT candidate_id
    FROM audit_window
    WHERE event_type = 'rolled_back'
),
rollback_stats AS (
    SELECT
        (SELECT COUNT(*) FROM applied_candidates) AS applied_candidate_count,
        (SELECT COUNT(*) FROM rolled_back_candidates) AS rolled_back_candidate_count,
        (
            SELECT COUNT(*)
            FROM applied_candidates a
            JOIN rolled_back_candidates r
              ON r.candidate_id = a.candidate_id
        ) AS false_positive_candidate_count
),
latest_replay AS (
    SELECT
        r.candidate_id,
        r.projected_margin_delta_bps,
        r.projected_win_rate_delta_bps,
        r.projected_approval_latency_delta_seconds,
        r.hard_violation_count,
        r.deterministic_pass,
        datetime(r.replayed_at) AS replayed_at,
        ROW_NUMBER() OVER (
            PARTITION BY r.candidate_id
            ORDER BY datetime(r.replayed_at) DESC, r.id DESC
        ) AS row_num
    FROM policy_replay_evaluation r
    JOIN params p
      ON datetime(r.replayed_at) >= p.window_start
     AND datetime(r.replayed_at) < p.window_end
),
projected_outcomes AS (
    SELECT
        candidate_id,
        projected_margin_delta_bps,
        projected_win_rate_delta_bps,
        projected_approval_latency_delta_seconds,
        hard_violation_count,
        deterministic_pass
    FROM latest_replay
    WHERE row_num = 1
),
latest_realized AS (
    SELECT
        a.candidate_id,
        CAST(json_extract(a.event_payload_json, '$.realized_margin_delta_bps') AS INTEGER) AS realized_margin_delta_bps,
        CAST(json_extract(a.event_payload_json, '$.realized_win_rate_delta_bps') AS INTEGER) AS realized_win_rate_delta_bps,
        CAST(json_extract(a.event_payload_json, '$.realized_approval_latency_delta_seconds') AS INTEGER) AS realized_approval_latency_delta_seconds,
        datetime(a.occurred_at) AS observed_at,
        ROW_NUMBER() OVER (
            PARTITION BY a.candidate_id
            ORDER BY datetime(a.occurred_at) DESC, a.id DESC
        ) AS row_num
    FROM policy_lifecycle_audit a
    JOIN params p
      ON datetime(a.occurred_at) >= p.window_start
     AND datetime(a.occurred_at) < p.window_end
    WHERE a.event_type = 'monitoring_started'
),
realized_outcomes AS (
    SELECT
        candidate_id,
        realized_margin_delta_bps,
        realized_win_rate_delta_bps,
        realized_approval_latency_delta_seconds
    FROM latest_realized
    WHERE row_num = 1
      AND realized_margin_delta_bps IS NOT NULL
),
projected_realized AS (
    SELECT
        p.candidate_id,
        p.projected_margin_delta_bps,
        p.projected_win_rate_delta_bps,
        p.projected_approval_latency_delta_seconds,
        r.realized_margin_delta_bps,
        r.realized_win_rate_delta_bps,
        r.realized_approval_latency_delta_seconds,
        (p.projected_margin_delta_bps - r.realized_margin_delta_bps) AS margin_gap_bps
    FROM projected_outcomes p
    JOIN realized_outcomes r
      ON r.candidate_id = p.candidate_id
),
approved_candidates AS (
    SELECT DISTINCT d.candidate_id
    FROM policy_approval_decision d
    JOIN params p
      ON datetime(d.decided_at) < p.window_end
    WHERE d.decision = 'approved'
),
hard_violation_candidates AS (
    SELECT candidate_id
    FROM projected_outcomes
    WHERE hard_violation_count > 0
),
hard_violation_stats AS (
    SELECT
        COUNT(*) AS hard_violation_candidate_count,
        COUNT(CASE WHEN a.candidate_id IS NULL THEN 1 END) AS hard_violation_blocked_count
    FROM hard_violation_candidates h
    LEFT JOIN approved_candidates a
      ON a.candidate_id = h.candidate_id
),
replay_determinism AS (
    SELECT
        COUNT(*) AS replay_total_count,
        COUNT(CASE WHEN deterministic_pass = 1 THEN 1 END) AS deterministic_pass_count
    FROM policy_replay_evaluation r
    JOIN params p
      ON datetime(r.replayed_at) >= p.window_start
     AND datetime(r.replayed_at) < p.window_end
),
rollback_mttr AS (
    SELECT
        COUNT(*) AS rollback_count,
        AVG(
            CASE
                WHEN json_extract(r.rollback_metadata_json, '$.rollback_triggered_at') IS NULL
                    THEN NULL
                WHEN strftime('%s', datetime(r.rolled_back_at)) >= strftime(
                    '%s',
                    datetime(json_extract(r.rollback_metadata_json, '$.rollback_triggered_at'))
                )
                    THEN strftime('%s', datetime(r.rolled_back_at))
                       - strftime('%s', datetime(json_extract(r.rollback_metadata_json, '$.rollback_triggered_at')))
                ELSE 0
            END
        ) AS rollback_mttr_seconds,
        AVG(
            CASE
                WHEN strftime('%s', datetime(r.rolled_back_at)) >= strftime('%s', datetime(ap.applied_at))
                    THEN strftime('%s', datetime(r.rolled_back_at))
                       - strftime('%s', datetime(ap.applied_at))
                ELSE 0
            END
        ) AS rollback_mttr_proxy_seconds
    FROM policy_rollback_record r
    JOIN policy_apply_record ap
      ON ap.id = r.apply_record_id
    JOIN params p
      ON datetime(r.rolled_back_at) >= p.window_start
     AND datetime(r.rolled_back_at) < p.window_end
),
policy_improvement AS (
    SELECT
        COUNT(*) AS realized_candidate_count,
        AVG(realized_margin_delta_bps) AS avg_realized_margin_delta_bps,
        AVG(realized_win_rate_delta_bps) AS avg_realized_win_rate_delta_bps,
        AVG(realized_approval_latency_delta_seconds) AS avg_realized_approval_latency_delta_seconds,
        AVG(
            (0.40 * realized_margin_delta_bps)
            + (0.40 * COALESCE(realized_win_rate_delta_bps, 0))
            - (0.20 * COALESCE(realized_approval_latency_delta_seconds, 0))
        ) AS weighted_realized_outcome_score
    FROM realized_outcomes
),
kpi_base AS (
    SELECT
        p.window_start,
        p.window_end,
        p.max_rollback_rate_bps,
        p.max_false_positive_rate_bps,
        p.max_projected_realized_margin_gap_bps,
        cc.candidate_throughput_count,
        cc.review_decision_count,
        cc.approved_count,
        CASE
            WHEN cc.review_decision_count <= 0 THEN 0
            ELSE CAST(ROUND((cc.approved_count * 10000.0) / cc.review_decision_count, 0) AS INTEGER)
        END AS adoption_rate_bps,
        rs.applied_candidate_count,
        rs.rolled_back_candidate_count,
        CASE
            WHEN rs.applied_candidate_count <= 0 THEN 0
            ELSE CAST(ROUND((rs.rolled_back_candidate_count * 10000.0) / rs.applied_candidate_count, 0) AS INTEGER)
        END AS rollback_rate_bps,
        rs.false_positive_candidate_count,
        CASE
            WHEN rs.applied_candidate_count <= 0 THEN 0
            ELSE CAST(ROUND((rs.false_positive_candidate_count * 10000.0) / rs.applied_candidate_count, 0) AS INTEGER)
        END AS false_positive_rate_bps,
        COALESCE(CAST(ROUND(al.avg_approval_latency_seconds, 0) AS INTEGER), 0)
            AS avg_approval_latency_seconds,
        (SELECT COUNT(*) FROM projected_outcomes) AS projected_outcome_sample_count,
        (SELECT COUNT(*) FROM realized_outcomes) AS realized_outcome_sample_count,
        (SELECT COUNT(*) FROM projected_realized) AS projected_vs_realized_sample_size,
        COALESCE(
            CAST(ROUND((SELECT AVG(projected_margin_delta_bps) FROM projected_outcomes), 0) AS INTEGER),
            0
        ) AS avg_projected_margin_delta_bps,
        COALESCE(
            CAST(ROUND((SELECT AVG(realized_margin_delta_bps) FROM realized_outcomes), 0) AS INTEGER),
            0
        ) AS avg_realized_margin_delta_bps,
        COALESCE(
            CAST(ROUND((SELECT AVG(margin_gap_bps) FROM projected_realized), 0) AS INTEGER),
            0
        ) AS avg_projected_vs_realized_margin_gap_bps,
        (SELECT COUNT(*) FROM projected_outcomes) AS replayed_candidate_count,
        (
            SELECT COUNT(*)
            FROM projected_outcomes pr
            JOIN approved_candidates ac
              ON ac.candidate_id = pr.candidate_id
        ) AS approved_replayed_candidate_count,
        CASE
            WHEN (SELECT COUNT(*) FROM projected_outcomes) <= 0 THEN 0
            ELSE CAST(
                ROUND(
                    (
                        (
                            SELECT COUNT(*)
                            FROM projected_outcomes pr
                            JOIN approved_candidates ac
                              ON ac.candidate_id = pr.candidate_id
                        ) * 10000.0
                    ) / (SELECT COUNT(*) FROM projected_outcomes),
                    0
                ) AS INTEGER
            )
        END AS candidate_acceptance_rate_bps,
        hvs.hard_violation_candidate_count,
        hvs.hard_violation_blocked_count,
        CASE
            WHEN hvs.hard_violation_candidate_count <= 0 THEN 0
            ELSE CAST(ROUND((hvs.hard_violation_blocked_count * 10000.0) / hvs.hard_violation_candidate_count, 0) AS INTEGER)
        END AS unsafe_candidate_block_rate_bps,
        COALESCE(
            CAST(ROUND(ABS((SELECT AVG(margin_gap_bps) FROM projected_realized)), 0) AS INTEGER),
            0
        ) AS projected_realized_margin_delta_error_bps,
        COALESCE(CAST(ROUND(mttr.rollback_mttr_seconds, 0) AS INTEGER), 0) AS rollback_mttr_seconds,
        COALESCE(CAST(ROUND(mttr.rollback_mttr_proxy_seconds, 0) AS INTEGER), 0)
            AS rollback_mttr_proxy_seconds,
        mttr.rollback_count,
        rd.replay_total_count,
        rd.deterministic_pass_count,
        CASE
            WHEN rd.replay_total_count <= 0 THEN 0
            ELSE CAST(ROUND((rd.deterministic_pass_count * 10000.0) / rd.replay_total_count, 0) AS INTEGER)
        END AS replay_determinism_rate_bps,
        pi.realized_candidate_count,
        COALESCE(CAST(ROUND(pi.avg_realized_margin_delta_bps, 0) AS INTEGER), 0)
            AS avg_realized_margin_delta_for_outcome_bps,
        COALESCE(CAST(ROUND(pi.avg_realized_win_rate_delta_bps, 0) AS INTEGER), 0)
            AS avg_realized_win_rate_delta_bps,
        COALESCE(CAST(ROUND(pi.avg_realized_approval_latency_delta_seconds, 0) AS INTEGER), 0)
            AS avg_realized_approval_latency_delta_seconds,
        COALESCE(CAST(ROUND(pi.weighted_realized_outcome_score, 0) AS INTEGER), 0)
            AS weighted_realized_outcome_score
    FROM params p
    CROSS JOIN candidate_counts cc
    CROSS JOIN rollback_stats rs
    CROSS JOIN hard_violation_stats hvs
    CROSS JOIN replay_determinism rd
    CROSS JOIN rollback_mttr mttr
    CROSS JOIN policy_improvement pi
    LEFT JOIN approval_latency al
      ON 1 = 1
)
SELECT
    window_start,
    window_end,
    candidate_throughput_count,
    review_decision_count,
    approved_count,
    adoption_rate_bps,
    applied_candidate_count,
    rolled_back_candidate_count,
    rollback_rate_bps,
    false_positive_candidate_count,
    false_positive_rate_bps,
    avg_approval_latency_seconds,
    projected_outcome_sample_count,
    realized_outcome_sample_count,
    projected_vs_realized_sample_size,
    avg_projected_margin_delta_bps,
    avg_realized_margin_delta_bps,
    avg_projected_vs_realized_margin_gap_bps,
    replayed_candidate_count,
    approved_replayed_candidate_count,
    candidate_acceptance_rate_bps,
    hard_violation_candidate_count,
    hard_violation_blocked_count,
    unsafe_candidate_block_rate_bps,
    projected_realized_margin_delta_error_bps,
    rollback_mttr_seconds,
    rollback_mttr_proxy_seconds,
    rollback_count,
    replay_total_count,
    deterministic_pass_count,
    replay_determinism_rate_bps,
    realized_candidate_count,
    avg_realized_margin_delta_for_outcome_bps,
    avg_realized_win_rate_delta_bps,
    avg_realized_approval_latency_delta_seconds,
    weighted_realized_outcome_score,
    max_rollback_rate_bps,
    max_false_positive_rate_bps,
    max_projected_realized_margin_gap_bps,
    CASE WHEN rollback_rate_bps > max_rollback_rate_bps THEN 1 ELSE 0 END AS rollback_spike_alert,
    CASE WHEN false_positive_rate_bps > max_false_positive_rate_bps THEN 1 ELSE 0 END AS false_positive_alert,
    CASE
        WHEN ABS(avg_projected_vs_realized_margin_gap_bps) > max_projected_realized_margin_gap_bps
            THEN 1
        ELSE 0
    END AS margin_gap_alert,
    TRIM(
        CASE WHEN rollback_rate_bps > max_rollback_rate_bps THEN 'rollback_spike,' ELSE '' END
        || CASE WHEN false_positive_rate_bps > max_false_positive_rate_bps THEN 'false_positive_spike,' ELSE '' END
        || CASE
            WHEN ABS(avg_projected_vs_realized_margin_gap_bps) > max_projected_realized_margin_gap_bps
                THEN 'margin_gap_drift,'
            ELSE ''
        END,
        ','
    ) AS alert_reason_codes
FROM kpi_base;

-- ============================================================================
-- Query ID: clo_kpi_projected_vs_realized_detail_v1
-- Purpose:
--   Candidate-level projected-vs-realized comparison for dashboards, anomaly
--   investigation, and approval/replay postmortems.
-- ============================================================================
WITH
params AS (
    SELECT
        datetime('now', '-30 days') AS window_start,
        datetime('now') AS window_end
),
latest_replay AS (
    SELECT
        r.candidate_id,
        r.projected_margin_delta_bps,
        r.projected_win_rate_delta_bps,
        r.projected_approval_latency_delta_seconds,
        datetime(r.replayed_at) AS replayed_at,
        ROW_NUMBER() OVER (
            PARTITION BY r.candidate_id
            ORDER BY datetime(r.replayed_at) DESC, r.id DESC
        ) AS row_num
    FROM policy_replay_evaluation r
    JOIN params p
      ON datetime(r.replayed_at) >= p.window_start
     AND datetime(r.replayed_at) < p.window_end
),
latest_realized AS (
    SELECT
        a.candidate_id,
        CAST(json_extract(a.event_payload_json, '$.realized_margin_delta_bps') AS INTEGER) AS realized_margin_delta_bps,
        CAST(json_extract(a.event_payload_json, '$.realized_win_rate_delta_bps') AS INTEGER) AS realized_win_rate_delta_bps,
        CAST(json_extract(a.event_payload_json, '$.realized_approval_latency_delta_seconds') AS INTEGER) AS realized_approval_latency_delta_seconds,
        datetime(a.occurred_at) AS monitored_at,
        ROW_NUMBER() OVER (
            PARTITION BY a.candidate_id
            ORDER BY datetime(a.occurred_at) DESC, a.id DESC
        ) AS row_num
    FROM policy_lifecycle_audit a
    JOIN params p
      ON datetime(a.occurred_at) >= p.window_start
     AND datetime(a.occurred_at) < p.window_end
    WHERE a.event_type = 'monitoring_started'
),
projected AS (
    SELECT
        candidate_id,
        projected_margin_delta_bps,
        projected_win_rate_delta_bps,
        projected_approval_latency_delta_seconds,
        replayed_at
    FROM latest_replay
    WHERE row_num = 1
),
realized AS (
    SELECT
        candidate_id,
        realized_margin_delta_bps,
        realized_win_rate_delta_bps,
        realized_approval_latency_delta_seconds,
        monitored_at
    FROM latest_realized
    WHERE row_num = 1
      AND realized_margin_delta_bps IS NOT NULL
)
SELECT
    p.candidate_id,
    p.replayed_at,
    r.monitored_at,
    p.projected_margin_delta_bps,
    r.realized_margin_delta_bps,
    (p.projected_margin_delta_bps - r.realized_margin_delta_bps) AS margin_gap_bps,
    p.projected_win_rate_delta_bps,
    r.realized_win_rate_delta_bps,
    p.projected_approval_latency_delta_seconds,
    r.realized_approval_latency_delta_seconds
FROM projected p
JOIN realized r
  ON r.candidate_id = p.candidate_id
ORDER BY ABS(p.projected_margin_delta_bps - r.realized_margin_delta_bps) DESC, p.candidate_id ASC;
