use async_trait::async_trait;
use quotey_core::chrono::{DateTime, Utc};
use quotey_core::domain::precedent::{
    PrecedentApprovalPathEvidence, PrecedentApprovalPathId, PrecedentDealOutcome,
    PrecedentDecisionStatus, PrecedentFingerprint, PrecedentOutcomeStatus,
    PrecedentSimilarityEvidence, PrecedentSimilarityEvidenceId,
};
use quotey_core::domain::quote::QuoteId;
use sqlx::{sqlite::SqliteRow, Row};

use super::RepositoryError;
use crate::DbPool;

#[async_trait]
pub trait PrecedentRepository: Send + Sync {
    async fn save_fingerprint(
        &self,
        fingerprint: PrecedentFingerprint,
    ) -> Result<(), RepositoryError>;

    async fn get_latest_fingerprint_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<PrecedentFingerprint>, RepositoryError>;

    async fn save_deal_outcome(&self, outcome: PrecedentDealOutcome)
        -> Result<(), RepositoryError>;

    async fn get_latest_deal_outcome_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<PrecedentDealOutcome>, RepositoryError>;

    async fn save_approval_path_evidence(
        &self,
        evidence: PrecedentApprovalPathEvidence,
    ) -> Result<(), RepositoryError>;

    async fn get_latest_approval_path_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<PrecedentApprovalPathEvidence>, RepositoryError>;

    async fn save_similarity_evidence(
        &self,
        evidence: PrecedentSimilarityEvidence,
    ) -> Result<(), RepositoryError>;

    async fn list_similarity_evidence_for_quote(
        &self,
        quote_id: &QuoteId,
        min_similarity: f64,
        limit: i32,
    ) -> Result<Vec<PrecedentSimilarityEvidence>, RepositoryError>;
}

pub struct SqlPrecedentRepository {
    pool: DbPool,
}

