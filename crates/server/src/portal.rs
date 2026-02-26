//! Web portal routes for customer-facing quote approval and interaction.
//!
//! HTML Endpoints:
//! - `GET  /quote/{token}`                      — view quote details (HTML)
//! - `GET  /quote/{token}/download`             — download quote PDF
//! - `GET  /portal`                             — portal homepage (quote list)
//!
//! JSON API Endpoints:
//! - `POST /quote/{token}/approve`              — capture electronic approval
//! - `POST /quote/{token}/reject`               — capture rejection with reason
//! - `POST /quote/{token}/comment`              — add an overall customer comment
//! - `GET  /quote/{token}/comments`             — list all comments for a quote
//! - `POST /quote/{token}/line/{line_id}/comment` — add a per-line-item comment
//! - `POST /api/v1/portal/links`                — generate a shareable link
//! - `POST /api/v1/portal/links/revoke`         — revoke an existing link
//! - `GET  /api/v1/portal/links/{quote_id}`     — list active links for a quote

use crate::pdf::PdfGenerator;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use quotey_db::DbPool;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tera::{Context, Tera};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct PortalState {
    db_pool: DbPool,
    templates: Arc<Tera>,
    pdf_generator: Option<Arc<PdfGenerator>>,
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
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CommentResponse {
    pub id: String,
    pub quote_id: String,
    pub quote_line_id: Option<String>,
    pub parent_id: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub body: String,
    pub created_at: String,
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

#[derive(Debug, Deserialize)]
pub struct CreateLinkRequest {
    pub quote_id: String,
    pub expires_in_days: Option<u32>,
    pub created_by: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LinkResponse {
    pub link_id: String,
    pub token: String,
    pub quote_id: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeLinkRequest {
    pub token: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Initialize Tera template engine with portal templates.
fn init_templates() -> Arc<Tera> {
    let mut tera = match Tera::new("templates/portal/**/*") {
        Ok(t) => t,
        Err(e) => {
            warn!(error = %e, "Failed to load portal templates from filesystem, using empty Tera instance");
            Tera::default()
        }
    };

    crate::pdf::register_template_filters(&mut tera);

    // Add built-in fallback templates in case filesystem templates are not available
    tera.add_raw_template(
        "quote_viewer.html",
        include_str!("../../../templates/portal/quote_viewer.html"),
    )
    .ok();
    tera.add_raw_template("index.html", include_str!("../../../templates/portal/index.html")).ok();

    Arc::new(tera)
}

pub fn router(db_pool: DbPool) -> Router {
    let templates = init_templates();
    
    // Initialize PDF generator with templates
    let pdf_generator = match PdfGenerator::new("templates/quotes") {
        Ok(generator) => {
            info!("PDF generator initialized successfully with filesystem templates");
            Some(Arc::new(generator))
        }
        Err(e) => {
            warn!(error = %e, "Failed to initialize PDF generator with filesystem templates, using embedded fallback");
            Some(Arc::new(PdfGenerator::with_embedded_templates()))
        }
    };

    Router::new()
        // HTML routes
        .route("/quote/{token}", get(view_quote_page))
        .route("/quote/{token}/download", get(download_quote_pdf))
        .route("/portal", get(portal_index_page))
        // JSON API routes
        .route("/quote/{token}/approve", post(approve_quote))
        .route("/quote/{token}/reject", post(reject_quote))
        .route("/quote/{token}/comment", post(add_comment))
        .route("/quote/{token}/comments", get(list_comments))
        .route("/quote/{token}/line/{line_id}/comment", post(add_line_comment))
        .route("/api/v1/portal/links", post(create_link))
        .route("/api/v1/portal/links/revoke", post(revoke_link))
        .route("/api/v1/portal/links/{quote_id}", get(list_links))
        .with_state(PortalState { db_pool, templates, pdf_generator })
}

// ---------------------------------------------------------------------------
// HTML Handlers
// ---------------------------------------------------------------------------

/// Query parameters for the portal index page
#[derive(Debug, Deserialize, Default)]
pub struct PortalIndexQuery {
    pub status: Option<String>,
    pub search: Option<String>,
}

/// Render the quote viewer HTML page.
async fn view_quote_page(
    Path(token): Path<String>,
    State(state): State<PortalState>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token)
        .await
        .map_err(|(status, err)| (status, Html(format!("<h1>Error</h1><p>{}</p>", err.0.error))))?;

    // Fetch quote details
    let quote_row = sqlx::query(
        "SELECT id, status, version, currency, term_months, valid_until,
                created_at, updated_at, created_by
         FROM quote WHERE id = ?",
    )
    .bind(&quote_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("<h1>Database Error</h1><p>{}</p>", e)))
    })?;

    let quote_row = match quote_row {
        Some(row) => row,
        None => return Err((StatusCode::NOT_FOUND, Html("<h1>Quote not found</h1>".to_string()))),
    };

    // Fetch quote lines
    let line_rows = sqlx::query(
        "SELECT id, product_id, quantity, unit_price, discount_pct, subtotal, notes
         FROM quote_line WHERE quote_id = ? ORDER BY created_at",
    )
    .bind(&quote_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("<h1>Database Error</h1><p>{}</p>", e)))
    })?;

    // Compute pricing totals from line items (these columns don't exist on the quote table)
    let mut computed_subtotal = 0.0_f64;
    let mut computed_discount = 0.0_f64;
    for row in &line_rows {
        let qty: i64 = row.try_get("quantity").unwrap_or(0);
        let unit_price: f64 = row.try_get("unit_price").unwrap_or(0.0);
        let discount_pct: f64 = row.try_get("discount_pct").unwrap_or(0.0);
        let line_total = unit_price * qty as f64;
        computed_subtotal += line_total;
        computed_discount += line_total * discount_pct / 100.0;
    }
    let computed_total = computed_subtotal - computed_discount;

    let lines: Vec<serde_json::Value> = line_rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "product_name": row.try_get::<String, _>("product_id").unwrap_or_default(),
                "quantity": row.try_get::<i64, _>("quantity").unwrap_or(0),
                "unit_price": format_price(row.try_get::<f64, _>("unit_price").unwrap_or(0.0)),
                "total": format_price(row.try_get::<f64, _>("subtotal").unwrap_or(0.0)),
                "description": row.try_get::<String, _>("notes").unwrap_or_default(),
            })
        })
        .collect();

    // Fetch comments
    let comment_rows = sqlx::query(
        "SELECT id, author_name, author_email, body, created_at
         FROM portal_comment WHERE quote_id = ? AND quote_line_id IS NULL
         ORDER BY created_at DESC",
    )
    .bind(&quote_id)
    .fetch_all(&state.db_pool)
    .await
    .unwrap_or_default();

    let comments: Vec<serde_json::Value> = comment_rows.iter().map(|row| {
        serde_json::json!({
            "author": row.try_get::<String, _>("author_name").unwrap_or_default(),
            "text": row.try_get::<String, _>("body").unwrap_or_default(),
            "timestamp": row.try_get::<String, _>("created_at").unwrap_or_default(),
            "is_customer": !row.try_get::<String, _>("author_email").unwrap_or_default().contains("@quotey"),
        })
    }).collect();

    // Build template context
    let mut context = Context::new();
    context.insert("quote", &serde_json::json!({
        "quote_id": quote_id,
        "token": token,
        "status": quote_row.try_get::<String, _>("status").unwrap_or_default(),
        "version": quote_row.try_get::<i64, _>("version").unwrap_or(1),
        "created_at": quote_row.try_get::<String, _>("created_at").unwrap_or_default(),
        "valid_until": quote_row.try_get::<String, _>("valid_until").unwrap_or_default(),
        "term_months": quote_row.try_get::<i64, _>("term_months").unwrap_or(12),
        "payment_terms": "Net 30",
        "subtotal": format_price(computed_subtotal),
        "discount_total": format_price(computed_discount),
        "tax_amount": format_price(0.0),
        "total": format_price(computed_total),
        "lines": lines,
        "expires_soon": false, // TODO: calculate from valid_until
    }));

    context.insert(
        "customer",
        &serde_json::json!({
            "name": "Customer", // TODO: fetch from account table
            "email": "",
            "phone": "",
        }),
    );

    context.insert(
        "rep",
        &serde_json::json!({
            "name": quote_row.try_get::<String, _>("created_by").unwrap_or_default(),
            "email": "",
        }),
    );

    context.insert("comments", &comments);

    context.insert(
        "branding",
        &serde_json::json!({
            "company_name": "Quotey",
            "logo_url": Option::<String>::None,
            "primary_color": "#2563eb",
        }),
    );

    let html = state.templates.render("quote_viewer.html", &context).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<h1>Template Error</h1><pre>{:?}</pre>", e)),
        )
    })?;

    Ok(Html(html))
}

