use crate::connection::DbPool;
use crate::repositories::RepositoryError;
use serde_json::Value;
use sqlx::Executor;

/// Canonical E2E seeds and verification contract for the three core quote flows.
const SEED_FLOWS: &[SeedFlowContract] = &[
    SeedFlowContract {
        flow_type: "net_new",
        quote_id: "quote-netnew-001",
        status: "draft",
        current_step: "gather_requirements",
        step_number: 1,
        expected_line_count: 3,
        required_fields: &["billing_country", "payment_terms"],
        missing_fields: &["billing_country", "payment_terms"],
        account_id: "acct-netnew-001",
        account_name: "Acme Corp",
        deal_id: "deal-netnew-001",
        deal_name: "Acme Corp - New License",
        policy_profile: "standard",
        product_ids: &["prod-plan-ent", "prod-sso", "prod-support-premium"],
        product_names: &["Enterprise Plan", "SSO Add-on", "Premium Support"],
        requested_discount_pct: None,
        threshold_pct: None,
        requires_approval_request: false,
        prior_quote_id: None,
        description: "Enterprise new license - draft state",
    },
    SeedFlowContract {
        flow_type: "renewal",
        quote_id: "quote-renewal-001",
        status: "priced",
        current_step: "validate_expansion",
        step_number: 3,
        expected_line_count: 3,
        required_fields: &["prior_quote_id"],
        missing_fields: &[],
        account_id: "acct-renewal-001",
        account_name: "Globex Industries",
        deal_id: "deal-renewal-001",
        deal_name: "Globex - Annual Renewal",
        policy_profile: "renewal",
        product_ids: &["prod-plan-ent", "prod-support-premium", "prod-onboarding"],
        product_names: &["Enterprise Plan", "Premium Support", "Onboarding"],
        requested_discount_pct: None,
        threshold_pct: None,
        requires_approval_request: false,
        prior_quote_id: Some("quote-renewal-prior-001"),
        description: "Annual renewal with expansion - priced state",
    },
    SeedFlowContract {
        flow_type: "discount_exception",
        quote_id: "quote-discount-001",
        status: "approval",
        current_step: "awaiting_approval",
        step_number: 4,
        expected_line_count: 2,
        required_fields: &["approval_decision"],
        missing_fields: &["approval_decision"],
        account_id: "acct-discount-001",
        account_name: "Initech LLC",
        deal_id: "deal-discount-001",
        deal_name: "Initech - Expansion Deal",
        policy_profile: "discount_exception",
        product_ids: &["prod-plan-pro", "prod-sso"],
        product_names: &["Pro Plan", "SSO Add-on"],
        requested_discount_pct: Some(25),
        threshold_pct: Some(20),
        requires_approval_request: true,
        prior_quote_id: None,
        description: "25% discount requiring approval - approval state",
    },
];

const SEED_QUOTE_IDS: &[&str] =
    &["quote-netnew-001", "quote-renewal-prior-001", "quote-renewal-001", "quote-discount-001"];

const SEED_FLOW_STATE_IDS: &[&str] = &["fs-netnew-001", "fs-renewal-001", "fs-discount-001"];

const SEED_AUDIT_EVENT_IDS: &[&str] =
    &["ae-netnew-001", "ae-renewal-001", "ae-discount-001", "ae-discount-002"];

/// E2E Seed Dataset for 3 Core Quote Flows.
///
/// Provides deterministic fixtures for:
/// 1. Net-new quote flow
/// 2. Renewal flow
/// 3. Discount exception flow
pub struct E2ESeedDataset;

impl E2ESeedDataset {
    /// SQL fixture content for E2E seed data.
    pub const SQL: &str = include_str!("../../../config/fixtures/e2e_seed_data.sql");

