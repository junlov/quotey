//! Web portal routes for customer-facing quote approval and interaction.
//!
//! Endpoints:
//! - `POST /quote/{token}/approve` — capture electronic approval
//! - `POST /quote/{token}/reject`  — capture rejection with reason
//! - `POST /quote/{token}/comment` — add a customer comment

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use chrono::Utc;
use quotey_db::DbPool;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct PortalState {
    db_pool: DbPool,
}

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ApproveRequest {
    #[serde(rename = "approverName")]
    pub approver_name: String,
    #[serde(rename = "approverEmail")]
    pub approver_email: String,
    pub comments: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CommentRequest {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct PortalResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct PortalError {
    pub error: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn router(db_pool: DbPool) -> Router {
    Router::new()
        .route("/quote/{token}/approve", post(approve_quote))
        .route("/quote/{token}/reject", post(reject_quote))
        .route("/quote/{token}/comment", post(add_comment))
        .with_state(PortalState { db_pool })
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn approve_quote(
    Path(token): Path<String>,
    State(state): State<PortalState>,
    Json(body): Json<ApproveRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<PortalError>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token).await?;

    // Validate required fields
    let approver_name = body.approver_name.trim();
    let approver_email = body.approver_email.trim();
    if approver_name.is_empty() || approver_email.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError { error: "approver name and email are required".to_string() }),
        ));
    }

    let now = Utc::now();
    let approval_id = format!("PAPR-{}", &uuid_v4()[..12]);

    // Record the approval
    sqlx::query(
        "INSERT INTO approval_request
            (id, quote_id, approver_role, reason, justification, status,
             requested_by, created_at, updated_at)
         VALUES (?, ?, 'customer', 'Customer portal approval', ?, 'approved', ?, ?, ?)",
    )
    .bind(&approval_id)
    .bind(&quote_id)
    .bind(body.comments.as_deref().unwrap_or(""))
    .bind(format!("portal:{}:{}", approver_email, approver_name))
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    // Update quote status to Approved
    sqlx::query("UPDATE quote SET status = 'approved', updated_at = ? WHERE id = ?")
        .bind(now.to_rfc3339())
        .bind(&quote_id)
        .execute(&state.db_pool)
        .await
        .map_err(db_error)?;

    // Record audit event
    record_audit_event(
        &state.db_pool,
        &quote_id,
        "portal.approval",
        &format!("Quote approved by {} ({}) via web portal", approver_name, approver_email),
    )
    .await;

    info!(
        event_name = "portal.quote.approved",
        correlation_id = %approval_id,
        quote_id = %quote_id,
        approver_name = %approver_name,
        approver_email = %approver_email,
        "quote approved via web portal"
    );

    Ok(Json(PortalResponse {
        success: true,
        message: format!(
            "Quote {quote_id} approved successfully. Your sales rep has been notified."
        ),
    }))
}

async fn reject_quote(
    Path(token): Path<String>,
    State(state): State<PortalState>,
    Json(body): Json<RejectRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<PortalError>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token).await?;

    let reason = body.reason.trim();
    if reason.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError { error: "rejection reason is required".to_string() }),
        ));
    }

    let now = Utc::now();
    let rejection_id = format!("PREJ-{}", &uuid_v4()[..12]);

    // Record the rejection
    sqlx::query(
        "INSERT INTO approval_request
            (id, quote_id, approver_role, reason, justification, status,
             requested_by, created_at, updated_at)
         VALUES (?, ?, 'customer', ?, '', 'rejected', 'portal:customer', ?, ?)",
    )
    .bind(&rejection_id)
    .bind(&quote_id)
    .bind(reason)
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    // Update quote status to Rejected
    sqlx::query("UPDATE quote SET status = 'rejected', updated_at = ? WHERE id = ?")
        .bind(now.to_rfc3339())
        .bind(&quote_id)
        .execute(&state.db_pool)
        .await
        .map_err(db_error)?;

    // Record audit event
    record_audit_event(
        &state.db_pool,
        &quote_id,
        "portal.rejection",
        &format!("Quote declined via web portal: {reason}"),
    )
    .await;

    info!(
        event_name = "portal.quote.rejected",
        correlation_id = %rejection_id,
        quote_id = %quote_id,
        "quote rejected via web portal"
    );

    Ok(Json(PortalResponse {
        success: true,
        message: "Quote declined. Your sales rep has been notified.".to_string(),
    }))
}

