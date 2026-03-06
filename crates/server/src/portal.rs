//! Web portal routes for customer-facing quote approval and interaction.
//!
//! HTML Endpoints:
//! - `GET  /quote/{token}`                      — view quote details (HTML)
//! - `GET  /quote/{token}/download`             — download quote PDF
//! - `GET  /portal`                             — portal homepage (quote list)
//! - `GET  /approvals`                          — mobile-first approvals list (PWA entry)
//! - `GET  /approvals/{id}`                     — approval detail route (manager decision view)
//! - `GET  /settings`                           — PWA settings (notifications/cache)
//! - `GET  /manifest.webmanifest`               — PWA manifest alias
//! - `GET  /sw.js`                              — service worker alias
//! - `GET  /portal/manifest.webmanifest`        — PWA manifest
//! - `GET  /portal/sw.js`                       — service worker script
//!
//! JSON API Endpoints:
//! - `POST /quote/{token}/approve`              — capture electronic approval
//! - `POST /quote/{token}/reject`               — capture rejection with reason
//! - `POST /quote/{token}/comment`              — add an overall customer comment
//! - `GET  /quote/{token}/comments`             — list all comments for a quote
//! - `POST /quote/{token}/line/{line_id}/comment` — add a per-line-item comment
//! - `POST /quote/{token}/assumptions`          — update quote assumptions and recalculate
//! - `POST /api/v1/portal/links`                — generate a shareable link
//! - `POST /api/v1/portal/links/regenerate`     — regenerate link (revokes older active links)
//! - `POST /api/v1/portal/links/revoke`         — revoke an existing link
//! - `GET  /api/v1/portal/links/{quote_id}`     — list active links for a quote
//! - `POST /api/v1/portal/push/subscribe`       — register browser push subscription
//! - `POST /api/v1/portal/push/unsubscribe`     — revoke browser push subscription

use crate::pdf::PdfGenerator;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse},
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, Duration, Timelike, Utc};
use quotey_core::{AuthChannel, AuthContext, AuthMethod, AuthPrincipal, AuthStrength};
use quotey_db::DbPool;
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Row};
use std::sync::Arc;
use tera::{Context, Tera};
use tracing::{error, info, warn};
use uuid::Uuid;

/// Escape HTML special characters to prevent XSS in error pages.
fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Return a generic HTML error page for database errors.
/// The detailed error is logged server-side but NOT exposed to the user.
fn redacted_db_error(e: sqlx::Error) -> (StatusCode, Html<String>) {
    warn!(error = %e, "portal database error (redacted from user response)");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(
            "<h1>Service Unavailable</h1><p>A database error occurred. Please try again later.</p>"
                .to_string(),
        ),
    )
}

/// Return a generic HTML error page for template rendering errors.
/// The detailed error is logged server-side but NOT exposed to the user.
fn redacted_template_error(e: tera::Error) -> (StatusCode, Html<String>) {
    warn!(error = ?e, "portal template render error (redacted from user response)");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html("<h1>Service Unavailable</h1><p>A rendering error occurred. Please try again later.</p>".to_string()),
    )
}

/// White-label branding configuration for the portal.
/// Loaded from environment variables or defaults to Quotey branding.
#[derive(Debug, Clone, Serialize)]
pub struct BrandingConfig {
    pub company_name: String,
    pub logo_url: Option<String>,
    pub primary_color: String,
    pub support_email: Option<String>,
    pub terms_footer: Option<String>,
    /// When true, "Powered by Quotey" footer text is hidden.
    pub white_label: bool,
}

impl Default for BrandingConfig {
    fn default() -> Self {
        Self {
            company_name: "Quotey".to_string(),
            logo_url: None,
            primary_color: "#2563eb".to_string(),
            support_email: None,
            terms_footer: None,
            white_label: false,
        }
    }
}

impl BrandingConfig {
    /// Load branding from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            company_name: std::env::var("QUOTEY_BRAND_NAME")
                .unwrap_or_else(|_| "Quotey".to_string()),
            logo_url: std::env::var("QUOTEY_BRAND_LOGO_URL").ok(),
            primary_color: std::env::var("QUOTEY_BRAND_COLOR")
                .unwrap_or_else(|_| "#2563eb".to_string()),
            support_email: std::env::var("QUOTEY_BRAND_SUPPORT_EMAIL").ok(),
            terms_footer: std::env::var("QUOTEY_BRAND_TERMS_FOOTER").ok(),
            white_label: std::env::var("QUOTEY_WHITE_LABEL")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
        }
    }
}

#[derive(Clone)]
pub struct PortalState {
    db_pool: DbPool,
    templates: Arc<Tera>,
    pdf_generator: Option<Arc<PdfGenerator>>,
    branding: BrandingConfig,
    rep_notifications: PortalRepNotificationConfig,
}

#[derive(Debug, Clone, Default)]
struct PortalRepNotificationConfig {
    slack_bot_token: Option<String>,
    fallback_channel: Option<String>,
}

impl PortalRepNotificationConfig {
    fn from_env() -> Self {
        let slack_bot_token = std::env::var("QUOTEY_SLACK_BOT_TOKEN")
            .ok()
            .or_else(|| std::env::var("SLACK_BOT_TOKEN").ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let fallback_channel = std::env::var("QUOTEY_PORTAL_REP_NOTIFICATION_CHANNEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        Self { slack_bot_token, fallback_channel }
    }
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
    #[serde(rename = "authMethod", default)]
    pub auth_method: Option<String>,
    #[serde(rename = "biometricAssertion", default)]
    pub biometric_assertion: Option<String>,
    #[serde(rename = "fallbackPassword", default)]
    pub fallback_password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectRequest {
    pub reason: String,
    #[serde(rename = "authMethod", default)]
    pub auth_method: Option<String>,
    #[serde(rename = "biometricAssertion", default)]
    pub biometric_assertion: Option<String>,
    #[serde(rename = "fallbackPassword", default)]
    pub fallback_password: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalActionAuth {
    None,
    Biometric,
    Password,
}

impl ApprovalActionAuth {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Biometric => "biometric",
            Self::Password => "password",
        }
    }

    fn auth_method(self) -> AuthMethod {
        match self {
            Self::None => AuthMethod::None,
            Self::Biometric => AuthMethod::WebAuthn,
            Self::Password => AuthMethod::Password,
        }
    }

    fn auth_strength(self) -> AuthStrength {
        match self {
            Self::None => AuthStrength::Anonymous,
            Self::Biometric => AuthStrength::PossessionAndBiometric,
            Self::Password => AuthStrength::PossessionAndKnowledge,
        }
    }
}

fn portal_actor_id_for_email(email: &str) -> String {
    let normalized = email.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        "portal:customer".to_string()
    } else {
        format!("portal:{normalized}")
    }
}

fn portal_approval_auth_context(
    auth: ApprovalActionAuth,
    approver_name: &str,
    approver_email: &str,
) -> AuthContext {
    AuthContext {
        channel: AuthChannel::Portal,
        method: auth.auth_method(),
        strength: auth.auth_strength(),
        principal: AuthPrincipal {
            actor_id: portal_actor_id_for_email(approver_email),
            display_name: Some(approver_name.trim().to_string()).filter(|value| !value.is_empty()),
        },
        token_fingerprint: None,
        session_id: None,
    }
}

fn portal_rejection_auth_context(auth: ApprovalActionAuth) -> AuthContext {
    AuthContext {
        channel: AuthChannel::Portal,
        method: auth.auth_method(),
        strength: auth.auth_strength(),
        principal: AuthPrincipal {
            actor_id: "portal:customer".to_string(),
            display_name: Some("Portal Customer".to_string()),
        },
        token_fingerprint: None,
        session_id: None,
    }
}

fn validate_portal_approval_auth(
    method_raw: &str,
    biometric_assertion: Option<&str>,
    fallback_password: Option<&str>,
) -> Result<ApprovalActionAuth, (StatusCode, Json<PortalError>)> {
    let method = method_raw.trim().to_ascii_lowercase();
    match method.as_str() {
        "biometric" => {
            let assertion = biometric_assertion.unwrap_or("").trim();
            if assertion.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(PortalError::validation(
                        "biometricAssertion",
                        "is required when authMethod=biometric",
                    )),
                ));
            }
            Ok(ApprovalActionAuth::Biometric)
        }
        "password" => {
            let provided = fallback_password.unwrap_or("").trim();
            if provided.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(PortalError::validation(
                        "fallbackPassword",
                        "is required when authMethod=password",
                    )),
                ));
            }
            let expected = std::env::var("QUOTEY_PORTAL_APPROVAL_FALLBACK_PASSWORD")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            if let Some(expected_password) = expected {
                if provided != expected_password {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(PortalError {
                            error: "Invalid fallback password".to_string(),
                            category: Some(PortalErrorCategory::PermissionDenied),
                            recovery_hint: Some(
                                "Re-enter the fallback password or retry biometric authentication."
                                    .to_string(),
                            ),
                            retry_after_seconds: None,
                        }),
                    ));
                }
            }
            Ok(ApprovalActionAuth::Password)
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation("authMethod", "must be one of: biometric, password")),
        )),
    }
}

#[derive(Debug, Deserialize)]
pub struct CommentRequest {
    pub text: String,
    pub author_name: Option<String>,
    pub author_email: Option<String>,
    pub parent_id: Option<String>,
}

/// Request to update quote assumptions (tax, payment terms, billing country)
#[derive(Debug, Deserialize)]
pub struct UpdateAssumptionsRequest {
    /// Tax rate as decimal (e.g., 0.08 for 8%)
    pub tax_rate: Option<f64>,
    /// Payment terms: 'net_30', 'net_60', 'net_90', 'upfront'
    pub payment_terms: Option<String>,
    /// Billing country code (ISO 3166-1 alpha-2)
    pub billing_country: Option<String>,
    /// Currency code (ISO 4217)
    pub currency: Option<String>,
}

/// Response for assumption updates including recalculated totals
#[derive(Debug, Serialize)]
pub struct AssumptionsUpdateResponse {
    pub success: bool,
    pub message: String,
    pub quote_id: String,
    /// Updated assumptions
    pub assumptions: serde_json::Value,
    /// Updated totals after recalculation
    pub totals: serde_json::Value,
    /// Whether the quote has any remaining assumptions
    pub has_assumptions: bool,
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

/// Error category for user-facing portal errors
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PortalErrorCategory {
    NotFound,
    ValidationError,
    PermissionDenied,
    RateLimited,
    ServiceUnavailable,
    InternalError,
}

#[derive(Debug, Serialize)]
pub struct PortalError {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<PortalErrorCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u32>,
}

impl PortalError {
    fn not_found(resource: &str) -> Self {
        Self {
            error: format!("{resource} not found"),
            category: Some(PortalErrorCategory::NotFound),
            recovery_hint: Some("Check the link or contact the sender for a new link.".to_string()),
            retry_after_seconds: None,
        }
    }

    fn validation(field: &str, reason: &str) -> Self {
        Self {
            error: format!("Invalid {field}: {reason}"),
            category: Some(PortalErrorCategory::ValidationError),
            recovery_hint: Some(format!("Correct the {field} and try again.")),
            retry_after_seconds: None,
        }
    }

    fn expired() -> Self {
        Self {
            error: "This link has expired".to_string(),
            category: Some(PortalErrorCategory::NotFound),
            recovery_hint: Some("Contact the sender for a new link.".to_string()),
            retry_after_seconds: None,
        }
    }

    #[allow(dead_code)]
    fn rate_limited(retry_after: u32) -> Self {
        Self {
            error: "Too many requests".to_string(),
            category: Some(PortalErrorCategory::RateLimited),
            recovery_hint: Some("Please wait before trying again.".to_string()),
            retry_after_seconds: Some(retry_after),
        }
    }