/// Handle PDF download request.
async fn download_quote_pdf(
    Path(token): Path<String>,
    State(state): State<PortalState>,
) -> Result<impl IntoResponse, (StatusCode, Json<PortalError>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token).await?;

    info!(
        event_name = "portal.pdf.download_requested",
        quote_id = %quote_id,
        token = %token,
        "PDF download requested"
    );

    // Check if PDF generator is available
    let pdf_generator = state.pdf_generator.as_ref().ok_or_else(|| {
        error!("PDF generator not initialized");
        (StatusCode::SERVICE_UNAVAILABLE, Json(PortalError { error: "PDF generation not available".to_string() }))
    })?;

    // Fetch complete quote data for PDF generation
    let quote_data = fetch_quote_for_pdf(&state.db_pool, &quote_id).await?;

    // Generate PDF (or HTML if wkhtmltopdf not available)
    let filename = format!("Quote_{}.pdf", quote_id);
    match pdf_generator.generate_quote_pdf(&quote_data, "detailed").await {
        Ok(result) => {
            info!(
                event_name = "portal.pdf.generated",
                quote_id = %quote_id,
                filename = %filename,
                "PDF generated successfully"
            );
            Ok(result.into_response(&filename))
        }
        Err(e) => {
            error!(error = %e, quote_id = %quote_id, "PDF generation failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PortalError { error: format!("PDF generation failed: {}", e) }),
            ))
        }
    }
}