    /// Load E2E seed dataset into the database.
    pub async fn load(pool: &DbPool) -> Result<SeedResult, RepositoryError> {
        let mut tx = pool.begin().await?;

        tx.execute(sqlx::query(Self::SQL)).await?;
        tx.commit().await?;

        let flows_seeded = SEED_FLOWS
            .iter()
            .map(|flow| FlowSeedInfo {
                flow_type: flow.flow_type,
                quote_id: flow.quote_id,
                description: flow.description,
            })
            .collect::<Vec<_>>();

        Ok(SeedResult { flows_seeded })
    }

    /// Verify that seed data exists and matches the contract.
    pub async fn verify(pool: &DbPool) -> Result<VerificationResult, RepositoryError> {
        let mut checks = Vec::new();

        let quoted_audits = sql_array_from_ids(SEED_AUDIT_EVENT_IDS);
        let expected_audit_total = (SEED_AUDIT_EVENT_IDS.len()) as i64;
        let existing_audit_count: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(1) FROM audit_event WHERE id IN {quoted_audits}"
        ))
        .fetch_one(pool)
        .await?;
        checks.push(("audit-events", existing_audit_count == expected_audit_total));

        for flow in SEED_FLOWS {
            let quote_exists: i64 = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM quote WHERE id = ?1 AND status = ?2)",
            )
            .bind(flow.quote_id)
            .bind(flow.status)
            .fetch_one(pool)
            .await?;
            checks.push((flow.quote_id, quote_exists == 1));

            let line_count: i64 =
                sqlx::query_scalar("SELECT COUNT(1) FROM quote_line WHERE quote_id = ?1")
                    .bind(flow.quote_id)
                    .fetch_one(pool)
                    .await?;
            checks.push((flow.line_count_label(), line_count == flow.expected_line_count));

            let state_ok: i64 = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM flow_state WHERE quote_id = ?1 AND flow_type = ?2 AND current_step = ?3 AND step_number = ?4)",
            )
            .bind(flow.quote_id)
            .bind(flow.flow_type)
            .bind(flow.current_step)
            .bind(flow.step_number)
            .fetch_one(pool)
            .await?;
            checks.push((flow.state_label(), state_ok == 1));

            checks.push((flow.metadata_label(), Self::verify_flow_metadata(pool, flow).await?));
            checks
                .push((flow.required_fields_label(), Self::verify_flow_fields(pool, flow).await?));
            checks.push((flow.product_ids_label(), Self::verify_product_ids(pool, flow).await?));
            checks.push((
                flow.discount_policy_label(),
                Self::verify_discount_policy(pool, flow).await?,
            ));

            let quote_created: i64 = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM audit_event WHERE quote_id = ?1 AND event_type = 'quote.created')",
            )
            .bind(flow.quote_id)
            .fetch_one(pool)
            .await?;
            checks.push((flow.audit_created_label(), quote_created == 1));

            if flow.requires_approval_request {
                let approval_requested: i64 = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM audit_event WHERE quote_id = ?1 AND event_type = 'approval.requested')",
                )
                .bind(flow.quote_id)
                .fetch_one(pool)
                .await?;
                checks.push(("approval.requested event", approval_requested == 1));
            }

            if let Some(prior_quote_id) = flow.prior_quote_id {
                let prior_quote_exists: i64 =
                    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM quote WHERE id = ?1)")
                        .bind(prior_quote_id)
                        .fetch_one(pool)
                        .await?;
                checks.push(("prior-quote-exists", prior_quote_exists == 1));
            }
        }

        let all_present = checks.iter().all(|(_, exists)| *exists);
        Ok(VerificationResult { all_present, checks })
    }

    async fn verify_flow_metadata(
        pool: &DbPool,
        flow: &SeedFlowContract,
    ) -> Result<bool, RepositoryError> {
        let metadata_row = sqlx::query_as::<_, (String, String, String)>(
            "SELECT COALESCE(required_fields_json, '[]'), COALESCE(missing_fields_json, '[]'), COALESCE(metadata_json, '{}') FROM flow_state WHERE quote_id = ?",
        )
        .bind(flow.quote_id)
        .fetch_one(pool)
        .await?;
        let (required_fields_json, missing_fields_json, metadata_json) = metadata_row;

        let required_fields: Vec<String> = serde_json::from_str(&required_fields_json)
            .map_err(|error| RepositoryError::Decode(error.to_string()))?;
        let missing_fields: Vec<String> = serde_json::from_str(&missing_fields_json)
            .map_err(|error| RepositoryError::Decode(error.to_string()))?;
        if !json_string_list_matches(&required_fields, flow.required_fields) {
            return Ok(false);
        }
        if !json_string_list_matches(&missing_fields, flow.missing_fields) {
            return Ok(false);
        }

        let metadata: Value = serde_json::from_str(&metadata_json)
            .map_err(|error| RepositoryError::Decode(error.to_string()))?;
        if metadata.get("account_id").and_then(Value::as_str) != Some(flow.account_id) {
            return Ok(false);
        }
        if metadata.get("account_name").and_then(Value::as_str) != Some(flow.account_name) {
            return Ok(false);
        }
        if metadata.get("deal_id").and_then(Value::as_str) != Some(flow.deal_id) {
            return Ok(false);
        }
        if metadata.get("deal_name").and_then(Value::as_str) != Some(flow.deal_name) {
            return Ok(false);
        }
        if metadata.get("policy_profile").and_then(Value::as_str) != Some(flow.policy_profile) {
            return Ok(false);
        }
        let product_names =
            metadata.get("product_names").and_then(Value::as_array).and_then(|values| {
                values
                    .iter()
                    .map(|value| value.as_str().map(String::from))
                    .collect::<Option<Vec<_>>>()
            });
        if let Some(product_names) = product_names {
            if !json_string_list_matches(&product_names, flow.product_names) {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
        if metadata.get("channel").and_then(Value::as_str) != Some("e2e") {
            return Ok(false);
        }

        if let Some(expected_prior_quote) = flow.prior_quote_id {
            if metadata.get("prior_quote_id").and_then(Value::as_str) != Some(expected_prior_quote)
            {
                return Ok(false);
            }
        } else if metadata.get("prior_quote_id").is_some() {
            return Ok(false);
        }

        Ok(true)
    }

    async fn verify_flow_fields(
        pool: &DbPool,
        flow: &SeedFlowContract,
    ) -> Result<bool, RepositoryError> {
        let required_fields: String =
            sqlx::query_scalar("SELECT required_fields_json FROM flow_state WHERE quote_id = ?")
                .bind(flow.quote_id)
                .fetch_one(pool)
                .await?;
        let parsed: Vec<String> = serde_json::from_str(&required_fields)
            .map_err(|error| RepositoryError::Decode(error.to_string()))?;
        Ok(json_string_list_matches(&parsed, flow.required_fields))
    }

    async fn verify_product_ids(
        pool: &DbPool,
        flow: &SeedFlowContract,
    ) -> Result<bool, RepositoryError> {
        let product_ids_json: String =
            sqlx::query_scalar("SELECT json_extract(COALESCE(metadata_json, '{}'), '$.product_ids') FROM flow_state WHERE quote_id = ?")
                .bind(flow.quote_id)
                .fetch_one(pool)
                .await?;
        if product_ids_json.is_empty() || product_ids_json == "null" {
            return Ok(false);
        }

        let product_ids: Vec<String> = serde_json::from_str(&product_ids_json)
            .map_err(|error| RepositoryError::Decode(error.to_string()))?;
        Ok(json_string_list_matches(&product_ids, flow.product_ids))
    }

    async fn verify_discount_policy(
        pool: &DbPool,
        flow: &SeedFlowContract,
    ) -> Result<bool, RepositoryError> {
        let discount_row = sqlx::query_scalar::<_, String>(
            "SELECT COALESCE(metadata_json, '{}') FROM flow_state WHERE quote_id = ?",
        )
        .bind(flow.quote_id)
        .fetch_one(pool)
        .await?;

        let metadata: Value = serde_json::from_str(&discount_row)
            .map_err(|error| RepositoryError::Decode(error.to_string()))?;
        if let Some(expected_discount) = flow.requested_discount_pct {
            let actual = metadata
                .get("requested_discount_pct")
                .and_then(Value::as_u64)
                .and_then(|v| u8::try_from(v).ok());
            if actual != Some(expected_discount) {
                return Ok(false);
            }
            let actual_threshold = metadata
                .get("threshold_pct")
                .and_then(Value::as_u64)
                .and_then(|v| u8::try_from(v).ok());
            if actual_threshold != flow.threshold_pct {
                return Ok(false);
            }
        } else if metadata.get("requested_discount_pct").is_some()
            || metadata.get("threshold_pct").is_some()
        {
            return Ok(false);
        }

        Ok(true)
    }

    /// Clean up seeded fixtures from a test database.
    pub async fn clean(pool: &DbPool) -> Result<(), RepositoryError> {
        let mut tx = pool.begin().await?;

        let quoted_audits = sql_array_from_ids(SEED_AUDIT_EVENT_IDS);
        let quoted_flows = sql_array_from_ids(SEED_FLOW_STATE_IDS);
        let quoted_quotes = sql_array_from_ids(SEED_QUOTE_IDS);

        sqlx::query(&format!("DELETE FROM audit_event WHERE id IN {quoted_audits}"))
            .execute(&mut *tx)
            .await?;
        sqlx::query(&format!("DELETE FROM flow_state WHERE id IN {quoted_flows}"))
            .execute(&mut *tx)
            .await?;
        sqlx::query(&format!("DELETE FROM quote_line WHERE quote_id IN {quoted_quotes}"))
            .execute(&mut *tx)
            .await?;
        sqlx::query(&format!("DELETE FROM quote WHERE id IN {quoted_quotes}"))
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct SeedFlowContract {
    flow_type: &'static str,
    quote_id: &'static str,
    status: &'static str,
    current_step: &'static str,
    step_number: i64,
    expected_line_count: i64,
    required_fields: &'static [&'static str],
    missing_fields: &'static [&'static str],
    account_id: &'static str,
    deal_id: &'static str,
    policy_profile: &'static str,
    product_ids: &'static [&'static str],
    account_name: &'static str,
    deal_name: &'static str,
    product_names: &'static [&'static str],
    requested_discount_pct: Option<u8>,
    threshold_pct: Option<u8>,
    requires_approval_request: bool,
    prior_quote_id: Option<&'static str>,
    description: &'static str,
}

