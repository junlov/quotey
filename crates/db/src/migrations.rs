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

    type TestResult<T> = Result<T, String>;

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
        "deal_autopsy",
        "decision_fork",
        "attribution_score",
        "attribution_node",
        "attribution_edge",
        "genome_query_audit",
        "counterfactual_simulation",
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
        "idx_deal_autopsy_quote",
        "idx_deal_autopsy_outcome_created",
        "idx_decision_fork_autopsy_seq",
        "idx_decision_fork_type_stage",
        "idx_attribution_score_autopsy",
        "idx_attribution_score_fork",
        "idx_attribution_score_contribution",
        "idx_attribution_node_type_stage_seg",
        "idx_attribution_node_hash",
        "idx_attribution_edge_source",
        "idx_attribution_edge_target",
        "idx_attribution_edge_weight",
        "idx_genome_query_audit_type_queried",
        "idx_counterfactual_quote",
        "idx_counterfactual_autopsy",
        // 0019 — product catalog
        "product_family",
        "product",
        "product_fts",
        "product_attribute",
        "product_bundle_member",
        "idx_product_sku",
        "idx_product_family_id",
        "idx_product_active",
        "idx_product_type",
        "idx_product_attribute_product",
        // 0020 — quote enrich (indexes only; columns added to existing tables)
        "idx_quote_account_id",
        "idx_quote_deal_id",
        // 0021 — approval_request table
        "approval_request",
        "idx_approval_request_quote_id",
        "idx_approval_request_status",
        "idx_approval_request_approver_role",
        // 0022 — suggestion feedback
        "suggestion_feedback",
        "idx_suggestion_feedback_request_id",
        "idx_suggestion_feedback_customer_product",
        "idx_suggestion_feedback_product_id",
        // 0023 — portal link
        "portal_link",
        "idx_portal_link_token",
        "idx_portal_link_quote_id",
        // 0024 — portal comment
        "portal_comment",
        "idx_portal_comment_quote_id",
        "idx_portal_comment_line_id",
        "idx_portal_comment_parent",
        // 0025 — crm integration
        "crm_integration",
        "idx_crm_integration_provider",
        "idx_crm_integration_status",
        "crm_oauth_state",
        "idx_crm_oauth_state_expires_at",
        "idx_crm_oauth_state_provider",
        "crm_field_mapping",
        "idx_crm_field_mapping_provider_direction",
        "idx_crm_field_mapping_quotey_field",
        "idx_crm_field_mapping_crm_field",
        "crm_sync_event",
        "idx_crm_sync_event_provider",
        "idx_crm_sync_event_direction",
        "idx_crm_sync_event_status",
        "idx_crm_sync_event_quote_id",
        "idx_crm_sync_event_created_at",
    ];

    async fn managed_object_count(pool: &sqlx::SqlitePool, object_name: &str) -> TestResult<i64> {
        sqlx::query_scalar("SELECT COUNT(*) AS count FROM sqlite_master WHERE name = ?")
            .bind(object_name)
            .fetch_one(pool)
            .await
            .map_err(|err| format!("load schema object count for {object_name}: {err}"))
    }

    async fn assert_managed_object_count(
        pool: &sqlx::SqlitePool,
        object_name: &str,
        expected_count: i64,
    ) -> TestResult<()> {
        let count = managed_object_count(pool, object_name).await?;
        if count != expected_count {
            return Err(format!(
                "schema object {object_name} has count {count}, expected {expected_count}",
            ));
        }
        Ok(())
    }

    #[tokio::test]
    async fn migrations_create_baseline_tables() -> TestResult<()> {
        let pool = connect_with_settings("sqlite::memory:", 1, 30)
            .await
            .map_err(|err| format!("connect failed: {err}"))?;
        run_pending(&pool).await.map_err(|err| format!("run migrations failed: {err}"))?;

        for &object_name in MANAGED_SCHEMA_OBJECTS {
            assert_managed_object_count(&pool, object_name, 1).await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn migrations_are_reversible() -> TestResult<()> {
        let pool = connect_with_settings("sqlite::memory:", 1, 30)
            .await
            .map_err(|err| format!("connect failed: {err}"))?;
        run_pending(&pool).await.map_err(|err| format!("run migrations failed: {err}"))?;

        MIGRATOR.undo(&pool, 0).await.map_err(|err| format!("undo migrations failed: {err}"))?;

        assert_managed_object_count(&pool, "quote", 0).await?;

        Ok(())
    }

    #[tokio::test]
    async fn migrations_up_down_up_preserves_schema_signature() -> TestResult<()> {
        let pool = connect_with_settings("sqlite::memory:", 1, 30)
            .await
            .map_err(|err| format!("connect failed: {err}"))?;
        run_pending(&pool).await.map_err(|err| format!("run migrations failed: {err}"))?;

        let initial_signature = managed_schema_signature(&pool).await?;
        if initial_signature.len() != MANAGED_SCHEMA_OBJECTS.len() {
            return Err(format!(
                "initial migration pass should create all managed schema objects: expected {} entries, got {}",
                MANAGED_SCHEMA_OBJECTS.len(),
                initial_signature.len(),
            ));
        }

        MIGRATOR.undo(&pool, 0).await.map_err(|err| format!("undo migrations failed: {err}"))?;

        let after_down_signature = managed_schema_signature(&pool).await?;
        if !after_down_signature.is_empty() {
            return Err(format!(
                "managed schema objects should be removed after full undo, found {} objects",
                after_down_signature.len(),
            ));
        }

        run_pending(&pool).await.map_err(|err| format!("re-run migrations failed: {err}"))?;

        let after_second_up_signature = managed_schema_signature(&pool).await?;
        if after_second_up_signature != initial_signature {
            return Err("up/down/up should preserve migration-managed schema signature".to_string());
        }

        Ok(())
    }

    async fn managed_schema_signature(
        pool: &sqlx::SqlitePool,
    ) -> TestResult<Vec<(String, String, String)>> {
        let mut signature: Vec<(String, String, String)> = sqlx::query(
            "SELECT type, name, IFNULL(sql, '') AS sql
             FROM sqlite_master
             WHERE type IN ('table', 'index')",
        )
        .fetch_all(pool)
        .await
        .map_err(|err| format!("load schema objects failed: {err}"))?
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
        Ok(signature)
    }
}
