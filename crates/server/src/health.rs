use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use quotey_db::DbPool;
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Clone)]
pub struct HealthState {
    db_pool: DbPool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HealthCheck {
    pub status: &'static str,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: HealthCheck,
    pub database: HealthCheck,
    pub checked_at: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct SimilarDealsQuery {
    pub limit: Option<u32>,
    pub min_similarity: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SimilarDealResponse {
    pub quote_id: String,
    pub customer_name: String,
    pub similarity_score: f64,
    pub outcome: String,
    pub final_price: f64,
    pub close_date: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ApiErrorResponse {
    pub error: String,
}

#[derive(Debug, sqlx::FromRow)]
struct SimilarDealRow {
    quote_id: String,
    customer_name: String,
    similarity_score: f64,
    outcome: String,
    final_price: f64,
    close_date: Option<String>,
}

pub fn router(db_pool: DbPool) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/quotes/{id}/similar-deals", get(similar_deals))
        .with_state(HealthState { db_pool: db_pool.clone() })
        .merge(crate::portal::router(db_pool))
}

pub async fn spawn(bind_address: &str, port: u16, db_pool: DbPool) -> std::io::Result<()> {
    let address = format!("{bind_address}:{port}");
    let listener = tokio::net::TcpListener::bind(&address).await?;

    info!(
        event_name = "system.health.start",
        correlation_id = "bootstrap",
        quote_id = "unknown",
        thread_id = "unknown",
        bind_address = %address,
        "health endpoint started"
    );

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router(db_pool)).await {
            error!(
                event_name = "system.health.error",
                correlation_id = "bootstrap",
                quote_id = "unknown",
                thread_id = "unknown",
                error = %error,
                "health endpoint server terminated unexpectedly"
            );
        }
    });

    Ok(())
}

pub async fn health(State(state): State<HealthState>) -> (StatusCode, Json<HealthResponse>) {
    let database = database_check(&state.db_pool).await;
    let ready = database.status == "ready";

    let payload = HealthResponse {
        status: if ready { "ready" } else { "degraded" },
        service: HealthCheck {
            status: "ready",
            detail: "quotey-server runtime initialized".to_string(),
        },
        database,
        checked_at: Utc::now().to_rfc3339(),
    };

    let status_code = if ready { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (status_code, Json(payload))
}

pub async fn similar_deals(
    Path(quote_id): Path<String>,
    Query(params): Query<SimilarDealsQuery>,
    State(state): State<HealthState>,
) -> Result<Json<Vec<SimilarDealResponse>>, (StatusCode, Json<ApiErrorResponse>)> {
    let limit = params.limit.unwrap_or(5).clamp(1, 20) as i64;
    let min_similarity = params.min_similarity.unwrap_or(0.7).clamp(0.0, 1.0);

    let quote_exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM quote WHERE id = ?")
        .bind(&quote_id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(internal_api_error)?;

    if quote_exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse { error: format!("quote `{quote_id}` not found") }),
        ));
    }

    let rows: Vec<SimilarDealRow> = sqlx::query_as(
        r#"
        WITH source AS (
            SELECT id
            FROM configuration_fingerprints
            WHERE quote_id = ?
            ORDER BY created_at DESC
            LIMIT 1
        )
        SELECT
            candidate.quote_id AS quote_id,
            COALESCE(candidate_quote.created_by, 'unknown') AS customer_name,
            sc.similarity_score AS similarity_score,
            COALESCE(deal_outcome.outcome, candidate.outcome_status, 'pending') AS outcome,
            COALESCE(deal_outcome.final_price, candidate.final_price) AS final_price,
            COALESCE(deal_outcome.close_date, candidate.close_date) AS close_date
        FROM source
        JOIN similarity_cache sc ON sc.source_fingerprint_id = source.id
        JOIN configuration_fingerprints candidate ON candidate.id = sc.candidate_fingerprint_id
        LEFT JOIN deal_outcomes deal_outcome ON deal_outcome.quote_id = candidate.quote_id
        LEFT JOIN quote candidate_quote ON candidate_quote.id = candidate.quote_id
        WHERE sc.similarity_score >= ?
        ORDER BY sc.similarity_score DESC
        LIMIT ?
        "#,
    )
    .bind(&quote_id)
    .bind(min_similarity)
    .bind(limit)
    .fetch_all(&state.db_pool)
    .await
    .map_err(internal_api_error)?;

    let deals = rows
        .into_iter()
        .map(|row| SimilarDealResponse {
            quote_id: row.quote_id,
            customer_name: row.customer_name,
            similarity_score: row.similarity_score,
            outcome: row.outcome,
            final_price: row.final_price,
            close_date: row.close_date.unwrap_or_default(),
        })
        .collect();

    Ok(Json(deals))
}

async fn database_check(pool: &DbPool) -> HealthCheck {
    match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(pool).await {
        Ok(_) => HealthCheck { status: "ready", detail: "database query succeeded".to_string() },
        Err(error) => {
            HealthCheck { status: "degraded", detail: format!("database query failed: {error}") }
        }
    }
}

fn internal_api_error(error: sqlx::Error) -> (StatusCode, Json<ApiErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiErrorResponse { error: format!("database query failed: {error}") }),
    )
}