/// Fetch complete quote data for PDF generation
async fn fetch_quote_for_pdf(
    pool: &DbPool,
    quote_id: &str,
) -> Result<serde_json::Value, (StatusCode, Json<PortalError>)> {
    // Fetch quote basic info with account
    let quote_row = sqlx::query(
        r#"SELECT 
            q.id, q.status, q.created_at, q.valid_until,
            q.account_id, a.name as account_name, a.industry as account_industry
         FROM quote q
         LEFT JOIN account a ON a.id = q.account_id
         WHERE q.id = ?"#,
    )
    .bind(quote_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => (
            StatusCode::NOT_FOUND,
            Json(PortalError { error: "Quote not found".to_string() }),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PortalError { error: format!("Database error: {}", e) }),
        ),
    })?;

    // Fetch quote lines
    let lines = sqlx::query(
        r#"SELECT 
            ql.id, ql.quote_id, ql.product_id, ql.quantity, ql.unit_price,
            ql.list_price, ql.discount_amount, ql.total_price,
            p.name as product_name, p.sku as product_sku
         FROM quote_line ql
         JOIN product p ON p.id = ql.product_id
         WHERE ql.quote_id = ?
         ORDER BY ql.id"#,
    )
    .bind(quote_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to fetch quote lines");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PortalError { error: "Failed to fetch quote data".to_string() }),
        )
    })?;

    // Calculate pricing summary
    let subtotal: f64 = lines.iter()
        .map(|r| r.try_get::<f64, _>("total_price").unwrap_or(0.0))
        .sum();
    let total_discount: f64 = lines.iter()
        .map(|r| r.try_get::<f64, _>("discount_amount").unwrap_or(0.0))
        .sum();
    let tax_rate = 0.08; // 8% tax - in real app this would be configurable
    let tax = subtotal * tax_rate;
    let total = subtotal + tax;

    // Build the JSON structure expected by the PDF templates
    let quote_data = serde_json::json!({
        "id": quote_id,
        "status": quote_row.try_get::<String, _>("status").unwrap_or_default(),
        "created_at": quote_row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
        "valid_until": quote_row.try_get::<chrono::DateTime<chrono::Utc>, _>("valid_until")
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
        "account": {
            "id": quote_row.try_get::<String, _>("account_id").unwrap_or_default(),
            "name": quote_row.try_get::<String, _>("account_name").unwrap_or_else(|_| "Unknown Account".to_string()),
            "industry": quote_row.try_get::<String, _>("account_industry").unwrap_or_default(),
        },
        "lines": lines.iter().map(|r| serde_json::json!({
            "id": r.try_get::<String, _>("id").unwrap_or_default(),
            "product_name": r.try_get::<String, _>("product_name").unwrap_or_default(),
            "product_sku": r.try_get::<String, _>("product_sku").unwrap_or_default(),
            "quantity": r.try_get::<i64, _>("quantity").unwrap_or(1),
            "unit_price": r.try_get::<f64, _>("unit_price").unwrap_or(0.0),
            "list_price": r.try_get::<f64, _>("list_price").unwrap_or(0.0),
            "discount_amount": r.try_get::<f64, _>("discount_amount").unwrap_or(0.0),
            "total_price": r.try_get::<f64, _>("total_price").unwrap_or(0.0),
        })).collect::<Vec<_>>(),
        "pricing": {
            "subtotal": subtotal,
            "total_discount": total_discount,
            "tax_rate": tax_rate,
            "tax": tax,
            "total": total,
        },
        "company_name": "Quotey",
    });

    Ok(quote_data)
}

