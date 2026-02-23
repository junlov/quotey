use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use chrono::Utc;
use quotey_db::DbPool;
use serde::Serialize;
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

pub fn router(db_pool: DbPool) -> Router {
    Router::new().route("/health", get(health)).with_state(HealthState { db_pool })
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

async fn database_check(pool: &DbPool) -> HealthCheck {
    match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(pool).await {
        Ok(_) => HealthCheck { status: "ready", detail: "database query succeeded".to_string() },
        Err(error) => {
            HealthCheck { status: "degraded", detail: format!("database query failed: {error}") }
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{extract::State, http::StatusCode, Json};
    use quotey_db::connect_with_settings;

    use crate::health::{health, HealthState};

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
}