impl SeedFlowContract {
    fn line_count_label(&self) -> &'static str {
        match self.flow_type {
            "net_new" => "quote-netnew-line-count",
            "renewal" => "quote-renewal-line-count",
            _ => "quote-discount-line-count",
        }
    }

    fn state_label(&self) -> &'static str {
        match self.flow_type {
            "net_new" => "flow-netnew-state",
            "renewal" => "flow-renewal-state",
            _ => "flow-discount-state",
        }
    }

    fn metadata_label(&self) -> &'static str {
        match self.flow_type {
            "net_new" => "flow-netnew-metadata",
            "renewal" => "flow-renewal-metadata",
            _ => "flow-discount-metadata",
        }
    }

    fn required_fields_label(&self) -> &'static str {
        match self.flow_type {
            "net_new" => "flow-netnew-required-fields",
            "renewal" => "flow-renewal-required-fields",
            _ => "flow-discount-required-fields",
        }
    }

    fn product_ids_label(&self) -> &'static str {
        match self.flow_type {
            "net_new" => "flow-netnew-product-ids",
            "renewal" => "flow-renewal-product-ids",
            _ => "flow-discount-product-ids",
        }
    }

    fn discount_policy_label(&self) -> &'static str {
        match self.flow_type {
            "net_new" => "flow-netnew-discount-policy",
            "renewal" => "flow-renewal-discount-policy",
            _ => "flow-discount-discount-policy",
        }
    }

    fn audit_created_label(&self) -> &'static str {
        match self.flow_type {
            "net_new" => "audit-netnew-created",
            "renewal" => "audit-renewal-created",
            _ => "audit-discount-created",
        }
    }
}