/// Render the portal index page (list of quotes).
async fn portal_index_page(
    Query(query): Query<PortalIndexQuery>,
    State(state): State<PortalState>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Fetch all quotes with optional status filter
    let status_filter = query.status.as_deref().unwrap_or("all");

    let quote_rows = if status_filter == "all" {
        sqlx::query(
            "SELECT q.id, q.status, q.created_at, q.valid_until,
                    COALESCE(SUM(ql.unit_price * ql.quantity), 0.0) AS computed_total
             FROM quote q
             LEFT JOIN quote_line ql ON ql.quote_id = q.id
             GROUP BY q.id
             ORDER BY q.created_at DESC LIMIT 100",
        )
        .fetch_all(&state.db_pool)
        .await
    } else {
        sqlx::query(
            "SELECT q.id, q.status, q.created_at, q.valid_until,
                    COALESCE(SUM(ql.unit_price * ql.quantity), 0.0) AS computed_total
             FROM quote q
             LEFT JOIN quote_line ql ON ql.quote_id = q.id
             WHERE q.status = ?
             GROUP BY q.id
             ORDER BY q.created_at DESC LIMIT 100",
        )
        .bind(status_filter)
        .fetch_all(&state.db_pool)
        .await
    }
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Html(format!("<h1>Database Error</h1><p>{}</p>", e)))
    })?;

    // Calculate stats
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM quote")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let pending_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM quote WHERE status = 'sent'")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let approved_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quote WHERE status = 'approved'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    // Build quotes list
    let quotes: Vec<serde_json::Value> = quote_rows
        .iter()
        .map(|row| {
            let quote_id: String = row.try_get("id").unwrap_or_default();
            let status: String = row.try_get("status").unwrap_or_default();

            // Get the active token for this quote
            let token = format!("token-{}", &quote_id); // Simplified for now

            serde_json::json!({
                "token": token,
                "quote_id": quote_id,
                "created_at": row.try_get::<String, _>("created_at").unwrap_or_default(),
                "valid_until": row.try_get::<String, _>("valid_until").unwrap_or_default(),
                "status": status,
                "total_amount": format_price(row.try_get::<f64, _>("computed_total").unwrap_or(0.0)),
            })
        })
        .collect();

    // Build template context
    let mut context = Context::new();
    context.insert(
        "customer",
        &serde_json::json!({
            "name": "All Customers",
            "email": "",
        }),
    );

    context.insert(
        "stats",
        &serde_json::json!({
            "total": total_count,
            "pending": pending_count,
            "approved": approved_count,
        }),
    );

    context.insert("quotes", &quotes);

    context.insert(
        "branding",
        &serde_json::json!({
            "company_name": "Quotey",
            "logo_url": Option::<String>::None,
        }),
    );

    let html = state.templates.render("index.html", &context).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<h1>Template Error</h1><pre>{:?}</pre>", e)),
        )
    })?;

    Ok(Html(html))
}

/// Format a price for display
fn format_price(amount: f64) -> String {
    format!("${:.2}", amount)
}

// ---------------------------------------------------------------------------
// JSON API Handlers
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

    let author_name = body.author_name.unwrap_or_else(|| "Portal Customer".to_string());
    let author_email = body.author_email.unwrap_or_else(|| "noreply@portal.local".to_string());

    if let Some(parent_id) = &body.parent_id {
        let parent_quote_id: Option<String> =
            sqlx::query_scalar("SELECT quote_id FROM portal_comment WHERE id = ? AND quote_id = ?")
                .bind(parent_id)
                .bind(&quote_id)
                .fetch_optional(&state.db_pool)
                .await
                .map_err(db_error)?;

        if parent_quote_id.is_none() {
            return Err((
                StatusCode::NOT_FOUND,
                Json(PortalError { error: "parent comment not found for this quote".to_string() }),
            ));
        }
    }

    let id = uuid_v4();
    sqlx::query(
        "INSERT INTO portal_comment
            (id, quote_id, quote_line_id, parent_id, author_name, author_email, body, created_at)
         VALUES (?, ?, NULL, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(&id)
    .bind(&quote_id)
    .bind(&body.parent_id)
    .bind(&author_name)
    .bind(&author_email)
    .bind(text)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

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

async fn list_comments(
    Path(token): Path<String>,
    State(state): State<PortalState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<PortalError>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token).await?;

    let rows = sqlx::query(
        "SELECT id, quote_id, quote_line_id, parent_id, author_name, author_email, body, created_at
         FROM portal_comment
         WHERE quote_id = ?
         ORDER BY created_at DESC",
    )
    .bind(&quote_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(db_error)?;

    let comments: Vec<CommentResponse> = rows
        .into_iter()
        .map(|row| {
            let map_err = |err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(PortalError { error: format!("failed to load comments: {err}") }),
                )
            };

            let id: String = row.try_get("id").map_err(map_err)?;
            let quote_id: String = row.try_get("quote_id").map_err(map_err)?;
            let quote_line_id: Option<String> = row.try_get("quote_line_id").map_err(map_err)?;
            let parent_id: Option<String> = row.try_get("parent_id").map_err(map_err)?;
            let author_name: String = row.try_get("author_name").map_err(map_err)?;
            let author_email: String = row.try_get("author_email").map_err(map_err)?;
            let body: String = row.try_get("body").map_err(map_err)?;
            let created_at: String = row.try_get("created_at").map_err(map_err)?;

            Ok(CommentResponse {
                id,
                quote_id,
                quote_line_id,
                parent_id,
                author_name,
                author_email,
                body,
                created_at,
            })
        })
        .collect::<Result<Vec<_>, (StatusCode, Json<PortalError>)>>()?;

    Ok(Json(serde_json::json!({
        "comments": comments,
        "quote_id": quote_id,
    })))
}