    fn service_unavailable(service: &str) -> Self {
        Self {
            error: format!("{service} temporarily unavailable"),
            category: Some(PortalErrorCategory::ServiceUnavailable),
            recovery_hint: Some("Wait a moment and try again.".to_string()),
            retry_after_seconds: Some(5),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct CreateLinkRequest {
    pub quote_id: String,
    pub expires_in_days: Option<u32>,
    pub created_by: Option<String>,
    /// Optional navigation origin (e.g., "slack") carried into portal URL query params.
    pub from: Option<String>,
    /// Optional suggested action from handoff source (e.g., "review", "approve").
    pub action: Option<String>,
    /// Optional compact handoff summary shown in the portal banner.
    pub context_summary: Option<String>,
    /// Optional assumptions handoff summary shown in the portal banner.
    pub assumptions_summary: Option<String>,
    /// Optional primary action for continuity and auto-scroll behavior.
    pub next_action: Option<String>,
}

impl CreateLinkRequest {
    fn normalized_handoff(&self) -> PortalHandoffQuery {
        PortalHandoffQuery {
            from: normalize_handoff_from(self.from.as_deref()),
            action: normalize_handoff_action(self.action.as_deref()),
            context_summary: sanitize_handoff_text(self.context_summary.as_deref(), 280),
            assumptions_summary: sanitize_handoff_text(self.assumptions_summary.as_deref(), 280),
            next_action: normalize_handoff_action(self.next_action.as_deref()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LinkResponse {
    pub link_id: String,
    pub token: String,
    pub quote_id: String,
    pub expires_at: String,
    /// Relative portal URL (`/quote/{token}` + preserved handoff query params when present).
    pub share_url: String,
}

#[derive(Debug, Deserialize)]
pub struct RevokeLinkRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct PushSubscriptionRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
    pub user_agent: Option<String>,
    pub device_label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PushUnsubscribeRequest {
    pub endpoint: String,
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
    tera.add_raw_template(
        "approvals.html",
        include_str!("../../../templates/portal/approvals.html"),
    )
    .ok();
    tera.add_raw_template("settings.html", include_str!("../../../templates/portal/settings.html"))
        .ok();
    tera.add_raw_template(
        "approval_detail.html",
        include_str!("../../../templates/portal/approval_detail.html"),
    )
    .ok();

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
        .route("/approvals", get(approvals_index_page))
        .route("/approvals/{id}", get(approval_detail_page))
        .route("/settings", get(approvals_settings_page))
        .route("/manifest.webmanifest", get(portal_manifest))
        .route("/sw.js", get(portal_service_worker))
        .route("/portal/manifest.webmanifest", get(portal_manifest))
        .route("/portal/sw.js", get(portal_service_worker))
        // JSON API routes
        .route("/quote/{token}/approve", post(approve_quote))
        .route("/quote/{token}/reject", post(reject_quote))
        .route("/quote/{token}/comment", post(add_comment))
        .route("/quote/{token}/comments", get(list_comments))
        .route("/quote/{token}/line/{line_id}/comment", post(add_line_comment))
        .route("/quote/{token}/assumptions", post(update_assumptions))
        .route("/api/v1/portal/links", post(create_link))
        .route("/api/v1/portal/links/regenerate", post(regenerate_link))
        .route("/api/v1/portal/links/revoke", post(revoke_link))
        .route("/api/v1/portal/links/{quote_id}", get(list_links))
        .route("/api/v1/portal/push/subscribe", post(subscribe_push))
        .route("/api/v1/portal/push/unsubscribe", post(unsubscribe_push))
        .route("/api/v1/portal/export/quotes", get(export_quotes_csv))
        .route("/api/analytics/export", get(export_quotes_csv))
        .route("/api/v1/portal/analytics/digest-schedule", get(get_digest_schedule))
        .route("/api/v1/portal/analytics/digest-schedule", post(upsert_digest_schedule))
        .route("/api/v1/portal/analytics/digest-dispatch/run", post(run_digest_dispatch))
        .with_state(PortalState {
            db_pool,
            templates,
            pdf_generator,
            branding: BrandingConfig::from_env(),
            rep_notifications: PortalRepNotificationConfig::from_env(),
        })
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

const INDEX_PENDING_STATUSES: [&str; 3] = ["draft", "pending", "sent"];

fn canonical_quote_status(raw_status: &str) -> &'static str {
    let lower = raw_status.to_ascii_lowercase();
    match lower.as_str() {
        "draft" | "pending" => "pending",
        "rejected" | "declined" => "declined",
        "expired" => "expired",
        "approved" => "approved",
        "sent" => "sent",
        _ => "pending",
    }
}

fn normalize_status_filter(raw_status: Option<&str>) -> Option<Vec<String>> {
    let normalized = raw_status.map(|value| value.trim().to_ascii_lowercase());
    match normalized.as_deref() {
        None | Some("") | Some("all") => None,
        Some("pending") => Some(INDEX_PENDING_STATUSES.iter().map(ToString::to_string).collect()),
        Some("declined") => Some(vec!["rejected".to_string(), "declined".to_string()]),
        Some("sent") => Some(vec!["sent".to_string()]),
        Some("approved") => Some(vec!["approved".to_string()]),
        Some("expired") => Some(vec!["expired".to_string()]),
        Some(other) => Some(vec![other.to_string()]),
    }
}

fn selected_status(raw_status: Option<&str>) -> String {
    let normalized = raw_status.map(|value| value.trim().to_ascii_lowercase());
    match normalized.as_deref() {
        Some(value) if !value.is_empty() => value.to_string(),
        _ => "all".to_string(),
    }
}

fn selected_search(raw_search: Option<&str>) -> String {
    raw_search.map(|value| value.trim().to_string()).filter(|v| !v.is_empty()).unwrap_or_default()
}

#[derive(Debug, Clone, Default)]
struct PortalHandoffQuery {
    from: Option<String>,
    action: Option<String>,
    context_summary: Option<String>,
    assumptions_summary: Option<String>,
    next_action: Option<String>,
}

impl PortalHandoffQuery {
    fn is_empty(&self) -> bool {
        self.from.is_none()
            && self.action.is_none()
            && self.context_summary.is_none()
            && self.assumptions_summary.is_none()
            && self.next_action.is_none()
    }

    fn primary_action(&self) -> Option<&str> {
        self.next_action.as_deref().or(self.action.as_deref())
    }
}

fn sanitize_handoff_text(value: Option<&str>, max_chars: usize) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(max_chars).collect())
}

fn normalize_handoff_from(value: Option<&str>) -> Option<String> {
    sanitize_handoff_text(value, 32).map(|v| v.to_ascii_lowercase())
}

fn normalize_handoff_action(value: Option<&str>) -> Option<String> {
    sanitize_handoff_text(value, 64).map(|v| v.to_ascii_lowercase())
}

fn build_quote_share_url(token: &str, handoff: &PortalHandoffQuery) -> String {
    let Ok(mut url) = reqwest::Url::parse(&format!("https://portal.local/quote/{token}")) else {
        return format!("/quote/{token}");
    };

    if !handoff.is_empty() {
        let mut query = url.query_pairs_mut();
        if let Some(from) = handoff.from.as_deref() {
            query.append_pair("from", from);
        }
        if let Some(action) = handoff.action.as_deref() {
            query.append_pair("action", action);
        }
        if let Some(next_action) = handoff.next_action.as_deref() {
            query.append_pair("next_action", next_action);
        }
        if let Some(summary) = handoff.context_summary.as_deref() {
            query.append_pair("context_summary", summary);
        }
        if let Some(summary) = handoff.assumptions_summary.as_deref() {
            query.append_pair("assumptions_summary", summary);
        }
    }

    match url.query() {
        Some(query) => format!("{}?{query}", url.path()),
        None => url.path().to_string(),
    }
}

/// Render the quote viewer HTML page.
#[derive(Debug, Deserialize, Default)]
struct ViewQuoteParams {
    /// Origin of the navigation (e.g., "slack")
    from: Option<String>,
    /// Suggested action for the viewer (e.g., "review", "approve", "comment")
    action: Option<String>,
    /// Optional context summary forwarded from the handoff source.
    context_summary: Option<String>,
    /// Optional assumptions summary forwarded from the handoff source.
    assumptions_summary: Option<String>,
    /// Optional normalized primary action for continuity/autoscroll.
    next_action: Option<String>,
}

impl ViewQuoteParams {
    fn normalized_handoff(&self) -> PortalHandoffQuery {
        PortalHandoffQuery {
            from: normalize_handoff_from(self.from.as_deref()),
            action: normalize_handoff_action(self.action.as_deref()),
            context_summary: sanitize_handoff_text(self.context_summary.as_deref(), 280),
            assumptions_summary: sanitize_handoff_text(self.assumptions_summary.as_deref(), 280),
            next_action: normalize_handoff_action(self.next_action.as_deref()),
        }
    }
}

async fn view_quote_page(
    Path(token): Path<String>,
    params: Query<ViewQuoteParams>,
    State(state): State<PortalState>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let quote_id =
        resolve_quote_by_token(&state.db_pool, &token).await.map_err(|(status, err)| {
            let hint = err
                .0
                .recovery_hint
                .as_deref()
                .unwrap_or("Check the link or contact the sender for a new one.");
            (
                status,
                Html(format!(
                    "<h1>Quote Unavailable</h1><p>{}</p><p style=\"color:#666\">{}</p>",
                    escape_html(&err.0.error),
                    escape_html(hint),
                )),
            )
        })?;

    // Fetch quote details with assumption tracking
    let quote_row = sqlx::query(
        "SELECT id, status, version, currency, term_months, valid_until,
                created_at, updated_at, created_by, account_id,
                currency_explicit, tax_rate_explicit, tax_rate_value,
                payment_terms, payment_terms_explicit, billing_country, billing_country_explicit
         FROM quote WHERE id = ?",
    )
    .bind(&quote_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(redacted_db_error)?;

    let quote_row = match quote_row {
        Some(row) => row,
        None => return Err((StatusCode::NOT_FOUND, Html("<h1>Quote not found</h1>".to_string()))),
    };

    // Get quote version for pricing snapshot lookup
    let quote_version: i64 = quote_row.try_get("version").unwrap_or(1);

    // Fetch authoritative pricing snapshot from database (single source of truth for totals)
    let pricing_snapshot_row = sqlx::query(
        "SELECT subtotal, discount_total, tax_total, total, currency, pricing_trace_json
         FROM quote_pricing_snapshot
         WHERE quote_id = ? AND version = ?
         LIMIT 1",
    )
    .bind(&quote_id)
    .bind(quote_version)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(redacted_db_error)?;

    // Use authoritative totals from pricing snapshot when available
    let authoritative_subtotal: f64;
    let authoritative_discount: f64;
    let authoritative_tax: f64;
    let authoritative_total: f64;
    let has_snapshot: bool;
    let pricing_trace: Option<serde_json::Value>;

    match &pricing_snapshot_row {
        Some(row) => {
            authoritative_subtotal = row.try_get("subtotal").unwrap_or(0.0);
            authoritative_discount = row.try_get("discount_total").unwrap_or(0.0);
            authoritative_tax = row.try_get("tax_total").unwrap_or(0.0);
            authoritative_total = row.try_get("total").unwrap_or(0.0);
            has_snapshot = true;
            pricing_trace = row
                .try_get::<Option<String>, _>("pricing_trace_json")
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok());
        }
        None => {
            authoritative_subtotal = 0.0;
            authoritative_discount = 0.0;
            authoritative_tax = 0.0;
            authoritative_total = 0.0;
            has_snapshot = false;
            pricing_trace = None;
        }
    };

    // Fetch quote lines
    let line_rows = sqlx::query(
        "SELECT ql.id, ql.quantity, ql.unit_price, ql.subtotal, ql.discount_pct, ql.notes,
                COALESCE(p.name, ql.product_id) as product_name, ql.product_id
         FROM quote_line ql
         LEFT JOIN product p ON p.id = ql.product_id
         WHERE ql.quote_id = ?
         ORDER BY ql.created_at",
    )
    .bind(&quote_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(redacted_db_error)?;

    // Use authoritative totals from pricing snapshot, or compute from line items as fallback
    let (final_subtotal, final_discount, final_tax, final_total) = if has_snapshot {
        // Use authoritative totals from pricing snapshot
        (authoritative_subtotal, authoritative_discount, authoritative_tax, authoritative_total)
    } else {
        // Fallback: compute from line items (for quotes without pricing snapshot)
        let mut computed_subtotal = 0.0_f64;
        let mut computed_discount = 0.0_f64;
        for row in &line_rows {
            let qty: i64 = row.try_get("quantity").unwrap_or(0);
            let unit_price: f64 = row.try_get("unit_price").unwrap_or(0.0);
            let base_subtotal = match row.try_get::<Option<f64>, _>("subtotal") {
                Ok(Some(value)) => value,
                _ => unit_price * qty as f64,
            };
            let discount_pct: f64 =
                row.try_get::<f64, _>("discount_pct").unwrap_or(0.0).clamp(0.0, 100.0);
            computed_subtotal += base_subtotal;
            computed_discount += base_subtotal * discount_pct / 100.0;
        }
        let computed_total = computed_subtotal - computed_discount;
        (computed_subtotal, computed_discount, 0.0, computed_total)
    };

    let lines: Vec<serde_json::Value> = line_rows
        .iter()
        .map(|row| {
            let quantity = row.try_get::<i64, _>("quantity").unwrap_or(0);
            let unit_price: f64 = row.try_get("unit_price").unwrap_or(0.0);
            let subtotal = match row.try_get::<Option<f64>, _>("subtotal") {
                Ok(Some(value)) => value,
                _ => unit_price * quantity as f64,
            };
            let discount_pct: f64 =
                row.try_get::<f64, _>("discount_pct").unwrap_or(0.0).clamp(0.0, 100.0);
            let line_total = subtotal * (1.0 - discount_pct / 100.0);

            serde_json::json!({
                "product_name": row.try_get::<String, _>("product_name").unwrap_or_default(),
                "quantity": quantity,
                "unit_price": format_price(unit_price),
                "total": format_price(line_total),
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
    .map_err(redacted_db_error)?;

    let comments: Vec<serde_json::Value> = comment_rows
        .iter()
        .map(|row| {
            let author_email = row.try_get::<String, _>("author_email").unwrap_or_default();

            serde_json::json!({
                "author": row.try_get::<String, _>("author_name").unwrap_or_default(),
                "text": row.try_get::<String, _>("body").unwrap_or_default(),
                "timestamp": row.try_get::<String, _>("created_at").unwrap_or_default(),
                "is_customer": !author_email.starts_with("portal:"),
            })
        })
        .collect();

    let valid_until = quote_row.try_get::<String, _>("valid_until").unwrap_or_default();
    let created_at_value = quote_row.try_get::<String, _>("created_at").unwrap_or_default();
    let quote_age_days = created_at_value
        .parse::<chrono::DateTime<chrono::Utc>>()
        .map(|created_at| (Utc::now() - created_at).num_days().max(0))
        .unwrap_or(0);
    let days_to_expiry = valid_until
        .parse::<chrono::DateTime<chrono::Utc>>()
        .map(|expires_at| (expires_at - Utc::now()).num_days())
        .unwrap_or(0);
    let discount_percent = if final_subtotal > 0.0 {
        ((final_discount / final_subtotal) * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    };

    let account_id_value: Option<String> =
        quote_row.try_get::<Option<String>, _>("account_id").unwrap_or(None);
    let (prior_quotes_count, prior_approved_count, similar_deals) = if let Some(account_id) =
        &account_id_value
    {
        let prior_quotes_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM quote WHERE account_id = ? AND id != ?")
                .bind(account_id)
                .bind(&quote_id)
                .fetch_one(&state.db_pool)
                .await
                .unwrap_or(0);

        let prior_approved_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM quote WHERE account_id = ? AND id != ? AND status = 'approved'",
        )
        .bind(account_id)
        .bind(&quote_id)
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

        let similar_rows = sqlx::query(
            "SELECT id, status, created_at
             FROM quote
             WHERE account_id = ? AND id != ?
             ORDER BY created_at DESC
             LIMIT 3",
        )
        .bind(account_id)
        .bind(&quote_id)
        .fetch_all(&state.db_pool)
        .await
        .unwrap_or_default();

        let similar_deals: Vec<serde_json::Value> = similar_rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "quote_id": row.try_get::<String, _>("id").unwrap_or_default(),
                    "status": canonical_quote_status(&row.try_get::<String, _>("status").unwrap_or_default()),
                    "created_at": row.try_get::<String, _>("created_at").unwrap_or_default(),
                })
            })
            .collect();

        (prior_quotes_count, prior_approved_count, similar_deals)
    } else {
        (0, 0, Vec::new())
    };

    let latest_rep_note: Option<String> = sqlx::query_scalar(
        "SELECT body
         FROM portal_comment
         WHERE quote_id = ? AND author_email LIKE 'portal:%'
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(&quote_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    let latest_customer_context: Option<String> = sqlx::query_scalar(
        "SELECT body
         FROM portal_comment
         WHERE quote_id = ? AND author_email NOT LIKE 'portal:%'
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(&quote_id)
    .fetch_optional(&state.db_pool)
    .await
    .unwrap_or(None);

    // Build template context
    let mut context = Context::new();
    let raw_status = quote_row.try_get::<String, _>("status").unwrap_or_default();
    let display_status = canonical_quote_status(&raw_status);
    let expires_soon = valid_until
        .parse::<chrono::DateTime<chrono::Utc>>()
        .map(|expires_at| (expires_at - Utc::now()).num_days() <= 3)
        .unwrap_or(false);

    // Extract assumption flags from database
    let currency_explicit: bool =
        quote_row.try_get::<i64, _>("currency_explicit").unwrap_or(0) == 1;
    let tax_rate_explicit: bool =
        quote_row.try_get::<i64, _>("tax_rate_explicit").unwrap_or(0) == 1;
    let tax_rate_value: f64 = quote_row.try_get::<f64, _>("tax_rate_value").unwrap_or(0.0);
    let payment_terms: String =
        quote_row.try_get::<String, _>("payment_terms").unwrap_or_else(|_| "net_30".to_string());
    let payment_terms_explicit: bool =
        quote_row.try_get::<i64, _>("payment_terms_explicit").unwrap_or(0) == 1;
    let billing_country: Option<String> =
        quote_row.try_get::<Option<String>, _>("billing_country").unwrap_or(None);
    let billing_country_explicit: bool =
        quote_row.try_get::<i64, _>("billing_country_explicit").unwrap_or(0) == 1;
    let currency: String =
        quote_row.try_get::<String, _>("currency").unwrap_or_else(|_| "USD".to_string());

    // Build assumptions list for display
    let mut assumptions: Vec<serde_json::Value> = Vec::new();
    if !currency_explicit {
        assumptions.push(serde_json::json!({
            "field": "currency",
            "value": &currency,
            "label": "Currency",
            "description": format!("Using default currency: {}", currency)
        }));
    }
    if !tax_rate_explicit {
        assumptions.push(serde_json::json!({
            "field": "tax_rate",
            "value": format!("{:.0}%", tax_rate_value * 100.0),
            "label": "Tax Rate",
            "description": "Tax not applicable or not configured"
        }));
    }
    if !payment_terms_explicit {
        let payment_terms_display = match payment_terms.as_str() {
            "net_30" => "Net 30",
            "net_60" => "Net 60",
            "net_90" => "Net 90",
            "upfront" => "Upfront",
            _ => "Net 30",
        };
        assumptions.push(serde_json::json!({
            "field": "payment_terms",
            "value": payment_terms_display,
            "label": "Payment Terms",
            "description": format!("Using default payment terms: {}", payment_terms_display)
        }));
    }
    if billing_country.is_none() || !billing_country_explicit {
        assumptions.push(serde_json::json!({
            "field": "billing_country",
            "value": billing_country.as_deref().unwrap_or("Not specified"),
            "label": "Billing Country",
            "description": "Billing country not specified - may affect tax calculation"
        }));
    }

    let has_assumptions = !assumptions.is_empty();

    // Build pricing rationale for details-on-demand panel
    let pricing_rationale = build_pricing_rationale(
        final_subtotal,
        final_discount,
        final_tax,
        final_total,
        tax_rate_value,
        &lines,
        pricing_trace.as_ref(),
        has_snapshot,
    );

    context.insert("quote", &serde_json::json!({
        "quote_id": quote_id,
        "token": token,
        "status": display_status,
        "version": quote_row.try_get::<i64, _>("version").unwrap_or(1),
        "created_at": created_at_value,
        "valid_until": valid_until,
        "term_months": quote_row.try_get::<i64, _>("term_months").unwrap_or(12),
        "currency": currency,
        "payment_terms": match payment_terms.as_str() {
            "net_30" => "Net 30",
            "net_60" => "Net 60",
            "net_90" => "Net 90",
            "upfront" => "Upfront",
            _ => "Net 30",
        },
        "payment_terms_raw": payment_terms,
        "subtotal": format_price(final_subtotal),
        "discount_total": if final_discount > 0.0 { format_price(final_discount) } else { "".to_string() },
        "tax_rate": format!("{:.0}%", tax_rate_value * 100.0),
        "tax_rate_raw": tax_rate_value,
        "tax_amount": if final_tax > 0.0 { format_price(final_tax) } else { "".to_string() },
        "total": format_price(final_total),
        "lines": lines,
        "expires_soon": expires_soon,
        "billing_country": billing_country,
        "assumptions": assumptions,
        "has_assumptions": has_assumptions,
        "currency_explicit": currency_explicit,
        "tax_rate_explicit": tax_rate_explicit,
        "payment_terms_explicit": payment_terms_explicit,
        "billing_country_explicit": billing_country_explicit,
        "pricing_rationale": pricing_rationale,
    }));

    context.insert(
        "decision_context",
        &serde_json::json!({
            "quote_age_days": quote_age_days,
            "days_to_expiry": days_to_expiry,
            "line_count": line_rows.len(),
            "discount_amount": format_price(final_discount),
            "discount_percent": format!("{discount_percent:.1}%"),
            "prior_quotes_count": prior_quotes_count,
            "prior_approved_count": prior_approved_count,
            "rep_justification": latest_rep_note.unwrap_or_else(|| "No rep justification has been attached yet.".to_string()),
            "competitive_context": latest_customer_context.unwrap_or_else(|| "No competitive context has been shared yet.".to_string()),
            "similar_deals": similar_deals,
        }),
    );

    context.insert(
        "customer",
        &serde_json::json!({
            "name": account_id_value.clone().unwrap_or_else(|| "Customer".to_string()),
            "email": "customer@example.com",
            "phone": "",
        }),
    );

    context.insert(
        "rep",
        &serde_json::json!({
            "name": quote_row.try_get::<String, _>("created_by").unwrap_or_default(),
            "email": if quote_row.try_get::<String, _>("created_by").unwrap_or_default().contains('@') {
                quote_row.try_get::<String, _>("created_by").unwrap_or_default()
            } else {
                String::new()
            },
        }),
    );

    context.insert("comments", &comments);

    // Slack-to-portal state continuity: pass preserved navigation context to template.
    let handoff = params.normalized_handoff();
    if let Some(ref from) = handoff.from {
        context.insert("from", from);
    }
    if let Some(ref action) = handoff.action {
        context.insert("action", action);
    }
    if let Some(ref next_action) = handoff.next_action {
        context.insert("next_action", next_action);
    }
    if let Some(ref context_summary) = handoff.context_summary {
        context.insert("handoff_context_summary", context_summary);
    }
    if let Some(ref assumptions_summary) = handoff.assumptions_summary {
        context.insert("handoff_assumptions_summary", assumptions_summary);
    }
    let handoff_action = handoff.primary_action().map(ToString::to_string);
    if let Some(ref handoff_action) = handoff_action {
        context.insert("handoff_action", handoff_action);
    }

    context.insert("branding", &state.branding);

    let html =
        state.templates.render("quote_viewer.html", &context).map_err(redacted_template_error)?;

    // Funnel telemetry: pricing rendered (quote viewed with pricing)
    record_funnel_event(
        &state.db_pool,
        quotey_core::audit::funnel::PRICING_RENDERED,
        Some(&quote_id),
        "portal:viewer",
        "success",
        &[("has_snapshot", if has_snapshot { "true" } else { "false" })],
    )
    .await;

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
        token = %redact_token(&token),
        "PDF download requested"
    );

    // Check if PDF generator is available
    let pdf_generator = state.pdf_generator.as_ref().ok_or_else(|| {
        error!("PDF generator not initialized");
        (StatusCode::SERVICE_UNAVAILABLE, Json(PortalError::service_unavailable("PDF generator")))
    })?;

    // Fetch complete quote data for PDF generation
    let quote_data =
        fetch_quote_for_pdf(&state.db_pool, &quote_id, &state.branding.company_name).await?;

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
            // Funnel telemetry: PDF download
            record_funnel_event(
                &state.db_pool,
                quotey_core::audit::funnel::PDF_DOWNLOAD,
                Some(&quote_id),
                "portal:viewer",
                "success",
                &[],
            )
            .await;
            Ok(result.into_response(&filename))
        }
        Err(e) => {
            error!(error = %e, quote_id = %quote_id, "PDF generation failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(PortalError::service_unavailable("PDF generator")),
            ))
        }
    }
}

/// Fetch complete quote data for PDF generation
async fn fetch_quote_for_pdf(
    pool: &DbPool,
    quote_id: &str,
    company_name: &str,
) -> Result<serde_json::Value, (StatusCode, Json<PortalError>)> {
    // Fetch quote basic info
    let quote_row = sqlx::query(
        r#"SELECT
            q.id, q.status, q.created_at, q.valid_until, q.currency,
            q.account_id
         FROM quote q
         WHERE q.id = ?"#,
    )
    .bind(quote_id)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::RowNotFound => (StatusCode::NOT_FOUND, Json(PortalError::not_found("quote"))),
        _ => {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(PortalError::service_unavailable("database")))
        }
    })?;

    // Fetch quote lines
    let account_id: Option<String> =
        quote_row.try_get::<Option<String>, _>("account_id").unwrap_or(None);
    let account_name = account_id.clone().unwrap_or_else(|| "Unknown Account".to_string());

    let lines = sqlx::query(
        r#"SELECT
            ql.id, ql.quote_id, ql.product_id, ql.quantity, ql.unit_price, ql.subtotal, ql.discount_pct,
            p.name as product_name, p.sku as product_sku
         FROM quote_line ql
         LEFT JOIN product p ON p.id = ql.product_id
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
            Json(PortalError::service_unavailable("database")),
        )
    })?;

    // Calculate pricing summary using fold to properly accumulate totals
    let (subtotal, total_discount, lines): (f64, f64, Vec<serde_json::Value>) = lines.iter().fold(
        (0.0_f64, 0.0_f64, Vec::new()),
        |(mut sub_acc, mut disc_acc, mut lines_acc), r| {
            let quantity: i64 = r.try_get("quantity").unwrap_or(0);
            let unit_price: f64 = r.try_get("unit_price").unwrap_or(0.0);
            let base_subtotal = match r.try_get::<Option<f64>, _>("subtotal") {
                Ok(Some(value)) => value,
                _ => unit_price * quantity as f64,
            };
            let discount_pct: f64 =
                r.try_get::<f64, _>("discount_pct").unwrap_or(0.0).clamp(0.0, 100.0);
            let discount_amount = base_subtotal * discount_pct / 100.0;
            let total_price = base_subtotal - discount_amount;

            sub_acc += base_subtotal;
            disc_acc += discount_amount;

            lines_acc.push(serde_json::json!({
                "id": r.try_get::<String, _>("id").unwrap_or_default(),
                "product_name": r.try_get::<String, _>("product_name").unwrap_or_default(),
                "product_sku": r.try_get::<String, _>("product_sku").unwrap_or_default(),
                "quantity": quantity,
                "unit_price": unit_price,
                "subtotal": base_subtotal,
                "discount_pct": discount_pct,
                "discount_amount": discount_amount,
                "total_price": total_price,
            }));

            (sub_acc, disc_acc, lines_acc)
        },
    );

    let discounted_total = subtotal - total_discount;
    let tax_rate = 0.0;
    let tax = discounted_total * tax_rate;
    let total = discounted_total + tax;
    let raw_status = quote_row.try_get::<String, _>("status").unwrap_or_default();

    // Build the JSON structure expected by the PDF templates
    let quote_data = serde_json::json!({
        "id": quote_id,
        "status": canonical_quote_status(&raw_status),
        "created_at": quote_row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
        "valid_until": quote_row.try_get::<chrono::DateTime<chrono::Utc>, _>("valid_until")
            .map(|d| d.to_rfc3339())
            .unwrap_or_default(),
        "currency": quote_row.try_get::<String, _>("currency").unwrap_or_default(),
        "account": {
            "id": account_id.clone().unwrap_or_default(),
            "name": account_name,
            "industry": "",
        },
        "lines": lines,
        "pricing": {
            "subtotal": subtotal,
            "total_discount": total_discount,
            "discount_total": total_discount,
            "tax_rate": tax_rate,
            "tax": tax,
            "tax_total": tax,
            "total": total,
        },
        "company_name": company_name,
        "quote_id": quote_id,
        "status_text": canonical_quote_status(&raw_status),
    });