fn json_string_list_matches(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len() && actual.iter().zip(expected).all(|(a, b)| a == b)
}

fn sql_array_from_ids(ids: &[&str]) -> String {
    let quoted = ids.iter().map(|id| format!("'{}'", id)).collect::<Vec<_>>().join(",");
    format!("({quoted})")
}

#[derive(Debug)]
pub struct SeedResult {
    pub flows_seeded: Vec<FlowSeedInfo>,
}

#[derive(Debug)]
pub struct FlowSeedInfo {
    pub flow_type: &'static str,
    pub quote_id: &'static str,
    pub description: &'static str,
}

#[derive(Debug)]
pub struct VerificationResult {
    pub all_present: bool,
    pub checks: Vec<(&'static str, bool)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connect_with_settings, migrations};

    #[test]
    fn sql_fixture_is_valid() {
        assert!(!E2ESeedDataset::SQL.is_empty());
    }

    #[tokio::test]
    async fn verify_seed_contract_and_idempotency() {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 30)
            .await
            .expect("connect to test database");

        migrations::run_pending(&pool).await.expect("run migrations");

        let first = E2ESeedDataset::load(&pool).await.expect("load seed fixtures");
        let first_verification = E2ESeedDataset::verify(&pool).await.expect("verify seed fixtures");
        assert!(first_verification.all_present);
        assert_eq!(first.flows_seeded.len(), 3);

        let second = E2ESeedDataset::load(&pool).await.expect("reload seed fixtures");
        let second_verification =
            E2ESeedDataset::verify(&pool).await.expect("re-verify seed fixtures");
        assert!(second_verification.all_present);
        assert_eq!(second.flows_seeded.len(), 3);
        assert_eq!(first_verification.checks, second_verification.checks);
    }