#[cfg(test)]
mod tests {
    use axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        Json,
    };
    use chrono::Utc;
    use quotey_db::{connect_with_settings, migrations};

    use crate::health::{health, similar_deals, HealthState, SimilarDealsQuery};

    #[tokio::test]
    async fn health_returns_ready_when_database_is_reachable() {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 5)
            .await
            .expect("pool should connect");

        let (status, Json(payload)) = health(State(HealthState { db_pool: pool.clone() })).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload.status, "ready");
        assert_eq!(payload.database.status, "ready");
        assert_eq!(payload.service.status, "ready");

        pool.close().await;
    }

    #[tokio::test]
    async fn health_returns_service_unavailable_when_database_is_unavailable() {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 5)
            .await
            .expect("pool should connect");
        pool.close().await;

        let (status, Json(payload)) = health(State(HealthState { db_pool: pool })).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(payload.status, "degraded");
        assert_eq!(payload.database.status, "degraded");
        assert_eq!(payload.service.status, "ready");
    }

    #[tokio::test]
    async fn similar_deals_returns_ranked_results_with_defaults() {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 5)
            .await
            .expect("pool should connect");
        migrations::run_pending(&pool).await.expect("migrations should run");
        seed_similarity_fixture(&pool).await;

        let Json(deals) = similar_deals(
            Path("Q-2026-0001".to_string()),
            Query(SimilarDealsQuery::default()),
            State(HealthState { db_pool: pool.clone() }),
        )
        .await
        .expect("similar deals should succeed");

        assert_eq!(deals.len(), 1);
        assert_eq!(deals[0].quote_id, "Q-2026-0002");
        assert_eq!(deals[0].customer_name, "Acme Corp");
        assert_eq!(deals[0].outcome, "won");
        assert_eq!(deals[0].final_price, 47_000.0);
        assert_eq!(deals[0].close_date, "2026-02-01");

        pool.close().await;
    }

    #[tokio::test]
    async fn similar_deals_respects_limit_and_min_similarity_query_params() {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 5)
            .await
            .expect("pool should connect");
        migrations::run_pending(&pool).await.expect("migrations should run");
        seed_similarity_fixture(&pool).await;

        let Json(deals) = similar_deals(
            Path("Q-2026-0001".to_string()),
            Query(SimilarDealsQuery { limit: Some(1), min_similarity: Some(0.6) }),
            State(HealthState { db_pool: pool.clone() }),
        )
        .await
        .expect("similar deals should succeed");

        assert_eq!(deals.len(), 1);
        assert_eq!(deals[0].quote_id, "Q-2026-0002");

        pool.close().await;
    }

    #[tokio::test]
    async fn similar_deals_returns_not_found_for_missing_quote() {
        let pool = connect_with_settings("sqlite::memory:?cache=shared", 1, 5)
            .await
            .expect("pool should connect");
        migrations::run_pending(&pool).await.expect("migrations should run");

        let (status, Json(error)) = similar_deals(
            Path("Q-404".to_string()),
            Query(SimilarDealsQuery::default()),
            State(HealthState { db_pool: pool.clone() }),
        )
        .await
        .expect_err("missing quote should return not found");

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(error.error.contains("Q-404"));

        pool.close().await;
    }

    async fn seed_similarity_fixture(pool: &sqlx::SqlitePool) {
        let now = Utc::now().to_rfc3339();

        for (id, created_by) in [
            ("Q-2026-0001", "Source Customer"),
            ("Q-2026-0002", "Acme Corp"),
            ("Q-2026-0003", "Globex"),
        ] {
            sqlx::query(
                "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
                 VALUES (?, 'draft', 'USD', ?, ?, ?)",
            )
            .bind(id)
            .bind(created_by)
            .bind(&now)
            .bind(&now)
            .execute(pool)
            .await
            .expect("seed quote");
        }

        sqlx::query(
            "INSERT INTO configuration_fingerprints
             (id, quote_id, fingerprint_hash, configuration_vector, outcome_status, final_price, close_date, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("fp-source")
        .bind("Q-2026-0001")
        .bind("hash-source")
        .bind(vec![1_u8, 2, 3])
        .bind("pending")
        .bind(50_000.0)
        .bind("2026-01-01")
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed source fingerprint");

        sqlx::query(
            "INSERT INTO configuration_fingerprints
             (id, quote_id, fingerprint_hash, configuration_vector, outcome_status, final_price, close_date, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("fp-candidate-1")
        .bind("Q-2026-0002")
        .bind("hash-candidate-1")
        .bind(vec![2_u8, 3, 4])
        .bind("won")
        .bind(45_000.0)
        .bind("2026-01-15")
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed first candidate fingerprint");

        sqlx::query(
            "INSERT INTO configuration_fingerprints
             (id, quote_id, fingerprint_hash, configuration_vector, outcome_status, final_price, close_date, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("fp-candidate-2")
        .bind("Q-2026-0003")
        .bind("hash-candidate-2")
        .bind(vec![9_u8, 8, 7])
        .bind("lost")
        .bind(30_000.0)
        .bind("2026-01-20")
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed second candidate fingerprint");

        sqlx::query(
            "INSERT INTO similarity_cache
             (id, source_fingerprint_id, candidate_fingerprint_id, similarity_score, algorithm_version, computed_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("sim-1")
        .bind("fp-source")
        .bind("fp-candidate-1")
        .bind(0.91_f64)
        .bind("v1")
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed first similarity row");

        sqlx::query(
            "INSERT INTO similarity_cache
             (id, source_fingerprint_id, candidate_fingerprint_id, similarity_score, algorithm_version, computed_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("sim-2")
        .bind("fp-source")
        .bind("fp-candidate-2")
        .bind(0.65_f64)
        .bind("v1")
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed second similarity row");

        sqlx::query(
            "INSERT INTO deal_outcomes
             (id, quote_id, outcome, final_price, close_date, customer_segment, product_mix_json, sales_cycle_days, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind("deal-1")
        .bind("Q-2026-0002")
        .bind("won")
        .bind(47_000.0)
        .bind("2026-02-01")
        .bind("enterprise")
        .bind("[]")
        .bind(35_i64)
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed deal outcome");
    }
}