    Ok(quote_data)
}

/// Render the portal index page (list of quotes).
async fn portal_index_page(
    Query(query): Query<PortalIndexQuery>,
    State(state): State<PortalState>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    // Fetch all quotes with optional status filter
    let status_filter = normalize_status_filter(query.status.as_deref());
    let selected_filter = selected_status(query.status.as_deref());
    let selected_search_value = selected_search(query.search.as_deref());
    let search_pattern = selected_search_value.to_ascii_lowercase();
    let search_pattern =
        if search_pattern.is_empty() { None } else { Some(format!("%{}%", search_pattern)) };

    let now = Utc::now().to_rfc3339();
    let mut query_builder = QueryBuilder::new(
        r#"
        SELECT q.id, q.status, q.created_at, q.valid_until,
            COALESCE(SUM(
                COALESCE(ql.subtotal, COALESCE(ql.unit_price, 0.0) * COALESCE(ql.quantity, 0))
                * (1.0 - (MAX(0.0, MIN(COALESCE(ql.discount_pct, 0.0), 100.0)) / 100.0))
            ), 0.0) AS computed_total,
            (
                SELECT pl.token
                FROM portal_link pl
                WHERE pl.quote_id = q.id
                  AND pl.revoked = 0
                  AND pl.expires_at > "#,
    );
    query_builder.push_bind(now.as_str());
    query_builder.push(
        r#"
                ORDER BY pl.created_at DESC
                LIMIT 1
            ) AS token
        FROM quote q
        LEFT JOIN quote_line ql ON ql.quote_id = q.id
        "#,
    );
    query_builder.push(" WHERE 1=1");

    if let Some(filter_values) = &status_filter {
        query_builder.push(" AND q.status IN (");
        let mut separated = query_builder.separated(", ");
        for status in filter_values {
            separated.push_bind(status);
        }
        query_builder.push(")");
    }

    if let Some(pattern) = search_pattern.clone() {
        query_builder.push(" AND (LOWER(q.id) LIKE ");
        query_builder.push_bind(pattern.clone());
        query_builder.push(" OR LOWER(q.account_id) LIKE ");
        query_builder.push_bind(pattern.clone());
        query_builder.push(" OR LOWER(q.created_by) LIKE ");
        query_builder.push_bind(pattern);
        query_builder.push(")");
    }

    query_builder.push(" GROUP BY q.id ORDER BY q.created_at DESC LIMIT 100");

    let quote_rows =
        query_builder.build().fetch_all(&state.db_pool).await.map_err(redacted_db_error)?;

    // Calculate stats
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM quote")
        .fetch_one(&state.db_pool)
        .await
        .unwrap_or(0);

    let pending_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quote WHERE status IN (?, ?, ?)")
            .bind(INDEX_PENDING_STATUSES[0])
            .bind(INDEX_PENDING_STATUSES[1])
            .bind(INDEX_PENDING_STATUSES[2])
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    let approved_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quote WHERE status = 'approved'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    let total_expired_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quote WHERE status = 'expired'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    let declined_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM quote WHERE status IN ('rejected', 'declined')")
            .fetch_one(&state.db_pool)
            .await
            .unwrap_or(0);

    // Fetch pending quotes for "Pending Approvals" section (limit 6, filtered on backend)
    let pending_quote_rows = sqlx::query(
        r#"
        SELECT q.id, q.status, q.created_at, q.valid_until,
            COALESCE(SUM(
                COALESCE(ql.subtotal, COALESCE(ql.unit_price, 0.0) * COALESCE(ql.quantity, 0))
                * (1.0 - (MAX(0.0, MIN(COALESCE(ql.discount_pct, 0.0), 100.0)) / 100.0))
            ), 0.0) AS computed_total,
            (
                SELECT pl.token
                FROM portal_link pl
                WHERE pl.quote_id = q.id
                  AND pl.revoked = 0
                  AND pl.expires_at > ?
                ORDER BY pl.created_at DESC
                LIMIT 1
            ) AS token
        FROM quote q
        LEFT JOIN quote_line ql ON ql.quote_id = q.id
        WHERE q.status IN ('draft', 'pending', 'sent')
        GROUP BY q.id
        ORDER BY q.created_at DESC
        LIMIT 6
        "#,
    )
    .bind(&now)
    .fetch_all(&state.db_pool)
    .await
    .map_err(redacted_db_error)?;

    // Build pending quotes list
    let pending_quotes: Vec<serde_json::Value> = pending_quote_rows
        .iter()
        .filter_map(|row: &sqlx::sqlite::SqliteRow| {
            let quote_id: String = row.try_get("id").unwrap_or_default();
            let status = canonical_quote_status(&row.try_get::<String, _>("status").unwrap_or_default());
            let link_token: Option<String> = row.try_get::<Option<String>, _>("token").unwrap_or(None);

            let token = link_token?;

            Some(serde_json::json!({
                "token": token,
                "quote_id": quote_id,
                "created_at": row.try_get::<String, _>("created_at").unwrap_or_default(),
                "valid_until": row.try_get::<String, _>("valid_until").unwrap_or_default(),
                "status": status,
                "total_amount": format_price(row.try_get::<f64, _>("computed_total").unwrap_or(0.0)),
            }))
        })
        .collect();

    // Build quotes list — only include quotes with valid portal link tokens
    let quotes: Vec<serde_json::Value> = quote_rows
        .iter()
        .filter_map(|row: &sqlx::sqlite::SqliteRow| {
            let quote_id: String = row.try_get("id").unwrap_or_default();
            let status = canonical_quote_status(&row.try_get::<String, _>("status").unwrap_or_default());
            let link_token: Option<String> = row.try_get::<Option<String>, _>("token").unwrap_or(None);

            // Only show quotes that have a valid portal link token — never expose raw quote IDs
            let token = link_token?;

            Some(serde_json::json!({
                "token": token,
                "quote_id": quote_id,
                "created_at": row.try_get::<String, _>("created_at").unwrap_or_default(),
                "valid_until": row.try_get::<String, _>("valid_until").unwrap_or_default(),
                "status": status,
                "total_amount": format_price(row.try_get::<f64, _>("computed_total").unwrap_or(0.0)),
                "total_amount_raw": row.try_get::<f64, _>("computed_total").unwrap_or(0.0),
            }))
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
            "expired": total_expired_count,
            "declined": declined_count,
        }),
    );

    context.insert("quotes", &quotes);
    context.insert("pending_quotes", &pending_quotes);
    context.insert("status_filter", &selected_filter);
    context.insert("search", &selected_search_value);

    context.insert("branding", &state.branding);

    let html = state.templates.render("index.html", &context).map_err(redacted_template_error)?;

    Ok(Html(html))
}

/// Render the PWA approvals list route.
///
/// This route is purpose-built for manager mobile approvals.
async fn approvals_index_page(
    State(state): State<PortalState>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let now = Utc::now().to_rfc3339();
    let rows = sqlx::query(
        r#"
        SELECT
            ar.id AS approval_id,
            ar.quote_id AS quote_id,
            ar.approver_role AS approver_role,
            ar.reason AS reason,
            ar.created_at AS requested_at,
            COALESCE(NULLIF(q.account_id, ''), 'Unknown Customer') AS customer_name,
            COALESCE(
                (
                    SELECT s.total
                    FROM quote_pricing_snapshot s
                    WHERE s.quote_id = q.id
                    ORDER BY s.version DESC
                    LIMIT 1
                ),
                SUM(
                    COALESCE(ql.subtotal, COALESCE(ql.unit_price, 0.0) * COALESCE(ql.quantity, 0))
                    * (1.0 - (MAX(0.0, MIN(COALESCE(ql.discount_pct, 0.0), 100.0)) / 100.0))
                ),
                0.0
            ) AS total_amount,
            COALESCE(
                (
                    SELECT CASE
                        WHEN s.subtotal > 0 THEN ROUND((s.discount_total / s.subtotal) * 100.0, 1)
                        ELSE 0.0
                    END
                    FROM quote_pricing_snapshot s
                    WHERE s.quote_id = q.id
                    ORDER BY s.version DESC
                    LIMIT 1
                ),
                AVG(COALESCE(ql.discount_pct, 0.0)),
                0.0
            ) AS discount_pct
        FROM approval_request ar
        JOIN quote q ON q.id = ar.quote_id
        LEFT JOIN quote_line ql ON ql.quote_id = q.id
        WHERE ar.status = 'pending'
          AND EXISTS (
              SELECT 1
              FROM portal_link pl
              WHERE pl.quote_id = q.id
                AND pl.revoked = 0
                AND pl.expires_at > ?
          )
        GROUP BY ar.id, ar.quote_id, ar.approver_role, ar.reason, ar.created_at, q.id, q.account_id
        ORDER BY ar.created_at DESC
        LIMIT 100
        "#,
    )
    .bind(now.as_str())
    .fetch_all(&state.db_pool)
    .await
    .map_err(redacted_db_error)?;

    let approvals: Vec<serde_json::Value> = rows
        .iter()
        .map(|row: &sqlx::sqlite::SqliteRow| {
            let approval_id = row.try_get::<String, _>("approval_id").unwrap_or_default();
            let quote_id = row.try_get::<String, _>("quote_id").unwrap_or_default();
            let customer_name = row.try_get::<String, _>("customer_name").unwrap_or_default();
            let approver_role = row.try_get::<String, _>("approver_role").unwrap_or_default();
            let reason = row.try_get::<String, _>("reason").unwrap_or_default();
            let requested_at = row.try_get::<String, _>("requested_at").unwrap_or_default();
            let total_amount = row.try_get::<f64, _>("total_amount").unwrap_or(0.0);
            let discount_pct = row.try_get::<f64, _>("discount_pct").unwrap_or(0.0);

            serde_json::json!({
                "approval_id": approval_id,
                "quote_id": quote_id,
                "customer_name": customer_name,
                "approver_role": approver_role,
                "reason": reason,
                "requested_at": requested_at,
                "total_amount": format_price(total_amount),
                "discount_pct": format!("{discount_pct:.1}%"),
                "detail_href": format!("/approvals/{approval_id}"),
            })
        })
        .collect();

    let mut context = Context::new();
    context.insert("approvals", &approvals);
    context.insert("count", &approvals.len());
    context.insert("branding", &state.branding);

    let html =
        state.templates.render("approvals.html", &context).map_err(redacted_template_error)?;

    Ok(Html(html))
}

/// Render a full approval detail view with quote context, discount impact,
/// and approve/reject/need-info/snooze actions.
async fn approval_detail_page(
    Path(approval_id): Path<String>,
    State(state): State<PortalState>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let db_err = |e: sqlx::Error| {
        warn!(error = %e, "approval_detail database error (redacted from user response)");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<h1>Service Unavailable</h1><p>A database error occurred. Please try again later.</p>".to_string()),
        )
    };

    // Fetch approval request
    let approval_row = sqlx::query(
        "SELECT id, quote_id, approver_role, reason, justification, status,
                requested_by, expires_at, created_at
         FROM approval_request WHERE id = ?",
    )
    .bind(&approval_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(db_err)?;

    let approval_row = approval_row.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Html("<h1>Approval Not Found</h1><p>Approval request was not found.</p>".to_string()),
        )
    })?;

    let quote_id: String = approval_row.try_get("quote_id").unwrap_or_default();

    // Fetch active portal link token for action endpoints
    let now = Utc::now().to_rfc3339();
    let token: String = sqlx::query_scalar(
        "SELECT token FROM portal_link
         WHERE quote_id = ? AND revoked = 0 AND expires_at > ?
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&quote_id)
    .bind(&now)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(db_err)?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Html(
                "<h1>Link Unavailable</h1><p>No active portal link is available for this approval.</p>"
                    .to_string(),
            ),
        )
    })?;

    // Fetch quote details
    let quote_row =
        sqlx::query("SELECT id, status, currency, account_id, created_at FROM quote WHERE id = ?")
            .bind(&quote_id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(db_err)?;

    let quote_row = match quote_row {
        Some(r) => r,
        None => {
            return Err((StatusCode::NOT_FOUND, Html("<h1>Quote Not Found</h1>".to_string())));
        }
    };

    // Fetch quote lines with product names
    let line_rows = sqlx::query(
        r#"SELECT ql.id AS line_id, ql.quantity, ql.unit_price, ql.subtotal, ql.discount_pct,
                  p.name AS product_name, p.sku AS product_sku
           FROM quote_line ql
           LEFT JOIN product p ON p.id = ql.product_id
           WHERE ql.quote_id = ?
           ORDER BY ql.id"#,
    )
    .bind(&quote_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(db_err)?;

    // Build line items and compute totals
    let mut subtotal = 0.0_f64;
    let mut discount_total = 0.0_f64;
    let lines: Vec<serde_json::Value> = line_rows
        .iter()
        .map(|r| {
            let qty: i64 = r.try_get("quantity").unwrap_or(0);
            let unit_price: f64 = r.try_get("unit_price").unwrap_or(0.0);
            let base_subtotal = match r.try_get::<Option<f64>, _>("subtotal") {
                Ok(Some(v)) => v,
                _ => unit_price * qty as f64,
            };
            let discount_pct: f64 =
                r.try_get::<f64, _>("discount_pct").unwrap_or(0.0).clamp(0.0, 100.0);
            let discount_amount = base_subtotal * discount_pct / 100.0;
            let total_price = base_subtotal - discount_amount;

            subtotal += base_subtotal;
            discount_total += discount_amount;

            serde_json::json!({
                "id": r.try_get::<String, _>("line_id").unwrap_or_default(),
                "product_name": r.try_get::<String, _>("product_name").unwrap_or_default(),
                "sku": r.try_get::<String, _>("product_sku").unwrap_or_default(),
                "quantity": qty,
                "unit_price": format_price(unit_price),
                "discount_pct": discount_pct,
                "total_price": format_price(total_price),
            })
        })
        .collect();

    let total = subtotal - discount_total;
    let discount_pct =
        if subtotal > 0.0 { ((discount_total / subtotal) * 100.0).clamp(0.0, 100.0) } else { 0.0 };

    // Build template context
    let mut context = Context::new();

    context.insert(
        "approval",
        &serde_json::json!({
            "id": approval_id,
            "approver_role": approval_row.try_get::<String, _>("approver_role").unwrap_or_default(),
            "reason": approval_row.try_get::<String, _>("reason").unwrap_or_default(),
            "justification": approval_row.try_get::<String, _>("justification").unwrap_or_default(),
            "status": approval_row.try_get::<String, _>("status").unwrap_or_else(|_| "pending".to_string()),
            "requested_by": approval_row.try_get::<String, _>("requested_by").unwrap_or_default(),
            "created_at": approval_row.try_get::<String, _>("created_at").unwrap_or_default(),
        }),
    );

    let customer = quote_row
        .try_get::<Option<String>, _>("account_id")
        .unwrap_or(None)
        .unwrap_or_else(|| "Unknown Customer".to_string());

    context.insert(
        "quote",
        &serde_json::json!({
            "quote_id": quote_id,
            "token": token,
            "status": canonical_quote_status(&quote_row.try_get::<String, _>("status").unwrap_or_default()),
            "currency": quote_row.try_get::<String, _>("currency").unwrap_or_else(|_| "USD".to_string()),
            "customer": customer,
            "created_at": quote_row.try_get::<String, _>("created_at").unwrap_or_default(),
            "subtotal": format_price(subtotal),
            "discount_total": format_price(discount_total),
            "discount_pct": (discount_pct * 10.0).round() / 10.0,
            "total": format_price(total),
        }),
    );

    context.insert("lines", &lines);

    let html = state
        .templates
        .render("approval_detail.html", &context)
        .map_err(redacted_template_error)?;

    Ok(Html(html))
}

/// Render lightweight PWA settings for notifications and cache maintenance.
async fn approvals_settings_page(
    State(state): State<PortalState>,
) -> Result<Html<String>, (StatusCode, Html<String>)> {
    let mut context = Context::new();
    context.insert("branding", &state.branding);

    let html =
        state.templates.render("settings.html", &context).map_err(redacted_template_error)?;

    Ok(Html(html))
}

/// Format a price for display
fn format_price(amount: f64) -> String {
    format!("${:.2}", amount)
}

/// Build pricing rationale data structure for the details-on-demand panel.
/// Provides deterministic rule IDs, source explanations, and computation provenance.
#[allow(clippy::too_many_arguments)]
fn build_pricing_rationale(
    subtotal: f64,
    discount_total: f64,
    tax_total: f64,
    total: f64,
    tax_rate: f64,
    lines: &[serde_json::Value],
    pricing_trace: Option<&serde_json::Value>,
    has_snapshot: bool,
) -> serde_json::Value {
    // Build line item rationales
    let line_rationales: Vec<serde_json::Value> = lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let product_name = line.get("product_name").and_then(|v| v.as_str()).unwrap_or("Unknown");
            let quantity = line.get("quantity").and_then(|v| v.as_i64()).unwrap_or(0);
            let unit_price_str = line.get("unit_price").and_then(|v| v.as_str()).unwrap_or("$0.00");
            let total_str = line.get("total").and_then(|v| v.as_str()).unwrap_or("$0.00");

            // Parse unit price from string (remove $ and parse)
            let unit_price = unit_price_str
                .trim_start_matches('$')
                .replace(',', "")
                .parse::<f64>()
                .unwrap_or(0.0);
            let line_subtotal = unit_price * quantity as f64;

            serde_json::json!({
                "index": idx + 1,
                "product_name": product_name,
                "quantity": quantity,
                "unit_price": unit_price_str,
                "line_subtotal": format_price(line_subtotal),
                "line_total": total_str,
                "calculation": format!("{} × {} = {}", quantity, unit_price_str, format_price(line_subtotal)),
                "rule_id": format!("LINE-{:03}", idx + 1),
                "source": "quote_line",
            })
        })
        .collect();

    // Build discount rationale if applicable
    let discount_rationale = if discount_total > 0.0 {
        let discount_pct = if subtotal > 0.0 { (discount_total / subtotal) * 100.0 } else { 0.0 };
        Some(serde_json::json!({
            "amount": format_price(discount_total),
            "percentage": format!("{:.1}%", discount_pct),
            "calculation": format!("{} × {:.1}% = {}", format_price(subtotal), discount_pct, format_price(discount_total)),
            "rule_id": "DISCOUNT-001",
            "source": "line_item_discounts",
            "description": "Sum of per-line discounts",
        }))
    } else {
        None
    };

    // Build tax rationale
    let taxable_amount = subtotal - discount_total;
    let tax_rationale = if tax_total > 0.0 {
        Some(serde_json::json!({
            "amount": format_price(tax_total),
            "rate": format!("{:.0}%", tax_rate * 100.0),
            "taxable_amount": format_price(taxable_amount),
            "calculation": format!("{} × {:.0}% = {}", format_price(taxable_amount), tax_rate * 100.0, format_price(tax_total)),
            "rule_id": "TAX-001",
            "source": "tax_rate_configuration",
            "description": "Tax applied to discounted subtotal",
        }))
    } else {
        Some(serde_json::json!({
            "amount": "$0.00",
            "rate": format!("{:.0}%", tax_rate * 100.0),
            "taxable_amount": format_price(taxable_amount),
            "calculation": "Tax not applicable or rate is 0%",
            "rule_id": "TAX-EXEMPT",
            "source": "tax_exemption",
            "description": "No tax applied (exempt or 0% rate)",
        }))
    };

    // Build total calculation chain
    let total_calculation = if discount_total > 0.0 && tax_total > 0.0 {
        format!(
            "{} - {} + {} = {}",
            format_price(subtotal),
            format_price(discount_total),
            format_price(tax_total),
            format_price(total)
        )
    } else if discount_total > 0.0 {
        format!(
            "{} - {} = {}",
            format_price(subtotal),
            format_price(discount_total),
            format_price(total)
        )
    } else if tax_total > 0.0 {
        format!(
            "{} + {} = {}",
            format_price(subtotal),
            format_price(tax_total),
            format_price(total)
        )
    } else {
        format!("{} = {}", format_price(subtotal), format_price(total))
    };

    // Extract provenance from pricing trace if available
    let provenance = pricing_trace.map(|trace| {
        serde_json::json!({
            "priced_at": trace.get("priced_at").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            "priced_by": trace.get("priced_by").and_then(|v| v.as_str()).unwrap_or("Unknown"),
            "reason": trace.get("reason").and_then(|v| v.as_str()).unwrap_or("Standard pricing"),
            "snapshot_available": has_snapshot,
        })
    }).unwrap_or_else(|| {
        serde_json::json!({
            "priced_at": "Unknown",
            "priced_by": "fallback_calculation",
            "reason": if has_snapshot { "Pricing snapshot" } else { "Computed from line items (no snapshot)" },
            "snapshot_available": has_snapshot,
        })
    });

    serde_json::json!({
        "summary": {
            "subtotal": format_price(subtotal),
            "discount_total": format_price(discount_total),
            "tax_total": format_price(tax_total),
            "total": format_price(total),
            "calculation_chain": total_calculation,
        },
        "line_items": line_rationales,
        "discount": discount_rationale,
        "tax": tax_rationale,
        "provenance": provenance,
        "has_detailed_trace": pricing_trace.is_some(),
    })
}