    #[tokio::test]
    async fn verify_seed_flow_specific_properties() {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 30)
            .await
            .expect("connect to test database");

        migrations::run_pending(&pool).await.expect("run migrations");

        E2ESeedDataset::load(&pool).await.expect("load seed fixtures");

        let net_new_status: String = sqlx::query_scalar("SELECT status FROM quote WHERE id = ?1")
            .bind("quote-netnew-001")
            .fetch_one(&pool)
            .await
            .expect("query net-new status");
        assert_eq!(net_new_status, "draft");

        let net_new_prior: Option<String> =
            sqlx::query_scalar("SELECT json_extract(metadata_json, '$.prior_quote_id') FROM flow_state WHERE quote_id = ?1")
                .bind("quote-netnew-001")
                .fetch_one(&pool)
                .await
                .expect("query net-new prior quote");
        assert!(net_new_prior.is_none());

        let renewal_prior: String =
            sqlx::query_scalar("SELECT json_extract(metadata_json, '$.prior_quote_id') FROM flow_state WHERE quote_id = ?1")
                .bind("quote-renewal-001")
                .fetch_one(&pool)
                .await
                .expect("query renewal prior quote");
        assert_eq!(renewal_prior, "quote-renewal-prior-001");

        let discount_pct: i64 =
            sqlx::query_scalar("SELECT CAST(json_extract(metadata_json, '$.requested_discount_pct') AS INTEGER) FROM flow_state WHERE quote_id = ?1")
                .bind("quote-discount-001")
                .fetch_one(&pool)
                .await
                .expect("query discount percent");
        assert_eq!(discount_pct, 25);

        let threshold_pct: i64 =
            sqlx::query_scalar("SELECT CAST(json_extract(metadata_json, '$.threshold_pct') AS INTEGER) FROM flow_state WHERE quote_id = ?1")
                .bind("quote-discount-001")
                .fetch_one(&pool)
                .await
                .expect("query discount threshold");
        assert_eq!(threshold_pct, 20);