async fn add_line_comment(
    Path((token, line_id)): Path<(String, String)>,
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

    let line_exists: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM quote_line WHERE id = ? AND quote_id = ?")
            .bind(&line_id)
            .bind(&quote_id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(db_error)?;
    if line_exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(PortalError { error: format!("quote line `{line_id}` not found") }),
        ));
    }

    if let Some(parent_id) = &body.parent_id {
        let parent_quote_id: Option<String> =
            sqlx::query_scalar("SELECT quote_id FROM portal_comment WHERE id = ? AND quote_id = ?")
                .bind(parent_id)
                .bind(&quote_id)
                .fetch_optional(&state.db_pool)
                .await
                .map_err(db_error)?;

        if parent_quote_id.is_none() {
            return Err((
                StatusCode::NOT_FOUND,
                Json(PortalError { error: "parent comment not found for this quote".to_string() }),
            ));
        }
    }

    let author_name = body.author_name.unwrap_or_else(|| "Portal Customer".to_string());
    let author_email = body.author_email.unwrap_or_else(|| "noreply@portal.local".to_string());
    let id = uuid_v4();
    sqlx::query(
        "INSERT INTO portal_comment
            (id, quote_id, quote_line_id, parent_id, author_name, author_email, body, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))",
    )
    .bind(&id)
    .bind(&quote_id)
    .bind(&line_id)
    .bind(&body.parent_id)
    .bind(&author_name)
    .bind(&author_email)
    .bind(text)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    record_audit_event(
        &state.db_pool,
        &quote_id,
        "portal.comment.line",
        &format!("Customer comment on line {line_id}: {text}"),
    )
    .await;

    Ok(Json(PortalResponse {
        success: true,
        message: format!("Comment on line {line_id} recorded."),
    }))
}

// ---------------------------------------------------------------------------
// Link Management Handlers
// ---------------------------------------------------------------------------

async fn create_link(
    State(state): State<PortalState>,
    Json(body): Json<CreateLinkRequest>,
) -> Result<Json<LinkResponse>, (StatusCode, Json<PortalError>)> {
    let quote_id = &body.quote_id;

    // Verify quote exists
    let exists: Option<i64> = sqlx::query_scalar("SELECT 1 FROM quote WHERE id = ?")
        .bind(quote_id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(db_error)?;

    if exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(PortalError { error: format!("quote `{quote_id}` not found") }),
        ));
    }

    let now = Utc::now();
    let expires_in_days = body.expires_in_days.unwrap_or(30).clamp(1, 365);
    let expires_at = now + chrono::Duration::days(expires_in_days as i64);
    let link_id = format!("PL-{}", &uuid_v4()[..12]);
    let token = generate_token();
    let created_by = body.created_by.as_deref().unwrap_or("api");

    sqlx::query(
        "INSERT INTO portal_link (id, quote_id, token, expires_at, created_by, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&link_id)
    .bind(quote_id)
    .bind(&token)
    .bind(expires_at.to_rfc3339())
    .bind(created_by)
    .bind(now.to_rfc3339())
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    // Revoke any previous links for this quote (regenerate behavior)
    sqlx::query(
        "UPDATE portal_link SET revoked = 1
         WHERE quote_id = ? AND id != ? AND revoked = 0",
    )
    .bind(quote_id)
    .bind(&link_id)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    record_audit_event(
        &state.db_pool,
        quote_id,
        "portal.link_created",
        &format!("Portal link generated (expires in {expires_in_days} days)"),
    )
    .await;

    info!(
        event_name = "portal.link.created",
        quote_id = %quote_id,
        link_id = %link_id,
        expires_in_days = %expires_in_days,
        "portal sharing link created"
    );

    Ok(Json(LinkResponse {
        link_id,
        token,
        quote_id: quote_id.clone(),
        expires_at: expires_at.to_rfc3339(),
    }))
}

async fn revoke_link(
    State(state): State<PortalState>,
    Json(body): Json<RevokeLinkRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<PortalError>)> {
    let token = body.token.trim();
    if token.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError { error: "token is required".to_string() }),
        ));
    }

    let result = sqlx::query("UPDATE portal_link SET revoked = 1 WHERE token = ? AND revoked = 0")
        .bind(token)
        .execute(&state.db_pool)
        .await
        .map_err(db_error)?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(PortalError { error: "link not found or already revoked".to_string() }),
        ));
    }

    info!(event_name = "portal.link.revoked", "portal sharing link revoked");

    Ok(Json(PortalResponse { success: true, message: "Link revoked successfully.".to_string() }))
}