async fn add_comment(
    Path(token): Path<String>,
    State(state): State<PortalState>,
    Json(body): Json<CommentRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<PortalError>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token).await?;

    let text = body.text.trim();
    if text.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError { error: "comment text is required".to_string() }),
        ));
    }

    // Record audit event as a comment
    record_audit_event(
        &state.db_pool,
        &quote_id,
        "portal.comment",
        &format!("Customer comment: {text}"),
    )
    .await;

    info!(
        event_name = "portal.quote.comment",
        quote_id = %quote_id,
        "customer comment added via web portal"
    );

    Ok(Json(PortalResponse {
        success: true,
        message: "Comment added. Your sales rep will be notified.".to_string(),
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a sharing token to a quote ID.
///
/// For now, tokens are the quote ID itself. A future migration can add a
/// dedicated `quote_sharing_token` table for time-limited, revocable links.
async fn resolve_quote_by_token(
    pool: &DbPool,
    token: &str,
) -> Result<String, (StatusCode, Json<PortalError>)> {
    let row: Option<sqlx::sqlite::SqliteRow> = sqlx::query("SELECT id FROM quote WHERE id = ?")
        .bind(token)
        .fetch_optional(pool)
        .await
        .map_err(db_error)?;

    match row {
        Some(r) => {
            let id: String = r.try_get("id").map_err(|e| {
                db_error(sqlx::Error::ColumnDecode { index: "id".to_string(), source: Box::new(e) })
            })?;
            Ok(id)
        }
        None => {
            warn!(token = %token, "portal: invalid or expired quote token");
            Err((
                StatusCode::NOT_FOUND,
                Json(PortalError { error: "quote not found or link has expired".to_string() }),
            ))
        }
    }
}

/// Record an audit event for traceability.
///
/// Uses the existing `audit_event` schema from migration 0001:
///   id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json
async fn record_audit_event(pool: &DbPool, quote_id: &str, event_type: &str, detail: &str) {
    let now = Utc::now();
    let audit_id = format!("PAUD-{}", &uuid_v4()[..12]);

    let payload = serde_json::json!({ "detail": detail }).to_string();

    let result = sqlx::query(
        "INSERT INTO audit_event
            (id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json)
         VALUES (?, ?, 'portal', 'system', ?, ?, 'portal', ?)",
    )
    .bind(&audit_id)
    .bind(now.to_rfc3339())
    .bind(quote_id)
    .bind(event_type)
    .bind(&payload)
    .execute(pool)
    .await;

    if let Err(e) = result {
        error!(
            event_name = "portal.audit.write_failed",
            quote_id = %quote_id,
            error = %e,
            "failed to write portal audit event"
        );
    }
}

fn db_error(error: sqlx::Error) -> (StatusCode, Json<PortalError>) {
    error!(error = %error, "portal database error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(PortalError { error: "an internal error occurred".to_string() }),
    )
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    // Deterministic-ish unique ID from timestamp + thread ID for local-first use
    let thread_id = std::thread::current().id();
    format!("{:016x}{:?}", nanos, thread_id)
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(32)
        .collect()
}

#[cfg(test)]
mod tests {
    use axum::{extract::State, Json};
    use chrono::Utc;
    use quotey_db::{connect_with_settings, migrations};

    use super::*;

    async fn setup() -> (sqlx::SqlitePool, String) {
        let pool = connect_with_settings("sqlite::memory:", 1, 30).await.expect("connect");
        migrations::run_pending(&pool).await.expect("migrations");

        // Seed a quote
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES ('Q-TEST-001', 'sent', 'USD', 'test-rep', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed quote");

        (pool, "Q-TEST-001".to_string())
    }

    fn state(pool: sqlx::SqlitePool) -> State<PortalState> {
        State(PortalState { db_pool: pool })
    }

    #[tokio::test]
    async fn approve_quote_records_approval_and_updates_status() {
        let (pool, quote_id) = setup().await;

        let result = approve_quote(
            axum::extract::Path(quote_id.clone()),
            state(pool.clone()),
            Json(ApproveRequest {
                approver_name: "Jane Doe".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: Some("Looks great!".to_string()),
            }),
        )
        .await
        .expect("should succeed");

        assert!(result.0.success);
        assert!(result.0.message.contains(&quote_id));

        // Verify quote status updated
        let status: String = sqlx::query_scalar("SELECT status FROM quote WHERE id = ?")
            .bind(&quote_id)
            .fetch_one(&pool)
            .await
            .expect("fetch status");
        assert_eq!(status, "approved");

        // Verify approval record created
        let approval_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM approval_request WHERE quote_id = ?")
                .bind(&quote_id)
                .fetch_one(&pool)
                .await
                .expect("count approvals");
        assert_eq!(approval_count, 1);

        // Verify audit event recorded
        let audit_payload: String = sqlx::query_scalar(
            "SELECT payload_json FROM audit_event WHERE quote_id = ? AND event_type = 'portal.approval'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch audit");
        assert!(audit_payload.contains("approved"));
    }

    #[tokio::test]
    async fn approve_quote_rejects_empty_name() {
        let (pool, quote_id) = setup().await;

        let result = approve_quote(
            axum::extract::Path(quote_id),
            state(pool),
            Json(ApproveRequest {
                approver_name: "  ".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn reject_quote_records_rejection_and_updates_status() {
        let (pool, quote_id) = setup().await;

        let result = reject_quote(
            axum::extract::Path(quote_id.clone()),
            state(pool.clone()),
            Json(RejectRequest { reason: "Pricing too high for our budget".to_string() }),
        )
        .await
        .expect("should succeed");

        assert!(result.0.success);

        // Verify quote status updated
        let status: String = sqlx::query_scalar("SELECT status FROM quote WHERE id = ?")
            .bind(&quote_id)
            .fetch_one(&pool)
            .await
            .expect("fetch status");
        assert_eq!(status, "rejected");

        // Verify rejection record
        let rejection_status: String = sqlx::query_scalar(
            "SELECT status FROM approval_request WHERE quote_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch rejection");
        assert_eq!(rejection_status, "rejected");
    }

    #[tokio::test]
    async fn reject_quote_rejects_empty_reason() {
        let (pool, quote_id) = setup().await;

        let result = reject_quote(
            axum::extract::Path(quote_id),
            state(pool),
            Json(RejectRequest { reason: "".to_string() }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn add_comment_records_audit_event() {
        let (pool, quote_id) = setup().await;

        let result = add_comment(
            axum::extract::Path(quote_id.clone()),
            state(pool.clone()),
            Json(CommentRequest { text: "Can we discuss pricing?".to_string() }),
        )
        .await
        .expect("should succeed");

        assert!(result.0.success);

        // Verify audit event
        let payload: String = sqlx::query_scalar(
            "SELECT payload_json FROM audit_event WHERE quote_id = ? AND event_type = 'portal.comment'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch audit");
        assert!(payload.contains("Can we discuss pricing?"));
    }

    #[tokio::test]
    async fn add_comment_rejects_empty_text() {
        let (pool, quote_id) = setup().await;

        let result = add_comment(
            axum::extract::Path(quote_id),
            state(pool),
            Json(CommentRequest { text: "  ".to_string() }),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn approve_nonexistent_quote_returns_not_found() {
        let (pool, _) = setup().await;

        let result = approve_quote(
            axum::extract::Path("Q-NONEXISTENT".to_string()),
            state(pool),
            Json(ApproveRequest {
                approver_name: "Jane".to_string(),
                approver_email: "jane@test.com".to_string(),
                comments: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.0.error.contains("not found"));
    }
}