        let approval_request_events: i64 =
            sqlx::query_scalar("SELECT COUNT(1) FROM audit_event WHERE quote_id = ?1 AND event_type = 'approval.requested'")
                .bind("quote-discount-001")
                .fetch_one(&pool)
                .await
                .expect("query discount approval events");
        assert_eq!(approval_request_events, 1);
    }

    #[test]
    fn seed_contract_json_matches_rust_seed_constants() {
        let contract: serde_json::Value =
            serde_json::from_str(include_str!("../../../config/fixtures/e2e_seed_contract.json"))
                .expect("e2e seed contract JSON must parse");

        assert_eq!(contract["dataset_version"].as_str(), Some("bd-3vp2.7.2"));
        assert_eq!(contract["seed_dataset"].as_str(), Some("deterministic_e2e_core_flows"));

        let contract_flows = contract["flows"].as_array().expect("flows should be an array");
        assert_eq!(contract_flows.len(), SEED_FLOWS.len());

        for flow in SEED_FLOWS {
            let contract_flow = contract_flows
                .iter()
                .find(|candidate| candidate["flow_type"].as_str() == Some(flow.flow_type))
                .expect("contract should include all canonical flow types");

            assert_eq!(contract_flow["flow_type"].as_str(), Some(flow.flow_type));
            assert_eq!(contract_flow["quote_id"].as_str(), Some(flow.quote_id));
            assert_eq!(contract_flow["status"].as_str(), Some(flow.status));
            assert_eq!(contract_flow["current_step"].as_str(), Some(flow.current_step));
            assert_eq!(
                contract_flow["step_number"].as_u64().unwrap_or_default(),
                flow.step_number as u64
            );
            assert_eq!(
                contract_flow["expected_line_count"].as_u64().unwrap_or_default(),
                flow.expected_line_count as u64
            );
            assert_eq!(
                contract_flow["required_fields"]
                    .as_array()
                    .expect("required_fields should be an array")
                    .iter()
                    .map(|value| value.as_str().unwrap_or_default())
                    .collect::<Vec<_>>(),
                flow.required_fields
            );
            assert_eq!(
                contract_flow["missing_fields"]
                    .as_array()
                    .expect("missing_fields should be an array")
                    .iter()
                    .map(|value| value.as_str().unwrap_or_default())
                    .collect::<Vec<_>>(),
                flow.missing_fields
            );
            assert_eq!(contract_flow["account_id"].as_str(), Some(flow.account_id));
            assert_eq!(contract_flow["account_name"].as_str(), Some(flow.account_name));
            assert_eq!(contract_flow["deal_id"].as_str(), Some(flow.deal_id));
            assert_eq!(contract_flow["deal_name"].as_str(), Some(flow.deal_name));
            assert_eq!(contract_flow["policy_profile"].as_str(), Some(flow.policy_profile));
            assert_eq!(
                contract_flow["product_ids"]
                    .as_array()
                    .expect("product_ids should be an array")
                    .iter()
                    .map(|value| value.as_str().unwrap_or_default())
                    .collect::<Vec<_>>(),
                flow.product_ids
            );
            assert_eq!(
                contract_flow["product_names"]
                    .as_array()
                    .expect("product_names should be an array")
                    .iter()
                    .map(|value| value.as_str().unwrap_or_default())
                    .collect::<Vec<_>>(),
                flow.product_names
            );

            if let Some(requested) = flow.requested_discount_pct {
                assert_eq!(
                    contract_flow["requested_discount_pct"].as_u64(),
                    Some(u64::from(requested))
                );
            } else {
                assert!(contract_flow.get("requested_discount_pct").is_none());
            }
            if let Some(threshold) = flow.threshold_pct {
                assert_eq!(contract_flow["threshold_pct"].as_u64(), Some(u64::from(threshold)));
            } else {
                assert!(contract_flow.get("threshold_pct").is_none());
            }

            if let Some(prior_quote_id) = flow.prior_quote_id {
                assert_eq!(contract_flow["prior_quote_id"].as_str(), Some(prior_quote_id));
            } else {
                assert!(contract_flow.get("prior_quote_id").is_none_or(Value::is_null));
            }
        }
    }
}
