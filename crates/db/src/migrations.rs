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
        "quote_sessions",
        "session_participants",
        "session_operations",
        "quote_ledger",
        "ledger_verifications",
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
        "idx_quote_sessions_quote_id",
        "idx_quote_sessions_status",
        "idx_session_participants_user_id",
        "idx_session_operations_session_id",
        "idx_session_operations_timestamp",
        "idx_quote_ledger_quote_id",
        "idx_quote_ledger_content_hash",
        "idx_ledger_verifications_entry_id",
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
        assert_eq!(quote_sessions_count, 1);
        assert_eq!(session_participants_count, 1);
        assert_eq!(session_operations_count, 1);
        assert_eq!(quote_ledger_count, 1);
        assert_eq!(ledger_verifications_count, 1);
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