async fn portal_manifest() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/manifest+json; charset=utf-8")],
        include_str!("../../../templates/portal/manifest.webmanifest"),
    )
}

async fn portal_service_worker() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        include_str!("../../../templates/portal/sw.js"),
    )
}

// ---------------------------------------------------------------------------
// JSON API Handlers
// ---------------------------------------------------------------------------

async fn subscribe_push(
    State(state): State<PortalState>,
    Json(body): Json<PushSubscriptionRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<PortalError>)> {
    ensure_push_subscription_table(&state.db_pool).await.map_err(db_error)?;

    let endpoint = body.endpoint.trim();
    let p256dh = body.p256dh.trim();
    let auth = body.auth.trim();
    if endpoint.is_empty() || p256dh.is_empty() || auth.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation(
                "push subscription",
                "endpoint, p256dh, and auth are required",
            )),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let subscription_id = format!("PUSH-{}", &uuid_v4()[..12]);
    sqlx::query(
        "INSERT INTO portal_push_subscription
            (id, endpoint, p256dh, auth, user_agent, device_label, revoked, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?)
         ON CONFLICT(endpoint) DO UPDATE SET
            p256dh = excluded.p256dh,
            auth = excluded.auth,
            user_agent = excluded.user_agent,
            device_label = excluded.device_label,
            revoked = 0,
            updated_at = excluded.updated_at",
    )
    .bind(subscription_id)
    .bind(endpoint)
    .bind(p256dh)
    .bind(auth)
    .bind(body.user_agent.as_deref().map(str::trim).filter(|value| !value.is_empty()))
    .bind(body.device_label.as_deref().map(str::trim).filter(|value| !value.is_empty()))
    .bind(&now)
    .bind(&now)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    record_audit_event(
        &state.db_pool,
        None,
        "portal.push.subscription.created",
        "Portal push subscription registered",
    )
    .await;

    Ok(Json(PortalResponse {
        success: true,
        message: "Push notifications enabled for this device".to_string(),
    }))
}

async fn unsubscribe_push(
    State(state): State<PortalState>,
    Json(body): Json<PushUnsubscribeRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<PortalError>)> {
    ensure_push_subscription_table(&state.db_pool).await.map_err(db_error)?;

    let endpoint = body.endpoint.trim();
    if endpoint.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation("endpoint", "is required")),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE portal_push_subscription
         SET revoked = 1, updated_at = ?
         WHERE endpoint = ? AND revoked = 0",
    )
    .bind(&now)
    .bind(endpoint)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(PortalError::not_found("push subscription"))));
    }

    record_audit_event(
        &state.db_pool,
        None,
        "portal.push.subscription.revoked",
        "Portal push subscription revoked",
    )
    .await;

    Ok(Json(PortalResponse {
        success: true,
        message: "Push notifications disabled for this device".to_string(),
    }))
}

async fn approve_quote(
    Path(token): Path<String>,
    State(state): State<PortalState>,
    headers: HeaderMap,
    Json(body): Json<ApproveRequest>,
) -> Result<Json<PortalResponse>, (StatusCode, Json<PortalError>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token).await?;

    // Validate required fields
    let approver_name = body.approver_name.trim();
    let approver_email = body.approver_email.trim();
    if approver_name.is_empty() || approver_email.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation("approver info", "name and email are required")),
        ));
    }
    let auth_method =
        match body.auth_method.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            Some(method) => validate_portal_approval_auth(
                method,
                body.biometric_assertion.as_deref(),
                body.fallback_password.as_deref(),
            )?,
            None => ApprovalActionAuth::None,
        };
    let auth_context = portal_approval_auth_context(auth_method, approver_name, approver_email);

    let now = Utc::now();
    let quote_version: i64 = sqlx::query_scalar("SELECT version FROM quote WHERE id = ?")
        .bind(&quote_id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(db_error)?
        .unwrap_or(1);
    let requester_ip = extract_requester_ip(&headers);
    let approval_metadata = serde_json::json!({
        "comments": body.comments.as_deref().unwrap_or(""),
        "approver_name": approver_name,
        "approver_email": approver_email,
        "quote_version": quote_version,
        "requester_ip": requester_ip,
        "captured_at": now.to_rfc3339(),
        "auth_method": auth_method.as_str(),
        "biometric_assertion_present": body
            .biometric_assertion
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty()),
        "fallback_password_used": matches!(auth_method, ApprovalActionAuth::Password),
        "auth_context": auth_context,
    });
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
    .bind(approval_metadata.to_string())
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
    record_audit_event_with_auth(
        &state.db_pool,
        Some(&quote_id),
        "portal.approval",
        &format!(
            "Quote approved by {} ({}) via web portal [version={}, ip={}, auth={}]",
            approver_name,
            approver_email,
            quote_version,
            requester_ip,
            auth_method.as_str()
        ),
        Some(&auth_context),
    )
    .await;

    // Funnel telemetry: approval action
    let funnel_actor = auth_context.principal.actor_id.clone();
    record_funnel_event(
        &state.db_pool,
        quotey_core::audit::funnel::APPROVAL_ACTION,
        Some(&quote_id),
        &funnel_actor,
        "success",
        &[("action", "approved")],
    )
    .await;

    info!(
        event_name = "portal.quote.approved",
        correlation_id = %approval_id,
        quote_id = %quote_id,
        approver_name = %approver_name,
        approver_email = %approver_email,
        requester_ip = %requester_ip,
        quote_version = %quote_version,
        auth_method = %auth_method.as_str(),
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
            Json(PortalError::validation("reason", "rejection reason is required")),
        ));
    }
    let auth_method =
        match body.auth_method.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
            Some(method) => validate_portal_approval_auth(
                method,
                body.biometric_assertion.as_deref(),
                body.fallback_password.as_deref(),
            )?,
            None => ApprovalActionAuth::None,
        };
    let auth_context = portal_rejection_auth_context(auth_method);

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
    record_audit_event_with_auth(
        &state.db_pool,
        Some(&quote_id),
        "portal.rejection",
        &format!("Quote declined via web portal: {reason} [auth={}]", auth_method.as_str()),
        Some(&auth_context),
    )
    .await;

    // Funnel telemetry: rejection action
    let funnel_actor = auth_context.principal.actor_id.clone();
    record_funnel_event(
        &state.db_pool,
        quotey_core::audit::funnel::APPROVAL_ACTION,
        Some(&quote_id),
        &funnel_actor,
        "rejected",
        &[("action", "rejected")],
    )
    .await;

    info!(
        event_name = "portal.quote.rejected",
        correlation_id = %rejection_id,
        quote_id = %quote_id,
        auth_method = %auth_method.as_str(),
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
            Json(PortalError::validation("comment text", "comment text is required")),
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
            return Err((StatusCode::NOT_FOUND, Json(PortalError::not_found("parent comment"))));
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
        Some(&quote_id),
        "portal.comment",
        &format!("Customer comment: {text}"),
    )
    .await;

    // Funnel telemetry: comment added
    record_funnel_event(
        &state.db_pool,
        quotey_core::audit::funnel::COMMENT_ADDED,
        Some(&quote_id),
        &format!("portal:{author_email}"),
        "success",
        &[("comment_type", "overall")],
    )
    .await;

    info!(
        event_name = "portal.quote.comment",
        quote_id = %quote_id,
        "customer comment added via web portal"
    );

    notify_rep_about_comment(&state, &quote_id, "overall", None, &author_name, &author_email, text)
        .await;

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
            let map_err = |_err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(PortalError::service_unavailable("comment loader")),
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
            Json(PortalError::validation("comment text", "comment text is required")),
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
        return Err((StatusCode::NOT_FOUND, Json(PortalError::not_found("quote line"))));
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
            return Err((StatusCode::NOT_FOUND, Json(PortalError::not_found("parent comment"))));
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
        Some(&quote_id),
        "portal.comment.line",
        &format!("Customer comment on line {line_id}: {text}"),
    )
    .await;

    // Funnel telemetry: line comment added
    record_funnel_event(
        &state.db_pool,
        quotey_core::audit::funnel::COMMENT_ADDED,
        Some(&quote_id),
        &format!("portal:{author_email}"),
        "success",
        &[("comment_type", "line_item"), ("line_id", &line_id)],
    )
    .await;

    notify_rep_about_comment(
        &state,
        &quote_id,
        "line_item",
        Some(&line_id),
        &author_name,
        &author_email,
        text,
    )
    .await;

    Ok(Json(PortalResponse {
        success: true,
        message: format!("Comment on line {line_id} recorded. Your sales rep will be notified."),
    }))
}

/// Update quote assumptions and recalculate totals.
///
/// This endpoint allows portal users to override assumed values (tax rate,
/// payment terms, billing country, currency) and see updated totals in real-time.
async fn update_assumptions(
    Path(token): Path<String>,
    State(state): State<PortalState>,
    Json(body): Json<UpdateAssumptionsRequest>,
) -> Result<Json<AssumptionsUpdateResponse>, (StatusCode, Json<PortalError>)> {
    let quote_id = resolve_quote_by_token(&state.db_pool, &token).await?;

    // Fetch current quote data
    let current = sqlx::query(
        "SELECT currency, tax_rate_value, payment_terms, billing_country,
                currency_explicit, tax_rate_explicit, payment_terms_explicit, billing_country_explicit,
                version
         FROM quote WHERE id = ?",
    )
    .bind(&quote_id)
    .fetch_one(&state.db_pool)
    .await
    .map_err(db_error)?;

    // Get current values (use existing if not provided in request)
    let new_tax_rate =
        body.tax_rate.unwrap_or_else(|| current.try_get::<f64, _>("tax_rate_value").unwrap_or(0.0));
    let new_payment_terms = body.payment_terms.unwrap_or_else(|| {
        current.try_get::<String, _>("payment_terms").unwrap_or_else(|_| "net_30".to_string())
    });
    let new_billing_country = body
        .billing_country
        .or_else(|| current.try_get::<Option<String>, _>("billing_country").unwrap_or(None));
    let new_currency = body.currency.unwrap_or_else(|| {
        current.try_get::<String, _>("currency").unwrap_or_else(|_| "USD".to_string())
    });

    // Validate payment terms
    let valid_payment_terms = ["net_30", "net_60", "net_90", "upfront"];
    if !valid_payment_terms.contains(&new_payment_terms.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation(
                "payment_terms",
                "must be one of: net_30, net_60, net_90, upfront",
            )),
        ));
    }

    // Validate tax rate (0-100%)
    if !(0.0..=1.0).contains(&new_tax_rate) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation("tax_rate", "must be between 0.0 and 1.0 (0% to 100%)")),
        ));
    }

    let now = Utc::now();

    // Update quote with explicit values
    sqlx::query(
        "UPDATE quote SET
            tax_rate_value = ?,
            tax_rate_explicit = 1,
            payment_terms = ?,
            payment_terms_explicit = 1,
            billing_country = ?,
            billing_country_explicit = 1,
            currency = ?,
            currency_explicit = 1,
            updated_at = ?
         WHERE id = ?",
    )
    .bind(new_tax_rate)
    .bind(&new_payment_terms)
    .bind(&new_billing_country)
    .bind(&new_currency)
    .bind(now.to_rfc3339())
    .bind(&quote_id)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    // Fetch quote lines to recalculate totals
    let lines = sqlx::query(
        "SELECT quantity, unit_price, subtotal, discount_pct
         FROM quote_line WHERE quote_id = ?",
    )
    .bind(&quote_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(db_error)?;

    // Recalculate totals
    let mut subtotal = 0.0_f64;
    let mut discount_total = 0.0_f64;
    for row in &lines {
        let qty: i64 = row.try_get("quantity").unwrap_or(0);
        let unit_price: f64 = row.try_get("unit_price").unwrap_or(0.0);
        let line_subtotal = match row.try_get::<Option<f64>, _>("subtotal") {
            Ok(Some(value)) => value,
            _ => unit_price * qty as f64,
        };
        let discount_pct: f64 =
            row.try_get::<f64, _>("discount_pct").unwrap_or(0.0).clamp(0.0, 100.0);
        subtotal += line_subtotal;
        discount_total += line_subtotal * discount_pct / 100.0;
    }

    let discounted_subtotal = subtotal - discount_total;
    let tax_amount = discounted_subtotal * new_tax_rate;
    let total = discounted_subtotal + tax_amount;

    // Update or create pricing snapshot with new totals
    let snapshot_id = format!("PSNAP-{}", &uuid_v4()[..12]);
    let version: i64 = current.try_get("version").unwrap_or(1);

    // Delete old snapshot for this version (assumption updates create new pricing state)
    sqlx::query("DELETE FROM quote_pricing_snapshot WHERE quote_id = ? AND version = ?")
        .bind(&quote_id)
        .bind(version)
        .execute(&state.db_pool)
        .await
        .map_err(db_error)?;

    // Create new snapshot
    let pricing_trace = serde_json::json!({
        "quote_id": &quote_id,
        "version": version,
        "priced_at": now.to_rfc3339(),
        "priced_by": "portal_assumption_update",
        "reason": "User updated assumptions via portal",
        "lines": lines.len(),
        "tax_rate": new_tax_rate,
        "payment_terms": &new_payment_terms,
    });

    sqlx::query(
        "INSERT INTO quote_pricing_snapshot
            (id, quote_id, version, subtotal, discount_total, tax_total, total, currency, pricing_trace_json, priced_at, priced_by)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&snapshot_id)
    .bind(&quote_id)
    .bind(version)
    .bind(subtotal)
    .bind(discount_total)
    .bind(tax_amount)
    .bind(total)
    .bind(&new_currency)
    .bind(pricing_trace.to_string())
    .bind(now.to_rfc3339())
    .bind("portal")
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    // Record audit event
    let changes = serde_json::json!({
        "tax_rate": new_tax_rate,
        "payment_terms": &new_payment_terms,
        "billing_country": &new_billing_country,
        "currency": &new_currency,
    });
    record_audit_event(
        &state.db_pool,
        Some(&quote_id),
        "portal.assumptions_updated",
        &format!("Assumptions updated via portal: {}", changes),
    )
    .await;

    // Funnel telemetry: assumption review
    record_funnel_event(
        &state.db_pool,
        quotey_core::audit::funnel::ASSUMPTION_REVIEW,
        Some(&quote_id),
        "portal:customer",
        "success",
        &[],
    )
    .await;

    info!(
        event_name = "portal.assumptions.updated",
        quote_id = %quote_id,
        tax_rate = %new_tax_rate,
        "assumptions updated via portal"
    );

    // Build assumptions list for response
    let assumptions = serde_json::json!([
        {
            "field": "currency",
            "value": &new_currency,
            "explicit": true,
            "label": "Currency",
        },
        {
            "field": "tax_rate",
            "value": format!("{:.1}%", new_tax_rate * 100.0),
            "explicit": true,
            "label": "Tax Rate",
        },
        {
            "field": "payment_terms",
            "value": &new_payment_terms,
            "explicit": true,
            "label": "Payment Terms",
        },
        {
            "field": "billing_country",
            "value": new_billing_country.as_deref().unwrap_or("Not specified"),
            "explicit": new_billing_country.is_some(),
            "label": "Billing Country",
        },
    ]);

    let totals = serde_json::json!({
        "subtotal": format_price(subtotal),
        "subtotal_raw": subtotal,
        "discount_total": format_price(discount_total),
        "discount_total_raw": discount_total,
        "tax_amount": format_price(tax_amount),
        "tax_amount_raw": tax_amount,
        "tax_rate": new_tax_rate,
        "total": format_price(total),
        "total_raw": total,
        "currency": &new_currency,
    });

    Ok(Json(AssumptionsUpdateResponse {
        success: true,
        message: "Assumptions updated successfully. Quote totals have been recalculated."
            .to_string(),
        quote_id,
        assumptions,
        totals,
        has_assumptions: new_billing_country.is_none(),
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
        return Err((StatusCode::NOT_FOUND, Json(PortalError::not_found("quote"))));
    }

    let now = Utc::now();
    let expires_in_days = body.expires_in_days.unwrap_or(30).clamp(1, 365);
    let expires_at = now + chrono::Duration::days(expires_in_days as i64);
    let link_id = format!("PL-{}", &uuid_v4()[..12]);
    let token = generate_token();
    let handoff = body.normalized_handoff();
    let share_url = build_quote_share_url(&token, &handoff);
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
        Some(quote_id),
        "portal.link_created",
        &format!("Portal link generated (expires in {expires_in_days} days)"),
    )
    .await;

    // Funnel telemetry: session start (link creation initiates customer funnel)
    record_funnel_event(
        &state.db_pool,
        quotey_core::audit::funnel::SESSION_START,
        Some(quote_id),
        &format!("portal:{created_by}"),
        "success",
        &[],
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
        share_url,
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
            Json(PortalError::validation("token", "token is required")),
        ));
    }

    let result = sqlx::query("UPDATE portal_link SET revoked = 1 WHERE token = ? AND revoked = 0")
        .bind(token)
        .execute(&state.db_pool)
        .await
        .map_err(db_error)?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, Json(PortalError::not_found("portal link"))));
    }

    info!(event_name = "portal.link.revoked", "portal sharing link revoked");

    Ok(Json(PortalResponse { success: true, message: "Link revoked successfully.".to_string() }))
}