impl SqlPrecedentRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PrecedentRepository for SqlPrecedentRepository {
    async fn save_fingerprint(
        &self,
        fingerprint: PrecedentFingerprint,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO configuration_fingerprints (
                id, quote_id, fingerprint_hash, configuration_vector,
                outcome_status, final_price, close_date, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                quote_id = excluded.quote_id,
                fingerprint_hash = excluded.fingerprint_hash,
                configuration_vector = excluded.configuration_vector,
                outcome_status = excluded.outcome_status,
                final_price = excluded.final_price,
                close_date = excluded.close_date,
                created_at = excluded.created_at
            "#,
        )
        .bind(&fingerprint.id)
        .bind(&fingerprint.quote_id.0)
        .bind(&fingerprint.fingerprint_hash)
        .bind(&fingerprint.configuration_vector)
        .bind(fingerprint.outcome_status.as_str())
        .bind(fingerprint.final_price)
        .bind(fingerprint.close_date.as_deref())
        .bind(fingerprint.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_latest_fingerprint_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<PrecedentFingerprint>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, quote_id, fingerprint_hash, configuration_vector,
                outcome_status, final_price, close_date, created_at
            FROM configuration_fingerprints
            WHERE quote_id = ?
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(&quote_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|value| fingerprint_from_row(&value)).transpose()
    }

    async fn save_deal_outcome(
        &self,
        outcome: PrecedentDealOutcome,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO deal_outcomes (
                id, quote_id, outcome, final_price, close_date,
                customer_segment, product_mix_json, sales_cycle_days, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                quote_id = excluded.quote_id,
                outcome = excluded.outcome,
                final_price = excluded.final_price,
                close_date = excluded.close_date,
                customer_segment = excluded.customer_segment,
                product_mix_json = excluded.product_mix_json,
                sales_cycle_days = excluded.sales_cycle_days,
                created_at = excluded.created_at
            "#,
        )
        .bind(&outcome.id)
        .bind(&outcome.quote_id.0)
        .bind(outcome.outcome_status.as_str())
        .bind(outcome.final_price)
        .bind(&outcome.close_date)
        .bind(outcome.customer_segment.as_deref())
        .bind(&outcome.product_mix_json)
        .bind(outcome.sales_cycle_days)
        .bind(outcome.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_latest_deal_outcome_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<PrecedentDealOutcome>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, quote_id, outcome, final_price, close_date,
                customer_segment, product_mix_json, sales_cycle_days, created_at
            FROM deal_outcomes
            WHERE quote_id = ?
            ORDER BY close_date DESC, created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(&quote_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|value| deal_outcome_from_row(&value)).transpose()
    }

    async fn save_approval_path_evidence(
        &self,
        evidence: PrecedentApprovalPathEvidence,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO precedent_approval_path_evidence (
                id, quote_id, route_version, route_payload_json,
                decision_status, decision_actor_id, decision_reason,
                routed_by_actor_id, idempotency_key, correlation_id,
                routed_at, decided_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(quote_id, route_version) DO UPDATE SET
                route_payload_json = excluded.route_payload_json,
                decision_status = excluded.decision_status,
                decision_actor_id = excluded.decision_actor_id,
                decision_reason = excluded.decision_reason,
                routed_by_actor_id = excluded.routed_by_actor_id,
                idempotency_key = excluded.idempotency_key,
                correlation_id = excluded.correlation_id,
                routed_at = excluded.routed_at,
                decided_at = excluded.decided_at,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&evidence.id.0)
        .bind(&evidence.quote_id.0)
        .bind(evidence.route_version)
        .bind(&evidence.route_payload_json)
        .bind(evidence.decision_status.as_str())
        .bind(evidence.decision_actor_id.as_deref())
        .bind(evidence.decision_reason.as_deref())
        .bind(&evidence.routed_by_actor_id)
        .bind(&evidence.idempotency_key)
        .bind(&evidence.correlation_id)
        .bind(evidence.routed_at.to_rfc3339())
        .bind(evidence.decided_at.map(|value| value.to_rfc3339()))
        .bind(evidence.created_at.to_rfc3339())
        .bind(evidence.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_latest_approval_path_for_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<Option<PrecedentApprovalPathEvidence>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT
                id, quote_id, route_version, route_payload_json,
                decision_status, decision_actor_id, decision_reason,
                routed_by_actor_id, idempotency_key, correlation_id,
                routed_at, decided_at, created_at, updated_at
            FROM precedent_approval_path_evidence
            WHERE quote_id = ?
            ORDER BY route_version DESC, routed_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(&quote_id.0)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|value| approval_path_from_row(&value)).transpose()
    }

    async fn save_similarity_evidence(
        &self,
        evidence: PrecedentSimilarityEvidence,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO precedent_similarity_evidence (
                id, source_quote_id, source_fingerprint_id,
                candidate_quote_id, candidate_fingerprint_id,
                similarity_score, strategy_version, score_components_json,
                evidence_payload_json, idempotency_key, correlation_id,
                computed_at, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(source_fingerprint_id, candidate_fingerprint_id, strategy_version) DO UPDATE SET
                source_quote_id = excluded.source_quote_id,
                candidate_quote_id = excluded.candidate_quote_id,
                similarity_score = excluded.similarity_score,
                score_components_json = excluded.score_components_json,
                evidence_payload_json = excluded.evidence_payload_json,
                idempotency_key = excluded.idempotency_key,
                correlation_id = excluded.correlation_id,
                computed_at = excluded.computed_at,
                created_at = excluded.created_at
            "#,
        )
        .bind(&evidence.id.0)
        .bind(&evidence.source_quote_id.0)
        .bind(&evidence.source_fingerprint_id)
        .bind(&evidence.candidate_quote_id.0)
        .bind(&evidence.candidate_fingerprint_id)
        .bind(evidence.similarity_score)
        .bind(&evidence.strategy_version)
        .bind(&evidence.score_components_json)
        .bind(&evidence.evidence_payload_json)
        .bind(&evidence.idempotency_key)
        .bind(&evidence.correlation_id)
        .bind(evidence.computed_at.to_rfc3339())
        .bind(evidence.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_similarity_evidence_for_quote(
        &self,
        quote_id: &QuoteId,
        min_similarity: f64,
        limit: i32,
    ) -> Result<Vec<PrecedentSimilarityEvidence>, RepositoryError> {
        let min_similarity =
            if min_similarity.is_finite() { min_similarity.clamp(0.0, 1.0) } else { 0.0 };
        let limit = limit.clamp(1, 100);

        let rows = sqlx::query(
            r#"
            SELECT
                id, source_quote_id, source_fingerprint_id,
                candidate_quote_id, candidate_fingerprint_id,
                similarity_score, strategy_version, score_components_json,
                evidence_payload_json, idempotency_key, correlation_id,
                computed_at, created_at
            FROM precedent_similarity_evidence
            WHERE source_quote_id = ? AND similarity_score >= ?
            ORDER BY similarity_score DESC, computed_at DESC, candidate_quote_id ASC
            LIMIT ?
            "#,
        )
        .bind(&quote_id.0)
        .bind(min_similarity)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(similarity_evidence_from_row).collect()
    }
}

fn fingerprint_from_row(row: &SqliteRow) -> Result<PrecedentFingerprint, RepositoryError> {
    let outcome_status_raw: String = row.try_get("outcome_status")?;
    let outcome_status = PrecedentOutcomeStatus::parse(&outcome_status_raw).ok_or_else(|| {
        RepositoryError::Decode(format!(
            "invalid precedent fingerprint outcome_status: {}",
            outcome_status_raw
        ))
    })?;

    Ok(PrecedentFingerprint {
        id: row.try_get("id")?,
        quote_id: QuoteId(row.try_get("quote_id")?),
        fingerprint_hash: row.try_get("fingerprint_hash")?,
        configuration_vector: row.try_get("configuration_vector")?,
        outcome_status,
        final_price: row.try_get("final_price")?,
        close_date: row.try_get("close_date")?,
        created_at: parse_rfc3339(
            "precedent fingerprint created_at",
            &row.try_get::<String, _>("created_at")?,
        )?,
    })
}

fn deal_outcome_from_row(row: &SqliteRow) -> Result<PrecedentDealOutcome, RepositoryError> {
    let outcome_raw: String = row.try_get("outcome")?;
    let outcome_status = PrecedentOutcomeStatus::parse(&outcome_raw).ok_or_else(|| {
        RepositoryError::Decode(format!("invalid precedent deal outcome status: {}", outcome_raw))
    })?;

    Ok(PrecedentDealOutcome {
        id: row.try_get("id")?,
        quote_id: QuoteId(row.try_get("quote_id")?),
        outcome_status,
        final_price: row.try_get("final_price")?,
        close_date: row.try_get("close_date")?,
        customer_segment: row.try_get("customer_segment")?,
        product_mix_json: row.try_get("product_mix_json")?,
        sales_cycle_days: row.try_get("sales_cycle_days")?,
        created_at: parse_rfc3339(
            "precedent deal outcome created_at",
            &row.try_get::<String, _>("created_at")?,
        )?,
    })
}

fn approval_path_from_row(
    row: &SqliteRow,
) -> Result<PrecedentApprovalPathEvidence, RepositoryError> {
    let decision_status_raw: String = row.try_get("decision_status")?;
    let decision_status =
        PrecedentDecisionStatus::parse(&decision_status_raw).ok_or_else(|| {
            RepositoryError::Decode(format!(
                "invalid precedent approval decision_status: {}",
                decision_status_raw
            ))
        })?;

    let routed_at =
        parse_rfc3339("precedent approval routed_at", &row.try_get::<String, _>("routed_at")?)?;
    let decided_at = row
        .try_get::<Option<String>, _>("decided_at")?
        .as_deref()
        .map(|ts| parse_rfc3339("precedent approval decided_at", ts))
        .transpose()?;
    let created_at =
        parse_rfc3339("precedent approval created_at", &row.try_get::<String, _>("created_at")?)?;
    let updated_at =
        parse_rfc3339("precedent approval updated_at", &row.try_get::<String, _>("updated_at")?)?;

    Ok(PrecedentApprovalPathEvidence {
        id: PrecedentApprovalPathId(row.try_get("id")?),
        quote_id: QuoteId(row.try_get("quote_id")?),
        route_version: row.try_get("route_version")?,
        route_payload_json: row.try_get("route_payload_json")?,
        decision_status,
        decision_actor_id: row.try_get("decision_actor_id")?,
        decision_reason: row.try_get("decision_reason")?,
        routed_by_actor_id: row.try_get("routed_by_actor_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        correlation_id: row.try_get("correlation_id")?,
        routed_at,
        decided_at,
        created_at,
        updated_at,
    })
}

fn similarity_evidence_from_row(
    row: &SqliteRow,
) -> Result<PrecedentSimilarityEvidence, RepositoryError> {
    Ok(PrecedentSimilarityEvidence {
        id: PrecedentSimilarityEvidenceId(row.try_get("id")?),
        source_quote_id: QuoteId(row.try_get("source_quote_id")?),
        source_fingerprint_id: row.try_get("source_fingerprint_id")?,
        candidate_quote_id: QuoteId(row.try_get("candidate_quote_id")?),
        candidate_fingerprint_id: row.try_get("candidate_fingerprint_id")?,
        similarity_score: row.try_get("similarity_score")?,
        strategy_version: row.try_get("strategy_version")?,
        score_components_json: row.try_get("score_components_json")?,
        evidence_payload_json: row.try_get("evidence_payload_json")?,
        idempotency_key: row.try_get("idempotency_key")?,
        correlation_id: row.try_get("correlation_id")?,
        computed_at: parse_rfc3339(
            "precedent similarity computed_at",
            &row.try_get::<String, _>("computed_at")?,
        )?,
        created_at: parse_rfc3339(
            "precedent similarity created_at",
            &row.try_get::<String, _>("created_at")?,
        )?,
    })
}

fn parse_rfc3339(field: &str, value: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(value).map(|ts| ts.with_timezone(&Utc)).map_err(|err| {
        RepositoryError::Decode(format!("invalid {} timestamp '{}': {}", field, value, err))
    })
}

#[cfg(test)]
mod tests {
    use quotey_core::chrono::{DateTime, Utc};
    use quotey_core::domain::precedent::{
        PrecedentApprovalPathEvidence, PrecedentApprovalPathId, PrecedentDealOutcome,
        PrecedentDecisionStatus, PrecedentFingerprint, PrecedentOutcomeStatus,
        PrecedentSimilarityEvidence, PrecedentSimilarityEvidenceId,
    };
    use quotey_core::domain::quote::QuoteId;

    use super::{PrecedentRepository, SqlPrecedentRepository};
    use crate::{connect_with_settings, migrations, DbPool};

    type TestResult<T> = Result<T, String>;

    #[tokio::test]
    async fn sql_precedent_repo_round_trip_for_fingerprints_and_outcomes() -> TestResult<()> {
        let pool = setup_pool().await?;
        let quote_id = QuoteId("Q-PRE-100".to_string());
        insert_quote(&pool, &quote_id).await?;

        let repo = SqlPrecedentRepository::new(pool.clone());

        let first = PrecedentFingerprint {
            id: "fp-pre-100-a".to_string(),
            quote_id: quote_id.clone(),
            fingerprint_hash: "hash-a".to_string(),
            configuration_vector: vec![1, 2, 3],
            outcome_status: PrecedentOutcomeStatus::Pending,
            final_price: 42_000.0,
            close_date: None,
            created_at: parse_ts("2026-02-24T01:00:00Z")?,
        };
        let second = PrecedentFingerprint {
            id: "fp-pre-100-b".to_string(),
            quote_id: quote_id.clone(),
            fingerprint_hash: "hash-b".to_string(),
            configuration_vector: vec![4, 5, 6],
            outcome_status: PrecedentOutcomeStatus::Won,
            final_price: 47_500.0,
            close_date: Some("2026-02-24".to_string()),
            created_at: parse_ts("2026-02-24T01:01:00Z")?,
        };

        repo.save_fingerprint(first)
            .await
            .map_err(|error| format!("save first fingerprint: {error}"))?;
        repo.save_fingerprint(second.clone())
            .await
            .map_err(|error| format!("save second fingerprint: {error}"))?;

        let latest = repo
            .get_latest_fingerprint_for_quote(&quote_id)
            .await
            .map_err(|error| format!("load latest fingerprint: {error}"))?;
        let latest = latest.ok_or_else(|| "fingerprint exists".to_string())?;
        if latest.id != second.id {
            return Err(format!(
                "latest fingerprint id mismatch: {:?} != {:?}",
                latest.id, second.id
            ));
        }
        if latest.fingerprint_hash != "hash-b" {
            return Err(format!(
                "latest fingerprint hash mismatch: {:?} != hash-b",
                latest.fingerprint_hash
            ));
        }

        let outcome = PrecedentDealOutcome {
            id: "deal-pre-100".to_string(),
            quote_id: quote_id.clone(),
            outcome_status: PrecedentOutcomeStatus::Won,
            final_price: 48_100.0,
            close_date: "2026-02-25".to_string(),
            customer_segment: Some("enterprise".to_string()),
            product_mix_json: "[\"plan-pro\",\"support-premium\"]".to_string(),
            sales_cycle_days: Some(33),
            created_at: parse_ts("2026-02-25T10:00:00Z")?,
        };

        repo.save_deal_outcome(outcome.clone())
            .await
            .map_err(|error| format!("save outcome: {error}"))?;

        let fetched_outcome = repo
            .get_latest_deal_outcome_for_quote(&quote_id)
            .await
            .map_err(|error| format!("load outcome: {error}"))?;
        let fetched_outcome = fetched_outcome.ok_or_else(|| "outcome exists".to_string())?;
        if fetched_outcome.id != outcome.id {
            return Err(format!(
                "outcome id mismatch: {:?} != {:?}",
                fetched_outcome.id, outcome.id
            ));
        }
        if fetched_outcome.outcome_status != PrecedentOutcomeStatus::Won {
            return Err(format!(
                "outcome status mismatch: {:?} != {:?}",
                fetched_outcome.outcome_status,
                PrecedentOutcomeStatus::Won
            ));
        }
        if fetched_outcome.sales_cycle_days != Some(33) {
            return Err(format!(
                "outcome sales_cycle_days mismatch: {:?} != {:?}",
                fetched_outcome.sales_cycle_days,
                Some(33)
            ));
        }

        pool.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn sql_precedent_repo_round_trip_for_approval_and_similarity_evidence() -> TestResult<()>
    {
        let pool = setup_pool().await?;
        let source_quote = QuoteId("Q-PRE-SRC".to_string());
        let candidate_a = QuoteId("Q-PRE-A".to_string());
        let candidate_b = QuoteId("Q-PRE-B".to_string());

        insert_quote(&pool, &source_quote).await?;
        insert_quote(&pool, &candidate_a).await?;
        insert_quote(&pool, &candidate_b).await?;

        let repo = SqlPrecedentRepository::new(pool.clone());

        let source_fp = PrecedentFingerprint {
            id: "fp-pre-src".to_string(),
            quote_id: source_quote.clone(),
            fingerprint_hash: "hash-src".to_string(),
            configuration_vector: vec![1, 1, 1],
            outcome_status: PrecedentOutcomeStatus::Pending,
            final_price: 39_000.0,
            close_date: None,
            created_at: parse_ts("2026-02-24T02:00:00Z")?,
        };
        let candidate_a_fp = PrecedentFingerprint {
            id: "fp-pre-a".to_string(),
            quote_id: candidate_a.clone(),
            fingerprint_hash: "hash-a".to_string(),
            configuration_vector: vec![1, 1, 2],
            outcome_status: PrecedentOutcomeStatus::Won,
            final_price: 41_000.0,
            close_date: Some("2026-02-20".to_string()),
            created_at: parse_ts("2026-02-24T02:01:00Z")?,
        };
        let candidate_b_fp = PrecedentFingerprint {
            id: "fp-pre-b".to_string(),
            quote_id: candidate_b.clone(),
            fingerprint_hash: "hash-b".to_string(),
            configuration_vector: vec![9, 9, 9],
            outcome_status: PrecedentOutcomeStatus::Lost,
            final_price: 21_000.0,
            close_date: Some("2026-02-18".to_string()),
            created_at: parse_ts("2026-02-24T02:02:00Z")?,
        };

        repo.save_fingerprint(source_fp)
            .await
            .map_err(|error| format!("save source fingerprint: {error}"))?;
        repo.save_fingerprint(candidate_a_fp)
            .await
            .map_err(|error| format!("save candidate a fingerprint: {error}"))?;
        repo.save_fingerprint(candidate_b_fp)
            .await
            .map_err(|error| format!("save candidate b fingerprint: {error}"))?;

        let approval_v1 = PrecedentApprovalPathEvidence {
            id: PrecedentApprovalPathId("pre-appr-1".to_string()),
            quote_id: source_quote.clone(),
            route_version: 1,
            route_payload_json: "{\"path\":[\"manager\"]}".to_string(),
            decision_status: PrecedentDecisionStatus::Pending,
            decision_actor_id: None,
            decision_reason: None,
            routed_by_actor_id: "U-SYSTEM".to_string(),
            idempotency_key: "idem-pre-appr-1".to_string(),
            correlation_id: "corr-pre-1".to_string(),
            routed_at: parse_ts("2026-02-24T02:10:00Z")?,
            decided_at: None,
            created_at: parse_ts("2026-02-24T02:10:00Z")?,
            updated_at: parse_ts("2026-02-24T02:10:00Z")?,
        };
        let approval_v2 = PrecedentApprovalPathEvidence {
            id: PrecedentApprovalPathId("pre-appr-2".to_string()),
            quote_id: source_quote.clone(),
            route_version: 2,
            route_payload_json: "{\"path\":[\"manager\",\"vp_sales\"]}".to_string(),
            decision_status: PrecedentDecisionStatus::Approved,
            decision_actor_id: Some("U-VP-1".to_string()),
            decision_reason: Some("strategic account".to_string()),
            routed_by_actor_id: "U-SYSTEM".to_string(),
            idempotency_key: "idem-pre-appr-2".to_string(),
            correlation_id: "corr-pre-2".to_string(),
            routed_at: parse_ts("2026-02-24T02:20:00Z")?,
            decided_at: Some(parse_ts("2026-02-24T02:21:00Z")?),
            created_at: parse_ts("2026-02-24T02:20:00Z")?,
            updated_at: parse_ts("2026-02-24T02:21:00Z")?,
        };

        repo.save_approval_path_evidence(approval_v1)
            .await
            .map_err(|error| format!("save approval v1: {error}"))?;
        repo.save_approval_path_evidence(approval_v2.clone())
            .await
            .map_err(|error| format!("save approval v2: {error}"))?;
        repo.save_approval_path_evidence(PrecedentApprovalPathEvidence {
            id: PrecedentApprovalPathId("pre-appr-2-rewrite".to_string()),
            quote_id: source_quote.clone(),
            route_version: 2,
            route_payload_json: "{\"path\":[\"manager\",\"vp_sales\",\"finance\"]}".to_string(),
            decision_status: PrecedentDecisionStatus::Escalated,
            decision_actor_id: Some("U-MGR-1".to_string()),
            decision_reason: Some("requires finance confirmation".to_string()),
            routed_by_actor_id: "U-SYSTEM".to_string(),
            idempotency_key: "idem-pre-appr-2-rewrite".to_string(),
            correlation_id: "corr-pre-2-rewrite".to_string(),
            routed_at: parse_ts("2026-02-24T02:22:00Z")?,
            decided_at: Some(parse_ts("2026-02-24T02:23:00Z")?),
            created_at: parse_ts("2026-02-24T02:22:00Z")?,
            updated_at: parse_ts("2026-02-24T02:23:00Z")?,
        })
        .await
        .map_err(|error| format!("rewrite approval route version: {error}"))?;

        let latest_approval = repo
            .get_latest_approval_path_for_quote(&source_quote)
            .await
            .map_err(|error| format!("load latest approval: {error}"))?;
        let latest_approval = latest_approval.ok_or_else(|| "approval exists".to_string())?;
        if latest_approval.route_version != 2 {
            return Err(format!(
                "latest approval route_version mismatch: {:?} != 2",
                latest_approval.route_version
            ));
        }
        if latest_approval.decision_status != PrecedentDecisionStatus::Escalated {
            return Err(format!(
                "latest approval decision_status mismatch: {:?} != {:?}",
                latest_approval.decision_status,
                PrecedentDecisionStatus::Escalated
            ));
        }
        if latest_approval.decision_actor_id.as_deref() != Some("U-MGR-1") {
            return Err("latest approval actor id mismatch".to_string());
        }
        if latest_approval.decision_reason.as_deref() != Some("requires finance confirmation") {
            return Err("latest approval reason mismatch".to_string());
        }

        let similarity_a = PrecedentSimilarityEvidence {
            id: PrecedentSimilarityEvidenceId("pre-sim-a".to_string()),
            source_quote_id: source_quote.clone(),
            source_fingerprint_id: "fp-pre-src".to_string(),
            candidate_quote_id: candidate_a.clone(),
            candidate_fingerprint_id: "fp-pre-a".to_string(),
            similarity_score: 0.91,
            strategy_version: "simhash-v1".to_string(),
            score_components_json: "{\"hamming_distance\":11}".to_string(),
            evidence_payload_json: "{\"normalization\":\"v1\"}".to_string(),
            idempotency_key: "idem-pre-sim-a".to_string(),
            correlation_id: "corr-pre-sim-1".to_string(),
            computed_at: parse_ts("2026-02-24T02:30:00Z")?,
            created_at: parse_ts("2026-02-24T02:30:00Z")?,
        };
        let similarity_b = PrecedentSimilarityEvidence {
            id: PrecedentSimilarityEvidenceId("pre-sim-b".to_string()),
            source_quote_id: source_quote.clone(),
            source_fingerprint_id: "fp-pre-src".to_string(),
            candidate_quote_id: candidate_b.clone(),
            candidate_fingerprint_id: "fp-pre-b".to_string(),
            similarity_score: 0.73,
            strategy_version: "simhash-v1".to_string(),
            score_components_json: "{\"hamming_distance\":34}".to_string(),
            evidence_payload_json: "{\"normalization\":\"v1\"}".to_string(),
            idempotency_key: "idem-pre-sim-b".to_string(),
            correlation_id: "corr-pre-sim-2".to_string(),
            computed_at: parse_ts("2026-02-24T02:31:00Z")?,
            created_at: parse_ts("2026-02-24T02:31:00Z")?,
        };

        repo.save_similarity_evidence(similarity_a.clone())
            .await
            .map_err(|error| format!("save similarity a: {error}"))?;
        repo.save_similarity_evidence(similarity_b)
            .await
            .map_err(|error| format!("save similarity b: {error}"))?;
        repo.save_similarity_evidence(PrecedentSimilarityEvidence {
            id: PrecedentSimilarityEvidenceId("pre-sim-a-rewrite".to_string()),
            source_quote_id: source_quote.clone(),
            source_fingerprint_id: "fp-pre-src".to_string(),
            candidate_quote_id: candidate_a.clone(),
            candidate_fingerprint_id: "fp-pre-a".to_string(),
            similarity_score: 0.95,
            strategy_version: "simhash-v1".to_string(),
            score_components_json: "{\"hamming_distance\":6}".to_string(),
            evidence_payload_json: "{\"normalization\":\"v1\",\"adjustment\":\"rewrite\"}"
                .to_string(),
            idempotency_key: "idem-pre-sim-a-rewrite".to_string(),
            correlation_id: "corr-pre-sim-1-rewrite".to_string(),
            computed_at: parse_ts("2026-02-24T02:32:00Z")?,
            created_at: parse_ts("2026-02-24T02:32:00Z")?,
        })
        .await
        .map_err(|error| format!("rewrite similarity evidence: {error}"))?;

        let filtered = repo
            .list_similarity_evidence_for_quote(&source_quote, 0.8, 10)
            .await
            .map_err(|error| format!("list filtered similarities: {error}"))?;
        if filtered.len() != 1 {
            return Err(format!("expected 1 filtered row, got {}", filtered.len()));
        }
        if filtered[0].id != similarity_a.id {
            return Err(format!(
                "filtered similarity id mismatch: {:?} != {:?}",
                filtered[0].id, similarity_a.id
            ));
        }
        if (filtered[0].similarity_score - 0.95).abs() >= f64::EPSILON {
            return Err("filtered similarity score mismatch".to_string());
        }
        if filtered[0].candidate_quote_id != candidate_a {
            return Err(format!(
                "filtered similarity candidate mismatch: {:?} != {:?}",
                filtered[0].candidate_quote_id, candidate_a
            ));
        }

        let top_one = repo
            .list_similarity_evidence_for_quote(&source_quote, 0.0, 1)
            .await
            .map_err(|error| format!("list top similarity: {error}"))?;
        if top_one.len() != 1 {
            return Err(format!("expected 1 top row, got {}", top_one.len()));
        }
        if top_one[0].similarity_score != 0.95 {
            return Err(format!(
                "top similarity score mismatch: {:?} != {:?}",
                top_one[0].similarity_score, 0.95
            ));
        }

        let with_nan_threshold = repo
            .list_similarity_evidence_for_quote(&source_quote, f64::NAN, 10)
            .await
            .map_err(|error| format!("list nan threshold similarities: {error}"))?;
        if with_nan_threshold.len() != 2 {
            return Err(format!(
                "expected 2 rows for nan threshold, got {}",
                with_nan_threshold.len()
            ));
        }

        pool.close().await;
        Ok(())
    }

    async fn setup_pool() -> TestResult<DbPool> {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 30)
            .await
            .map_err(|error| format!("connect test pool: {error}"))?;
        migrations::run_pending(&pool).await.map_err(|error| format!("run migrations: {error}"))?;
        Ok(pool)
    }

    async fn insert_quote(pool: &DbPool, quote_id: &QuoteId) -> TestResult<()> {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES (?, 'draft', 'USD', 'U-PRE', ?, ?)",
        )
        .bind(&quote_id.0)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|error| format!("insert quote fixture: {error}"))?;
        Ok(())
    }

    fn parse_ts(value: &str) -> TestResult<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(value)
            .map(|timestamp| timestamp.with_timezone(&Utc))
            .map_err(|error| format!("parse rfc3339 timestamp `{value}`: {error}"))
    }
}
