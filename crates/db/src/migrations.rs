use sqlx::migrate::{MigrateError, Migrator};

use crate::DbPool;

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

pub async fn run_pending(pool: &DbPool) -> Result<(), MigrateError> {
    MIGRATOR.run(pool).await
}

#[cfg(test)]
mod tests {
    use sqlx::Row;

    use super::run_pending;
    use crate::{connect_with_settings, migrations::MIGRATOR};

    const MANAGED_SCHEMA_OBJECTS: &[&str] = &[
        "quote",
        "quote_line",
        "flow_state",
        "audit_event",
        "emoji_approvals",
        "approval_audit_log",
        "configuration_fingerprints",
        "similarity_cache",
        "dialogue_sessions",
        "dialogue_turns",
        "policy_rules",
        "explanation_cache",
        "explanation_requests",
        "explanation_evidence",
        "explanation_audit",
        "explanation_response_cache",
        "explanation_request_stats",
        "quote_sessions",
        "session_participants",
        "session_operations",
        "quote_ledger",
        "ledger_verifications",
        "quote_pricing_snapshot",
        "constraint_nodes",
        "constraint_edges",
        "archaeology_queries",
        "buying_signals",
        "ghost_quotes",
        "approval_authorities",
        "org_hierarchy",
        "routing_rules",
        "deal_outcomes",
        "win_probability_models",
        "prediction_cache",
        "execution_queue_task",
        "execution_idempotency_ledger",
        "execution_queue_transition_audit",
        "deal_flight_scenario_run",
        "deal_flight_scenario_variant",
        "deal_flight_scenario_delta",
        "deal_flight_scenario_audit",
        "policy_candidate",
        "policy_replay_evaluation",
        "policy_approval_decision",
        "policy_apply_record",
        "policy_rollback_record",
        "policy_lifecycle_audit",
        "precedent_approval_path_evidence",
        "precedent_similarity_evidence",
        "idx_quote_status",
        "idx_quote_created_at",
        "idx_quote_line_quote_id",
        "idx_flow_state_quote_id",
        "idx_audit_event_quote_id",
        "idx_audit_event_timestamp",
        "idx_audit_event_type",
        "idx_emoji_approvals_quote_id",
        "idx_emoji_approvals_approver_user_id",
        "idx_approval_audit_log_quote_id",
        "idx_configuration_fingerprints_quote_id",
        "idx_configuration_fingerprints_fingerprint_hash",
        "idx_similarity_cache_source_candidate_version",
        "idx_similarity_cache_source_fingerprint_id",
        "idx_similarity_cache_candidate_fingerprint_id",
        "idx_dialogue_sessions_slack_thread_id",
        "idx_dialogue_sessions_user_id",
        "idx_dialogue_turns_session_turn_number",
        "idx_policy_rules_rule_category",
        "idx_explanation_cache_rule_id",
        "idx_explanation_cache_quote_id",
        "idx_explanation_requests_quote",
        "idx_explanation_requests_thread",
        "idx_explanation_requests_correlation",
        "idx_explanation_requests_status",
        "idx_explanation_evidence_request",
        "idx_explanation_evidence_source",
        "idx_explanation_audit_request",
        "idx_explanation_audit_event_type",
        "idx_explanation_audit_correlation",
        "idx_explanation_response_cache_quote",
        "idx_explanation_response_cache_expires",
        "idx_quote_sessions_quote_id",
        "idx_quote_sessions_status",
        "idx_session_participants_user_id",
        "idx_session_operations_session_id",
        "idx_session_operations_timestamp",
        "idx_quote_ledger_quote_id",
        "idx_quote_ledger_content_hash",
        "idx_ledger_verifications_entry_id",
        "idx_quote_pricing_snapshot_quote_version",
        "idx_quote_pricing_snapshot_ledger_entry",
        "idx_constraint_nodes_config_id",
        "idx_constraint_nodes_node_key",
        "idx_constraint_edges_config_id",
        "idx_archaeology_queries_config_id",
        "idx_buying_signals_matched_rep_id",
        "idx_buying_signals_status",
        "idx_ghost_quotes_signal_id",
        "idx_approval_authorities_role",
        "idx_org_hierarchy_manager_id",
        "idx_routing_rules_criteria",
        "idx_deal_outcomes_quote_id",
        "idx_deal_outcomes_outcome",
        "idx_prediction_cache_quote_model",
        "idx_execution_queue_task_quote_state_available",
        "idx_execution_queue_task_idempotency_key",
        "idx_execution_idempotency_quote_state",
        "idx_execution_idempotency_expires_at",
        "idx_execution_queue_transition_task_occurred",
        "idx_execution_queue_transition_quote_occurred",
        "idx_sim_run_quote_created",
        "idx_sim_run_thread_created",
        "idx_sim_run_correlation",
        "idx_sim_run_status_created",
        "idx_sim_variant_run_rank",
        "idx_sim_variant_run_selected",
        "idx_sim_delta_variant_type",
        "idx_sim_audit_run_occurred",
        "idx_sim_audit_event_occurred",
        "idx_sim_audit_correlation",
        "idx_policy_candidate_status_created",
        "idx_policy_candidate_base_version",
        "idx_policy_replay_candidate_replayed",
        "idx_policy_replay_checksum",
        "idx_policy_approval_candidate_decided",
        "idx_policy_approval_stale_expires",
        "idx_policy_apply_candidate_applied",
        "idx_policy_apply_idempotency",
        "idx_policy_rollback_candidate_rolled",
        "idx_policy_rollback_apply",
        "idx_policy_audit_candidate_occurred",
        "idx_policy_audit_event_occurred",
        "idx_policy_audit_correlation",
        "idx_precedent_approval_quote_routed",
        "idx_precedent_approval_status_decided",
        "idx_precedent_similarity_source_quote_score",
        "idx_precedent_similarity_source_fp_version",
        "idx_precedent_similarity_candidate_quote",
        "idx_precedent_similarity_correlation",
    ];