async fn regenerate_link(
    State(state): State<PortalState>,
    Json(body): Json<CreateLinkRequest>,
) -> Result<Json<LinkResponse>, (StatusCode, Json<PortalError>)> {
    let result = create_link(State(state.clone()), Json(body)).await?;

    record_audit_event(
        &state.db_pool,
        Some(&result.0.quote_id),
        "portal.link_regenerated",
        "Portal link regenerated and prior active links revoked",
    )
    .await;

    Ok(result)
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
        .map(|r| {
            let token = r.try_get::<String, _>("token").unwrap_or_default();
            LinkResponse {
                link_id: r.try_get("id").unwrap_or_default(),
                share_url: build_quote_share_url(&token, &PortalHandoffQuery::default()),
                token,
                quote_id: r.try_get("quote_id").unwrap_or_default(),
                expires_at: r.try_get("expires_at").unwrap_or_default(),
            }
        })
        .collect();

    Ok(Json(links))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// CSV Export
// ---------------------------------------------------------------------------

const DIGEST_SCHEDULE_SINGLETON_ID: i64 = 1;
const DIGEST_DAYS: [&str; 7] =
    ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"];

#[derive(Debug, Deserialize)]
struct DigestScheduleRequest {
    enabled: bool,
    day_of_week: String,
    time_utc: String,
    #[serde(default)]
    recipient_email: Option<String>,
}

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
struct DigestScheduleResponse {
    enabled: bool,
    day_of_week: String,
    time_utc: String,
    recipient_email: Option<String>,
    updated_at: String,
}

#[derive(Debug, Deserialize, Default)]
struct DigestDispatchRequest {
    #[serde(default)]
    force: bool,
}

#[derive(Debug, Serialize)]
struct DigestDispatchResponse {
    executed: bool,
    status: String,
    reason: String,
    recipient_email: Option<String>,
    week_key: String,
    sent_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct DigestMetrics {
    total_quotes: i64,
    approved_quotes_last_7_days: i64,
    pending_quotes: i64,
    total_pipeline_value: f64,
}

fn normalize_digest_day(raw: &str) -> Option<String> {
    let normalized = raw.trim().to_ascii_lowercase();
    if DIGEST_DAYS.contains(&normalized.as_str()) {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_digest_time(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let (hour_raw, minute_raw) = trimmed.split_once(':')?;
    if hour_raw.len() != 2 || minute_raw.len() != 2 {
        return None;
    }
    let hour = hour_raw.parse::<u8>().ok()?;
    let minute = minute_raw.parse::<u8>().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(format!("{hour:02}:{minute:02}"))
}

fn digest_time_components(time_utc: &str) -> Option<(u32, u32)> {
    let (hour, minute) = time_utc.split_once(':')?;
    let hour = hour.parse::<u32>().ok()?;
    let minute = minute.parse::<u32>().ok()?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some((hour, minute))
}

fn weekday_name(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Mon => "monday",
        chrono::Weekday::Tue => "tuesday",
        chrono::Weekday::Wed => "wednesday",
        chrono::Weekday::Thu => "thursday",
        chrono::Weekday::Fri => "friday",
        chrono::Weekday::Sat => "saturday",
        chrono::Weekday::Sun => "sunday",
    }
}

fn digest_is_due(schedule: &DigestScheduleResponse, now: chrono::DateTime<Utc>) -> bool {
    if weekday_name(now.weekday()) != schedule.day_of_week {
        return false;
    }

    let Some((schedule_hour, schedule_minute)) = digest_time_components(&schedule.time_utc) else {
        return false;
    };
    let now_minutes = now.hour() * 60 + now.minute();
    let schedule_minutes = schedule_hour * 60 + schedule_minute;
    now_minutes >= schedule_minutes
}

fn digest_week_key(now: chrono::DateTime<Utc>) -> String {
    let week = now.iso_week();
    format!("{}-W{:02}", week.year(), week.week())
}

async fn get_or_create_digest_schedule(
    pool: &DbPool,
) -> Result<DigestScheduleResponse, (StatusCode, Json<PortalError>)> {
    ensure_digest_schedule_table(pool).await.map_err(db_error)?;

    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO analytics_digest_schedule
            (id, enabled, day_of_week, time_utc, recipient_email, updated_at)
         VALUES (?, 0, 'monday', '09:00', NULL, ?)
         ON CONFLICT(id) DO NOTHING",
    )
    .bind(DIGEST_SCHEDULE_SINGLETON_ID)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(db_error)?;

    let row = sqlx::query(
        "SELECT enabled, day_of_week, time_utc, recipient_email, updated_at
         FROM analytics_digest_schedule
         WHERE id = ?",
    )
    .bind(DIGEST_SCHEDULE_SINGLETON_ID)
    .fetch_one(pool)
    .await
    .map_err(db_error)?;

    Ok(DigestScheduleResponse {
        enabled: row.try_get::<i64, _>("enabled").unwrap_or(0) == 1,
        day_of_week: row
            .try_get::<String, _>("day_of_week")
            .unwrap_or_else(|_| "monday".to_string()),
        time_utc: row.try_get::<String, _>("time_utc").unwrap_or_else(|_| "09:00".to_string()),
        recipient_email: row.try_get::<Option<String>, _>("recipient_email").unwrap_or(None),
        updated_at: row.try_get::<String, _>("updated_at").unwrap_or(now),
    })
}

async fn get_digest_schedule(
    State(state): State<PortalState>,
) -> Result<Json<DigestScheduleResponse>, (StatusCode, Json<PortalError>)> {
    let schedule = get_or_create_digest_schedule(&state.db_pool).await?;
    Ok(Json(schedule))
}

async fn upsert_digest_schedule(
    State(state): State<PortalState>,
    Json(body): Json<DigestScheduleRequest>,
) -> Result<Json<DigestScheduleResponse>, (StatusCode, Json<PortalError>)> {
    let day_of_week = normalize_digest_day(&body.day_of_week).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation(
                "day_of_week",
                "day_of_week must be a valid weekday name (monday..sunday)",
            )),
        )
    })?;
    let time_utc = normalize_digest_time(&body.time_utc).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation("time_utc", "time_utc must use 24h HH:MM format in UTC")),
        )
    })?;

    let recipient_email = body
        .recipient_email
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    if body.enabled && recipient_email.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(PortalError::validation(
                "recipient_email",
                "recipient_email is required when digest scheduling is enabled",
            )),
        ));
    }

    ensure_digest_schedule_table(&state.db_pool).await.map_err(db_error)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO analytics_digest_schedule
            (id, enabled, day_of_week, time_utc, recipient_email, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
            enabled = excluded.enabled,
            day_of_week = excluded.day_of_week,
            time_utc = excluded.time_utc,
            recipient_email = excluded.recipient_email,
            updated_at = excluded.updated_at",
    )
    .bind(DIGEST_SCHEDULE_SINGLETON_ID)
    .bind(if body.enabled { 1_i64 } else { 0_i64 })
    .bind(&day_of_week)
    .bind(&time_utc)
    .bind(recipient_email.as_deref())
    .bind(&now)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    record_audit_event(
        &state.db_pool,
        None,
        "portal.analytics.digest_schedule.updated",
        &format!(
            "digest schedule updated: enabled={}, day={}, time_utc={}",
            body.enabled, day_of_week, time_utc
        ),
    )
    .await;

    Ok(Json(DigestScheduleResponse {
        enabled: body.enabled,
        day_of_week,
        time_utc,
        recipient_email,
        updated_at: now,
    }))
}

async fn build_digest_metrics(pool: &DbPool, now: chrono::DateTime<Utc>) -> DigestMetrics {
    let total_quotes: i64 =
        match sqlx::query_scalar("SELECT COUNT(*) FROM quote").fetch_one(pool).await {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, "digest metrics fallback: total_quotes");
                0
            }
        };

    let approved_since = (now - Duration::days(7)).to_rfc3339();
    let approved_quotes_last_7_days: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM quote WHERE status = 'approved' AND created_at >= ?",
    )
    .bind(&approved_since)
    .fetch_one(pool)
    .await
    {
        Ok(value) => value,
        Err(error) => {
            warn!(error = %error, "digest metrics fallback: approved_quotes_last_7_days");
            0
        }
    };

    let pending_quotes: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM quote WHERE status IN ('draft', 'pending', 'sent')",
    )
    .fetch_one(pool)
    .await
    {
        Ok(value) => value,
        Err(error) => {
            warn!(error = %error, "digest metrics fallback: pending_quotes");
            0
        }
    };

    let total_pipeline_value: f64 = match sqlx::query_scalar(
        "SELECT COALESCE(SUM(ps.total), 0)
         FROM quote q
         LEFT JOIN quote_pricing_snapshot ps ON ps.quote_id = q.id AND ps.version = q.version
         WHERE q.status IN ('draft', 'pending', 'sent')",
    )
    .fetch_one(pool)
    .await
    {
        Ok(value) => value,
        Err(error) => {
            warn!(error = %error, "digest metrics fallback: total_pipeline_value");
            0.0
        }
    };

    DigestMetrics {
        total_quotes,
        approved_quotes_last_7_days,
        pending_quotes,
        total_pipeline_value,
    }
}

async fn send_digest_via_webhook(
    webhook_url: &str,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(webhook_url)
        .json(payload)
        .send()
        .await
        .map_err(|error| format!("request_failed: {error}"))?;

    if !response.status().is_success() {
        return Err(format!("http_status={}", response.status()));
    }

    Ok(())
}

async fn run_digest_dispatch(
    State(state): State<PortalState>,
    Json(body): Json<DigestDispatchRequest>,
) -> Result<Json<DigestDispatchResponse>, (StatusCode, Json<PortalError>)> {
    let schedule = get_or_create_digest_schedule(&state.db_pool).await?;
    ensure_digest_delivery_table(&state.db_pool).await.map_err(db_error)?;

    let now = Utc::now();
    let now_rfc3339 = now.to_rfc3339();
    let week_key = digest_week_key(now);
    let recipient_email = schedule.recipient_email.clone();

    if !schedule.enabled {
        return Ok(Json(DigestDispatchResponse {
            executed: false,
            status: "skipped".to_string(),
            reason: "digest scheduling is disabled".to_string(),
            recipient_email,
            week_key,
            sent_at: None,
        }));
    }

    let Some(recipient) = recipient_email.clone() else {
        return Ok(Json(DigestDispatchResponse {
            executed: false,
            status: "skipped".to_string(),
            reason: "recipient_email is not configured".to_string(),
            recipient_email: None,
            week_key,
            sent_at: None,
        }));
    };

    if !body.force && !digest_is_due(&schedule, now) {
        return Ok(Json(DigestDispatchResponse {
            executed: false,
            status: "skipped".to_string(),
            reason: "digest is not due yet for the configured schedule".to_string(),
            recipient_email: Some(recipient),
            week_key,
            sent_at: None,
        }));
    }

    if !body.force {
        let already_sent: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM analytics_digest_delivery
             WHERE week_key = ? AND recipient_email = ? AND status = 'sent'",
        )
        .bind(&week_key)
        .bind(&recipient)
        .fetch_one(&state.db_pool)
        .await
        .map_err(db_error)?;

        if already_sent > 0 {
            return Ok(Json(DigestDispatchResponse {
                executed: false,
                status: "skipped".to_string(),
                reason: "digest already sent for this recipient/week".to_string(),
                recipient_email: Some(recipient),
                week_key,
                sent_at: None,
            }));
        }
    }

    let metrics = build_digest_metrics(&state.db_pool, now).await;
    let base_url = std::env::var("QUOTEY_PORTAL_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string());
    let export_link = format!("{base_url}/api/v1/portal/export/quotes?days=7");
    let dashboard_link = format!("{base_url}/portal");

    let digest_payload = serde_json::json!({
        "recipient_email": recipient,
        "subject": format!("Quotey weekly digest ({week_key})"),
        "body": format!(
            "Weekly Quotey digest\n\nTotal quotes: {}\nApproved last 7 days: {}\nPending quotes: {}\nPipeline value: {:.2}\n\nDashboard: {}\nCSV export (7d): {}",
            metrics.total_quotes,
            metrics.approved_quotes_last_7_days,
            metrics.pending_quotes,
            metrics.total_pipeline_value,
            dashboard_link,
            export_link
        ),
        "metrics": metrics,
        "links": {
            "dashboard": dashboard_link,
            "export_csv_7d": export_link
        },
        "schedule": {
            "day_of_week": schedule.day_of_week,
            "time_utc": schedule.time_utc,
            "force": body.force
        }
    });

    let webhook_url = std::env::var("QUOTEY_ANALYTICS_DIGEST_WEBHOOK_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let (status, reason, executed) = match webhook_url {
        Some(url) => match send_digest_via_webhook(&url, &digest_payload).await {
            Ok(()) => ("sent".to_string(), "digest delivered via webhook".to_string(), true),
            Err(error) => ("failed".to_string(), format!("digest delivery failed: {error}"), false),
        },
        None => (
            "skipped".to_string(),
            "missing QUOTEY_ANALYTICS_DIGEST_WEBHOOK_URL".to_string(),
            false,
        ),
    };

    let delivery_id = format!("PDIG-{}", &uuid_v4()[..12]);
    sqlx::query(
        "INSERT INTO analytics_digest_delivery
            (id, week_key, recipient_email, status, reason, payload_json, sent_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&delivery_id)
    .bind(&week_key)
    .bind(recipient_email.as_deref())
    .bind(&status)
    .bind(&reason)
    .bind(digest_payload.to_string())
    .bind(&now_rfc3339)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    let event_type = match status.as_str() {
        "sent" => "portal.analytics.digest.sent",
        "failed" => "portal.analytics.digest.failed",
        _ => "portal.analytics.digest.skipped",
    };
    record_audit_event(
        &state.db_pool,
        None,
        event_type,
        &format!("status={status}; reason={reason}; week_key={week_key}"),
    )
    .await;

    Ok(Json(DigestDispatchResponse {
        executed,
        status,
        reason,
        recipient_email,
        week_key,
        sent_at: Some(now_rfc3339),
    }))
}

#[derive(Debug, Deserialize, Default)]
struct ExportQuotesQuery {
    /// Optional start date (ISO 8601) for filtering
    start: Option<String>,
    /// Optional end date (ISO 8601) for filtering
    end: Option<String>,
    /// Optional relative lookback window in days when explicit start/end are absent
    days: Option<i64>,
    /// Optional status filter
    status: Option<String>,
    /// Optional comma-separated list of columns in desired output order.
    /// Example: `quote_id,total,status`
    columns: Option<String>,
}

const EXPORT_DEFAULT_COLUMNS: [&str; 12] = [
    "quote_id",
    "status",
    "currency",
    "created_by",
    "account_id",
    "created_at",
    "updated_at",
    "valid_until",
    "subtotal",
    "discount_total",
    "tax_total",
    "total",
];

fn parse_export_columns(raw: Option<&str>) -> Result<Vec<String>, PortalError> {
    let Some(raw_columns) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(EXPORT_DEFAULT_COLUMNS.iter().map(|column| (*column).to_string()).collect());
    };

    let mut selected = Vec::new();
    for candidate in raw_columns.split(',') {
        let normalized = candidate.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if !EXPORT_DEFAULT_COLUMNS.contains(&normalized.as_str()) {
            return Err(PortalError::validation(
                "columns",
                &format!(
                    "unsupported column '{normalized}'. Supported columns: {}",
                    EXPORT_DEFAULT_COLUMNS.join(", ")
                ),
            ));
        }
        if !selected.contains(&normalized) {
            selected.push(normalized);
        }
    }

    if selected.is_empty() {
        return Err(PortalError::validation(
            "columns",
            "at least one valid export column is required",
        ));
    }

    Ok(selected)
}

/// Export quotes as CSV with optional date range and status filter.
async fn export_quotes_csv(
    Query(params): Query<ExportQuotesQuery>,
    State(state): State<PortalState>,
) -> Result<impl IntoResponse, (StatusCode, Json<PortalError>)> {
    let selected_columns = parse_export_columns(params.columns.as_deref())
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(error)))?;

    let mut conditions = vec!["1=1".to_string()];
    let mut binds: Vec<String> = Vec::new();

    if let Some(ref start) = params.start {
        conditions.push("q.created_at >= ?".to_string());
        binds.push(start.clone());
    }
    if let Some(ref end) = params.end {
        conditions.push("q.created_at <= ?".to_string());
        binds.push(end.clone());
    }
    if params.start.is_none() && params.end.is_none() {
        if let Some(days) = params.days.filter(|value| *value > 0 && *value <= 3650) {
            let start = (Utc::now() - Duration::days(days)).to_rfc3339();
            conditions.push("q.created_at >= ?".to_string());
            binds.push(start);
        }
    }
    if let Some(ref status) = params.status {
        conditions.push("q.status = ?".to_string());
        binds.push(status.clone());
    }

    let sql = format!(
        "SELECT q.id, q.status, q.currency, q.created_by, q.account_id,
                q.created_at, q.updated_at, q.valid_until,
                COALESCE(ps.subtotal, 0) AS subtotal,
                COALESCE(ps.discount_total, 0) AS discount_total,
                COALESCE(ps.tax_total, 0) AS tax_total,
                COALESCE(ps.total, 0) AS total
         FROM quote q
         LEFT JOIN quote_pricing_snapshot ps ON ps.quote_id = q.id AND ps.version = q.version
         WHERE {}
         ORDER BY q.created_at DESC",
        conditions.join(" AND ")
    );

    let mut query = sqlx::query(&sql);
    for bind in &binds {
        query = query.bind(bind);
    }

    let rows = query.fetch_all(&state.db_pool).await.map_err(|e| {
        warn!(error = %e, "export_quotes_csv: database error");
        (StatusCode::INTERNAL_SERVER_ERROR, Json(PortalError::service_unavailable("database")))
    })?;

    let mut csv = format!("{}\n", selected_columns.join(","));

    for row in &rows {
        let mut values = Vec::with_capacity(selected_columns.len());
        for column in &selected_columns {
            let value = match column.as_str() {
                "quote_id" => row.try_get::<String, _>("id").unwrap_or_default(),
                "status" => row.try_get::<String, _>("status").unwrap_or_default(),
                "currency" => row.try_get::<String, _>("currency").unwrap_or_default(),
                "created_by" => row.try_get::<String, _>("created_by").unwrap_or_default(),
                "account_id" => row
                    .try_get::<Option<String>, _>("account_id")
                    .unwrap_or_default()
                    .unwrap_or_default(),
                "created_at" => row.try_get::<String, _>("created_at").unwrap_or_default(),
                "updated_at" => row.try_get::<String, _>("updated_at").unwrap_or_default(),
                "valid_until" => row
                    .try_get::<Option<String>, _>("valid_until")
                    .unwrap_or_default()
                    .unwrap_or_default(),
                "subtotal" => format!("{:.2}", row.try_get::<f64, _>("subtotal").unwrap_or(0.0)),
                "discount_total" => {
                    format!("{:.2}", row.try_get::<f64, _>("discount_total").unwrap_or(0.0))
                }
                "tax_total" => format!("{:.2}", row.try_get::<f64, _>("tax_total").unwrap_or(0.0)),
                "total" => format!("{:.2}", row.try_get::<f64, _>("total").unwrap_or(0.0)),
                _ => String::new(),
            };
            values.push(csv_escape(&value));
        }
        csv.push_str(&values.join(","));
        csv.push('\n');
    }

    let filename = format!("quotey-quotes-export-{}.csv", Utc::now().format("%Y%m%d-%H%M%S"));

    let mut headers = HeaderMap::new();
    headers
        .insert(header::CONTENT_TYPE, header::HeaderValue::from_static("text/csv; charset=utf-8"));
    let disposition = header::HeaderValue::from_str(&format!(
        "attachment; filename=\"{filename}\""
    ))
    .map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(PortalError::service_unavailable("response headers")),
        )
    })?;
    headers.insert(header::CONTENT_DISPOSITION, disposition);

    Ok((StatusCode::OK, headers, csv))
}

/// Escape a value for CSV output (RFC 4180 compliant).
fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

// ---------------------------------------------------------------------------
// Token Resolution
// ---------------------------------------------------------------------------

/// Resolve a sharing token to a quote ID.
///
/// Resolve a quote by portal link token. Strict token-only validation —
/// no fallback to raw quote ID. Returns the quote_id on success, or an
/// appropriate error with audit logging for all failure modes.
async fn resolve_quote_by_token(
    pool: &DbPool,
    token: &str,
) -> Result<String, (StatusCode, Json<PortalError>)> {
    let now = Utc::now().to_rfc3339();

    // Only accept valid portal_link tokens — no raw ID fallback
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

    // Check for expired/revoked link to give a specific error and audit trail
    let expired_row: Option<sqlx::sqlite::SqliteRow> =
        sqlx::query("SELECT quote_id, revoked, expires_at FROM portal_link WHERE token = ?")
            .bind(token)
            .fetch_optional(pool)
            .await
            .map_err(db_error)?;

    if let Some(r) = expired_row {
        let quote_id: String = r.try_get("quote_id").unwrap_or_default();
        let revoked: bool = r.try_get("revoked").unwrap_or(false);
        if revoked {
            warn!(
                token = %redact_token(token), quote_id = %quote_id,
                reason = "revoked",
                "portal: token access denied — link revoked"
            );
            record_audit_event(pool, Some(&quote_id), "portal.token_denied", "link revoked").await;
            return Err((
                StatusCode::GONE,
                Json(PortalError {
                    error: "this quote link has been revoked".to_string(),
                    category: Some(PortalErrorCategory::NotFound),
                    recovery_hint: Some("Contact the sender for a new link.".to_string()),
                    retry_after_seconds: None,
                }),
            ));
        }
        warn!(
            token = %redact_token(token), quote_id = %quote_id,
            reason = "expired",
            "portal: token access denied — link expired"
        );
        record_audit_event(pool, Some(&quote_id), "portal.token_denied", "link expired").await;
        return Err((StatusCode::GONE, Json(PortalError::expired())));
    }

    warn!(
        token = %redact_token(token),
        reason = "unknown_token",
        "portal: access denied — token not found"
    );
    record_audit_event(pool, None, "portal.token_denied", "unknown token").await;
    Err((StatusCode::NOT_FOUND, Json(PortalError::not_found("quote"))))
}

/// Record an audit event for traceability.
///
/// Uses the existing `audit_event` schema from migration 0001:
///   id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json
async fn record_audit_event(pool: &DbPool, quote_id: Option<&str>, event_type: &str, detail: &str) {
    record_audit_event_with_auth(pool, quote_id, event_type, detail, None).await;
}

async fn record_audit_event_with_auth(
    pool: &DbPool,
    quote_id: Option<&str>,
    event_type: &str,
    detail: &str,
    auth_context: Option<&AuthContext>,
) {
    let now = Utc::now();
    let audit_id = format!("PAUD-{}", &uuid_v4()[..12]);

    let payload = match auth_context {
        Some(context) => serde_json::json!({ "detail": detail, "auth": context }).to_string(),
        None => serde_json::json!({ "detail": detail }).to_string(),
    };
    let metadata_json = auth_context.map(|context| {
        serde_json::json!({
            "auth_channel": context.channel,
            "auth_method": context.method,
            "auth_strength": context.strength,
            "auth_principal": context.principal.actor_id,
            "auth_display_name": context.principal.display_name,
            "token_fingerprint": context.token_fingerprint,
            "session_id": context.session_id,
        })
        .to_string()
    });
    let actor = auth_context
        .map(|context| context.principal.actor_id.clone())
        .unwrap_or_else(|| "portal".to_string());
    let actor_type = if auth_context.is_some() { "human" } else { "system" };

    let result = sqlx::query(
        "INSERT INTO audit_event
            (id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json)
         VALUES (?, ?, ?, ?, ?, ?, 'portal', ?, ?)",
    )
    .bind(&audit_id)
    .bind(now.to_rfc3339())
    .bind(&actor)
    .bind(actor_type)
    .bind(quote_id)
    .bind(event_type)
    .bind(&payload)
    .bind(metadata_json)
    .execute(pool)
    .await;

    if let Err(e) = result {
        error!(
            event_name = "portal.audit.write_failed",
            quote_id = ?quote_id,
            error = %e,
            "failed to write portal audit event"
        );
    }
}