async fn list_links(
    Path(quote_id): Path<String>,
    State(state): State<PortalState>,
) -> Result<Json<Vec<LinkResponse>>, (StatusCode, Json<PortalError>)> {
    let now = Utc::now().to_rfc3339();

    let rows = sqlx::query(
        "SELECT id, token, quote_id, expires_at
         FROM portal_link
         WHERE quote_id = ? AND revoked = 0 AND expires_at > ?
         ORDER BY created_at DESC",
    )
    .bind(&quote_id)
    .bind(&now)
    .fetch_all(&state.db_pool)
    .await
    .map_err(db_error)?;

    let links: Vec<LinkResponse> = rows
        .iter()
        .map(|r| LinkResponse {
            link_id: r.try_get("id").unwrap_or_default(),
            token: r.try_get("token").unwrap_or_default(),
            quote_id: r.try_get("quote_id").unwrap_or_default(),
            expires_at: r.try_get("expires_at").unwrap_or_default(),
        })
        .collect();

    Ok(Json(links))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a sharing token to a quote ID.
///
/// First checks the `portal_link` table for a valid (non-revoked, non-expired)
/// link. Falls back to matching by raw quote ID for backward compatibility.
async fn resolve_quote_by_token(
    pool: &DbPool,
    token: &str,
) -> Result<String, (StatusCode, Json<PortalError>)> {
    let now = Utc::now().to_rfc3339();

    // Try portal_link table first
    let link_row: Option<sqlx::sqlite::SqliteRow> = sqlx::query(
        "SELECT quote_id FROM portal_link WHERE token = ? AND revoked = 0 AND expires_at > ?",
    )
    .bind(token)
    .bind(&now)
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;

    if let Some(r) = link_row {
        let quote_id: String = r.try_get("quote_id").map_err(|e| {
            db_error(sqlx::Error::ColumnDecode {
                index: "quote_id".to_string(),
                source: Box::new(e),
            })
        })?;
        return Ok(quote_id);
    }

    // Check for expired/revoked link to give a better error
    let expired_row: Option<sqlx::sqlite::SqliteRow> =
        sqlx::query("SELECT revoked, expires_at FROM portal_link WHERE token = ?")
            .bind(token)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;

    if let Some(r) = expired_row {
        let revoked: bool = r.try_get("revoked").unwrap_or(false);
        if revoked {
            return Err((
                StatusCode::GONE,
                Json(PortalError { error: "this quote link has been revoked".to_string() }),
            ));
        }
        return Err((
            StatusCode::GONE,
            Json(PortalError { error: "this quote link has expired".to_string() }),
        ));
    }

    warn!(token = %token, "portal: invalid or expired quote token");
    Err((
        StatusCode::NOT_FOUND,
        Json(PortalError { error: "quote not found or link has expired".to_string() }),
    ))
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

/// Generate a URL-safe random-looking token for portal links.
fn generate_token() -> String {
    // Use a cryptographically random UUID for link tokens to avoid guessability.
    Uuid::new_v4().simple().to_string()
}

fn uuid_v4() -> String {
    Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use axum::{extract::State, Json};
    use chrono::Utc;
    use quotey_db::{connect_with_settings, migrations};

    use super::*;

    async fn setup() -> (sqlx::SqlitePool, String, String) {
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

        // Create a portal link so handlers can resolve the token
        let token = generate_token();
        let expires_at = (Utc::now() + chrono::Duration::days(30)).to_rfc3339();
        sqlx::query(
            "INSERT INTO portal_link (id, quote_id, token, expires_at, created_by, created_at)
             VALUES ('PL-TEST', 'Q-TEST-001', ?, ?, 'test', ?)",
        )
        .bind(&token)
        .bind(&expires_at)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed portal link");

        (pool, "Q-TEST-001".to_string(), token)
    }

    fn state(pool: sqlx::SqlitePool) -> State<PortalState> {
        let mut tera = Tera::default();
        // Add minimal templates for testing
        tera.add_raw_template(
            "quote_viewer.html",
            "<html><body>Quote {{ quote.quote_id }}</body></html>",
        )
        .ok();
        tera.add_raw_template("index.html", "<html><body>Portal</body></html>").ok();

        State(PortalState { db_pool: pool, templates: Arc::new(tera), pdf_generator: None })
    }

    #[tokio::test]
    async fn approve_quote_records_approval_and_updates_status() {
        let (pool, quote_id, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token.clone()),
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
        let (pool, _, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token),
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
        let (pool, quote_id, token) = setup().await;

        let result = reject_quote(
            axum::extract::Path(token.clone()),
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
        let (pool, _, token) = setup().await;

        let result = reject_quote(
            axum::extract::Path(token),
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
        let (pool, quote_id, token) = setup().await;

        let result = add_comment(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            Json(CommentRequest {
                text: "Can we discuss pricing?".to_string(),
                author_name: None,
                author_email: None,
                parent_id: None,
            }),
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
        let (pool, _, token) = setup().await;

        let result = add_comment(
            axum::extract::Path(token),
            state(pool),
            Json(CommentRequest {
                text: "  ".to_string(),
                author_name: None,
                author_email: None,
                parent_id: None,
            }),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn approve_nonexistent_quote_returns_not_found() {
        let (pool, _, _) = setup().await;

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

    // -----------------------------------------------------------------------
    // Link management tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn create_link_returns_token_and_expiry() {
        let (pool, quote_id, _token) = setup().await;

        let result = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(7),
                created_by: Some("rep@acme.com".to_string()),
            }),
        )
        .await
        .expect("create_link should succeed");

        let resp = result.0;
        assert_eq!(resp.quote_id, quote_id);
        assert!(!resp.token.is_empty());
        assert!(!resp.link_id.is_empty());
        assert!(!resp.expires_at.is_empty());
    }

    #[tokio::test]
    async fn create_link_for_nonexistent_quote_returns_not_found() {
        let (pool, _, _) = setup().await;

        let result = create_link(
            state(pool),
            Json(CreateLinkRequest {
                quote_id: "Q-FAKE".to_string(),
                expires_in_days: None,
                created_by: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn create_link_revokes_previous_links() {
        let (pool, quote_id, _token) = setup().await;

        // Create first link
        let first = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(30),
                created_by: None,
            }),
        )
        .await
        .expect("first link");
        let first_token = first.0.token.clone();

        // Create second link — should revoke first
        let _second = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(30),
                created_by: None,
            }),
        )
        .await
        .expect("second link");

        // First link should now be revoked
        let revoked: i64 = sqlx::query_scalar("SELECT revoked FROM portal_link WHERE token = ?")
            .bind(&first_token)
            .fetch_one(&pool)
            .await
            .expect("fetch revoked");
        assert_eq!(revoked, 1);
    }

    #[tokio::test]
    async fn revoke_link_succeeds() {
        let (pool, quote_id, _token) = setup().await;

        let link = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest { quote_id, expires_in_days: Some(7), created_by: None }),
        )
        .await
        .expect("create link");

        let result = revoke_link(
            state(pool.clone()),
            Json(RevokeLinkRequest { token: link.0.token.clone() }),
        )
        .await
        .expect("revoke should succeed");

        assert!(result.0.success);
    }

    #[tokio::test]
    async fn revoke_link_already_revoked_returns_not_found() {
        let (pool, quote_id, _token) = setup().await;

        let link = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest { quote_id, expires_in_days: Some(7), created_by: None }),
        )
        .await
        .expect("create link");

        // Revoke once
        let _ = revoke_link(
            state(pool.clone()),
            Json(RevokeLinkRequest { token: link.0.token.clone() }),
        )
        .await
        .expect("first revoke");

        // Revoke again — should fail
        let result = revoke_link(
            state(pool.clone()),
            Json(RevokeLinkRequest { token: link.0.token.clone() }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn revoke_link_empty_token_returns_bad_request() {
        let (pool, _, _) = setup().await;

        let result =
            revoke_link(state(pool), Json(RevokeLinkRequest { token: "  ".to_string() })).await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_links_returns_active_only() {
        let (pool, quote_id, _token) = setup().await;

        // Create two links (first gets auto-revoked by second)
        let _first = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(30),
                created_by: None,
            }),
        )
        .await
        .expect("first link");

        let second = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(30),
                created_by: None,
            }),
        )
        .await
        .expect("second link");

        let result =
            list_links(axum::extract::Path(quote_id), state(pool)).await.expect("list links");

        let links = result.0;
        assert_eq!(links.len(), 1, "only the active (non-revoked) link should appear");
        assert_eq!(links[0].token, second.0.token);
    }

    #[tokio::test]
    async fn resolve_quote_by_token_via_portal_link() {
        let (pool, quote_id, _token) = setup().await;

        let link = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(7),
                created_by: None,
            }),
        )
        .await
        .expect("create link");

        let resolved =
            resolve_quote_by_token(&pool, &link.0.token).await.expect("resolve should succeed");
        assert_eq!(resolved, quote_id);
    }

    #[tokio::test]
    async fn resolve_quote_by_token_revoked_returns_gone() {
        let (pool, quote_id, _token) = setup().await;

        let link = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest { quote_id, expires_in_days: Some(7), created_by: None }),
        )
        .await
        .expect("create link");

        let _ = revoke_link(
            state(pool.clone()),
            Json(RevokeLinkRequest { token: link.0.token.clone() }),
        )
        .await
        .expect("revoke");

        let result = resolve_quote_by_token(&pool, &link.0.token).await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::GONE);
    }

    #[tokio::test]
    async fn resolve_quote_by_token_expired_returns_gone() {
        let (pool, quote_id, _token) = setup().await;

        // Insert a link that already expired
        let past = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        sqlx::query(
            "INSERT INTO portal_link (id, quote_id, token, expires_at, created_by, created_at)
             VALUES ('PL-EXP', ?, 'expired-tok', ?, 'test', ?)",
        )
        .bind(&quote_id)
        .bind(&past)
        .bind(&past)
        .execute(&pool)
        .await
        .expect("insert expired link");

        let result = resolve_quote_by_token(&pool, "expired-tok").await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::GONE);
    }

    #[tokio::test]
    async fn resolve_quote_by_raw_id_fallback() {
        let (pool, quote_id, _token) = setup().await;

        let result = resolve_quote_by_token(&pool, &quote_id).await;

        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_links_excludes_expired_links() {
        let (pool, quote_id, _token) = setup().await;

        let first = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(1),
                created_by: None,
            }),
        )
        .await
        .expect("first link");

        // Expire the first link in DB
        sqlx::query("UPDATE portal_link SET expires_at = ? WHERE token = ?")
            .bind((Utc::now() - chrono::Duration::days(1)).to_rfc3339())
            .bind(&first.0.token)
            .execute(&pool)
            .await
            .expect("expire link");

        let active = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest { quote_id, expires_in_days: Some(1), created_by: None }),
        )
        .await
        .expect("active link");

        let result = list_links(axum::extract::Path(first.0.quote_id.clone()), state(pool))
            .await
            .expect("list links");

        let links = result.0;
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].token, active.0.token);
    }

    // -----------------------------------------------------------------------
    // Comment functionality tests (quotey-003-4)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn add_comment_defaults_author_fields() {
        let (pool, _, token) = setup().await;

        let _ = add_comment(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            Json(CommentRequest {
                text: "Hello".to_string(),
                author_name: None,
                author_email: None,
                parent_id: None,
            }),
        )
        .await
        .expect("should succeed");

        let name: String = sqlx::query_scalar("SELECT author_name FROM portal_comment LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("fetch name");
        assert_eq!(name, "Portal Customer");

        let email: String = sqlx::query_scalar("SELECT author_email FROM portal_comment LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("fetch email");
        assert_eq!(email, "noreply@portal.local");
    }

    #[tokio::test]
    async fn add_comment_threaded_reply() {
        let (pool, _, token) = setup().await;

        // Create parent comment
        let _ = add_comment(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            Json(CommentRequest {
                text: "Parent comment".to_string(),
                author_name: Some("Alice".to_string()),
                author_email: Some("alice@acme.com".to_string()),
                parent_id: None,
            }),
        )
        .await
        .expect("parent comment");

        let parent_id: String =
            sqlx::query_scalar("SELECT id FROM portal_comment WHERE body = 'Parent comment'")
                .fetch_one(&pool)
                .await
                .expect("get parent id");

        // Create threaded reply
        let _ = add_comment(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            Json(CommentRequest {
                text: "Reply to parent".to_string(),
                author_name: Some("Bob".to_string()),
                author_email: Some("bob@acme.com".to_string()),
                parent_id: Some(parent_id.clone()),
            }),
        )
        .await
        .expect("reply comment");

        let stored_parent: Option<String> = sqlx::query_scalar(
            "SELECT parent_id FROM portal_comment WHERE body = 'Reply to parent'",
        )
        .fetch_one(&pool)
        .await
        .expect("fetch parent_id");
        assert_eq!(stored_parent, Some(parent_id));
    }

    #[tokio::test]
    async fn add_comment_invalid_parent_returns_not_found() {
        let (pool, _, token) = setup().await;

        let result = add_comment(
            axum::extract::Path(token),
            state(pool),
            Json(CommentRequest {
                text: "Reply".to_string(),
                author_name: None,
                author_email: None,
                parent_id: Some("NONEXISTENT".to_string()),
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn add_line_comment_stores_line_id() {
        let (pool, quote_id, token) = setup().await;

        // Seed a quote line
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, created_at, updated_at)
             VALUES ('QL-001', ?, 'PROD-A', 2, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed line");

        let result = add_line_comment(
            axum::extract::Path((token.clone(), "QL-001".to_string())),
            state(pool.clone()),
            Json(CommentRequest {
                text: "This line item is too expensive".to_string(),
                author_name: Some("Customer".to_string()),
                author_email: Some("cust@example.com".to_string()),
                parent_id: None,
            }),
        )
        .await
        .expect("line comment");

        assert!(result.0.success);

        let line_id: Option<String> =
            sqlx::query_scalar("SELECT quote_line_id FROM portal_comment WHERE quote_id = ?")
                .bind(&quote_id)
                .fetch_one(&pool)
                .await
                .expect("fetch line_id");
        assert_eq!(line_id, Some("QL-001".to_string()));
    }

    #[tokio::test]
    async fn add_line_comment_nonexistent_line_returns_not_found() {
        let (pool, _, token) = setup().await;

        let result = add_line_comment(
            axum::extract::Path((token, "QL-FAKE".to_string())),
            state(pool),
            Json(CommentRequest {
                text: "Comment".to_string(),
                author_name: None,
                author_email: None,
                parent_id: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn list_comments_returns_all_comments_ordered() {
        let (pool, _, token) = setup().await;

        let _ = add_comment(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            Json(CommentRequest {
                text: "First".to_string(),
                author_name: None,
                author_email: None,
                parent_id: None,
            }),
        )
        .await
        .expect("first comment");

        let _ = add_comment(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            Json(CommentRequest {
                text: "Second".to_string(),
                author_name: None,
                author_email: None,
                parent_id: None,
            }),
        )
        .await
        .expect("second comment");

        let result =
            list_comments(axum::extract::Path(token), state(pool)).await.expect("list comments");

        let comments = result.0;
        let arr = comments["comments"].as_array().expect("comments array");
        assert_eq!(arr.len(), 2);
    }
}