    #[tokio::test]
    async fn migrations_create_baseline_tables() {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        run_pending(&pool).await.expect("run migrations");

        let quote_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'quote'",
        )
        .fetch_one(&pool)
        .await
        .expect("check quote table")
        .get::<i64, _>("count");

        let flow_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'flow_state'",
        )
        .fetch_one(&pool)
        .await
        .expect("check flow_state table")
        .get::<i64, _>("count");

        let audit_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'audit_event'",
        )
        .fetch_one(&pool)
        .await
        .expect("check audit_event table")
        .get::<i64, _>("count");

        let emoji_approval_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'emoji_approvals'",
        )
        .fetch_one(&pool)
        .await
        .expect("check emoji_approvals table")
        .get::<i64, _>("count");

        let approval_audit_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'approval_audit_log'",
        )
        .fetch_one(&pool)
        .await
        .expect("check approval_audit_log table")
        .get::<i64, _>("count");

        let configuration_fingerprint_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'configuration_fingerprints'",
        )
        .fetch_one(&pool)
        .await
        .expect("check configuration_fingerprints table")
        .get::<i64, _>("count");

        let similarity_cache_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'similarity_cache'",
        )
        .fetch_one(&pool)
        .await
        .expect("check similarity_cache table")
        .get::<i64, _>("count");

        let dialogue_sessions_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'dialogue_sessions'",
        )
        .fetch_one(&pool)
        .await
        .expect("check dialogue_sessions table")
        .get::<i64, _>("count");

        let dialogue_turns_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'dialogue_turns'",
        )
        .fetch_one(&pool)
        .await
        .expect("check dialogue_turns table")
        .get::<i64, _>("count");

        let policy_rules_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'policy_rules'",
        )
        .fetch_one(&pool)
        .await
        .expect("check policy_rules table")
        .get::<i64, _>("count");

        let explanation_cache_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'explanation_cache'",
        )
        .fetch_one(&pool)
        .await
        .expect("check explanation_cache table")
        .get::<i64, _>("count");

        let explanation_requests_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'explanation_requests'",
        )
        .fetch_one(&pool)
        .await
        .expect("check explanation_requests table")
        .get::<i64, _>("count");

        let explanation_evidence_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'explanation_evidence'",
        )
        .fetch_one(&pool)
        .await
        .expect("check explanation_evidence table")
        .get::<i64, _>("count");

        let explanation_audit_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'explanation_audit'",
        )
        .fetch_one(&pool)
        .await
        .expect("check explanation_audit table")
        .get::<i64, _>("count");

        let explanation_response_cache_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'explanation_response_cache'",
        )
        .fetch_one(&pool)
        .await
        .expect("check explanation_response_cache table")
        .get::<i64, _>("count");

        let explanation_request_stats_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'explanation_request_stats'",
        )
        .fetch_one(&pool)
        .await
        .expect("check explanation_request_stats table")
        .get::<i64, _>("count");

        let quote_sessions_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'quote_sessions'",
        )
        .fetch_one(&pool)
        .await
        .expect("check quote_sessions table")
        .get::<i64, _>("count");

        let session_participants_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'session_participants'",
        )
        .fetch_one(&pool)
        .await
        .expect("check session_participants table")
        .get::<i64, _>("count");

        let session_operations_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'session_operations'",
        )
        .fetch_one(&pool)
        .await
        .expect("check session_operations table")
        .get::<i64, _>("count");

        let quote_ledger_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'quote_ledger'",
        )
        .fetch_one(&pool)
        .await
        .expect("check quote_ledger table")
        .get::<i64, _>("count");

        let ledger_verifications_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'ledger_verifications'",
        )
        .fetch_one(&pool)
        .await
        .expect("check ledger_verifications table")
        .get::<i64, _>("count");

        let quote_pricing_snapshot_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'quote_pricing_snapshot'",
        )
        .fetch_one(&pool)
        .await
        .expect("check quote_pricing_snapshot table")
        .get::<i64, _>("count");

        let constraint_nodes_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'constraint_nodes'",
        )
        .fetch_one(&pool)
        .await
        .expect("check constraint_nodes table")
        .get::<i64, _>("count");

        let constraint_edges_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'constraint_edges'",
        )
        .fetch_one(&pool)
        .await
        .expect("check constraint_edges table")
        .get::<i64, _>("count");

        let archaeology_queries_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'archaeology_queries'",
        )
        .fetch_one(&pool)
        .await
        .expect("check archaeology_queries table")
        .get::<i64, _>("count");

        let buying_signals_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'buying_signals'",
        )
        .fetch_one(&pool)
        .await
        .expect("check buying_signals table")
        .get::<i64, _>("count");

        let ghost_quotes_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'ghost_quotes'",
        )
        .fetch_one(&pool)
        .await
        .expect("check ghost_quotes table")
        .get::<i64, _>("count");

        let approval_authorities_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'approval_authorities'",
        )
        .fetch_one(&pool)
        .await
        .expect("check approval_authorities table")
        .get::<i64, _>("count");

        let org_hierarchy_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'org_hierarchy'",
        )
        .fetch_one(&pool)
        .await
        .expect("check org_hierarchy table")
        .get::<i64, _>("count");

        let routing_rules_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'routing_rules'",
        )
        .fetch_one(&pool)
        .await
        .expect("check routing_rules table")
        .get::<i64, _>("count");

        let deal_outcomes_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'deal_outcomes'",
        )
        .fetch_one(&pool)
        .await
        .expect("check deal_outcomes table")
        .get::<i64, _>("count");

        let win_probability_models_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'win_probability_models'",
        )
        .fetch_one(&pool)
        .await
        .expect("check win_probability_models table")
        .get::<i64, _>("count");

        let prediction_cache_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'prediction_cache'",
        )
        .fetch_one(&pool)
        .await
        .expect("check prediction_cache table")
        .get::<i64, _>("count");

        let execution_queue_task_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'execution_queue_task'",
        )
        .fetch_one(&pool)
        .await
        .expect("check execution_queue_task table")
        .get::<i64, _>("count");

        let execution_idempotency_ledger_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'execution_idempotency_ledger'",
        )
        .fetch_one(&pool)
        .await
        .expect("check execution_idempotency_ledger table")
        .get::<i64, _>("count");

        let execution_queue_transition_audit_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'execution_queue_transition_audit'",
        )
        .fetch_one(&pool)
        .await
        .expect("check execution_queue_transition_audit table")
        .get::<i64, _>("count");

        let deal_flight_scenario_run_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'deal_flight_scenario_run'",
        )
        .fetch_one(&pool)
        .await
        .expect("check deal_flight_scenario_run table")
        .get::<i64, _>("count");

        let deal_flight_scenario_variant_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'deal_flight_scenario_variant'",
        )
        .fetch_one(&pool)
        .await
        .expect("check deal_flight_scenario_variant table")
        .get::<i64, _>("count");

        let deal_flight_scenario_delta_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'deal_flight_scenario_delta'",
        )
        .fetch_one(&pool)
        .await
        .expect("check deal_flight_scenario_delta table")
        .get::<i64, _>("count");

        let deal_flight_scenario_audit_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'deal_flight_scenario_audit'",
        )
        .fetch_one(&pool)
        .await
        .expect("check deal_flight_scenario_audit table")
        .get::<i64, _>("count");

        let policy_candidate_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'policy_candidate'",
        )
        .fetch_one(&pool)
        .await
        .expect("check policy_candidate table")
        .get::<i64, _>("count");

        let policy_replay_evaluation_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'policy_replay_evaluation'",
        )
        .fetch_one(&pool)
        .await
        .expect("check policy_replay_evaluation table")
        .get::<i64, _>("count");

        let policy_approval_decision_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'policy_approval_decision'",
        )
        .fetch_one(&pool)
        .await
        .expect("check policy_approval_decision table")
        .get::<i64, _>("count");

        let policy_apply_record_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'policy_apply_record'",
        )
        .fetch_one(&pool)
        .await
        .expect("check policy_apply_record table")
        .get::<i64, _>("count");

        let policy_rollback_record_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'policy_rollback_record'",
        )
        .fetch_one(&pool)
        .await
        .expect("check policy_rollback_record table")
        .get::<i64, _>("count");

        let policy_lifecycle_audit_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'policy_lifecycle_audit'",
        )
        .fetch_one(&pool)
        .await
        .expect("check policy_lifecycle_audit table")
        .get::<i64, _>("count");

        let precedent_approval_path_evidence_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'precedent_approval_path_evidence'",
        )
        .fetch_one(&pool)
        .await
        .expect("check precedent_approval_path_evidence table")
        .get::<i64, _>("count");

        let precedent_similarity_evidence_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'precedent_similarity_evidence'",
        )
        .fetch_one(&pool)
        .await
        .expect("check precedent_similarity_evidence table")
        .get::<i64, _>("count");

        assert_eq!(quote_count, 1);
        assert_eq!(flow_count, 1);
        assert_eq!(audit_count, 1);
        assert_eq!(emoji_approval_count, 1);
        assert_eq!(approval_audit_count, 1);
        assert_eq!(configuration_fingerprint_count, 1);
        assert_eq!(similarity_cache_count, 1);
        assert_eq!(dialogue_sessions_count, 1);
        assert_eq!(dialogue_turns_count, 1);
        assert_eq!(policy_rules_count, 1);
        assert_eq!(explanation_cache_count, 1);
        assert_eq!(explanation_requests_count, 1);
        assert_eq!(explanation_evidence_count, 1);
        assert_eq!(explanation_audit_count, 1);
        assert_eq!(explanation_response_cache_count, 1);
        assert_eq!(explanation_request_stats_count, 1);
        assert_eq!(quote_sessions_count, 1);
        assert_eq!(session_participants_count, 1);
        assert_eq!(session_operations_count, 1);
        assert_eq!(quote_ledger_count, 1);
        assert_eq!(ledger_verifications_count, 1);
        assert_eq!(quote_pricing_snapshot_count, 1);
        assert_eq!(constraint_nodes_count, 1);
        assert_eq!(constraint_edges_count, 1);
        assert_eq!(archaeology_queries_count, 1);
        assert_eq!(buying_signals_count, 1);
        assert_eq!(ghost_quotes_count, 1);
        assert_eq!(approval_authorities_count, 1);
        assert_eq!(org_hierarchy_count, 1);
        assert_eq!(routing_rules_count, 1);
        assert_eq!(deal_outcomes_count, 1);
        assert_eq!(win_probability_models_count, 1);
        assert_eq!(prediction_cache_count, 1);
        assert_eq!(execution_queue_task_count, 1);
        assert_eq!(execution_idempotency_ledger_count, 1);
        assert_eq!(execution_queue_transition_audit_count, 1);
        assert_eq!(deal_flight_scenario_run_count, 1);
        assert_eq!(deal_flight_scenario_variant_count, 1);
        assert_eq!(deal_flight_scenario_delta_count, 1);
        assert_eq!(deal_flight_scenario_audit_count, 1);
        assert_eq!(policy_candidate_count, 1);
        assert_eq!(policy_replay_evaluation_count, 1);
        assert_eq!(policy_approval_decision_count, 1);
        assert_eq!(policy_apply_record_count, 1);
        assert_eq!(policy_rollback_record_count, 1);
        assert_eq!(policy_lifecycle_audit_count, 1);
        assert_eq!(precedent_approval_path_evidence_count, 1);
        assert_eq!(precedent_similarity_evidence_count, 1);
    }

    #[tokio::test]
    async fn migrations_are_reversible() {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        run_pending(&pool).await.expect("run migrations");

        MIGRATOR.undo(&pool, 0).await.expect("undo migrations");

        let quote_count = sqlx::query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'quote'",
        )
        .fetch_one(&pool)
        .await
        .expect("check quote table removed")
        .get::<i64, _>("count");

        assert_eq!(quote_count, 0);
    }

    #[tokio::test]
    async fn migrations_up_down_up_preserves_schema_signature() {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        run_pending(&pool).await.expect("run migrations");

        let initial_signature = managed_schema_signature(&pool).await;
        assert_eq!(
            initial_signature.len(),
            MANAGED_SCHEMA_OBJECTS.len(),
            "initial migration pass should create all managed schema objects",
        );

        MIGRATOR.undo(&pool, 0).await.expect("undo migrations");

        let after_down_signature = managed_schema_signature(&pool).await;
        assert!(
            after_down_signature.is_empty(),
            "managed schema objects should be removed after full undo",
        );

        run_pending(&pool).await.expect("re-run migrations");

        let after_second_up_signature = managed_schema_signature(&pool).await;
        assert_eq!(
            after_second_up_signature, initial_signature,
            "up/down/up should preserve migration-managed schema signature",
        );
    }

    async fn managed_schema_signature(pool: &sqlx::SqlitePool) -> Vec<(String, String, String)> {
        let mut signature: Vec<(String, String, String)> = sqlx::query(
            "SELECT type, name, IFNULL(sql, '') AS sql
             FROM sqlite_master
             WHERE type IN ('table', 'index')",
        )
        .fetch_all(pool)
        .await
        .expect("load schema objects")
        .into_iter()
        .filter_map(|row| {
            let name = row.get::<String, _>("name");
            if MANAGED_SCHEMA_OBJECTS.contains(&name.as_str()) {
                Some((row.get::<String, _>("type"), name, row.get::<String, _>("sql")))
            } else {
                None
            }
        })
        .collect();
        signature.sort();
        signature
    }
}