fn looks_like_slack_member_id(value: &str) -> bool {
    if value.len() < 9 {
        return false;
    }

    let mut chars = value.chars();
    match chars.next() {
        Some('U' | 'W') => chars.all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit()),
        _ => false,
    }
}

fn compact_comment_text(text: &str, max_len: usize) -> String {
    let compacted = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compacted.chars().count() <= max_len {
        return compacted;
    }

    let mut truncated = compacted.chars().take(max_len).collect::<String>();
    truncated.push_str("...");
    truncated
}

async fn send_slack_rep_notification(
    bot_token: &str,
    channel: &str,
    message: &str,
) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(bot_token)
        .json(&serde_json::json!({
            "channel": channel,
            "text": message,
            "mrkdwn": true,
            "unfurl_links": false,
            "unfurl_media": false
        }))
        .send()
        .await
        .map_err(|error| format!("request_failed: {error}"))?;

    let status = response.status();
    let payload: serde_json::Value =
        response.json().await.map_err(|error| format!("response_parse_failed: {error}"))?;

    if !status.is_success() {
        return Err(format!("slack_http_status={status}"));
    }

    if !payload.get("ok").and_then(serde_json::Value::as_bool).unwrap_or(false) {
        let api_error =
            payload.get("error").and_then(serde_json::Value::as_str).unwrap_or("unknown_error");
        return Err(format!("slack_api_error={api_error}"));
    }

    Ok(())
}

async fn notify_rep_about_comment(
    state: &PortalState,
    quote_id: &str,
    comment_type: &str,
    line_id: Option<&str>,
    author_name: &str,
    author_email: &str,
    text: &str,
) {
    let rep_id = match sqlx::query_scalar::<_, String>("SELECT created_by FROM quote WHERE id = ?")
        .bind(quote_id)
        .fetch_optional(&state.db_pool)
        .await
    {
        Ok(rep) => rep.unwrap_or_else(|| "unknown-rep".to_string()),
        Err(error) => {
            warn!(
                event_name = "portal.rep_notification.resolve_failed",
                quote_id = %quote_id,
                error = %error,
                "failed to resolve quote owner for rep notification"
            );
            record_audit_event(
                &state.db_pool,
                Some(quote_id),
                "portal.rep_notification.failed",
                "unable to resolve quote owner",
            )
            .await;
            return;
        }
    };

    let queue_detail = match line_id {
        Some(line) => {
            format!("queued rep notification ({comment_type}) for {rep_id} on line {line}")
        }
        None => format!("queued rep notification ({comment_type}) for {rep_id}"),
    };
    record_audit_event(
        &state.db_pool,
        Some(quote_id),
        "portal.rep_notification.queued",
        &queue_detail,
    )
    .await;

    let Some(bot_token) = &state.rep_notifications.slack_bot_token else {
        record_audit_event(
            &state.db_pool,
            Some(quote_id),
            "portal.rep_notification.skipped",
            "missing QUOTEY_SLACK_BOT_TOKEN/SLACK_BOT_TOKEN",
        )
        .await;
        return;
    };

    let destination = if looks_like_slack_member_id(&rep_id) {
        Some(rep_id.clone())
    } else {
        state.rep_notifications.fallback_channel.clone()
    };

    let Some(channel) = destination else {
        record_audit_event(
            &state.db_pool,
            Some(quote_id),
            "portal.rep_notification.skipped",
            "quote owner is not a Slack member id and no QUOTEY_PORTAL_REP_NOTIFICATION_CHANNEL is configured",
        )
        .await;
        return;
    };

    let comment_preview = compact_comment_text(text, 220);
    let scope = match line_id {
        Some(line) => format!("line `{line}`"),
        None => "overall quote".to_string(),
    };
    let message = format!(
        ":speech_balloon: New portal customer comment on quote `{quote_id}` ({scope})\nAuthor: {author_name} <{author_email}>\n>{comment_preview}"
    );

    match send_slack_rep_notification(bot_token, &channel, &message).await {
        Ok(()) => {
            record_audit_event(
                &state.db_pool,
                Some(quote_id),
                "portal.rep_notification.sent",
                &format!("delivered rep notification to {channel}"),
            )
            .await;
        }
        Err(error) => {
            warn!(
                event_name = "portal.rep_notification.failed",
                quote_id = %quote_id,
                channel = %channel,
                error = %error,
                "failed to deliver Slack rep notification"
            );
            record_audit_event(
                &state.db_pool,
                Some(quote_id),
                "portal.rep_notification.failed",
                &format!("delivery failed for {channel}: {error}"),
            )
            .await;
        }
    }
}

/// Persist a funnel telemetry event to the audit_event table.
///
/// Funnel events track UX transitions (view, approve, comment, etc.)
/// with schema-versioned metadata so drop-off can be measured per step.
async fn record_funnel_event(
    pool: &DbPool,
    event_type: &str,
    quote_id: Option<&str>,
    actor: &str,
    outcome: &str,
    extra: &[(&str, &str)],
) {
    use quotey_core::audit::{funnel, FUNNEL_SCHEMA_VERSION};

    let now = Utc::now();
    let audit_id = format!("PFUN-{}", &uuid_v4()[..12]);

    let mut metadata = serde_json::Map::new();
    metadata.insert("schema_version".into(), FUNNEL_SCHEMA_VERSION.into());
    metadata.insert("funnel_step".into(), event_type.into());
    metadata.insert(
        "funnel_ordinal".into(),
        serde_json::Value::Number(funnel::step_ordinal(event_type).into()),
    );
    metadata.insert("channel".into(), "portal".into());
    metadata.insert("outcome".into(), outcome.into());
    for (k, v) in extra {
        metadata.insert((*k).to_string(), (*v).into());
    }

    let payload = serde_json::Value::Object(metadata).to_string();

    let result = sqlx::query(
        "INSERT INTO audit_event
            (id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json)
         VALUES (?, ?, ?, 'portal', ?, ?, 'funnel', ?, ?)",
    )
    .bind(&audit_id)
    .bind(now.to_rfc3339())
    .bind(actor)
    .bind(quote_id)
    .bind(event_type)
    .bind(&payload)
    .bind(&payload)
    .execute(pool)
    .await;

    if let Err(e) = result {
        warn!(
            event_name = "portal.funnel.write_failed",
            funnel_step = event_type,
            quote_id = ?quote_id,
            error = %e,
            "failed to write funnel telemetry event"
        );
    }
}

fn redact_token(token: &str) -> String {
    let keep = token.len().min(8);
    format!("{}***", &token[..keep])
}

fn db_error(error: sqlx::Error) -> (StatusCode, Json<PortalError>) {
    error!(error = %error, "portal database error");
    (StatusCode::INTERNAL_SERVER_ERROR, Json(PortalError::service_unavailable("database")))
}

async fn ensure_push_subscription_table(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS portal_push_subscription (
            id            TEXT PRIMARY KEY NOT NULL,
            endpoint      TEXT NOT NULL UNIQUE,
            p256dh        TEXT NOT NULL,
            auth          TEXT NOT NULL,
            user_agent    TEXT,
            device_label  TEXT,
            revoked       INTEGER NOT NULL DEFAULT 0,
            created_at    TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_portal_push_subscription_revoked
         ON portal_push_subscription(revoked)",
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_digest_schedule_table(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS analytics_digest_schedule (
            id              INTEGER PRIMARY KEY NOT NULL,
            enabled         INTEGER NOT NULL DEFAULT 0,
            day_of_week     TEXT NOT NULL DEFAULT 'monday',
            time_utc        TEXT NOT NULL DEFAULT '09:00',
            recipient_email TEXT,
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn ensure_digest_delivery_table(pool: &DbPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS analytics_digest_delivery (
            id              TEXT PRIMARY KEY NOT NULL,
            week_key        TEXT NOT NULL,
            recipient_email TEXT,
            status          TEXT NOT NULL,
            reason          TEXT NOT NULL,
            payload_json    TEXT NOT NULL,
            sent_at         TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_analytics_digest_delivery_week_recipient
         ON analytics_digest_delivery(week_key, recipient_email, status)",
    )
    .execute(pool)
    .await?;

    Ok(())
}

fn extract_requester_ip(headers: &HeaderMap) -> String {
    let from_forwarded = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    if let Some(ip) = from_forwarded {
        return ip;
    }

    for header in ["x-real-ip", "cf-connecting-ip"] {
        if let Some(ip) = headers
            .get(header)
            .and_then(|value| value.to_str().ok())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
        {
            return ip;
        }
    }

    "unknown".to_string()
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
    use axum::http::{HeaderMap, HeaderValue};
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
        tera.add_raw_template("approvals.html", "<html><body>Approvals</body></html>").ok();
        tera.add_raw_template("settings.html", "<html><body>Settings</body></html>").ok();
        tera.add_raw_template(
            "approval_detail.html",
            "<html><body>Approval {{ approval.id }}</body></html>",
        )
        .ok();

        State(PortalState {
            db_pool: pool,
            templates: Arc::new(tera),
            pdf_generator: None,
            branding: BrandingConfig::default(),
            rep_notifications: PortalRepNotificationConfig::default(),
        })
    }

    fn state_with_real_templates(pool: sqlx::SqlitePool) -> State<PortalState> {
        State(PortalState {
            db_pool: pool,
            templates: init_templates(),
            pdf_generator: None,
            branding: BrandingConfig::default(),
            rep_notifications: PortalRepNotificationConfig::default(),
        })
    }

    fn forwarded_headers(ip: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", HeaderValue::from_str(ip).expect("valid header"));
        headers
    }

    #[tokio::test]
    async fn approve_quote_records_approval_and_updates_status() {
        let (pool, quote_id, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "Jane Doe".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: Some("Looks great!".to_string()),
                auth_method: Some("password".to_string()),
                biometric_assertion: None,
                fallback_password: Some("local-test-pass".to_string()),
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
        let audit_payload_json: serde_json::Value =
            serde_json::from_str(&audit_payload).expect("audit payload json");
        assert_eq!(audit_payload_json["auth"]["channel"], serde_json::json!("portal"));
        assert_eq!(audit_payload_json["auth"]["method"], serde_json::json!("password"));
        assert_eq!(
            audit_payload_json["auth"]["strength"],
            serde_json::json!("possession_and_knowledge")
        );
        assert_eq!(
            audit_payload_json["auth"]["principal"]["actor_id"],
            serde_json::json!("portal:jane@acme.com")
        );
        assert!(audit_payload_json["detail"].as_str().unwrap_or_default().contains("approved"));

        // Verify captured approval metadata includes required fields
        let justification: String = sqlx::query_scalar(
            "SELECT justification FROM approval_request WHERE quote_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch justification");
        let metadata: serde_json::Value =
            serde_json::from_str(&justification).expect("justification should be json");
        assert_eq!(metadata["quote_version"], serde_json::json!(1));
        assert_eq!(metadata["requester_ip"], serde_json::json!("unknown"));
        assert_eq!(metadata["approver_email"], serde_json::json!("jane@acme.com"));
        assert_eq!(metadata["auth_context"]["channel"], serde_json::json!("portal"));
        assert_eq!(metadata["auth_context"]["method"], serde_json::json!("password"));
        assert_eq!(
            metadata["auth_context"]["strength"],
            serde_json::json!("possession_and_knowledge")
        );
        assert_eq!(
            metadata["auth_context"]["principal"]["actor_id"],
            serde_json::json!("portal:jane@acme.com")
        );
    }

    #[tokio::test]
    async fn approve_quote_rejects_empty_name() {
        let (pool, _, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token),
            state(pool),
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "  ".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: None,
                auth_method: Some("password".to_string()),
                biometric_assertion: None,
                fallback_password: Some("local-test-pass".to_string()),
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn approve_quote_allows_legacy_payload_without_auth_method() {
        let (pool, quote_id, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token),
            state(pool.clone()),
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "Legacy Approver".to_string(),
                approver_email: "legacy@example.com".to_string(),
                comments: None,
                auth_method: None,
                biometric_assertion: None,
                fallback_password: None,
            }),
        )
        .await
        .expect("legacy approval should continue to succeed");

        assert!(result.0.success);
        let status: String = sqlx::query_scalar("SELECT status FROM quote WHERE id = ?")
            .bind(&quote_id)
            .fetch_one(&pool)
            .await
            .expect("fetch quote status");
        assert_eq!(status, "approved");
    }

    #[tokio::test]
    async fn approve_quote_rejects_unknown_auth_method() {
        let (pool, _, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token),
            state(pool),
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "Jane Doe".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: None,
                auth_method: Some("magic".to_string()),
                biometric_assertion: None,
                fallback_password: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, body) = result.expect_err("unknown auth method must fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.error.contains("authMethod"));
    }

    #[tokio::test]
    async fn approve_quote_rejects_biometric_without_assertion() {
        let (pool, _, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token),
            state(pool),
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "Jane Doe".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: None,
                auth_method: Some("biometric".to_string()),
                biometric_assertion: None,
                fallback_password: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, body) = result.expect_err("missing biometric assertion must fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.error.contains("biometricAssertion"));
    }

    #[tokio::test]
    async fn approve_quote_records_biometric_auth_metadata() {
        let (pool, quote_id, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token),
            state(pool.clone()),
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "Jane Doe".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: Some("Biometric flow".to_string()),
                auth_method: Some("biometric".to_string()),
                biometric_assertion: Some("assertion-token-123".to_string()),
                fallback_password: None,
            }),
        )
        .await
        .expect("biometric approval should succeed");

        assert!(result.0.success);
        let justification: String = sqlx::query_scalar(
            "SELECT justification FROM approval_request WHERE quote_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch biometric justification");
        let metadata: serde_json::Value =
            serde_json::from_str(&justification).expect("justification should be json");
        assert_eq!(metadata["auth_method"], serde_json::json!("biometric"));
        assert_eq!(metadata["biometric_assertion_present"], serde_json::json!(true));
        assert_eq!(metadata["fallback_password_used"], serde_json::json!(false));
        assert_eq!(metadata["auth_context"]["channel"], serde_json::json!("portal"));
        assert_eq!(metadata["auth_context"]["method"], serde_json::json!("web_authn"));
        assert_eq!(
            metadata["auth_context"]["strength"],
            serde_json::json!("possession_and_biometric")
        );
        assert_eq!(
            metadata["auth_context"]["principal"]["actor_id"],
            serde_json::json!("portal:jane@acme.com")
        );
    }

    #[tokio::test]
    async fn reject_quote_records_rejection_and_updates_status() {
        let (pool, quote_id, token) = setup().await;

        let result = reject_quote(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            Json(RejectRequest {
                reason: "Pricing too high for our budget".to_string(),
                auth_method: Some("password".to_string()),
                biometric_assertion: None,
                fallback_password: Some("local-test-pass".to_string()),
            }),
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

        let (audit_payload, audit_metadata): (String, Option<String>) = sqlx::query_as(
            "SELECT payload_json, metadata_json
             FROM audit_event
             WHERE quote_id = ? AND event_type = 'portal.rejection'
             ORDER BY timestamp DESC
             LIMIT 1",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch rejection audit");

        let payload_json: serde_json::Value =
            serde_json::from_str(&audit_payload).expect("rejection payload json");
        assert_eq!(payload_json["auth"]["channel"], serde_json::json!("portal"));
        assert_eq!(payload_json["auth"]["method"], serde_json::json!("password"));
        assert_eq!(
            payload_json["auth"]["principal"]["actor_id"],
            serde_json::json!("portal:customer")
        );

        let metadata_json: serde_json::Value =
            serde_json::from_str(&audit_metadata.expect("rejection metadata"))
                .expect("rejection metadata json");
        assert_eq!(metadata_json["auth_channel"], serde_json::json!("portal"));
        assert_eq!(metadata_json["auth_method"], serde_json::json!("password"));
        assert_eq!(metadata_json["auth_strength"], serde_json::json!("possession_and_knowledge"));
        assert_eq!(metadata_json["auth_principal"], serde_json::json!("portal:customer"));
    }

    #[tokio::test]
    async fn reject_quote_rejects_unknown_auth_method() {
        let (pool, _, token) = setup().await;

        let result = reject_quote(
            axum::extract::Path(token),
            state(pool),
            Json(RejectRequest {
                reason: "Missing terms".to_string(),
                auth_method: Some("magic".to_string()),
                biometric_assertion: None,
                fallback_password: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, body) = result.expect_err("unknown auth method should fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.error.contains("authMethod"));
    }

    #[tokio::test]
    async fn reject_quote_rejects_empty_reason() {
        let (pool, _, token) = setup().await;

        let result = reject_quote(
            axum::extract::Path(token),
            state(pool),
            Json(RejectRequest {
                reason: "".to_string(),
                auth_method: Some("password".to_string()),
                biometric_assertion: None,
                fallback_password: Some("local-test-pass".to_string()),
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn reject_quote_allows_legacy_payload_without_auth_method() {
        let (pool, quote_id, token) = setup().await;

        let result = reject_quote(
            axum::extract::Path(token),
            state(pool.clone()),
            Json(RejectRequest {
                reason: "legacy rejection path".to_string(),
                auth_method: None,
                biometric_assertion: None,
                fallback_password: None,
            }),
        )
        .await
        .expect("legacy rejection should continue to succeed");

        assert!(result.0.success);
        let status: String = sqlx::query_scalar("SELECT status FROM quote WHERE id = ?")
            .bind(&quote_id)
            .fetch_one(&pool)
            .await
            .expect("fetch rejected status");
        assert_eq!(status, "rejected");
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
    async fn add_comment_queues_rep_notification_event() {
        let (pool, quote_id, token) = setup().await;

        let _ = add_comment(
            axum::extract::Path(token),
            state(pool.clone()),
            Json(CommentRequest {
                text: "Need legal language for cancellation clause.".to_string(),
                author_name: None,
                author_email: None,
                parent_id: None,
            }),
        )
        .await
        .expect("comment succeeds");

        let queued: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE quote_id = ? AND event_type = 'portal.rep_notification.queued'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("count queued notifications");
        assert_eq!(queued, 1);
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
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "Jane".to_string(),
                approver_email: "jane@test.com".to_string(),
                comments: None,
                auth_method: Some("password".to_string()),
                biometric_assertion: None,
                fallback_password: Some("local-test-pass".to_string()),
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.0.error.contains("not found"));
    }

    #[tokio::test]
    async fn approve_quote_captures_requester_ip_from_forwarded_header() {
        let (pool, quote_id, token) = setup().await;

        let result = approve_quote(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            forwarded_headers("203.0.113.7, 10.0.0.1"),
            Json(ApproveRequest {
                approver_name: "Jane Doe".to_string(),
                approver_email: "jane@acme.com".to_string(),
                comments: Some("LGTM".to_string()),
                auth_method: Some("password".to_string()),
                biometric_assertion: None,
                fallback_password: Some("local-test-pass".to_string()),
            }),
        )
        .await
        .expect("should succeed");

        assert!(result.0.success);

        let justification: String = sqlx::query_scalar(
            "SELECT justification FROM approval_request WHERE quote_id = ? ORDER BY created_at DESC LIMIT 1",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch justification");
        let metadata: serde_json::Value =
            serde_json::from_str(&justification).expect("justification should be json");
        assert_eq!(metadata["requester_ip"], serde_json::json!("203.0.113.7"));
    }

    #[tokio::test]
    async fn view_quote_page_renders_core_quote_details_and_actions() {
        let (pool, quote_id, token) = setup().await;
        let now = Utc::now().to_rfc3339();

        sqlx::query("UPDATE quote SET account_id = 'Acme Corp', valid_until = ? WHERE id = ?")
            .bind((Utc::now() + chrono::Duration::days(10)).to_rfc3339())
            .bind(&quote_id)
            .execute(&pool)
            .await
            .expect("update quote");

        sqlx::query(
            "INSERT INTO product (id, name, sku, base_price, currency, active, created_at, updated_at)
             VALUES ('PROD-PORTAL', 'Enterprise Plan', 'ENT-001', '100.0', 'USD', 1, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed product");

        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, notes, created_at, updated_at)
             VALUES ('QL-PORTAL-1', ?, 'PROD-PORTAL', 3, 100.0, 300.0, 10.0, 'Includes onboarding', ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed quote line");

        sqlx::query(
            "INSERT INTO portal_comment (id, quote_id, quote_line_id, parent_id, author_name, author_email, body, created_at)
             VALUES ('PC-PORTAL-1', ?, NULL, NULL, 'Customer A', 'customer@example.com', 'Can we discuss payment terms?', datetime('now'))",
        )
        .bind(&quote_id)
        .execute(&pool)
        .await
        .expect("seed comment");

        sqlx::query(
            "INSERT INTO portal_comment (id, quote_id, quote_line_id, parent_id, author_name, author_email, body, created_at)
             VALUES ('PC-PORTAL-2', ?, NULL, NULL, 'Rep A', 'portal:rep@acme.com', 'Discount requested to match competitor proposal.', datetime('now'))",
        )
        .bind(&quote_id)
        .execute(&pool)
        .await
        .expect("seed rep note");

        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, account_id, created_at, updated_at)
             VALUES ('Q-TEST-RELATED', 'approved', 'USD', 'test-rep', 'Acme Corp', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed similar deal");

        let html = view_quote_page(
            axum::extract::Path(token),
            axum::extract::Query(ViewQuoteParams::default()),
            state_with_real_templates(pool),
        )
        .await
        .expect("render quote page")
        .0;

        assert!(html.contains("Quote for"));
        assert!(html.contains("Acme Corp"));
        assert!(html.contains("Line Items"));
        assert!(html.contains("Enterprise Plan"));
        assert!(html.contains("Approve Quote"));
        assert!(html.contains("Decline"));
        assert!(html.contains("/download"));
        assert!(html.contains("Questions or Comments"));
        assert!(html.contains("Can we discuss payment terms?"));
        assert!(html.contains("Decision Context"));
        assert!(html.contains("Need Info"));
        assert!(html.contains("Snooze"));
        assert!(html.contains("Q-TEST-RELATED"));
        assert!(html.contains("Discount requested to match competitor proposal."));
    }

    #[tokio::test]
    async fn view_quote_page_approved_state_shows_confirmation_panel() {
        let (pool, quote_id, token) = setup().await;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE quote
             SET status = 'approved', account_id = 'Acme Corp', valid_until = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind((Utc::now() + chrono::Duration::days(10)).to_rfc3339())
        .bind(&now)
        .bind(&quote_id)
        .execute(&pool)
        .await
        .expect("update quote to approved");

        let html = view_quote_page(
            axum::extract::Path(token),
            axum::extract::Query(ViewQuoteParams::default()),
            state_with_real_templates(pool),
        )
        .await
        .expect("render quote page")
        .0;

        assert!(html.contains("Quote Approved"));
        assert!(html.contains("This quote has been approved."));
        assert!(html.contains("Download PDF"));
        assert!(!html.contains("Quote Actions"));
    }

    #[tokio::test]
    async fn view_quote_page_slack_handoff_renders_banner() {
        let (pool, _quote_id, token) = setup().await;

        let html = view_quote_page(
            axum::extract::Path(token),
            axum::extract::Query(ViewQuoteParams {
                from: Some("slack".to_string()),
                action: Some("review".to_string()),
                context_summary: Some(
                    "Customer asked to confirm annual commitment scope".to_string(),
                ),
                assumptions_summary: Some(
                    "Tax remains assumed at 0% until billing country confirmed".to_string(),
                ),
                next_action: Some("comment".to_string()),
            }),
            state_with_real_templates(pool),
        )
        .await
        .expect("render quote page with slack handoff")
        .0;

        assert!(html.contains("Opened from Slack"));
        assert!(html.contains("next action:"));
        assert!(html.contains("comment"));
        assert!(html.contains("Customer asked to confirm annual commitment scope"));
        assert!(html.contains("Tax remains assumed at 0% until billing country confirmed"));
    }

    #[tokio::test]
    async fn portal_index_page_shows_pending_approvals_summary_cards() {
        let (pool, quote_id, token) = setup().await;
        let now = Utc::now().to_rfc3339();
        let valid_until = (Utc::now() + chrono::Duration::days(15)).to_rfc3339();

        sqlx::query(
            "UPDATE quote SET status = 'pending', valid_until = ?, updated_at = ? WHERE id = ?",
        )
        .bind(&valid_until)
        .bind(&now)
        .bind(&quote_id)
        .execute(&pool)
        .await
        .expect("set quote pending");

        sqlx::query(
            "INSERT INTO product (id, name, sku, base_price, currency, active, created_at, updated_at)
             VALUES ('PROD-PENDING-1', 'Platform Seats', 'SEAT-001', '125.0', 'USD', 1, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed product");

        sqlx::query(
            "INSERT INTO quote_line
                (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, notes, created_at, updated_at)
             VALUES ('QL-PENDING-1', ?, 'PROD-PENDING-1', 2, 125.0, 250.0, 0.0, NULL, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed quote line");

        let html =
            portal_index_page(Query(PortalIndexQuery::default()), state_with_real_templates(pool))
                .await
                .expect("render portal index")
                .0;

        assert!(html.contains("Pending Approvals"));
        assert!(html.contains("waiting for decision"));
        assert!(html.contains(&quote_id));
        assert!(html.contains(&format!("/quote/{token}")));
    }

    #[tokio::test]
    async fn portal_index_page_hides_pending_summary_when_no_pending_quotes() {
        let (pool, quote_id, _token) = setup().await;
        let now = Utc::now().to_rfc3339();

        sqlx::query("UPDATE quote SET status = 'approved', updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&quote_id)
            .execute(&pool)
            .await
            .expect("set quote approved");

        let html =
            portal_index_page(Query(PortalIndexQuery::default()), state_with_real_templates(pool))
                .await
                .expect("render portal index")
                .0;

        assert!(!html.contains("Pending Approvals"));
    }

    #[tokio::test]
    async fn approvals_settings_page_renders_with_branding() {
        let (pool, _quote_id, _token) = setup().await;

        let html = approvals_settings_page(state_with_real_templates(pool))
            .await
            .expect("render settings page")
            .0;

        assert!(html.contains("Quotey Approvals Settings"));
        assert!(html.contains("Notification Settings"));
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
        )
        .await
        .expect("create_link should succeed");

        let resp = result.0;
        assert_eq!(resp.quote_id, quote_id);
        assert!(!resp.token.is_empty());
        assert!(!resp.link_id.is_empty());
        assert!(!resp.expires_at.is_empty());
        assert_eq!(resp.share_url, format!("/quote/{}", resp.token));
    }

    #[tokio::test]
    async fn create_link_preserves_handoff_query_state_in_share_url() {
        let (pool, quote_id, _token) = setup().await;

        let result = create_link(
            state(pool),
            Json(CreateLinkRequest {
                quote_id,
                expires_in_days: Some(7),
                created_by: Some("rep@acme.com".to_string()),
                from: Some(" Slack ".to_string()),
                action: Some("Review".to_string()),
                context_summary: Some("Customer asked for annual prepay options.".to_string()),
                assumptions_summary: Some(
                    "Tax is assumed 0% until billing country is set.".to_string(),
                ),
                next_action: Some("Approve".to_string()),
            }),
        )
        .await
        .expect("create_link should succeed");

        let resp = result.0;
        let parsed = reqwest::Url::parse(&format!("https://portal.local{}", resp.share_url))
            .expect("parse share URL");
        let pairs = parsed
            .query_pairs()
            .into_owned()
            .collect::<std::collections::HashMap<String, String>>();

        assert_eq!(parsed.path(), format!("/quote/{}", resp.token));
        assert_eq!(pairs.get("from").map(String::as_str), Some("slack"));
        assert_eq!(pairs.get("action").map(String::as_str), Some("review"));
        assert_eq!(pairs.get("next_action").map(String::as_str), Some("approve"));
        assert_eq!(
            pairs.get("context_summary").map(String::as_str),
            Some("Customer asked for annual prepay options.")
        );
        assert_eq!(
            pairs.get("assumptions_summary").map(String::as_str),
            Some("Tax is assumed 0% until billing country is set.")
        );
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
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
    async fn regenerate_link_returns_fresh_token_and_revokes_previous() {
        let (pool, quote_id, _token) = setup().await;

        let first = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(30),
                created_by: Some("regen-test".to_string()),
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
        )
        .await
        .expect("first link");

        let regenerated = regenerate_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(45),
                created_by: Some("regen-test".to_string()),
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
        )
        .await
        .expect("regenerated link");

        assert_ne!(first.0.token, regenerated.0.token);

        let revoked: i64 = sqlx::query_scalar("SELECT revoked FROM portal_link WHERE token = ?")
            .bind(&first.0.token)
            .fetch_one(&pool)
            .await
            .expect("fetch first revoked status");
        assert_eq!(revoked, 1);

        let links = list_links(axum::extract::Path(quote_id.clone()), state(pool.clone()))
            .await
            .expect("list links");
        assert_eq!(links.0.len(), 1, "only regenerated link should remain active");
        assert_eq!(links.0[0].token, regenerated.0.token);

        let regen_audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE quote_id = ? AND event_type = 'portal.link_regenerated'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("fetch regenerate audit count");
        assert!(regen_audit_count >= 1);
    }

    #[tokio::test]
    async fn revoke_link_succeeds() {
        let (pool, quote_id, _token) = setup().await;

        let link = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id,
                expires_in_days: Some(7),
                created_by: None,
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
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
            Json(CreateLinkRequest {
                quote_id,
                expires_in_days: Some(7),
                created_by: None,
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
        )
        .await
        .expect("second link");

        let result =
            list_links(axum::extract::Path(quote_id), state(pool)).await.expect("list links");

        let links = result.0;
        assert_eq!(links.len(), 1, "only the active (non-revoked) link should appear");
        assert_eq!(links[0].token, second.0.token);
        assert_eq!(links[0].share_url, format!("/quote/{}", links[0].token));
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
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
            Json(CreateLinkRequest {
                quote_id,
                expires_in_days: Some(7),
                created_by: None,
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
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
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
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
            Json(CreateLinkRequest {
                quote_id,
                expires_in_days: Some(1),
                created_by: None,
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
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

        let queued: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE quote_id = ? AND event_type = 'portal.rep_notification.queued'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("count queued line notifications");
        assert_eq!(queued, 1);
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

    // -----------------------------------------------------------------------
    // Subtotal regression tests (quotey-ux-001-10)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn fetch_quote_for_pdf_line_subtotal_is_pre_discount() {
        let (pool, quote_id, _token) = setup().await;

        // Seed a product
        sqlx::query(
            "INSERT INTO product (id, name, sku, base_price, currency, active, created_at, updated_at)
             VALUES ('PROD-1', 'Widget', 'SKU-1', '100.0', 'USD', 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .expect("seed product");

        // Seed two quote lines with discounts
        let now_str = Utc::now().to_rfc3339();
        // Line 1: qty 2, price 100, 10% discount => subtotal=200, total=180
        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, created_at, updated_at)
             VALUES ('QL-1', ?, 'PROD-1', 2, 100.0, 200.0, 10.0, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .expect("seed line 1");

        // Line 2: qty 3, price 50, 20% discount => subtotal=150, total=120
        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, created_at, updated_at)
             VALUES ('QL-2', ?, 'PROD-1', 3, 50.0, 150.0, 20.0, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .expect("seed line 2");

        let payload = fetch_quote_for_pdf(&pool, &quote_id, "Quotey").await.expect("fetch pdf");
        let lines = payload["lines"].as_array().expect("lines array");
        assert_eq!(lines.len(), 2);

        // Line-level subtotal should be PRE-discount (unit_price * qty)
        let line1_subtotal = lines[0]["subtotal"].as_f64().expect("line1 subtotal");
        assert!(
            (line1_subtotal - 200.0).abs() < 0.01,
            "line 1 subtotal should be 200 (pre-discount), got {}",
            line1_subtotal
        );

        let line2_subtotal = lines[1]["subtotal"].as_f64().expect("line2 subtotal");
        assert!(
            (line2_subtotal - 150.0).abs() < 0.01,
            "line 2 subtotal should be 150 (pre-discount), got {}",
            line2_subtotal
        );

        // Overall subtotal = sum of line subtotals (pre-discount)
        let pricing_subtotal = payload["pricing"]["subtotal"].as_f64().expect("pricing subtotal");
        assert!(
            (pricing_subtotal - 350.0).abs() < 0.01,
            "pricing.subtotal should be 350, got {}",
            pricing_subtotal
        );

        // Sum of line subtotals must equal pricing subtotal
        let sum_line_subtotals: f64 = lines.iter().map(|l| l["subtotal"].as_f64().unwrap()).sum();
        assert!(
            (sum_line_subtotals - pricing_subtotal).abs() < 0.01,
            "SUM(line.subtotal) {} must equal pricing.subtotal {}",
            sum_line_subtotals,
            pricing_subtotal
        );
    }

    // -----------------------------------------------------------------------
    // Token hardening regression tests (quotey-ux-001-11)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn resolve_token_unknown_returns_not_found() {
        let (pool, _, _) = setup().await;

        let result = resolve_quote_by_token(&pool, "completely-made-up-token").await;
        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn resolve_token_unknown_records_audit_event_without_quote() {
        let (pool, _, _) = setup().await;

        let _ = resolve_quote_by_token(&pool, "missing-token-for-audit").await;

        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE quote_id IS NULL AND event_type = 'portal.token_denied'",
        )
        .fetch_one(&pool)
        .await
        .expect("count unknown token denial audits");

        assert!(
            audit_count >= 1,
            "expected at least one unknown-token denial audit row, got {audit_count}"
        );
    }

    #[tokio::test]
    async fn resolve_token_revoked_records_audit_event() {
        let (pool, quote_id, _token) = setup().await;

        let link = create_link(
            state(pool.clone()),
            Json(CreateLinkRequest {
                quote_id: quote_id.clone(),
                expires_in_days: Some(7),
                created_by: None,
                from: None,
                action: None,
                context_summary: None,
                assumptions_summary: None,
                next_action: None,
            }),
        )
        .await
        .expect("create link");

        let _ = revoke_link(
            state(pool.clone()),
            Json(RevokeLinkRequest { token: link.0.token.clone() }),
        )
        .await
        .expect("revoke");

        let _ = resolve_quote_by_token(&pool, &link.0.token).await;

        // Verify audit event was recorded for the revoked access attempt
        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE quote_id = ? AND event_type = 'portal.token_denied'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("count audit events");
        assert!(
            audit_count >= 1,
            "expected at least 1 audit event for revoked token, got {audit_count}"
        );
    }

    #[tokio::test]
    async fn resolve_token_expired_records_audit_event() {
        let (pool, quote_id, _token) = setup().await;

        let past = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        sqlx::query(
            "INSERT INTO portal_link (id, quote_id, token, expires_at, created_by, created_at)
             VALUES ('PL-AUD-EXP', ?, 'audit-expired-tok', ?, 'test', ?)",
        )
        .bind(&quote_id)
        .bind(&past)
        .bind(&past)
        .execute(&pool)
        .await
        .expect("insert expired link");

        let _ = resolve_quote_by_token(&pool, "audit-expired-tok").await;

        // Verify audit event was recorded for the expired access attempt
        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE quote_id = ? AND event_type = 'portal.token_denied'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("count audit events");
        assert!(
            audit_count >= 1,
            "expected at least 1 audit event for expired token, got {audit_count}"
        );
    }

    #[tokio::test]
    async fn subscribe_push_persists_subscription() {
        let (pool, _, _) = setup().await;

        let result = subscribe_push(
            state(pool.clone()),
            Json(PushSubscriptionRequest {
                endpoint: "https://example.push/abc".to_string(),
                p256dh: "p256dh-key".to_string(),
                auth: "auth-key".to_string(),
                user_agent: Some("Mobile Safari".to_string()),
                device_label: Some("Manager iPhone".to_string()),
            }),
        )
        .await
        .expect("subscribe push");

        assert!(result.0.success);

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM portal_push_subscription WHERE endpoint = ? AND revoked = 0",
        )
        .bind("https://example.push/abc")
        .fetch_one(&pool)
        .await
        .expect("count push subscriptions");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn unsubscribe_push_marks_subscription_revoked() {
        let (pool, _, _) = setup().await;
        ensure_push_subscription_table(&pool).await.expect("ensure push table");
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO portal_push_subscription
                (id, endpoint, p256dh, auth, user_agent, device_label, revoked, created_at, updated_at)
             VALUES ('PUSH-TEST', 'https://example.push/remove', 'p', 'a', NULL, NULL, 0, ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed push subscription");

        let result = unsubscribe_push(
            state(pool.clone()),
            Json(PushUnsubscribeRequest { endpoint: "https://example.push/remove".to_string() }),
        )
        .await
        .expect("unsubscribe push");
        assert!(result.0.success);

        let revoked: i64 =
            sqlx::query_scalar("SELECT revoked FROM portal_push_subscription WHERE endpoint = ?")
                .bind("https://example.push/remove")
                .fetch_one(&pool)
                .await
                .expect("fetch revoked flag");
        assert_eq!(revoked, 1);
    }

    #[tokio::test]
    async fn approvals_index_route_renders_pending_quotes_view() {
        let (pool, quote_id, _) = setup().await;
        sqlx::query("UPDATE quote SET status = 'pending', account_id = 'Acme Corp' WHERE id = ?")
            .bind(&quote_id)
            .execute(&pool)
            .await
            .expect("mark quote pending");
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO approval_request
                (id, quote_id, approver_role, reason, justification, status, requested_by, expires_at, created_at, updated_at)
             VALUES ('APR-LIST-001', ?, 'sales_manager', 'Discount exceeds cap', '{}', 'pending', 'agent:test', NULL, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed approval list row");

        let html = approvals_index_page(state_with_real_templates(pool))
            .await
            .expect("render approvals page")
            .0;

        assert!(
            html.contains("Pending Approvals"),
            "approvals page should show the pending approvals section"
        );
        assert!(
            html.contains("APR-LIST-001"),
            "approvals page should show pending approval identifiers"
        );
    }

    #[tokio::test]
    async fn approvals_index_route_excludes_expired_links() {
        let (pool, quote_id, _) = setup().await;
        let expired = (Utc::now() - chrono::Duration::minutes(5)).to_rfc3339();
        sqlx::query("UPDATE portal_link SET expires_at = ? WHERE quote_id = ?")
            .bind(&expired)
            .bind(&quote_id)
            .execute(&pool)
            .await
            .expect("expire seeded portal link");

        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO approval_request
                (id, quote_id, approver_role, reason, justification, status, requested_by, expires_at, created_at, updated_at)
             VALUES ('APR-EXPIRED-LINK', ?, 'sales_manager', 'Discount exceeds cap', '{}', 'pending', 'agent:test', NULL, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed pending approval");

        let html = approvals_index_page(state_with_real_templates(pool))
            .await
            .expect("render approvals page")
            .0;

        assert!(
            !html.contains("APR-EXPIRED-LINK"),
            "approvals linked only to expired tokens must be excluded"
        );
    }

    #[tokio::test]
    async fn approval_detail_route_renders_approval_context() {
        let (pool, quote_id, _token) = setup().await;
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO approval_request
                (id, quote_id, approver_role, reason, justification, status, requested_by, expires_at, created_at, updated_at)
             VALUES ('APR-ROUTE-001', ?, 'sales_manager', 'discount cap', '{}', 'pending', 'agent:test', NULL, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed approval request");

        let html =
            approval_detail_page(axum::extract::Path("APR-ROUTE-001".to_string()), state(pool))
                .await
                .expect("approval detail render");

        assert!(
            html.0.contains("APR-ROUTE-001"),
            "approval detail page should contain the approval ID"
        );
    }

    #[tokio::test]
    async fn approval_detail_renders_quote_context_with_lines() {
        let (pool, quote_id, _) = setup().await;
        let now = Utc::now().to_rfc3339();

        // Seed product and quote line
        sqlx::query(
            "INSERT INTO product (id, name, sku, base_price, currency, active, created_at, updated_at)
             VALUES ('PROD-APR', 'Enterprise License', 'ENT-001', '500.0', 'USD', 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .expect("seed product");

        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, created_at, updated_at)
             VALUES ('QL-APR', ?, 'PROD-APR', 10, 500.0, 5000.0, 20.0, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed quote line");

        sqlx::query(
            "INSERT INTO approval_request
                (id, quote_id, approver_role, reason, justification, status, requested_by, expires_at, created_at, updated_at)
             VALUES ('APR-CTX-001', ?, 'sales_director', 'Discount exceeds 15% cap', 'Strategic account expansion', 'pending', 'rep@company.com', NULL, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed approval");

        let html = approval_detail_page(
            axum::extract::Path("APR-CTX-001".to_string()),
            state_with_real_templates(pool),
        )
        .await
        .expect("render approval detail with real templates");

        let body = &html.0;
        // Approval context
        assert!(body.contains("APR-CTX-001"), "should contain approval ID");
        assert!(body.contains("sales_director"), "should contain approver role");
        assert!(body.contains("Discount exceeds 15% cap"), "should contain approval reason");
        // Quote line context
        assert!(body.contains("Enterprise License"), "should contain product name");
        assert!(body.contains("20"), "should contain discount percentage");
        // Actions
        assert!(body.contains("Approve"), "should contain approve action");
        assert!(body.contains("Reject"), "should contain reject action");
    }

    #[tokio::test]
    async fn approval_detail_route_returns_not_found_for_unknown_approval() {
        let (pool, _, _) = setup().await;
        let result =
            approval_detail_page(axum::extract::Path("APR-MISSING".to_string()), state(pool)).await;

        assert!(result.is_err());
        let (status, body) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.0.contains("Approval Not Found"));
    }

    #[tokio::test]
    async fn approvals_settings_route_renders_settings_template() {
        let (pool, _, _) = setup().await;
        let html = approvals_settings_page(state_with_real_templates(pool))
            .await
            .expect("render settings page")
            .0;

        assert!(html.contains("Notification Settings"));
        assert!(html.contains("/api/v1/portal/push/subscribe"));
    }

    #[tokio::test]
    async fn portal_manifest_route_returns_manifest_json() {
        let response = portal_manifest().await.into_response();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        assert!(content_type.contains("application/manifest+json"));
    }

    #[tokio::test]
    async fn portal_service_worker_route_returns_script() {
        let response = portal_service_worker().await.into_response();
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        assert!(content_type.contains("application/javascript"));
    }

    // -----------------------------------------------------------------------
    // UX regression pack (quotey-ux-001-18)
    //
    // Covers critical UX and correctness scenarios for the subtotal
    // accumulation fix (quotey-ux-001-10) and the token hardening fix
    // (quotey-ux-001-11). Six scenarios total.
    // -----------------------------------------------------------------------

    /// R-001: Subtotal fallback when `subtotal` column is NULL.
    ///
    /// Exercises the COALESCE(subtotal, unit_price * quantity) fallback path
    /// in `fetch_quote_for_pdf`. The pre-discount invariant must hold even when
    /// the DB column is absent.
    #[tokio::test]
    async fn regression_subtotal_null_column_uses_unit_price_times_qty() {
        let (pool, quote_id, _) = setup().await;

        sqlx::query(
            "INSERT INTO product (id, name, sku, base_price, currency, active, created_at, updated_at)
             VALUES ('PROD-R1', 'Gadget', 'SKU-R1', '75.0', 'USD', 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .expect("seed product");

        let now_str = Utc::now().to_rfc3339();
        // Insert a line without explicit subtotal (NULL)
        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, created_at, updated_at)
             VALUES ('QL-R1', ?, 'PROD-R1', 4, 75.0, NULL, 15.0, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .expect("seed line with NULL subtotal");

        let payload = fetch_quote_for_pdf(&pool, &quote_id, "Quotey").await.expect("fetch pdf");
        let lines = payload["lines"].as_array().expect("lines array");
        assert_eq!(lines.len(), 1);

        // Fallback: subtotal = unit_price * qty = 75 * 4 = 300 (pre-discount)
        let line_subtotal = lines[0]["subtotal"].as_f64().expect("subtotal");
        assert!(
            (line_subtotal - 300.0).abs() < 0.01,
            "NULL subtotal should fall back to unit_price * qty = 300, got {line_subtotal}"
        );

        // Discount should be 15% of 300 = 45
        let discount_amount = lines[0]["discount_amount"].as_f64().expect("discount_amount");
        assert!(
            (discount_amount - 45.0).abs() < 0.01,
            "discount_amount should be 45, got {discount_amount}"
        );

        // Pricing.subtotal must match line subtotal sum
        let pricing_subtotal = payload["pricing"]["subtotal"].as_f64().expect("pricing subtotal");
        assert!(
            (pricing_subtotal - line_subtotal).abs() < 0.01,
            "pricing.subtotal should equal single-line subtotal"
        );
    }

    /// R-002: Subtotal with zero-discount and 100%-discount lines.
    ///
    /// Edge case: mixing 0% and 100% discounts. The pre-discount subtotal must
    /// still accumulate correctly; total_price for the 100%-off line should be 0.
    #[tokio::test]
    async fn regression_subtotal_zero_and_full_discount_lines() {
        let (pool, quote_id, _) = setup().await;

        sqlx::query(
            "INSERT INTO product (id, name, sku, base_price, currency, active, created_at, updated_at)
             VALUES ('PROD-R2', 'Service', 'SKU-R2', '50.0', 'USD', 1, datetime('now'), datetime('now'))",
        )
        .execute(&pool)
        .await
        .expect("seed product");

        let now_str = Utc::now().to_rfc3339();
        // Line A: no discount
        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, created_at, updated_at)
             VALUES ('QL-R2A', ?, 'PROD-R2', 2, 100.0, 200.0, 0.0, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .expect("seed line A (0% discount)");

        // Line B: 100% discount (free add-on)
        sqlx::query(
            "INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, created_at, updated_at)
             VALUES ('QL-R2B', ?, 'PROD-R2', 1, 50.0, 50.0, 100.0, ?, ?)",
        )
        .bind(&quote_id)
        .bind(&now_str)
        .bind(&now_str)
        .execute(&pool)
        .await
        .expect("seed line B (100% discount)");

        let payload = fetch_quote_for_pdf(&pool, &quote_id, "Quotey").await.expect("fetch pdf");
        let lines = payload["lines"].as_array().expect("lines array");
        assert_eq!(lines.len(), 2);

        // Line A: subtotal=200, total_price=200
        let a_subtotal = lines[0]["subtotal"].as_f64().unwrap();
        let a_total = lines[0]["total_price"].as_f64().unwrap();
        assert!((a_subtotal - 200.0).abs() < 0.01);
        assert!((a_total - 200.0).abs() < 0.01);

        // Line B: subtotal=50, total_price=0 (100% off)
        let b_subtotal = lines[1]["subtotal"].as_f64().unwrap();
        let b_total = lines[1]["total_price"].as_f64().unwrap();
        assert!((b_subtotal - 50.0).abs() < 0.01);
        assert!(b_total.abs() < 0.01, "100% discount should yield total_price=0, got {b_total}");

        // Pricing subtotal = 200 + 50 = 250 (pre-discount)
        let pricing_subtotal = payload["pricing"]["subtotal"].as_f64().unwrap();
        assert!(
            (pricing_subtotal - 250.0).abs() < 0.01,
            "pricing.subtotal should be 250 (pre-discount sum), got {pricing_subtotal}"
        );

        // Total = 200 + 0 = 200 (post-discount)
        let pricing_total = payload["pricing"]["total"].as_f64().unwrap();
        assert!(
            (pricing_total - 200.0).abs() < 0.01,
            "pricing.total should be 200 (post-discount), got {pricing_total}"
        );
    }

    /// R-003: Portal index excludes quotes without valid portal tokens.
    ///
    /// Verifies the filter_map fix: a quote without any portal_link must NOT
    /// appear in the rendered index, even though it exists in the database.
    #[tokio::test]
    async fn regression_portal_index_excludes_quotes_without_token() {
        let (pool, _existing_qid, _existing_tok) = setup().await;

        // Insert a second quote that has NO portal_link
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES ('Q-NO-LINK', 'sent', 'USD', 'test', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed linkless quote");

        let html = portal_index_page(
            Query(PortalIndexQuery { status: None, search: None }),
            state_with_real_templates(pool),
        )
        .await
        .expect("render index")
        .0;

        // The existing quote (Q-TEST-001) has a portal_link and should appear
        assert!(html.contains("Q-TEST-001"), "linked quote should appear in index");

        // The linkless quote should NOT appear
        assert!(
            !html.contains("Q-NO-LINK"),
            "quotes without valid portal_link tokens must not appear in the portal index"
        );
    }

    /// R-004: Token from one quote cannot access another quote.
    ///
    /// Verifies that `resolve_quote_by_token` returns the correct quote_id
    /// for its token, and that swapping tokens across quotes is impossible.
    #[tokio::test]
    async fn regression_token_isolation_across_quotes() {
        let (pool, quote_id_a, token_a) = setup().await;

        // Create a second quote with its own token
        let now = Utc::now().to_rfc3339();
        let expires = (Utc::now() + chrono::Duration::days(7)).to_rfc3339();
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES ('Q-OTHER', 'sent', 'USD', 'test', ?, ?)",
        )
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed second quote");

        let token_b = generate_token();
        sqlx::query(
            "INSERT INTO portal_link (id, quote_id, token, expires_at, created_by, created_at)
             VALUES ('PL-OTHER', 'Q-OTHER', ?, ?, 'test', ?)",
        )
        .bind(&token_b)
        .bind(&expires)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed second link");

        // Token A resolves to quote A
        let resolved_a = resolve_quote_by_token(&pool, &token_a).await.expect("resolve A");
        assert_eq!(resolved_a, quote_id_a);

        // Token B resolves to quote B
        let resolved_b = resolve_quote_by_token(&pool, &token_b).await.expect("resolve B");
        assert_eq!(resolved_b, "Q-OTHER");

        // They must not cross
        assert_ne!(resolved_a, resolved_b, "tokens must resolve to different quotes");
    }

    /// R-005: Raw quote ID is rejected by resolve_quote_by_token.
    ///
    /// Confirms the security hardening: passing a raw quote_id (e.g. "Q-TEST-001")
    /// instead of a portal_link token must return NOT_FOUND with an audit trail.
    #[tokio::test]
    async fn regression_raw_quote_id_rejected_with_audit() {
        let (pool, quote_id, _token) = setup().await;

        // Attempt to use the raw quote_id as a token
        let result = resolve_quote_by_token(&pool, &quote_id).await;
        assert!(result.is_err(), "raw quote ID must not resolve as a token");
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Verify an audit trail was left for the denied access
        let audit_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE event_type = 'portal.token_denied'",
        )
        .fetch_one(&pool)
        .await
        .expect("count denials");
        assert!(
            audit_count >= 1,
            "raw ID rejection should leave an audit trail, got {audit_count} events"
        );
    }

    /// R-006: End-to-end portal flow emits funnel telemetry events.
    ///
    /// Exercises the full success path — create_link → view_quote → approve —
    /// and verifies that funnel telemetry events are recorded in the audit trail
    /// for each step. This is a cross-cutting regression across both correctness
    /// fixes and the funnel instrumentation (quotey-ux-001-17).
    #[tokio::test]
    async fn regression_e2e_flow_emits_funnel_events() {
        let (pool, quote_id, token) = setup().await;

        // Step 1: View quote page (triggers PRICING_RENDERED funnel event)
        let _ = view_quote_page(
            axum::extract::Path(token.clone()),
            axum::extract::Query(ViewQuoteParams::default()),
            state_with_real_templates(pool.clone()),
        )
        .await;

        // Step 2: Approve (triggers APPROVAL_ACTION funnel event)
        let _ = approve_quote(
            axum::extract::Path(token.clone()),
            state(pool.clone()),
            HeaderMap::new(),
            Json(ApproveRequest {
                approver_name: "Regression Tester".to_string(),
                approver_email: "regtest@example.com".to_string(),
                comments: Some("LGTM".to_string()),
                auth_method: Some("password".to_string()),
                biometric_assertion: None,
                fallback_password: Some("local-test-pass".to_string()),
            }),
        )
        .await;

        // Verify funnel events were recorded
        let funnel_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM audit_event WHERE event_category = 'funnel' AND quote_id = ?",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("count funnel events");

        assert!(
            funnel_count >= 2,
            "expected at least 2 funnel events (pricing_rendered + approval_action), got {funnel_count}"
        );

        // Verify the specific event types are present
        let event_types: Vec<String> = sqlx::query_scalar(
            "SELECT event_type FROM audit_event WHERE event_category = 'funnel' AND quote_id = ? ORDER BY timestamp",
        )
        .bind(&quote_id)
        .fetch_all(&pool)
        .await
        .expect("fetch funnel event types");

        assert!(
            event_types.iter().any(|t| t == "funnel.pricing_rendered"),
            "missing funnel.pricing_rendered event, got: {:?}",
            event_types
        );
        assert!(
            event_types.iter().any(|t| t == "funnel.approval_action"),
            "missing funnel.approval_action event, got: {:?}",
            event_types
        );
    }

    #[tokio::test]
    async fn export_quotes_csv_returns_csv_with_headers() {
        let (pool, _quote_id, _token) = setup().await;

        let resp = export_quotes_csv(
            axum::extract::Query(ExportQuotesQuery::default()),
            State(PortalState {
                db_pool: pool,
                templates: init_templates(),
                pdf_generator: None,
                branding: BrandingConfig::default(),
                rep_notifications: PortalRepNotificationConfig::default(),
            }),
        )
        .await
        .expect("export csv");

        let resp = resp.into_response();
        assert_eq!(resp.status(), StatusCode::OK);

        let content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .expect("content-type header")
            .to_str()
            .unwrap();
        assert!(content_type.contains("text/csv"));

        let disposition = resp
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .expect("content-disposition header")
            .to_str()
            .unwrap();
        assert!(disposition.contains("attachment"));
        assert!(disposition.contains(".csv"));

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.expect("read body");
        let csv_str = String::from_utf8(body.to_vec()).expect("utf8");

        // Should have header row
        assert!(csv_str.starts_with("quote_id,"));
        // Should have at least one data row (from setup)
        let lines: Vec<&str> = csv_str.trim().lines().collect();
        assert!(lines.len() >= 2, "CSV should have header + at least 1 row, got {}", lines.len());
    }

    #[tokio::test]
    async fn export_quotes_csv_respects_column_selection_and_order() {
        let (pool, _quote_id, _token) = setup().await;

        let resp = export_quotes_csv(
            axum::extract::Query(ExportQuotesQuery {
                columns: Some("quote_id,total,status".to_string()),
                ..ExportQuotesQuery::default()
            }),
            State(PortalState {
                db_pool: pool,
                templates: init_templates(),
                pdf_generator: None,
                branding: BrandingConfig::default(),
                rep_notifications: PortalRepNotificationConfig::default(),
            }),
        )
        .await
        .expect("export csv with custom columns")
        .into_response();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = axum::body::to_bytes(resp.into_body(), 1_000_000).await.expect("read body");
        let csv_str = String::from_utf8(body.to_vec()).expect("utf8");
        let mut lines = csv_str.trim().lines();
        assert_eq!(lines.next().expect("header"), "quote_id,total,status");

        let first_row = lines.next().expect("first data row");
        let fields: Vec<&str> = first_row.split(',').collect();
        assert_eq!(fields.len(), 3, "expected exactly 3 selected columns in data row");
    }

    #[tokio::test]
    async fn export_quotes_csv_rejects_unknown_columns() {
        let (pool, _quote_id, _token) = setup().await;

        let result = export_quotes_csv(
            axum::extract::Query(ExportQuotesQuery {
                columns: Some("quote_id,totally_not_real".to_string()),
                ..ExportQuotesQuery::default()
            }),
            State(PortalState {
                db_pool: pool,
                templates: init_templates(),
                pdf_generator: None,
                branding: BrandingConfig::default(),
                rep_notifications: PortalRepNotificationConfig::default(),
            }),
        )
        .await;

        assert!(result.is_err(), "invalid columns should fail");
        let (status, Json(error)) = result.err().expect("invalid column error payload");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.contains("columns"));
    }

    #[tokio::test]
    async fn digest_schedule_defaults_then_persists_updates() {
        let (pool, _quote_id, _token) = setup().await;
        let state = PortalState {
            db_pool: pool.clone(),
            templates: init_templates(),
            pdf_generator: None,
            branding: BrandingConfig::default(),
            rep_notifications: PortalRepNotificationConfig::default(),
        };

        let initial = get_digest_schedule(State(state.clone())).await.expect("get digest").0;
        assert!(!initial.enabled);
        assert_eq!(initial.day_of_week, "monday");
        assert_eq!(initial.time_utc, "09:00");

        let updated = upsert_digest_schedule(
            State(state.clone()),
            Json(DigestScheduleRequest {
                enabled: true,
                day_of_week: "wednesday".to_string(),
                time_utc: "14:30".to_string(),
                recipient_email: Some("ops@example.com".to_string()),
            }),
        )
        .await
        .expect("upsert digest")
        .0;
        assert!(updated.enabled);
        assert_eq!(updated.day_of_week, "wednesday");
        assert_eq!(updated.time_utc, "14:30");
        assert_eq!(updated.recipient_email.as_deref(), Some("ops@example.com"));

        let loaded = get_digest_schedule(State(state)).await.expect("reload digest").0;
        assert!(loaded.enabled);
        assert_eq!(loaded.day_of_week, "wednesday");
        assert_eq!(loaded.time_utc, "14:30");
        assert_eq!(loaded.recipient_email.as_deref(), Some("ops@example.com"));
    }

    #[tokio::test]
    async fn digest_schedule_rejects_invalid_time_and_missing_email_when_enabled() {
        let (pool, _quote_id, _token) = setup().await;
        let state = PortalState {
            db_pool: pool,
            templates: init_templates(),
            pdf_generator: None,
            branding: BrandingConfig::default(),
            rep_notifications: PortalRepNotificationConfig::default(),
        };

        let invalid_time = upsert_digest_schedule(
            State(state.clone()),
            Json(DigestScheduleRequest {
                enabled: false,
                day_of_week: "monday".to_string(),
                time_utc: "99:99".to_string(),
                recipient_email: None,
            }),
        )
        .await;
        assert!(invalid_time.is_err());
        let (status, body) = invalid_time.expect_err("invalid time must fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.error.contains("time_utc"));

        let missing_email = upsert_digest_schedule(
            State(state),
            Json(DigestScheduleRequest {
                enabled: true,
                day_of_week: "monday".to_string(),
                time_utc: "09:00".to_string(),
                recipient_email: None,
            }),
        )
        .await;
        assert!(missing_email.is_err());
        let (status, body) = missing_email.expect_err("missing email must fail");
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body.error.contains("recipient_email"));
    }

    #[tokio::test]
    async fn run_digest_dispatch_skips_when_schedule_is_not_due() {
        let (pool, _quote_id, _token) = setup().await;
        let state = PortalState {
            db_pool: pool,
            templates: init_templates(),
            pdf_generator: None,
            branding: BrandingConfig::default(),
            rep_notifications: PortalRepNotificationConfig::default(),
        };

        let today = weekday_name(Utc::now().weekday()).to_string();
        let not_today = DIGEST_DAYS
            .iter()
            .find(|day| **day != today)
            .expect("an alternate weekday must exist")
            .to_string();

        let _ = upsert_digest_schedule(
            State(state.clone()),
            Json(DigestScheduleRequest {
                enabled: true,
                day_of_week: not_today,
                time_utc: "00:00".to_string(),
                recipient_email: Some("ops@example.com".to_string()),
            }),
        )
        .await
        .expect("save digest schedule");

        let response =
            run_digest_dispatch(State(state), Json(DigestDispatchRequest { force: false }))
                .await
                .expect("dispatch response");

        assert!(!response.0.executed);
        assert_eq!(response.0.status, "skipped");
        assert!(response.0.reason.contains("not due"));
    }

    #[tokio::test]
    async fn run_digest_dispatch_force_records_delivery_attempt() {
        let (pool, _quote_id, _token) = setup().await;
        let state = PortalState {
            db_pool: pool.clone(),
            templates: init_templates(),
            pdf_generator: None,
            branding: BrandingConfig::default(),
            rep_notifications: PortalRepNotificationConfig::default(),
        };

        let today = weekday_name(Utc::now().weekday()).to_string();
        let _ = upsert_digest_schedule(
            State(state.clone()),
            Json(DigestScheduleRequest {
                enabled: true,
                day_of_week: today,
                time_utc: "00:00".to_string(),
                recipient_email: Some("ops@example.com".to_string()),
            }),
        )
        .await
        .expect("save digest schedule");

        let previous_webhook = std::env::var("QUOTEY_ANALYTICS_DIGEST_WEBHOOK_URL").ok();
        std::env::set_var("QUOTEY_ANALYTICS_DIGEST_WEBHOOK_URL", "http://127.0.0.1:1/unreachable");

        let response =
            run_digest_dispatch(State(state), Json(DigestDispatchRequest { force: true }))
                .await
                .expect("forced dispatch response");

        if let Some(value) = previous_webhook {
            std::env::set_var("QUOTEY_ANALYTICS_DIGEST_WEBHOOK_URL", value);
        } else {
            std::env::remove_var("QUOTEY_ANALYTICS_DIGEST_WEBHOOK_URL");
        }

        assert!(!response.0.executed);
        assert_eq!(response.0.status, "failed");

        let delivery_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM analytics_digest_delivery")
                .fetch_one(&pool)
                .await
                .expect("count digest deliveries");
        assert_eq!(delivery_count, 1);
    }
}
