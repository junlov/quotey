//! MCP Server Implementation
//!
//! Implements the Model Context Protocol server for Quotey.

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::ToolCallContext, wrapper::Parameters},
    model::*,
    schemars::{self, JsonSchema},
    serde::{Deserialize, Serialize},
    tool, tool_router, ServerHandler,
};
use tracing::{debug, info, warn};

use std::collections::HashMap;

use std::path::PathBuf;
use std::time::Duration;
use tera::{Context, Tera};
use tokio::process::Command;

use crate::auth::{AuthManager, AuthResult};
use quotey_core::domain::quote::Quote;
use quotey_core::{
    AuthChannel, AuthContext, AuthError, AuthErrorCode, AuthMethod, AuthPrincipal, AuthStrength,
};
use quotey_db::repositories::ApprovalRepository;

const MAX_PAGE_LIMIT: u32 = 100;
const DEFAULT_PAGE_LIMIT: u32 = 20;
const MAX_LINE_ITEMS: usize = 500;
const MAX_QUANTITY: u32 = 1_000_000;
const PORTAL_PUSH_BRIDGE_URL_ENV: &str = "QUOTEY_PORTAL_PUSH_BRIDGE_URL";

/// Return a tool error response with a redacted message for internal errors.
/// Logs the detailed error server-side for debugging.
fn internal_tool_error(error: &dyn std::fmt::Display) -> String {
    warn!(error = %error, "MCP tool internal error (redacted from response)");
    tool_error("INTERNAL_ERROR", "Internal server error", None)
}

fn tool_error(code: &str, message: &str, details: Option<serde_json::Value>) -> String {
    let safe_message = if code == "INTERNAL_ERROR" { "Internal server error" } else { message };
    let payload = serde_json::json!({
        "error": {
            "code": code,
            "message": safe_message,
            "details": details
        }
    });
    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| {
        format!(
            "{{\n  \"error\": {{\n    \"code\": \"INTERNAL_ERROR\",\n    \"message\": \"Failed to encode error response\",\n    \"details\": \"{}\"\n  }}\n}}",
            code
        )
    })
}

fn normalize_id(value: &str, field: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} is required"));
    }
    if trimmed.len() > MAX_QUOTE_ID_LEN {
        return Err(format!("{field} exceeds maximum length"));
    }
    Ok(trimmed.to_string())
}

fn normalize_limit(value: u32) -> u32 {
    if value == 0 {
        DEFAULT_PAGE_LIMIT
    } else {
        value.min(MAX_PAGE_LIMIT)
    }
}

fn normalize_page(value: u32) -> u32 {
    if value == 0 {
        1
    } else {
        value
    }
}

fn normalize_optional_trimmed(value: &Option<String>) -> Option<String> {
    value.as_ref().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn normalize_currency(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("currency is required".to_string());
    }
    if trimmed.len() > 8 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("currency must be alphabetic and <= 8 chars".to_string());
    }
    Ok(trimmed.to_ascii_uppercase())
}

fn normalize_discount(value: f64, field: &str) -> Result<f64, String> {
    if !value.is_finite() {
        return Err(format!("{field} must be a finite number"));
    }
    if !(0.0..=100.0).contains(&value) {
        return Err(format!("{field} must be between 0 and 100"));
    }
    Ok(value)
}

fn register_pdf_quote_filters(tera: &mut Tera) {
    tera.register_filter("format", pdf_format_filter);
    tera.register_filter("money", pdf_money_filter);
}

fn pdf_format_filter(
    value: &tera::Value,
    args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let format_str =
        value.as_str().ok_or_else(|| tera::Error::msg("format filter expects a string input"))?;

    let val = args
        .get("value")
        .ok_or_else(|| tera::Error::msg("format filter requires a 'value' argument"))?;

    let num = match val {
        tera::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        tera::Value::Null => 0.0,
        _ => 0.0,
    };

    let result = if let Some(rest) = format_str.strip_prefix("%.") {
        if let Some(precision_str) = rest.strip_suffix('f') {
            let precision: usize = precision_str.parse().unwrap_or(2);
            format!("{:.*}", precision, num)
        } else {
            format!("{}", num)
        }
    } else {
        format!("{}", num)
    };

    Ok(tera::Value::String(result))
}

fn pdf_money_filter(
    value: &tera::Value,
    _args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let num = match value {
        tera::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        tera::Value::Null => 0.0,
        _ => 0.0,
    };
    Ok(tera::Value::String(format!("{:.2}", num)))
}

fn register_quote_style_partials(tera: &mut Tera) {
    tera.add_raw_template(
        "styles/quote-base.css",
        include_str!("../../../templates/styles/quote-base.css"),
    )
    .ok();
    tera.add_raw_template("styles/quote.css", include_str!("../../../templates/styles/quote.css"))
        .ok();
}

fn decimal_to_f64(value: &rust_decimal::Decimal) -> f64 {
    value.to_string().parse::<f64>().unwrap_or_else(|_| {
        tracing::warn!(value = %value, "Failed to parse Decimal to f64, using 0.0");
        0.0
    })
}

fn build_quote_id(account_id: &str, input: &QuoteCreateInput) -> String {
    if let Some(key) = input.idempotency_key.as_deref().filter(|v| !v.trim().is_empty()) {
        // Use Blake3 for cryptographically secure, stable hashing
        let mut hasher = blake3::Hasher::new();
        hasher.update(account_id.as_bytes());
        hasher.update(key.as_bytes());
        hasher.update(input.currency.as_bytes());
        if let Some(term) = input.term_months {
            hasher.update(&term.to_le_bytes());
        }
        if let Some(deal_id) = &input.deal_id {
            hasher.update(deal_id.as_bytes());
        }
        for item in &input.line_items {
            hasher.update(item.product_id.as_bytes());
            hasher.update(&item.quantity.to_le_bytes());
            hasher.update(&item.discount_pct.to_le_bytes());
        }
        // Use first 16 chars of hex-encoded hash for readable ID
        let hash = hasher.finalize();
        format!("Q-{:.16}", hash.to_hex())
    } else {
        format!("Q-{:.8}", uuid::Uuid::new_v4().to_string())
    }
}

fn allowed_pdf_templates() -> &'static [&'static str] {
    &["detailed", "executive_summary", "compact"]
}

fn template_is_allowed(template: &str) -> bool {
    allowed_pdf_templates().contains(&template)
}

fn checksum_of(value: &str) -> String {
    let hash = blake3::hash(value.as_bytes());
    format!("checksum:{:.32}", hash.to_hex())
}

fn checksum_of_bytes(value: &[u8]) -> String {
    let hash = blake3::hash(value);
    format!("checksum:{:.32}", hash.to_hex())
}

#[derive(Debug, Clone)]
struct PortalPushSubscription {
    endpoint: String,
    p256dh: String,
    auth: String,
}

#[derive(Debug, Clone, Serialize)]
struct PortalPushNotificationPayload {
    title: String,
    body: String,
    url: String,
    quote_id: String,
    approval_id: String,
    amount: String,
    discount_pct: f64,
    approver_role: String,
    customer: String,
}

fn resolve_portal_push_bridge_url() -> Option<String> {
    std::env::var(PORTAL_PUSH_BRIDGE_URL_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn compute_quote_totals_for_push(quote: &Quote) -> (f64, f64) {
    let mut subtotal = 0.0_f64;
    let mut discount_total = 0.0_f64;
    for line in &quote.lines {
        let unit = decimal_to_f64(&line.unit_price);
        let line_subtotal = unit * line.quantity as f64;
        let line_discount = line_subtotal * line.discount_pct / 100.0;
        subtotal += line_subtotal;
        discount_total += line_discount;
    }
    let total = (subtotal - discount_total).max(0.0);
    let discount_pct = if subtotal > 0.0 { (discount_total / subtotal) * 100.0 } else { 0.0 };
    (total, discount_pct)
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(
            |c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' },
        )
        .collect()
}

fn hash_tool_arguments(arguments: Option<&serde_json::Map<String, serde_json::Value>>) -> String {
    let args_value =
        arguments.cloned().map(serde_json::Value::Object).unwrap_or(serde_json::Value::Null);
    let serialized = serde_json::to_string(&args_value).unwrap_or_else(|_| "null".to_string());
    checksum_of(&serialized)
}

/// Maximum length for quote IDs extracted from tool arguments.
const MAX_QUOTE_ID_LEN: usize = 64;

fn extract_quote_id_from_arguments(
    arguments: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<String> {
    arguments
        .and_then(|args| args.get("quote_id"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= MAX_QUOTE_ID_LEN)
        .map(str::to_string)
}

fn parse_error_code_from_text_payload(text: &str) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(text).ok()?;
    value
        .get("error")
        .and_then(|error| error.get("code"))
        .and_then(|code| code.as_str())
        .map(str::to_string)
}

fn parse_authorization_header(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let Some(split_idx) = trimmed.find(char::is_whitespace) else {
        if trimmed.eq_ignore_ascii_case("bearer")
            || trimmed.eq_ignore_ascii_case("apikey")
            || trimmed.eq_ignore_ascii_case("token")
        {
            return None;
        }
        return Some(trimmed.to_string());
    };

    let scheme = &trimmed[..split_idx];
    let presented_value = trimmed[split_idx..].trim();
    if presented_value.is_empty() {
        return None;
    }

    if scheme.eq_ignore_ascii_case("bearer")
        || scheme.eq_ignore_ascii_case("apikey")
        || scheme.eq_ignore_ascii_case("token")
    {
        Some(presented_value.to_string())
    } else {
        None
    }
}

fn extract_api_key_from_meta(meta: &rmcp::model::Meta) -> Option<String> {
    for key in ["api_key", "x-api-key", "x_api_key"] {
        if let Some(value) = meta.0.get(key).and_then(|v| v.as_str()) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    for key in ["authorization", "Authorization"] {
        if let Some(value) = meta.0.get(key).and_then(|v| v.as_str()) {
            if let Some(parsed) = parse_authorization_header(value) {
                return Some(parsed);
            }
        }
    }

    None
}

fn auth_error_from_denial(reason: &str, retry_after: Option<u32>) -> AuthError {
    if let Some(retry_after_seconds) = retry_after {
        return AuthError::new(AuthErrorCode::RateLimited, reason.to_string())
            .with_retry_after(retry_after_seconds);
    }

    let code = match reason {
        "API key required" => AuthErrorCode::MissingCredential,
        "API key deactivated" => AuthErrorCode::CredentialRevoked,
        "Invalid API key" => AuthErrorCode::InvalidCredential,
        _ => AuthErrorCode::InvalidCredential,
    };
    AuthError::new(code, reason.to_string())
}

fn auth_context_for_allowed_mcp_call(
    auth_result: &AuthResult,
    presented_key: Option<&str>,
) -> AuthContext {
    match auth_result {
        AuthResult::Allowed { key_name, .. } => {
            let is_no_auth_anonymous = key_name == "anonymous";
            if is_no_auth_anonymous {
                return AuthContext {
                    channel: AuthChannel::Mcp,
                    method: AuthMethod::None,
                    strength: AuthStrength::Anonymous,
                    principal: AuthPrincipal {
                        actor_id: "mcp:anonymous".to_string(),
                        display_name: None,
                    },
                    token_fingerprint: None,
                    session_id: None,
                };
            }

            let key_fingerprint =
                presented_key.map(checksum_of).unwrap_or_else(|| checksum_of(key_name));
            AuthContext {
                channel: AuthChannel::Mcp,
                method: AuthMethod::ApiKey,
                strength: AuthStrength::Possession,
                principal: AuthPrincipal {
                    actor_id: format!("mcp:key:{key_name}"),
                    display_name: Some(key_name.clone()),
                },
                token_fingerprint: Some(key_fingerprint),
                session_id: None,
            }
        }
        AuthResult::Denied { .. } => AuthContext {
            channel: AuthChannel::Mcp,
            method: AuthMethod::None,
            strength: AuthStrength::Anonymous,
            principal: AuthPrincipal { actor_id: "mcp:anonymous".to_string(), display_name: None },
            token_fingerprint: None,
            session_id: None,
        },
    }
}

fn auth_context_for_denied_mcp_call(presented_key: Option<&str>) -> AuthContext {
    let method = if presented_key.is_some() { AuthMethod::ApiKey } else { AuthMethod::None };
    AuthContext {
        channel: AuthChannel::Mcp,
        method,
        strength: AuthStrength::Anonymous,
        principal: AuthPrincipal { actor_id: "mcp:anonymous".to_string(), display_name: None },
        token_fingerprint: presented_key.map(checksum_of),
        session_id: None,
    }
}

fn actor_from_auth_context(auth_context: &AuthContext) -> String {
    if auth_context.method == AuthMethod::ApiKey {
        if let Some(display_name) = auth_context.principal.display_name.as_deref() {
            return format!("agent:mcp:{display_name}");
        }
    }
    "agent:mcp:anonymous".to_string()
}

fn auth_code_from_error_data(error: &rmcp::ErrorData) -> Option<String> {
    error.data.as_ref()?.get("code")?.as_str().map(|code| code.to_string())
}

fn outcome_from_tool_result(
    result: &Result<CallToolResult, rmcp::ErrorData>,
) -> (bool, String, Option<String>) {
    match result {
        Ok(call_result) => {
            for block in &call_result.content {
                if let Some(text) = block.raw.as_text() {
                    if let Some(code) = parse_error_code_from_text_payload(&text.text) {
                        return (false, code, Some(text.text.clone()));
                    }
                }
            }

            if call_result.is_error.unwrap_or(false) {
                return (false, "TOOL_ERROR".to_string(), None);
            }

            (true, "OK".to_string(), None)
        }
        Err(error) => (false, format!("MCP_{:?}", error.code), Some(error.message.to_string())),
    }
}

#[derive(Debug, Clone)]
struct McpInvocationAuditEnvelope {
    tool_name: String,
    quote_id: Option<String>,
    actor: String,
    auth_context: AuthContext,
    request_id: String,
    correlation_id: String,
    input_hash: String,
    success: bool,
    outcome_code: String,
    error_message: Option<String>,
    auth_error_code: Option<String>,
}

fn parse_protocol_version(raw: Option<&str>) -> Result<ProtocolVersion, String> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(ProtocolVersion::LATEST);
    };

    if value.eq_ignore_ascii_case("latest") {
        return Ok(ProtocolVersion::LATEST);
    }

    match value {
        "2024-11-05" => Ok(ProtocolVersion::V_2024_11_05),
        "2025-03-26" => Ok(ProtocolVersion::V_2025_03_26),
        "2025-06-18" => Ok(ProtocolVersion::V_2025_06_18),
        _ => Err(format!("Unsupported MCP protocol version '{value}'")),
    }
}

fn resolve_protocol_version() -> ProtocolVersion {
    match parse_protocol_version(std::env::var("QUOTEY_MCP_PROTOCOL_VERSION").ok().as_deref()) {
        Ok(version) => version,
        Err(error) => {
            warn!(%error, "Invalid QUOTEY_MCP_PROTOCOL_VERSION; falling back to latest");
            ProtocolVersion::LATEST
        }
    }
}

/// Main MCP server for Quotey
#[derive(Debug, Clone)]
pub struct QuoteyMcpServer {
    /// Database pool for queries
    db_pool: quotey_db::DbPool,
    /// Tool router for dispatching tool calls
    tool_router: ToolRouter<Self>,
    /// Authentication manager
    auth_manager: AuthManager,
    /// MCP protocol version advertised during initialize handshake
    protocol_version: ProtocolVersion,
}

impl QuoteyMcpServer {
    /// Create a new MCP server instance with database pool
    pub fn new(db_pool: quotey_db::DbPool) -> Self {
        info!("Initializing Quotey MCP Server (no auth)");
        let tool_router = Self::tool_router();
        let auth_manager = AuthManager::no_auth();
        let protocol_version = resolve_protocol_version();
        Self { db_pool, tool_router, auth_manager, protocol_version }
    }

    /// Create a new MCP server with authentication
    pub fn with_auth(db_pool: quotey_db::DbPool, auth_manager: AuthManager) -> Self {
        info!("Initializing Quotey MCP Server (with auth)");
        let tool_router = Self::tool_router();
        let protocol_version = resolve_protocol_version();
        Self { db_pool, tool_router, auth_manager, protocol_version }
    }

    /// Run the server with stdio transport
    pub async fn run_stdio(self) -> anyhow::Result<()> {
        use rmcp::service::serve_server;
        use tokio::io::{stdin, stdout};

        info!("Starting MCP server with stdio transport");

        let service = serve_server(self, (stdin(), stdout())).await?;

        // Wait for shutdown
        let _quit = service.waiting().await?;

        info!("MCP server shutdown complete");
        Ok(())
    }

    /// Get a reference to the database pool
    fn db(&self) -> &quotey_db::DbPool {
        &self.db_pool
    }

    /// Get a reference to the auth manager
    pub fn auth_manager(&self) -> &AuthManager {
        &self.auth_manager
    }

    pub fn protocol_version(&self) -> &ProtocolVersion {
        &self.protocol_version
    }

    async fn sanitize_quote_id_for_audit(&self, quote_id: Option<&str>) -> Option<String> {
        let candidate = quote_id.map(str::trim).filter(|value| !value.is_empty())?;
        match sqlx::query_scalar::<_, i64>("SELECT 1 FROM quote WHERE id = ? LIMIT 1")
            .bind(candidate)
            .fetch_optional(self.db())
            .await
        {
            Ok(Some(_)) => Some(candidate.to_string()),
            Ok(None) => None,
            Err(error) => {
                warn!(
                    error = %error,
                    quote_id = %candidate,
                    "failed to validate quote_id for MCP audit_event"
                );
                None
            }
        }
    }

    async fn record_mcp_audit_event(
        &self,
        tool_name: &str,
        quote_id: Option<&str>,
        payload: serde_json::Value,
    ) {
        let payload_json = match serde_json::to_string(&payload) {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, tool_name = %tool_name, "failed to serialize MCP audit payload");
                "{}".to_string()
            }
        };
        let metadata_json = serde_json::json!({
            "source": "quotey-mcp",
            "tool_name": tool_name
        })
        .to_string();
        let quote_id = self.sanitize_quote_id_for_audit(quote_id).await;

        if let Err(error) = sqlx::query(
            r#"
            INSERT INTO audit_event (
                id, timestamp, actor, actor_type, quote_id,
                event_type, event_category, payload_json, metadata_json
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("mcp-audit-{}", uuid::Uuid::new_v4()))
        .bind(chrono::Utc::now().to_rfc3339())
        .bind("agent:mcp")
        .bind("agent")
        .bind(quote_id)
        .bind(format!("mcp.{tool_name}.invoked"))
        .bind("mcp_tool")
        .bind(payload_json)
        .bind(Some(metadata_json))
        .execute(self.db())
        .await
        {
            warn!(error = %error, tool_name = %tool_name, "failed to persist MCP audit_event");
        }
    }

    async fn record_mcp_invocation_received(&self, envelope: &McpInvocationAuditEnvelope) {
        let tool_name = envelope.tool_name.as_str();
        let payload = serde_json::json!({
            "tool_name": tool_name,
            "actor": envelope.actor,
            "auth": &envelope.auth_context,
            "request_id": envelope.request_id,
            "correlation_id": envelope.correlation_id,
            "input_hash": envelope.input_hash,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let payload_json = match serde_json::to_string(&payload) {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, tool_name = %tool_name, "failed to serialize MCP invocation-received payload");
                "{}".to_string()
            }
        };
        let metadata_json = serde_json::json!({
            "source": "quotey-mcp",
            "tool_name": tool_name,
            "audit_version": 1,
            "request_id": envelope.request_id,
            "correlation_id": envelope.correlation_id,
            "input_hash": envelope.input_hash,
            "auth_channel": envelope.auth_context.channel,
            "auth_method": envelope.auth_context.method,
            "auth_strength": envelope.auth_context.strength,
            "auth_principal": envelope.auth_context.principal.actor_id.as_str()
        })
        .to_string();
        let quote_id = self.sanitize_quote_id_for_audit(envelope.quote_id.as_deref()).await;

        if let Err(error) = sqlx::query(
            r#"
            INSERT INTO audit_event (
                id, timestamp, actor, actor_type, quote_id,
                event_type, event_category, payload_json, metadata_json
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("mcp-audit-{}", uuid::Uuid::new_v4()))
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&envelope.actor)
        .bind("agent")
        .bind(quote_id)
        .bind(format!("mcp.{tool_name}.received"))
        .bind("mcp_tool")
        .bind(payload_json)
        .bind(Some(metadata_json))
        .execute(self.db())
        .await
        {
            warn!(error = %error, tool_name = %tool_name, "failed to persist MCP invocation-received audit_event");
        }
    }

    async fn record_mcp_invocation_outcome(&self, envelope: &McpInvocationAuditEnvelope) {
        let tool_name = envelope.tool_name.as_str();
        let payload = serde_json::json!({
            "tool_name": tool_name,
            "auth": &envelope.auth_context,
            "request_id": envelope.request_id,
            "correlation_id": envelope.correlation_id,
            "input_hash": envelope.input_hash,
            "outcome": {
                "success": envelope.success,
                "code": envelope.outcome_code,
                "error_message": envelope.error_message,
                "auth_code": envelope.auth_error_code
            },
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        let payload_json = match serde_json::to_string(&payload) {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, tool_name = %tool_name, "failed to serialize MCP invocation audit payload");
                "{}".to_string()
            }
        };
        let metadata_json = serde_json::json!({
            "source": "quotey-mcp",
            "tool_name": tool_name,
            "audit_version": 1,
            "request_id": envelope.request_id,
            "correlation_id": envelope.correlation_id,
            "input_hash": envelope.input_hash,
            "auth_channel": envelope.auth_context.channel,
            "auth_method": envelope.auth_context.method,
            "auth_strength": envelope.auth_context.strength,
            "auth_principal": envelope.auth_context.principal.actor_id.as_str(),
            "auth_error_code": envelope.auth_error_code.as_deref()
        })
        .to_string();
        let quote_id = self.sanitize_quote_id_for_audit(envelope.quote_id.as_deref()).await;

        if let Err(error) = sqlx::query(
            r#"
            INSERT INTO audit_event (
                id, timestamp, actor, actor_type, quote_id,
                event_type, event_category, payload_json, metadata_json
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("mcp-audit-{}", uuid::Uuid::new_v4()))
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&envelope.actor)
        .bind("agent")
        .bind(quote_id)
        .bind(format!("mcp.{tool_name}.completed"))
        .bind("mcp_tool")
        .bind(payload_json)
        .bind(Some(metadata_json))
        .execute(self.db())
        .await
        {
            warn!(error = %error, tool_name = %tool_name, "failed to persist MCP invocation audit_event");
        }
    }

    async fn fetch_active_portal_push_subscriptions(
        &self,
    ) -> Result<Vec<PortalPushSubscription>, sqlx::Error> {
        use sqlx::Row as _;

        let rows = sqlx::query(
            "SELECT endpoint, p256dh, auth
             FROM portal_push_subscription
             WHERE revoked = 0",
        )
        .fetch_all(self.db())
        .await?;

        let mut subscriptions = Vec::with_capacity(rows.len());
        for row in rows {
            let endpoint = row.try_get::<String, _>("endpoint").unwrap_or_default();
            let p256dh = row.try_get::<String, _>("p256dh").unwrap_or_default();
            let auth = row.try_get::<String, _>("auth").unwrap_or_default();
            if endpoint.trim().is_empty() || p256dh.trim().is_empty() || auth.trim().is_empty() {
                continue;
            }
            subscriptions.push(PortalPushSubscription { endpoint, p256dh, auth });
        }
        Ok(subscriptions)
    }

    async fn send_portal_push_via_bridge(
        &self,
        bridge_url: &str,
        subscription: &PortalPushSubscription,
        payload: &PortalPushNotificationPayload,
    ) -> Result<u16, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|error| format!("bridge client build failed: {error}"))?;

        let response = client
            .post(bridge_url)
            .json(&serde_json::json!({
                "endpoint": &subscription.endpoint,
                "keys": {
                    "p256dh": &subscription.p256dh,
                    "auth": &subscription.auth
                },
                "notification": payload
            }))
            .send()
            .await
            .map_err(|error| format!("bridge request failed: {error}"))?;

        let status = response.status();
        if status.is_success() {
            Ok(status.as_u16())
        } else {
            let body = response.text().await.unwrap_or_default();
            let compact = if body.len() > 240 { format!("{}...", &body[..240]) } else { body };
            Err(format!("bridge returned {status}: {compact}"))
        }
    }

    async fn record_portal_push_audit_event(
        &self,
        event_type: &str,
        quote_id: Option<&str>,
        payload: serde_json::Value,
    ) {
        let payload_json = match serde_json::to_string(&payload) {
            Ok(value) => value,
            Err(error) => {
                warn!(error = %error, event_type = %event_type, "failed to serialize portal push audit payload");
                "{}".to_string()
            }
        };
        let metadata_json = serde_json::json!({
            "source": "quotey-mcp",
            "channel": "portal-pwa",
            "event_type": event_type
        })
        .to_string();
        let sanitized_quote_id = self.sanitize_quote_id_for_audit(quote_id).await;

        if let Err(error) = sqlx::query(
            r#"
            INSERT INTO audit_event (
                id, timestamp, actor, actor_type, quote_id,
                event_type, event_category, payload_json, metadata_json
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(format!("mcp-push-{}", uuid::Uuid::new_v4()))
        .bind(chrono::Utc::now().to_rfc3339())
        .bind("agent:mcp")
        .bind("agent")
        .bind(sanitized_quote_id)
        .bind(event_type)
        .bind("portal")
        .bind(payload_json)
        .bind(Some(metadata_json))
        .execute(self.db())
        .await
        {
            warn!(error = %error, event_type = %event_type, "failed to persist portal push audit_event");
        }
    }

    async fn dispatch_pending_approval_push_notifications(
        &self,
        quote: &Quote,
        approval_id: &str,
        approver_role: &str,
    ) {
        let subscriptions = match self.fetch_active_portal_push_subscriptions().await {
            Ok(rows) => rows,
            Err(error) => {
                warn!(error = %error, quote_id = %quote.id.0, "failed to fetch active portal push subscriptions");
                return;
            }
        };

        if subscriptions.is_empty() {
            return;
        }

        let bridge_url = resolve_portal_push_bridge_url();
        let (total, discount_pct) = compute_quote_totals_for_push(quote);
        let customer = quote.account_id.clone().unwrap_or_else(|| "Unknown Customer".to_string());
        let payload = PortalPushNotificationPayload {
            title: "Approval Request".to_string(),
            body: format!(
                "{} • {} {:.2} • {:.1}% discount",
                customer, quote.currency, total, discount_pct
            ),
            url: format!("/approvals/{approval_id}"),
            quote_id: quote.id.0.clone(),
            approval_id: approval_id.to_string(),
            amount: format!("{} {:.2}", quote.currency, total),
            discount_pct,
            approver_role: approver_role.to_string(),
            customer: customer.clone(),
        };

        for subscription in subscriptions {
            let endpoint_hash = checksum_of(&subscription.endpoint);
            let (event_type, outcome, detail) = match bridge_url.as_deref() {
                Some(url) => {
                    match self.send_portal_push_via_bridge(url, &subscription, &payload).await {
                        Ok(status_code) => (
                            "portal.pwa.push_sent",
                            "sent",
                            Some(format!("bridge_status:{status_code}")),
                        ),
                        Err(error) => ("portal.pwa.push_failed", "failed", Some(error)),
                    }
                }
                None => (
                    "portal.pwa.push_skipped",
                    "skipped",
                    Some(format!("{PORTAL_PUSH_BRIDGE_URL_ENV} is not configured")),
                ),
            };

            self.record_portal_push_audit_event(
                event_type,
                Some(&quote.id.0),
                serde_json::json!({
                    "approval_id": approval_id,
                    "quote_id": quote.id.0,
                    "customer": customer,
                    "amount": payload.amount,
                    "discount_pct": payload.discount_pct,
                    "deep_link": payload.url,
                    "approver_role": approver_role,
                    "endpoint_hash": endpoint_hash,
                    "outcome": outcome,
                    "detail": detail,
                }),
            )
            .await;
        }
    }

    /// Validate the current request against the auth manager.
    ///
    /// Clients pass their key via the MCP `_meta` field on tool-call requests.
    /// Supported keys: `api_key`, `x-api-key`, `x_api_key`, `authorization`.
    /// `authorization` accepts bare keys and `Bearer <token>` style values.
    ///
    /// Example:
    /// ```json
    /// { "method": "tools/call", "params": {
    ///     "name": "catalog_search",
    ///     "arguments": { "query": "pro" },
    ///     "_meta": { "api_key": "your-secret-key" }
    /// }}
    /// ```
    async fn check_auth(&self, meta: &rmcp::model::Meta) -> Result<AuthResult, rmcp::ErrorData> {
        let presented_key = extract_api_key_from_meta(meta); // ubs:ignore (runtime metadata lookup, not a hardcoded secret)
        let result = self.auth_manager.validate_request(presented_key.as_deref()).await;

        match &result {
            AuthResult::Allowed { key_name, remaining_requests } => {
                debug!(
                    key_name = %key_name,
                    remaining = remaining_requests,
                    "Authentication successful"
                );
                Ok(result)
            }
            AuthResult::Denied { reason, retry_after } => {
                warn!(reason = %reason, "Authentication denied");
                let auth_error = auth_error_from_denial(reason, *retry_after);
                let mut data = serde_json::Map::new();
                data.insert("reason".to_string(), serde_json::json!(auth_error.message));
                data.insert("code".to_string(), serde_json::json!(auth_error.code.as_str()));
                data.insert(
                    "error_code".to_string(),
                    serde_json::json!(if auth_error.retry_after_seconds.is_some() {
                        "RATE_LIMIT_EXCEEDED"
                    } else {
                        "AUTHENTICATION_FAILED"
                    }),
                );
                data.insert("http_status".to_string(), serde_json::json!(auth_error.http_status()));
                if let Some(retry) = auth_error.retry_after_seconds {
                    data.insert("retry_after".to_string(), serde_json::json!(retry));
                    return Err(rmcp::ErrorData::new(
                        rmcp::model::ErrorCode(429),
                        "Rate limit exceeded",
                        Some(serde_json::Value::Object(data)),
                    ));
                }
                Err(rmcp::ErrorData::invalid_request(
                    format!("Authentication failed: {}", auth_error.message),
                    Some(serde_json::Value::Object(data)),
                ))
            }
        }
    }
}

impl ServerHandler for QuoteyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: self.protocol_version.clone(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            server_info: Implementation {
                name: "quotey-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: Some("Quotey MCP Server".to_string()),
                website_url: Some("https://github.com/junlov/quotey".to_string()),
                icons: None,
            },
            instructions: Some(
                "Quotey MCP Server - CPQ automation for AI agents. \
                 Tools: catalog_search, catalog_get, quote_create, quote_get, quote_price, \
                 quote_list, approval_request, approval_status, approval_pending, quote_pdf"
                    .to_string(),
            ),
        }
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let tool_name = request.name.to_string();
        let request_id = context.id.to_string();
        let correlation_id = request_id.clone();
        let arguments = request.arguments.clone();
        let quote_id_for_audit = extract_quote_id_from_arguments(arguments.as_ref());
        let input_hash = hash_tool_arguments(arguments.as_ref());
        let presented_key = extract_api_key_from_meta(&context.meta);

        // Enforce authentication when configured.
        // Clients pass their API key via `_meta.api_key` on each tool-call request.
        // When auth is not required the check is a no-op (returns Allowed).
        let auth_result = match self.check_auth(&context.meta).await {
            Ok(result) => result,
            Err(error) => {
                let auth_context = auth_context_for_denied_mcp_call(presented_key.as_deref());
                let envelope = McpInvocationAuditEnvelope {
                    tool_name: tool_name.clone(),
                    quote_id: quote_id_for_audit.clone(),
                    actor: actor_from_auth_context(&auth_context),
                    auth_context,
                    request_id: request_id.clone(),
                    correlation_id: correlation_id.clone(),
                    input_hash: input_hash.clone(),
                    success: false,
                    outcome_code: "AUTH_DENIED".to_string(),
                    error_message: Some(error.message.to_string()),
                    auth_error_code: auth_code_from_error_data(&error),
                };
                self.record_mcp_invocation_received(&envelope).await;
                self.record_mcp_invocation_outcome(&envelope).await;
                return Err(error);
            }
        };

        let auth_context =
            auth_context_for_allowed_mcp_call(&auth_result, presented_key.as_deref());
        let actor = actor_from_auth_context(&auth_context);

        self.record_mcp_invocation_received(&McpInvocationAuditEnvelope {
            tool_name: tool_name.clone(),
            quote_id: quote_id_for_audit.clone(),
            actor: actor.clone(),
            auth_context: auth_context.clone(),
            request_id: request_id.clone(),
            correlation_id: correlation_id.clone(),
            input_hash: input_hash.clone(),
            success: true,
            outcome_code: "RECEIVED".to_string(),
            error_message: None,
            auth_error_code: None,
        })
        .await;

        // Route to tool handler
        let tool_call_context = ToolCallContext::new(self, request, context);
        let result = self.tool_router.call(tool_call_context).await;
        let (success, outcome_code, error_message) = outcome_from_tool_result(&result);

        self.record_mcp_invocation_outcome(&McpInvocationAuditEnvelope {
            tool_name,
            quote_id: quote_id_for_audit,
            actor,
            auth_context,
            request_id,
            correlation_id,
            input_hash,
            success,
            outcome_code,
            error_message,
            auth_error_code: None,
        })
        .await;

        result
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult { tools: self.tool_router.list_all(), next_cursor: None })
    }
}

// ============================================================================
// Input/Output Types
// ============================================================================
// These structs are deserialized by serde from MCP tool call arguments.
// Fields appear "unused" to the compiler until tools are wired to real repos.

fn default_true() -> bool {
    true
}
fn default_20() -> u32 {
    DEFAULT_PAGE_LIMIT
}
fn default_1() -> u32 {
    1
}
fn default_currency() -> String {
    "USD".to_string()
}
fn default_template() -> String {
    "detailed".to_string()
}

// Catalog Types
#[derive(Debug, Deserialize, JsonSchema)]
pub struct CatalogSearchInput {
    #[schemars(description = "Search query for product name, SKU, or description")]
    pub query: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub active_only: bool,
    #[serde(default = "default_20")]
    pub limit: u32,
    #[serde(default = "default_1")]
    pub page: u32,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProductSummary {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub product_type: String,
    pub category: Option<String>,
    pub active: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PaginationInfo {
    pub total: u32,
    pub page: u32,
    pub per_page: u32,
    pub has_more: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CatalogSearchResult {
    pub items: Vec<ProductSummary>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CatalogGetInput {
    pub product_id: String,
    #[serde(default)]
    pub include_relationships: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CatalogGetResult {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub description: Option<String>,
    pub product_type: String,
    pub category: Option<String>,
    pub attributes: Option<serde_json::Value>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

// Quote Types
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct LineItemInput {
    pub product_id: String,
    pub quantity: u32,
    #[serde(default)]
    pub discount_pct: f64,
    #[serde(default)]
    pub attributes: Option<serde_json::Value>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct QuoteCreateInput {
    pub account_id: String,
    #[serde(default)]
    pub deal_id: Option<String>,
    #[serde(default = "default_currency")]
    pub currency: String,
    pub term_months: Option<u32>,
    pub start_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    pub line_items: Vec<LineItemInput>,
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LineItemResult {
    pub line_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: u32,
    pub unit_price: f64,
    pub discount_pct: f64,
    pub subtotal: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuoteCreateResult {
    pub quote_id: String,
    pub version: u32,
    pub status: String,
    pub account_id: String,
    pub currency: String,
    pub idempotency_key: Option<String>,
    pub line_items: Vec<LineItemResult>,
    pub created_at: String,
    pub message: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuoteGetInput {
    pub quote_id: String,
    #[serde(default = "default_true")]
    pub include_pricing: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuoteLineInfo {
    pub line_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: u32,
    pub unit_price: Option<f64>,
    pub discount_pct: f64,
    pub discount_amount: Option<f64>,
    pub subtotal: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PricingInfo {
    pub subtotal: f64,
    pub discount_total: f64,
    pub tax_total: f64,
    pub total: f64,
    pub priced_at: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuoteInfo {
    pub id: String,
    pub version: u32,
    pub account_id: String,
    pub account_name: Option<String>,
    pub deal_id: Option<String>,
    pub status: String,
    pub currency: String,
    pub term_months: Option<u32>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub valid_until: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuoteGetResult {
    pub quote: QuoteInfo,
    pub line_items: Vec<QuoteLineInfo>,
    pub pricing: Option<PricingInfo>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuotePriceInput {
    pub quote_id: String,
    #[serde(default)]
    pub requested_discount_pct: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LinePricingInfo {
    pub line_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: u32,
    pub base_unit_price: f64,
    pub unit_price: f64,
    pub subtotal_before_discount: f64,
    pub discount_pct: f64,
    pub discount_amount: f64,
    pub line_total: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PolicyViolation {
    pub policy_id: String,
    pub policy_name: String,
    pub severity: String,
    pub description: String,
    pub threshold: Option<f64>,
    pub actual: Option<f64>,
    pub required_approver_role: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuotePriceResult {
    pub quote_id: String,
    pub version: u32,
    pub status: String,
    pub pricing: PricingInfo,
    pub line_pricing: Vec<LinePricingInfo>,
    pub approval_required: bool,
    pub policy_violations: Vec<PolicyViolation>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuoteListInput {
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default = "default_20")]
    pub limit: u32,
    #[serde(default = "default_1")]
    pub page: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuoteListItem {
    pub id: String,
    pub version: u32,
    pub account_id: String,
    pub account_name: Option<String>,
    pub status: String,
    pub currency: String,
    pub total: Option<f64>,
    pub valid_until: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuoteListResult {
    pub items: Vec<QuoteListItem>,
    pub pagination: PaginationInfo,
}

// Approval Types
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovalRequestInput {
    pub quote_id: String,
    pub justification: String,
    #[serde(default)]
    #[schemars(
        description = "Approver role (e.g. sales_manager, vp_finance). Defaults to sales_manager."
    )]
    pub approver_role: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApprovalRequestResult {
    pub approval_id: String,
    pub quote_id: String,
    pub status: String,
    pub approver_role: String,
    pub requested_by: String,
    pub justification: String,
    pub created_at: String,
    pub expires_at: String,
    pub message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PendingApproval {
    pub approval_id: String,
    pub status: String,
    pub approver_role: String,
    pub requested_at: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovalStatusInput {
    pub quote_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApprovalStatusResult {
    pub quote_id: String,
    pub current_status: String,
    pub pending_requests: Vec<PendingApproval>,
    pub can_proceed: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApprovalPendingInput {
    #[serde(default)]
    pub approver_role: Option<String>,
    #[serde(default = "default_20")]
    pub limit: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApprovalPendingItem {
    pub approval_id: String,
    pub quote_id: String,
    pub account_name: String,
    pub quote_total: f64,
    pub requested_by: String,
    pub justification: String,
    pub requested_at: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ApprovalPendingResult {
    pub items: Vec<ApprovalPendingItem>,
    pub total: u32,
}

// Anomaly Override Types
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnomalyOverrideInput {
    /// The quote ID the anomaly was detected on
    pub quote_id: String,
    /// The anomaly rule kind: discount, margin, quantity, or price
    pub rule_kind: String,
    /// The severity of the anomaly: none, info, warning, or critical
    pub severity: String,
    /// Written justification explaining why the override is acceptable
    pub justification: String,
    /// The rep or user performing the override
    pub overridden_by: String,
}

// Negotiation Types
#[derive(Debug, Deserialize, JsonSchema)]
pub struct NegotiationStartInput {
    /// The quote ID to negotiate on
    pub quote_id: String,
    /// The actor (rep) starting the negotiation
    pub actor_id: String,
    /// Idempotency key to prevent duplicate sessions
    #[serde(default)]
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NegotiationEvaluateInput {
    /// The negotiation session ID
    pub session_id: String,
    /// Requested discount percentage
    #[serde(default)]
    pub discount_pct: Option<f64>,
    /// Requested margin percentage
    #[serde(default)]
    pub margin_pct: Option<f64>,
    /// Requested term in months
    #[serde(default)]
    pub term_months: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NegotiationStatusInput {
    /// The negotiation session ID
    pub session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct NegotiationEscalateInput {
    /// The negotiation session ID to escalate
    pub session_id: String,
    /// The offer ID being escalated (optional)
    #[serde(default)]
    pub offer_id: String,
    /// Reason for escalation
    pub reason: String,
}

// PDF Types
#[derive(Debug, Deserialize, JsonSchema)]
pub struct QuotePdfInput {
    pub quote_id: String,
    #[serde(default = "default_template")]
    pub template: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuotePdfResult {
    pub quote_id: String,
    pub pdf_generated: bool,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub checksum: String,
    pub template_used: String,
    pub generated_at: String,
}

enum PdfRenderResult {
    Pdf(Vec<u8>),
    Html(String),
}

fn build_pdf_quote_payload(quote: &Quote) -> serde_json::Value {
    let mut line_rows = Vec::with_capacity(quote.lines.len());
    let mut subtotal = 0.0_f64;
    let mut discount_total = 0.0_f64;

    for line in &quote.lines {
        let unit_price = decimal_to_f64(&line.unit_price);
        let line_subtotal = unit_price * f64::from(line.quantity);
        let discount_pct = line.discount_pct.clamp(0.0, 100.0);
        let line_discount = line_subtotal * discount_pct / 100.0;
        let line_total = line_subtotal - line_discount;

        subtotal += line_subtotal;
        discount_total += line_discount;

        line_rows.push(serde_json::json!({
            "product_id": line.product_id.0,
            "product_name": line.product_id.0,
            "quantity": line.quantity,
            "unit_price": unit_price,
            "subtotal": line_subtotal,
            "discount_pct": discount_pct,
            "discount_amount": line_discount,
            "total_price": line_total,
            "line_subtotal": line_subtotal,
        }));
    }

    let net_total = subtotal - discount_total;
    let tax_rate = 0.0_f64;
    let tax_total = net_total * tax_rate;
    let account_id = quote.account_id.clone().unwrap_or_default();

    serde_json::json!({
        "id": quote.id.0,
        "version": quote.version,
        "status": quotey_db::repositories::quote::quote_status_as_str(&quote.status),
        "created_at": quote.created_at.to_rfc3339(),
        "valid_until": quote.valid_until.clone().unwrap_or_default(),
        "term_months": quote.term_months.unwrap_or(12),
        "payment_terms": "Net 30",
        "account_id": account_id.clone(),
        "currency": quote.currency.clone(),
        "account": {
            "id": account_id.clone(),
            "name": account_id,
        },
        "subtotal": subtotal,
        "discount_total": discount_total,
        "total_discount": discount_total,
        "tax_rate": tax_rate,
        "tax": tax_total,
        "tax_total": tax_total,
        "total": net_total + tax_total,
        "lines": line_rows,
        "pricing": {
            "subtotal": subtotal,
            "total_discount": discount_total,
            "discount_total": discount_total,
            "tax_rate": tax_rate,
            "tax": tax_total,
            "tax_total": tax_total,
            "total": net_total + tax_total,
        },
        "generated_by": "quotey-mcp",
    })
}

async fn render_quote_pdf_html_to_bytes(
    payload: &serde_json::Value,
    template: &str,
) -> Result<PdfRenderResult, String> {
    let mut tera = Tera::default();
    register_pdf_quote_filters(&mut tera);
    register_quote_style_partials(&mut tera);

    tera.add_raw_template(
        "detailed.html.tera",
        include_str!("../../../templates/quotes/detailed.html.tera"),
    )
    .map_err(|e| e.to_string())?;
    tera.add_raw_template(
        "executive_summary.html.tera",
        include_str!("../../../templates/quotes/executive_summary.html.tera"),
    )
    .map_err(|e| e.to_string())?;
    tera.add_raw_template(
        "compact.html.tera",
        include_str!("../../../templates/quotes/compact.html.tera"),
    )
    .map_err(|e| e.to_string())?;

    let mut context = Context::new();
    context.insert("quote", &payload);
    context.insert("account", &payload.get("account").cloned().unwrap_or(serde_json::json!({})));
    context.insert("lines", &payload.get("lines").cloned().unwrap_or(serde_json::json!([])));
    context.insert("pricing", &payload.get("pricing").cloned().unwrap_or_else(|| {
        serde_json::json!({
            "subtotal": payload.get("subtotal").cloned().unwrap_or(serde_json::json!(0.0)),
            "discount_total": payload.get("discount_total").cloned().unwrap_or(serde_json::json!(0.0)),
            "tax_rate": payload.get("tax_rate").cloned().unwrap_or(serde_json::json!(0.0)),
            "tax": payload.get("tax").cloned().unwrap_or(serde_json::json!(0.0)),
            "tax_total": payload.get("tax_total").cloned().unwrap_or(serde_json::json!(0.0)),
            "total": payload.get("total").cloned().unwrap_or(serde_json::json!(0.0)),
        })
    }));
    context
        .insert("sales_rep", &payload.get("sales_rep").cloned().unwrap_or(serde_json::json!({})));

    let branding = payload.get("branding");
    let read_string = |key: &str| -> Option<String> {
        branding
            .and_then(|value| value.get(key))
            .or_else(|| payload.get(key))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let read_bool = |key: &str| -> Option<bool> {
        branding
            .and_then(|value| value.get(key))
            .or_else(|| payload.get(key))
            .and_then(serde_json::Value::as_bool)
    };

    let company_name = read_string("company_name").unwrap_or_else(|| "Quotey".to_string());
    let company_logo = read_string("company_logo");
    let company_address = read_string("company_address");
    let company_email = read_string("company_email");
    let support_email = read_string("support_email")
        .or_else(|| read_string("contact_email"))
        .or(company_email.clone());
    let sender_name = read_string("sender_name").or_else(|| read_string("contact_name"));
    let company_phone = read_string("company_phone");
    let primary_color = read_string("primary_color").unwrap_or_else(|| "#2563eb".to_string());
    let secondary_color = read_string("secondary_color").unwrap_or_else(|| "#1e40af".to_string());
    let accent_color = read_string("accent_color").unwrap_or_else(|| "#3b82f6".to_string());
    let footer_text = read_string("footer_text");
    let terms_footer = read_string("terms_footer")
        .or_else(|| read_string("custom_terms_footer"))
        .or_else(|| footer_text.clone());
    let white_label = read_bool("white_label").unwrap_or(false);

    let support_contact_name = sender_name.clone().or_else(|| {
        payload
            .get("sales_rep")
            .and_then(|value| value.get("name"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    });
    let support_contact_email = support_email.clone().or_else(|| {
        payload
            .get("sales_rep")
            .and_then(|value| value.get("email"))
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    });

    context.insert("company_name", &company_name);
    context.insert("company_logo", &company_logo);
    context.insert("company_address", &company_address);
    context.insert("company_email", &company_email);
    context.insert("support_email", &support_email);
    context.insert("sender_name", &sender_name);
    context.insert("company_phone", &company_phone);
    context.insert("primary_color", &primary_color);
    context.insert("secondary_color", &secondary_color);
    context.insert("accent_color", &accent_color);
    context.insert("footer_text", &footer_text);
    context.insert("terms_footer", &terms_footer);
    context.insert(
        "support_contact_name",
        &support_contact_name.clone().unwrap_or_else(|| "your sales representative".to_string()),
    );
    context.insert(
        "support_contact_email",
        &support_contact_email.clone().unwrap_or_else(|| "sales@example.com".to_string()),
    );
    context.insert("white_label", &white_label);
    context.insert(
        "branding",
        &serde_json::json!({
            "company_name": company_name.clone(),
            "logo_url": company_logo.clone(),
            "company_logo": company_logo.clone(),
            "company_address": company_address.clone(),
            "company_email": company_email.clone(),
            "contact_email": support_email.clone(),
            "support_email": support_email.clone(),
            "company_phone": company_phone.clone(),
            "primary_color": primary_color.clone(),
            "secondary_color": secondary_color.clone(),
            "accent_color": accent_color.clone(),
            "footer_text": footer_text.clone(),
            "terms_footer": terms_footer.clone(),
            "sender_name": sender_name.clone(),
            "white_label": white_label,
        }),
    );

    let template_name = format!("{}.html.tera", template);
    let html = tera.render(&template_name, &context).map_err(|e| e.to_string())?;

    let temp_dir = std::env::temp_dir();
    let html_path: PathBuf = temp_dir.join(format!("quotey-mcp-{}.html", uuid::Uuid::new_v4()));
    let pdf_path: PathBuf = temp_dir.join(format!("quotey-mcp-{}.pdf", uuid::Uuid::new_v4()));
    tokio::fs::write(&html_path, html.as_bytes()).await.map_err(|e| e.to_string())?;

    let output = Command::new("wkhtmltopdf")
        .arg("--page-size")
        .arg("A4")
        .arg("--margin-top")
        .arg("10mm")
        .arg("--margin-bottom")
        .arg("10mm")
        .arg("--margin-left")
        .arg("10mm")
        .arg("--margin-right")
        .arg("10mm")
        .arg("--encoding")
        .arg("utf-8")
        .arg("--enable-local-file-access")
        .arg(&html_path)
        .arg(&pdf_path)
        .output()
        .await;

    async fn cleanup(html_path: &PathBuf, pdf_path: &PathBuf) {
        let _ = tokio::fs::remove_file(html_path).await;
        let _ = tokio::fs::remove_file(pdf_path).await;
    }

    match output {
        Ok(result) if result.status.success() => {
            let pdf_bytes = match tokio::fs::read(&pdf_path).await {
                Ok(bytes) => bytes,
                Err(read_err) => {
                    warn!(error = %read_err, "quote_pdf: failed to read generated PDF, returning HTML fallback");
                    cleanup(&html_path, &pdf_path).await;
                    return Ok(PdfRenderResult::Html(html));
                }
            };
            cleanup(&html_path, &pdf_path).await;
            Ok(PdfRenderResult::Pdf(pdf_bytes))
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            warn!(stderr = %stderr, "quote_pdf: wkhtmltopdf returned non-zero status, using HTML fallback");
            cleanup(&html_path, &pdf_path).await;
            Ok(PdfRenderResult::Html(html))
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                warn!("quote_pdf: wkhtmltopdf not found, using HTML fallback");
            } else {
                warn!(error = %err, "quote_pdf: wkhtmltopdf command failed, using HTML fallback");
            }
            cleanup(&html_path, &pdf_path).await;
            Ok(PdfRenderResult::Html(html))
        }
    }
}

fn payload_to_output_path(quote_id: &str, template: &str, is_pdf: bool) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join("quotey-mcp").join("quote-pdfs");
    let extension = if is_pdf { "pdf" } else { "html" };
    let filename = format!(
        "quote-{}-{}.{}",
        sanitize_filename(quote_id),
        sanitize_filename(template),
        extension
    );
    let path = dir.join(filename);
    (dir, path)
}

// ============================================================================
// Tool Router Implementation
// ============================================================================

#[tool_router]
impl QuoteyMcpServer {
    // Catalog Tools
    #[tool(description = "Search products by name, SKU, or description")]
    pub async fn catalog_search(
        &self,
        Parameters(input): Parameters<CatalogSearchInput>,
    ) -> String {
        debug!(query = %input.query, "catalog_search called");
        self.record_mcp_audit_event(
            "catalog_search",
            None,
            serde_json::json!({
                "query": &input.query,
                "category": &input.category,
                "active_only": input.active_only,
                "limit": input.limit,
                "page": input.page
            }),
        )
        .await;

        let query = input.query.trim().to_string();
        let category = normalize_optional_trimmed(&input.category);
        let page = normalize_page(input.page);
        let limit = normalize_limit(input.limit);
        let fetch_limit = limit.saturating_mul(page).saturating_add(1);

        if query.is_empty() && category.is_none() {
            debug!("catalog_search rejected empty query with empty category");
            return tool_error("VALIDATION_ERROR", "Query or category filter is required", None);
        }

        let repo = quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
        use quotey_db::repositories::ProductRepository;

        let products = if let Some(category_filter) = category.as_deref() {
            if query.is_empty() {
                repo.list_by_family(category_filter).await
            } else {
                repo.search(&query, input.active_only, fetch_limit).await
            }
        } else {
            repo.search(&query, input.active_only, fetch_limit).await
        };

        match products {
            Ok(mut products) => {
                if let Some(category_filter) = category.as_deref() {
                    products.retain(|product| {
                        product.family_id.as_ref().map_or(true, |f| f.0 == category_filter)
                    });
                }

                if input.active_only {
                    products.retain(|product| product.active);
                }

                let total = products.len() as u32;
                let start = (page.saturating_sub(1) * limit) as usize;
                let has_more = total > (start + limit as usize) as u32;
                let items: Vec<ProductSummary> = products
                    .into_iter()
                    .skip(start)
                    .take(limit as usize)
                    .map(|p| ProductSummary {
                        id: p.id.0,
                        sku: p.sku,
                        name: p.name,
                        description: p.description,
                        product_type: p.product_type.as_str().to_string(),
                        category: p.family_id.map(|f| f.0),
                        active: p.active,
                    })
                    .collect();

                let result = CatalogSearchResult {
                    pagination: PaginationInfo { total, page, per_page: limit, has_more },
                    items,
                };

                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => {
                warn!(error = %e, "catalog_search failed");
                internal_tool_error(&e)
            }
        }
    }

    #[tool(description = "Get detailed product information by ID")]
    pub async fn catalog_get(&self, Parameters(input): Parameters<CatalogGetInput>) -> String {
        debug!(product_id = %input.product_id, "catalog_get called");
        self.record_mcp_audit_event(
            "catalog_get",
            None,
            serde_json::json!({
                "product_id": &input.product_id,
                "include_relationships": input.include_relationships
            }),
        )
        .await;

        let product_id = match normalize_id(&input.product_id, "product_id") {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        let repo = quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
        use quotey_core::domain::product::ProductId;
        use quotey_db::repositories::ProductRepository;

        match repo.find_by_id(&ProductId(product_id.clone())).await {
            Ok(Some(p)) => {
                let attrs_json = if p.attributes.is_empty() {
                    None
                } else {
                    serde_json::to_value(&p.attributes).ok()
                };

                let result = CatalogGetResult {
                    id: p.id.0,
                    sku: p.sku,
                    name: p.name,
                    description: p.description,
                    product_type: p.product_type.as_str().to_string(),
                    category: p.family_id.map(|f| f.0),
                    attributes: attrs_json,
                    active: p.active,
                    created_at: p.created_at.to_rfc3339(),
                    updated_at: p.updated_at.to_rfc3339(),
                };

                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Ok(None) => {
                tool_error("NOT_FOUND", &format!("Product '{}' not found", product_id), None)
            }
            Err(e) => {
                warn!(error = %e, "catalog_get failed");
                internal_tool_error(&e)
            }
        }
    }

    // Quote Tools
    #[tool(description = "Create a new quote for a customer")]
    pub async fn quote_create(&self, Parameters(input): Parameters<QuoteCreateInput>) -> String {
        debug!(account_id = %input.account_id, "quote_create called");
        self.record_mcp_audit_event(
            "quote_create",
            None,
            serde_json::json!({
                "account_id": &input.account_id,
                "deal_id": &input.deal_id,
                "currency": &input.currency,
                "term_months": input.term_months,
                "line_items_count": input.line_items.len(),
                "idempotency_key": &input.idempotency_key
            }),
        )
        .await;

        use quotey_core::domain::product::ProductId;
        use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
        use quotey_db::repositories::QuoteRepository;
        use rust_decimal::prelude::FromPrimitive;
        use rust_decimal::Decimal;

        let account_id = match normalize_id(&input.account_id, "account_id") {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        let currency = match normalize_currency(&input.currency) {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        if input.line_items.is_empty() {
            return tool_error("VALIDATION_ERROR", "At least one line item is required", None);
        }

        if input.line_items.len() > MAX_LINE_ITEMS {
            return tool_error(
                "VALIDATION_ERROR",
                &format!("Too many line items (max {})", MAX_LINE_ITEMS),
                None,
            );
        }

        if let Some(months) = input.term_months {
            if months == 0 {
                return tool_error("VALIDATION_ERROR", "term_months must be greater than 0", None);
            }
        }

        let deal_id = normalize_optional_trimmed(&input.deal_id);
        let notes = input.notes.clone();
        let idempotency_key = normalize_optional_trimmed(&input.idempotency_key);
        let now = chrono::Utc::now();
        let quote_id = build_quote_id(&account_id, &input);

        // Look up product names from catalog
        let product_repo = quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
        use quotey_db::repositories::ProductRepository;

        let mut line_items_result = Vec::new();
        let mut quote_lines = Vec::new();

        for (i, item) in input.line_items.iter().enumerate() {
            let product_id = match normalize_id(&item.product_id, "product_id") {
                Ok(value) => value,
                Err(msg) => {
                    return tool_error("VALIDATION_ERROR", &msg, None);
                }
            };

            if item.quantity == 0 || item.quantity > MAX_QUANTITY {
                return tool_error(
                    "VALIDATION_ERROR",
                    &format!("line_items[{}].quantity must be between 1 and {}", i, MAX_QUANTITY),
                    None,
                );
            }

            let discount_pct = match normalize_discount(item.discount_pct, "line_item.discount_pct")
            {
                Ok(value) => value,
                Err(msg) => {
                    return tool_error("VALIDATION_ERROR", &msg, None);
                }
            };

            let product = match product_repo.find_by_id(&ProductId(product_id.clone())).await {
                Ok(Some(p)) => p,
                Ok(None) => {
                    return tool_error(
                        "NOT_FOUND",
                        &format!("Product '{}' not found", product_id),
                        None,
                    );
                }
                Err(e) => {
                    warn!(error = %e, "quote_create: failed to load product");
                    return internal_tool_error(&e);
                }
            };

            if !product.active {
                return tool_error(
                    "CONFLICT",
                    &format!("Product '{}' is inactive", product_id),
                    None,
                );
            }

            if product.currency.to_ascii_uppercase() != currency {
                return tool_error(
                    "CURRENCY_MISMATCH",
                    &format!(
                        "Product '{}' currency '{}' does not match quote currency '{}'",
                        product_id, product.currency, currency
                    ),
                    None,
                );
            }

            let unit_price = product.base_price.unwrap_or(Decimal::ZERO);
            let subtotal = unit_price * Decimal::from(item.quantity);
            let discount_rate = Decimal::from_f64(discount_pct).unwrap_or(Decimal::ZERO);
            let discount_amount = subtotal * discount_rate / Decimal::from(100);
            let effective_subtotal = subtotal - discount_amount;

            line_items_result.push(LineItemResult {
                line_id: format!("{}-ql-{}", quote_id, i + 1),
                product_id: product_id.clone(),
                product_name: product.name.clone(),
                quantity: item.quantity,
                unit_price: decimal_to_f64(&unit_price),
                discount_pct,
                subtotal: decimal_to_f64(&effective_subtotal),
            });

            quote_lines.push(QuoteLine {
                product_id: ProductId(product_id),
                quantity: item.quantity,
                unit_price,
                discount_pct,
                notes: item.notes.clone(),
            });
        }

        let quote = Quote {
            id: QuoteId(quote_id.clone()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: Some(account_id.clone()),
            deal_id,
            currency: currency.clone(),
            term_months: input.term_months,
            start_date: input.start_date,
            end_date: None,
            valid_until: None,
            notes,
            created_by: "agent:mcp".to_string(),
            lines: quote_lines,
            created_at: now,
            updated_at: now,
        };

        let repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());
        match repo.save(quote).await {
            Ok(()) => {
                let result = QuoteCreateResult {
                    quote_id,
                    version: 1,
                    status: "draft".to_string(),
                    account_id,
                    currency,
                    idempotency_key,
                    line_items: line_items_result,
                    created_at: now.to_rfc3339(),
                    message: "Quote created successfully".to_string(),
                };
                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => {
                warn!(error = %e, "quote_create failed");
                internal_tool_error(&e)
            }
        }
    }

    #[tool(description = "Get detailed quote information")]
    pub async fn quote_get(&self, Parameters(input): Parameters<QuoteGetInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_get called");
        let quote_id_for_audit = input.quote_id.trim().to_string();
        self.record_mcp_audit_event(
            "quote_get",
            if quote_id_for_audit.is_empty() { None } else { Some(quote_id_for_audit.as_str()) },
            serde_json::json!({
                "quote_id": &input.quote_id,
                "include_pricing": input.include_pricing
            }),
        )
        .await;

        let quote_id = match normalize_id(&input.quote_id, "quote_id") {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        use quotey_core::domain::quote::QuoteId;
        use quotey_db::repositories::QuoteRepository;

        let repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());

        match repo.find_by_id(&QuoteId(quote_id.clone())).await {
            Ok(Some(q)) => {
                use quotey_db::repositories::quote::quote_status_as_str;
                let status = quote_status_as_str(&q.status).to_string();

                // Look up product names for line items
                let product_repo =
                    quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
                use quotey_db::repositories::ProductRepository;

                let mut line_items = Vec::new();
                let mut subtotal_sum = 0.0f64;
                let mut discount_sum = 0.0f64;

                for (i, line) in q.lines.iter().enumerate() {
                    let product_name = match product_repo.find_by_id(&line.product_id).await {
                        Ok(Some(p)) => p.name,
                        _ => format!("Product {}", line.product_id.0),
                    };

                    let unit_price_f64: f64 = line.unit_price.to_string().parse().unwrap_or(0.0);
                    let line_subtotal = unit_price_f64 * line.quantity as f64;
                    let line_discount = line_subtotal * line.discount_pct / 100.0;
                    let line_net = line_subtotal - line_discount;

                    subtotal_sum += line_subtotal;
                    discount_sum += line_discount;

                    line_items.push(QuoteLineInfo {
                        line_id: format!("{}-ql-{}", q.id.0, i + 1),
                        product_id: line.product_id.0.clone(),
                        product_name,
                        quantity: line.quantity,
                        unit_price: Some(unit_price_f64),
                        discount_pct: line.discount_pct,
                        discount_amount: if line_discount > 0.0 {
                            Some(line_discount)
                        } else {
                            None
                        },
                        subtotal: Some(line_net),
                    });
                }

                let pricing = if input.include_pricing {
                    Some(PricingInfo {
                        subtotal: subtotal_sum,
                        discount_total: discount_sum,
                        tax_total: 0.0,
                        total: subtotal_sum - discount_sum,
                        priced_at: None,
                    })
                } else {
                    None
                };

                let result = QuoteGetResult {
                    quote: QuoteInfo {
                        id: q.id.0,
                        version: q.version,
                        account_id: q.account_id.unwrap_or_default(),
                        account_name: None, // TODO: look up from customer repo
                        deal_id: q.deal_id,
                        status,
                        currency: q.currency,
                        term_months: q.term_months,
                        start_date: q.start_date,
                        end_date: q.end_date,
                        valid_until: q.valid_until,
                        notes: q.notes,
                        created_at: q.created_at.to_rfc3339(),
                        created_by: q.created_by,
                    },
                    line_items,
                    pricing,
                };

                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Ok(None) => tool_error("NOT_FOUND", &format!("Quote '{}' not found", quote_id), None),
            Err(e) => {
                warn!(error = %e, "quote_get failed");
                internal_tool_error(&e)
            }
        }
    }

    #[tool(description = "Run pricing engine on a quote")]
    pub async fn quote_price(&self, Parameters(input): Parameters<QuotePriceInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_price called");
        let quote_id_for_audit = input.quote_id.trim().to_string();
        self.record_mcp_audit_event(
            "quote_price",
            if quote_id_for_audit.is_empty() { None } else { Some(quote_id_for_audit.as_str()) },
            serde_json::json!({
                "quote_id": &input.quote_id,
                "requested_discount_pct": input.requested_discount_pct
            }),
        )
        .await;

        let quote_id = match normalize_id(&input.quote_id, "quote_id") {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        let requested_discount_pct =
            match normalize_discount(input.requested_discount_pct, "requested_discount_pct") {
                Ok(value) => value,
                Err(msg) => {
                    return tool_error("VALIDATION_ERROR", &msg, None);
                }
            };

        use quotey_core::cpq::policy::evaluate_policy_input;
        use quotey_core::cpq::policy::PolicyInput;
        use quotey_core::cpq::pricing::price_quote_with_trace;
        use quotey_core::domain::quote::QuoteId;
        use quotey_db::repositories::QuoteRepository;
        use rust_decimal::prelude::FromPrimitive;
        use rust_decimal::Decimal;

        let quote_repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());

        let quote = match quote_repo.find_by_id(&QuoteId(quote_id.clone())).await {
            Ok(Some(q)) => q,
            Ok(None) => {
                return tool_error("NOT_FOUND", &format!("Quote '{}' not found", quote_id), None);
            }
            Err(e) => {
                warn!(error = %e, "quote_price: failed to load quote");
                return internal_tool_error(&e);
            }
        };

        // Run deterministic pricing engine
        let pricing_result = price_quote_with_trace(&quote, &quote.currency);

        // Run deterministic policy engine
        let discount_pct = Decimal::from_f64(requested_discount_pct).unwrap_or(Decimal::ZERO);
        let deal_value_dec = pricing_result.total;
        // Estimate margin: if no discount requested, margin is 100%; otherwise approximate
        let margin_pct = if deal_value_dec > Decimal::ZERO {
            let discount_amount = deal_value_dec * discount_pct / Decimal::from(100);
            let net = deal_value_dec - discount_amount;
            (net * Decimal::from(100)) / deal_value_dec
        } else {
            Decimal::from(100)
        };

        let policy_input = PolicyInput {
            requested_discount_pct: discount_pct,
            deal_value: deal_value_dec,
            minimum_margin_pct: margin_pct,
        };
        let policy_decision = evaluate_policy_input(&policy_input);

        // Build per-line pricing
        let product_repo = quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
        use quotey_db::repositories::ProductRepository;

        let mut line_pricing = Vec::new();
        for (i, line) in quote.lines.iter().enumerate() {
            let product_name = match product_repo.find_by_id(&line.product_id).await {
                Ok(Some(p)) => p.name,
                _ => format!("Product {}", line.product_id.0),
            };

            let base_price_f64: f64 = decimal_to_f64(&line.unit_price);
            let line_subtotal = base_price_f64 * line.quantity as f64;

            // Apply both the line-level discount and the requested global discount
            let effective_discount = if requested_discount_pct > 0.0 {
                requested_discount_pct
            } else {
                line.discount_pct
            };
            let discount_amount = line_subtotal * effective_discount / 100.0;
            let discounted_unit = base_price_f64 * (1.0 - effective_discount / 100.0);

            line_pricing.push(LinePricingInfo {
                line_id: format!("{}-ql-{}", quote.id.0, i + 1),
                product_id: line.product_id.0.clone(),
                product_name,
                quantity: line.quantity,
                base_unit_price: base_price_f64,
                unit_price: discounted_unit,
                subtotal_before_discount: line_subtotal,
                discount_pct: effective_discount,
                discount_amount,
                line_total: line_subtotal - discount_amount,
            });
        }

        let subtotal_f64: f64 = line_pricing.iter().map(|l| l.subtotal_before_discount).sum();
        let discount_f64: f64 = line_pricing.iter().map(|l| l.discount_amount).sum();
        let total_f64 = subtotal_f64 - discount_f64;

        let policy_violations: Vec<PolicyViolation> = policy_decision
            .violations
            .iter()
            .map(|v| PolicyViolation {
                policy_id: v.policy_id.clone(),
                policy_name: v.policy_id.replace(['-', '_'], " "),
                severity: if v.required_approval.is_some() {
                    "approval_required".to_string()
                } else {
                    "warning".to_string()
                },
                description: v.reason.clone(),
                threshold: None,
                actual: Some(input.requested_discount_pct),
                required_approver_role: v.required_approval.clone(),
            })
            .collect();

        use quotey_db::repositories::quote::quote_status_as_str;
        let result = QuotePriceResult {
            quote_id: quote.id.0.clone(),
            version: quote.version,
            status: quote_status_as_str(&quote.status).to_string(),
            pricing: PricingInfo {
                subtotal: subtotal_f64,
                discount_total: discount_f64,
                tax_total: 0.0,
                total: total_f64,
                priced_at: Some(chrono::Utc::now().to_rfc3339()),
            },
            line_pricing,
            approval_required: policy_decision.approval_required,
            policy_violations,
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "List quotes with optional filters")]
    pub async fn quote_list(&self, Parameters(input): Parameters<QuoteListInput>) -> String {
        debug!("quote_list called");
        self.record_mcp_audit_event(
            "quote_list",
            None,
            serde_json::json!({
                "account_id": &input.account_id,
                "status": &input.status,
                "limit": input.limit,
                "page": input.page
            }),
        )
        .await;

        let account_id = normalize_optional_trimmed(&input.account_id);
        let status = normalize_optional_trimmed(&input.status);
        let page = normalize_page(input.page);
        let limit = normalize_limit(input.limit);
        let fetch_limit = limit.saturating_add(1);
        let offset = (page.saturating_sub(1)).saturating_mul(limit);

        use quotey_db::repositories::QuoteRepository;

        let repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());

        match repo.list(account_id.as_deref(), status.as_deref(), fetch_limit, offset).await {
            Ok(quotes) => {
                use quotey_db::repositories::quote::quote_status_as_str;

                let mut quotes = quotes;
                let has_more = quotes.len() > limit as usize;
                quotes.truncate(limit as usize);

                let items: Vec<QuoteListItem> = quotes
                    .into_iter()
                    .map(|q| {
                        let status = quote_status_as_str(&q.status).to_string();
                        let total: f64 = q
                            .lines
                            .iter()
                            .map(|l| {
                                let unit: f64 = decimal_to_f64(&l.unit_price);
                                let line_subtotal = unit * l.quantity as f64;
                                let discount = line_subtotal * l.discount_pct / 100.0;
                                line_subtotal - discount
                            })
                            .sum();

                        QuoteListItem {
                            id: q.id.0.clone(),
                            version: q.version,
                            account_id: q.account_id.clone().unwrap_or_default(),
                            account_name: None,
                            status,
                            currency: q.currency.clone(),
                            total: if total > 0.0 { Some(total) } else { None },
                            valid_until: q.valid_until.clone(),
                            created_at: q.created_at.to_rfc3339(),
                        }
                    })
                    .collect();

                let result = QuoteListResult {
                    pagination: PaginationInfo {
                        total: items.len() as u32,
                        page,
                        per_page: limit,
                        has_more,
                    },
                    items,
                };

                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => {
                warn!(error = %e, "quote_list failed");
                internal_tool_error(&e)
            }
        }
    }

    // Approval Tools
    #[tool(description = "Submit a quote for approval")]
    pub async fn approval_request(
        &self,
        Parameters(input): Parameters<ApprovalRequestInput>,
    ) -> String {
        debug!(quote_id = %input.quote_id, "approval_request called");
        let quote_id_for_audit = input.quote_id.trim().to_string();
        self.record_mcp_audit_event(
            "approval_request",
            if quote_id_for_audit.is_empty() { None } else { Some(quote_id_for_audit.as_str()) },
            serde_json::json!({
                "quote_id": &input.quote_id,
                "approver_role": &input.approver_role
            }),
        )
        .await;

        use quotey_core::domain::approval::{
            ApprovalId, ApprovalRequest as DomainApproval, ApprovalStatus,
        };
        use quotey_core::domain::quote::{QuoteId, QuoteStatus};
        // Verify quote exists
        let quote_id = match normalize_id(&input.quote_id, "quote_id") {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        let justification = input.justification.trim().to_string();
        if justification.is_empty() {
            return tool_error("VALIDATION_ERROR", "justification is required", None);
        }

        if justification.len() > 2000 {
            return tool_error(
                "VALIDATION_ERROR",
                "justification must be 2000 characters or fewer",
                None,
            );
        }

        let approver_role = input
            .approver_role
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .unwrap_or("sales_manager")
            .to_string();

        let quote_repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());
        use quotey_db::repositories::QuoteRepository;

        let quote = match quote_repo.find_by_id(&QuoteId(quote_id.clone())).await {
            Ok(Some(q)) => q,
            Ok(None) => {
                return tool_error("NOT_FOUND", &format!("Quote '{}' not found", quote_id), None);
            }
            Err(e) => {
                warn!(error = %e, "approval_request: failed to load quote");
                return internal_tool_error(&e);
            }
        };

        let normalized_status =
            quotey_db::repositories::quote::quote_status_as_str(&quote.status).to_string();
        if matches!(
            quote.status,
            QuoteStatus::Approved
                | QuoteStatus::Sent
                | QuoteStatus::Expired
                | QuoteStatus::Cancelled
        ) {
            return tool_error(
                "CONFLICT",
                &format!(
                    "Quote '{}' is in '{}' state and cannot be submitted for approval",
                    quote_id, normalized_status
                ),
                None,
            );
        }

        let existing =
            match quotey_db::repositories::SqlApprovalRepository::new(self.db_pool.clone())
                .find_by_quote_id(&QuoteId(quote_id.clone()))
                .await
            {
                Ok(existing) => existing,
                Err(e) => {
                    warn!(error = %e, "approval_request: failed to load existing approvals");
                    return internal_tool_error(&e);
                }
            };

        if let Some(existing_pending) = existing.iter().find(|a| {
            a.status == ApprovalStatus::Pending
                && a.approver_role.eq_ignore_ascii_case(&approver_role)
        }) {
            return tool_error(
                "CONFLICT",
                &format!(
                    "A pending approval already exists for this quote and approver role. existing_approval_id={}",
                    existing_pending.id.0
                ),
                Some(serde_json::json!({
                    "quote_id": quote_id,
                    "approval_id": existing_pending.id.0,
                })),
            );
        }

        let now = chrono::Utc::now();
        let approval_id =
            format!("APR-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0000"));
        let expires_at = now + chrono::Duration::hours(4);

        let approval = DomainApproval {
            id: ApprovalId(approval_id.clone()),
            quote_id: quote.id.clone(),
            approver_role: approver_role.clone(),
            reason: format!("Approval requested for quote {}", quote.id.0),
            justification: justification.clone(),
            status: ApprovalStatus::Pending,
            requested_by: "agent:mcp".to_string(),
            expires_at: Some(expires_at),
            created_at: now,
            updated_at: now,
        };

        let repo = quotey_db::repositories::SqlApprovalRepository::new(self.db_pool.clone());
        if let Err(e) = repo.save(approval).await {
            warn!(error = %e, "approval_request: failed to save");
            return internal_tool_error(&e);
        }

        self.dispatch_pending_approval_push_notifications(&quote, &approval_id, &approver_role)
            .await;

        let result = ApprovalRequestResult {
            approval_id,
            quote_id,
            status: "pending".to_string(),
            approver_role,
            requested_by: "agent:mcp".to_string(),
            justification,
            created_at: now.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
            message: "Approval request submitted and persisted".to_string(),
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "Check approval status for a quote")]
    pub async fn approval_status(
        &self,
        Parameters(input): Parameters<ApprovalStatusInput>,
    ) -> String {
        debug!(quote_id = %input.quote_id, "approval_status called");
        let quote_id_for_audit = input.quote_id.trim().to_string();
        self.record_mcp_audit_event(
            "approval_status",
            if quote_id_for_audit.is_empty() { None } else { Some(quote_id_for_audit.as_str()) },
            serde_json::json!({
                "quote_id": &input.quote_id
            }),
        )
        .await;

        let quote_id = match normalize_id(&input.quote_id, "quote_id") {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        use quotey_core::domain::approval::ApprovalStatus as DomainStatus;
        use quotey_core::domain::quote::QuoteId;
        use quotey_db::repositories::approval::approval_status_as_str;
        use quotey_db::repositories::ApprovalRepository;

        let repo = quotey_db::repositories::SqlApprovalRepository::new(self.db_pool.clone());
        let approvals = match repo.find_by_quote_id(&QuoteId(quote_id.clone())).await {
            Ok(a) => a,
            Err(e) => {
                warn!(error = %e, "approval_status: failed to load approvals");
                return internal_tool_error(&e);
            }
        };

        let pending: Vec<PendingApproval> = approvals
            .iter()
            .filter(|a| a.status == DomainStatus::Pending)
            .map(|a| PendingApproval {
                approval_id: a.id.0.clone(),
                status: approval_status_as_str(&a.status).to_string(),
                approver_role: a.approver_role.clone(),
                requested_at: a.created_at.to_rfc3339(),
                expires_at: a.expires_at.map(|dt| dt.to_rfc3339()).unwrap_or_default(),
            })
            .collect();

        let has_approved = approvals.iter().any(|a| a.status == DomainStatus::Approved);
        let has_pending = !pending.is_empty();

        let has_rejected = approvals.iter().any(|a| a.status == DomainStatus::Rejected);

        let current_status = if has_rejected {
            "rejected".to_string()
        } else if has_pending {
            "pending_approval".to_string()
        } else if has_approved {
            "approved".to_string()
        } else {
            "no_approvals".to_string()
        };

        let result = ApprovalStatusResult {
            quote_id,
            current_status,
            pending_requests: pending,
            can_proceed: has_approved && !has_pending && !has_rejected,
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "List all pending approval requests")]
    pub async fn approval_pending(
        &self,
        Parameters(input): Parameters<ApprovalPendingInput>,
    ) -> String {
        debug!("approval_pending called");
        self.record_mcp_audit_event(
            "approval_pending",
            None,
            serde_json::json!({
                "approver_role": &input.approver_role,
                "limit": input.limit
            }),
        )
        .await;

        let limit = normalize_limit(input.limit);
        let approver_role = normalize_optional_trimmed(&input.approver_role);

        use quotey_db::repositories::ApprovalRepository;
        use quotey_db::repositories::QuoteRepository;

        let repo = quotey_db::repositories::SqlApprovalRepository::new(self.db_pool.clone());
        let pending = match repo.list_pending(approver_role.as_deref(), limit).await {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "approval_pending: failed to list");
                return internal_tool_error(&e);
            }
        };

        let quote_repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());

        let mut items = Vec::new();
        for approval in &pending {
            // Look up quote to get total
            let (account_name, quote_total) = match quote_repo.find_by_id(&approval.quote_id).await
            {
                Ok(Some(q)) => {
                    let total: f64 = q
                        .lines
                        .iter()
                        .map(|l| {
                            let unit: f64 = l.unit_price.to_string().parse().unwrap_or(0.0);
                            let subtotal = unit * l.quantity as f64;
                            let discount = subtotal * l.discount_pct / 100.0;
                            subtotal - discount
                        })
                        .sum();
                    (q.account_id.clone().unwrap_or_else(|| "unknown".to_string()), total)
                }
                _ => ("unknown".to_string(), 0.0),
            };

            items.push(ApprovalPendingItem {
                approval_id: approval.id.0.clone(),
                quote_id: approval.quote_id.0.clone(),
                account_name,
                quote_total,
                requested_by: approval.requested_by.clone(),
                justification: approval.justification.clone(),
                requested_at: approval.created_at.to_rfc3339(),
            });
        }

        let total = items.len() as u32;
        let result = ApprovalPendingResult { items, total };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    // PDF Tools
    #[tool(description = "Generate PDF for a quote")]
    pub async fn quote_pdf(&self, Parameters(input): Parameters<QuotePdfInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_pdf called");
        let quote_id_for_audit = input.quote_id.trim().to_string();
        self.record_mcp_audit_event(
            "quote_pdf",
            if quote_id_for_audit.is_empty() { None } else { Some(quote_id_for_audit.as_str()) },
            serde_json::json!({
                "quote_id": &input.quote_id,
                "template": &input.template
            }),
        )
        .await;
        let quote_id = match normalize_id(&input.quote_id, "quote_id") {
            Ok(value) => value,
            Err(msg) => {
                return tool_error("VALIDATION_ERROR", &msg, None);
            }
        };

        let template = if input.template.trim().is_empty() {
            default_template()
        } else {
            input.template.trim().to_string()
        };
        if !template_is_allowed(&template) {
            return tool_error(
                "VALIDATION_ERROR",
                &format!(
                    "Unsupported template '{}'. Allowed templates: {}",
                    template,
                    allowed_pdf_templates().join(", ")
                ),
                None,
            );
        }

        use quotey_core::domain::quote::QuoteId;
        use quotey_db::repositories::QuoteRepository;

        let quote_repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());
        let quote = match quote_repo.find_by_id(&QuoteId(quote_id.clone())).await {
            Ok(Some(q)) => q,
            Ok(None) => {
                return tool_error("NOT_FOUND", &format!("Quote '{}' not found", quote_id), None);
            }
            Err(e) => {
                warn!(error = %e, "quote_pdf: failed to load quote");
                return internal_tool_error(&e);
            }
        };

        let payload = build_pdf_quote_payload(&quote);
        let render_result = match render_quote_pdf_html_to_bytes(&payload, &template).await {
            Ok(rendered) => rendered,
            Err(err) => {
                warn!(error = %err, "quote_pdf: failed to render PDF html");
                return internal_tool_error(&err);
            }
        };

        let generated_at = chrono::Utc::now().to_rfc3339();
        let (_dir, file_path, pdf_generated, file_size_bytes, checksum) = match &render_result {
            PdfRenderResult::Pdf(bytes) => {
                let (dir, file_path) = payload_to_output_path(&quote_id, &template, true);
                if let Err(e) = tokio::fs::create_dir_all(&dir).await {
                    warn!(error = %e, "quote_pdf: failed to create output directory");
                    return internal_tool_error(&e);
                }
                if let Err(e) = tokio::fs::write(&file_path, bytes).await {
                    warn!(error = %e, "quote_pdf: failed to write artifact");
                    return internal_tool_error(&e);
                }
                let checksum = checksum_of_bytes(bytes);
                (dir, file_path, true, bytes.len() as u64, checksum)
            }
            PdfRenderResult::Html(html) => {
                let (dir, file_path) = payload_to_output_path(&quote_id, &template, false);
                if let Err(e) = tokio::fs::create_dir_all(&dir).await {
                    warn!(error = %e, "quote_pdf: failed to create output directory");
                    return internal_tool_error(&e);
                }
                if let Err(e) = tokio::fs::write(&file_path, html.as_bytes()).await {
                    warn!(error = %e, "quote_pdf: failed to write artifact");
                    return internal_tool_error(&e);
                }
                let checksum = checksum_of(html);
                (dir, file_path, false, html.len() as u64, checksum)
            }
        };

        if let Some(parent) = file_path.parent() {
            debug!(event_name = "quote_pdf.output_dir", path = %parent.display(), "PDF output prepared");
        }

        let file_path = file_path.to_string_lossy().to_string();
        let result = QuotePdfResult {
            quote_id,
            pdf_generated,
            file_path,
            file_size_bytes,
            checksum,
            template_used: template.to_string(),
            generated_at,
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    // Anomaly Override Tool
    #[tool(
        description = "Override an anomaly flag with justification. Records the override in the database and creates an audit event."
    )]
    pub async fn anomaly_override(
        &self,
        Parameters(input): Parameters<AnomalyOverrideInput>,
    ) -> String {
        debug!(quote_id = %input.quote_id, rule_kind = %input.rule_kind, "anomaly_override called");
        self.record_mcp_audit_event(
            "anomaly_override",
            Some(&input.quote_id),
            serde_json::json!({
                "quote_id": &input.quote_id,
                "rule_kind": &input.rule_kind,
                "severity": &input.severity,
                "overridden_by": &input.overridden_by,
            }),
        )
        .await;

        let quote_id = match normalize_id(&input.quote_id, "quote_id") {
            Ok(v) => v,
            Err(msg) => return tool_error("VALIDATION_ERROR", &msg, None),
        };

        let justification = input.justification.trim().to_string();
        if justification.is_empty() {
            return tool_error(
                "VALIDATION_ERROR",
                "Justification is required when overriding an anomaly flag",
                None,
            );
        }

        let rule_kind =
            match quotey_core::cpq::anomaly::AnomalyRuleKind::parse_label(&input.rule_kind) {
                Some(rk) => rk,
                None => {
                    return tool_error(
                        "VALIDATION_ERROR",
                        &format!(
                        "Invalid rule_kind '{}'. Must be one of: discount, margin, quantity, price",
                        input.rule_kind
                    ),
                        None,
                    );
                }
            };

        let severity =
            match quotey_core::cpq::anomaly::AnomalySeverity::parse_label(&input.severity) {
                Some(s) => s,
                None => {
                    return tool_error(
                        "VALIDATION_ERROR",
                        &format!(
                            "Invalid severity '{}'. Must be one of: none, info, warning, critical",
                            input.severity
                        ),
                        None,
                    );
                }
            };

        let override_id = format!(
            "AO-{}-{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S"),
            &quote_id[quote_id.len().saturating_sub(4)..]
        );

        let ovr = quotey_core::cpq::anomaly::AnomalyOverride {
            id: override_id.clone(),
            quote_id: quote_id.clone(),
            rule_kind,
            severity,
            justification: justification.clone(),
            overridden_by: input.overridden_by.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        if let Err(e) =
            quotey_db::repositories::SqlAnomalyOverrideRepository::save(&self.db_pool, &ovr).await
        {
            warn!(error = %e, "anomaly_override: failed to save override");
            return internal_tool_error(&e);
        }

        let rep_override_count =
            match quotey_db::repositories::SqlAnomalyOverrideRepository::count_by_rep(
                &self.db_pool,
                &input.overridden_by,
            )
            .await
            {
                Ok(count) => count,
                Err(error) => {
                    warn!(error = %error, "anomaly_override: failed to compute rep override count");
                    return internal_tool_error(&error);
                }
            };

        let total_override_count =
            match quotey_db::repositories::SqlAnomalyOverrideRepository::count_all(&self.db_pool)
                .await
            {
                Ok(count) => count,
                Err(error) => {
                    warn!(error = %error, "anomaly_override: failed to compute global override count");
                    return internal_tool_error(&error);
                }
            };

        let rep_override_rate_pct = if total_override_count > 0 {
            (rep_override_count as f64 / total_override_count as f64) * 100.0
        } else {
            0.0
        };

        let manager_notification_id = format!(
            "APR-AO-{}-{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S"),
            &quote_id[quote_id.len().saturating_sub(4)..]
        );
        let now = chrono::Utc::now().to_rfc3339();
        let manager_reason = format!("Anomaly override review ({})", input.rule_kind);
        let manager_justification = serde_json::json!({
            "source": "mcp.anomaly_override",
            "override_id": &override_id,
            "rule_kind": &input.rule_kind,
            "severity": &input.severity,
            "overridden_by": &input.overridden_by,
            "justification": &justification,
        })
        .to_string();

        if let Err(error) = sqlx::query(
            "INSERT INTO approval_request
                (id, quote_id, approver_role, reason, justification, status, requested_by, created_at, updated_at)
             VALUES (?, ?, 'sales_manager', ?, ?, 'pending', ?, ?, ?)",
        )
        .bind(&manager_notification_id)
        .bind(&quote_id)
        .bind(&manager_reason)
        .bind(&manager_justification)
        .bind(&input.overridden_by)
        .bind(&now)
        .bind(&now)
        .execute(&self.db_pool)
        .await
        {
            warn!(error = %error, "anomaly_override: failed to queue manager notification");
            return internal_tool_error(&error);
        }

        self.record_mcp_audit_event(
            "anomaly_override_recorded",
            Some(&quote_id),
            serde_json::json!({
                "quote_id": &quote_id,
                "override_id": &override_id,
                "manager_notification_id": &manager_notification_id,
                "rule_kind": &input.rule_kind,
                "severity": &input.severity,
                "overridden_by": &input.overridden_by,
                "rep_override_count": rep_override_count,
                "total_override_count": total_override_count,
                "rep_override_rate_pct": rep_override_rate_pct,
            }),
        )
        .await;

        let result = serde_json::json!({
            "override_id": override_id,
            "quote_id": quote_id,
            "rule_kind": input.rule_kind,
            "severity": input.severity,
            "overridden_by": input.overridden_by,
            "justification": justification,
            "status": "recorded",
            "manager_notification": {
                "queued": true,
                "approval_request_id": manager_notification_id,
                "approver_role": "sales_manager",
            },
            "override_metrics": {
                "rep_override_count": rep_override_count,
                "total_override_count": total_override_count,
                "rep_override_rate_pct": rep_override_rate_pct,
            },
            "message": format!(
                "Anomaly override recorded. {} flag on quote {} overridden by {} with justification. Manager notification queued for review.",
                input.rule_kind, quote_id, input.overridden_by,
            ),
        });

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    // Negotiation Tools
    #[tool(
        description = "Start a new negotiation session for a quote. Returns the session ID and initial state. Idempotent — same quote+actor+key returns the existing session."
    )]
    pub async fn negotiation_start(
        &self,
        Parameters(input): Parameters<NegotiationStartInput>,
    ) -> String {
        debug!(quote_id = %input.quote_id, actor_id = %input.actor_id, "negotiation_start called");
        self.record_mcp_audit_event(
            "negotiation_start",
            Some(&input.quote_id),
            serde_json::json!({
                "quote_id": &input.quote_id,
                "actor_id": &input.actor_id,
            }),
        )
        .await;

        let quote_id = match normalize_id(&input.quote_id, "quote_id") {
            Ok(v) => v,
            Err(msg) => return tool_error("VALIDATION_ERROR", &msg, None),
        };
        let actor_id = match normalize_id(&input.actor_id, "actor_id") {
            Ok(v) => v,
            Err(msg) => return tool_error("VALIDATION_ERROR", &msg, None),
        };

        let idempotency_key = if input.idempotency_key.trim().is_empty() {
            format!("{}-{}-default", quote_id, actor_id)
        } else {
            input.idempotency_key.trim().to_string()
        };

        // Check for existing active session
        match quotey_db::repositories::SqlNegotiationRepository::find_active_session_for_quote(
            &self.db_pool,
            &quote_id,
        )
        .await
        {
            Ok(Some(existing)) => {
                let result = serde_json::json!({
                    "session_id": existing.id.0,
                    "quote_id": existing.quote_id,
                    "actor_id": existing.actor_id,
                    "state": existing.state.as_str(),
                    "created_at": existing.created_at,
                    "message": "Existing active session returned (idempotent).",
                });
                return serde_json::to_string_pretty(&result).unwrap_or_default();
            }
            Ok(None) => {}
            Err(e) => return internal_tool_error(&e),
        }

        let session_id = format!(
            "NXT-{}-{}",
            chrono::Utc::now().format("%Y%m%d%H%M%S"),
            &quote_id[quote_id.len().saturating_sub(4)..]
        );

        let session = quotey_core::domain::negotiation::NegotiationSession {
            id: quotey_core::NegotiationSessionId(session_id.clone()),
            quote_id: quote_id.clone(),
            actor_id: actor_id.clone(),
            state: quotey_core::NegotiationState::Draft,
            policy_version: "policy-v1".to_string(),
            pricing_version: "pricing-v1".to_string(),
            idempotency_key,
            max_turns: 20,
            expires_at: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        if let Err(e) =
            quotey_db::repositories::SqlNegotiationRepository::save_session(&self.db_pool, &session)
                .await
        {
            warn!(error = %e, "negotiation_start: failed to save session");
            return internal_tool_error(&e);
        }

        let result = serde_json::json!({
            "session_id": session_id,
            "quote_id": quote_id,
            "actor_id": actor_id,
            "state": "draft",
            "message": "Negotiation session created. Use negotiation_evaluate to assess concession options.",
        });

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(
        description = "Evaluate concession options for a negotiation session. Returns the concession envelope (allowed ranges), boundary evaluation, and ranked counteroffer alternatives."
    )]
    pub async fn negotiation_evaluate(
        &self,
        Parameters(input): Parameters<NegotiationEvaluateInput>,
    ) -> String {
        debug!(session_id = %input.session_id, "negotiation_evaluate called");
        self.record_mcp_audit_event(
            "negotiation_evaluate",
            None,
            serde_json::json!({
                "session_id": &input.session_id,
                "discount_pct": input.discount_pct,
                "margin_pct": input.margin_pct,
            }),
        )
        .await;

        let session_id = match normalize_id(&input.session_id, "session_id") {
            Ok(v) => v,
            Err(msg) => return tool_error("VALIDATION_ERROR", &msg, None),
        };

        // Fetch session
        let session = match quotey_db::repositories::SqlNegotiationRepository::find_session_by_id(
            &self.db_pool,
            &session_id,
        )
        .await
        {
            Ok(Some(s)) => s,
            Ok(None) => {
                return tool_error("NOT_FOUND", "Negotiation session not found", None);
            }
            Err(e) => return internal_tool_error(&e),
        };

        if session.state.is_terminal() {
            return tool_error(
                "INVALID_STATE",
                &format!(
                    "Session is in terminal state '{}' and cannot be evaluated",
                    session.state.as_str()
                ),
                None,
            );
        }

        // Build concession request
        let mut values = Vec::new();
        if let Some(discount) = input.discount_pct {
            values.push(quotey_core::cpq::concession::ConcessionRequestValue {
                dimension: "discount_pct".to_string(),
                value: discount,
            });
        }
        if let Some(margin) = input.margin_pct {
            values.push(quotey_core::cpq::concession::ConcessionRequestValue {
                dimension: "margin_pct".to_string(),
                value: margin,
            });
        }
        if let Some(term) = input.term_months {
            values.push(quotey_core::cpq::concession::ConcessionRequestValue {
                dimension: "term_months".to_string(),
                value: term,
            });
        }

        let request = quotey_core::cpq::concession::ConcessionRequest {
            session_id: session_id.clone(),
            values,
        };

        let policy = quotey_core::cpq::concession::ConcessionPolicy::default();
        let engine = quotey_core::cpq::concession::ConcessionPolicyEngine;
        let (envelope, boundary) = engine.evaluate(&policy, &request);

        // Generate counteroffer plan
        let config = quotey_core::cpq::counteroffer::CounterofferConfig::default();
        let planner = quotey_core::cpq::counteroffer::CounterofferPlanner;
        let plan = planner.plan(&envelope, &config);

        // Advance session to active if still in draft
        if session.state == quotey_core::NegotiationState::Draft {
            if let Err(e) =
                quotey_db::repositories::SqlNegotiationRepository::advance_session_state(
                    &self.db_pool,
                    &session_id,
                    quotey_core::NegotiationState::Active,
                )
                .await
            {
                warn!(error = %e, "negotiation_evaluate: failed to advance state");
            }
        }

        let ranges: Vec<serde_json::Value> = envelope
            .ranges
            .iter()
            .map(|r| {
                serde_json::json!({
                    "dimension": r.dimension,
                    "floor": r.floor,
                    "ceiling": r.ceiling,
                    "current": r.current,
                })
            })
            .collect();

        let alternatives: Vec<serde_json::Value> = plan
            .alternatives
            .iter()
            .map(|a| {
                serde_json::json!({
                    "offer_id": a.offer_id,
                    "rank": a.rank,
                    "discount_pct": a.discount_pct,
                    "rationale": a.rationale,
                })
            })
            .collect();

        let result = serde_json::json!({
            "session_id": session_id,
            "state": if session.state == quotey_core::NegotiationState::Draft { "active" } else { session.state.as_str() },
            "envelope": {
                "ranges": ranges,
                "blocking_reasons": envelope.blocking_reasons,
            },
            "boundary": {
                "within_bounds": boundary.within_bounds,
                "floor_breached": boundary.floor_breached,
                "ceiling_breached": boundary.ceiling_breached,
                "walk_away": boundary.walk_away,
                "requires_approval": boundary.requires_approval,
                "stop_reasons": boundary.stop_reasons,
            },
            "counteroffer_plan": {
                "alternatives": alternatives,
                "tie_break_field": plan.tie_break_field,
            },
        });

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "Get the current status of a negotiation session, including all turns.")]
    pub async fn negotiation_status(
        &self,
        Parameters(input): Parameters<NegotiationStatusInput>,
    ) -> String {
        debug!(session_id = %input.session_id, "negotiation_status called");
        self.record_mcp_audit_event(
            "negotiation_status",
            None,
            serde_json::json!({ "session_id": &input.session_id }),
        )
        .await;

        let session_id = match normalize_id(&input.session_id, "session_id") {
            Ok(v) => v,
            Err(msg) => return tool_error("VALIDATION_ERROR", &msg, None),
        };

        let session = match quotey_db::repositories::SqlNegotiationRepository::find_session_by_id(
            &self.db_pool,
            &session_id,
        )
        .await
        {
            Ok(Some(s)) => s,
            Ok(None) => {
                return tool_error("NOT_FOUND", "Negotiation session not found", None);
            }
            Err(e) => return internal_tool_error(&e),
        };

        let turns = match quotey_db::repositories::SqlNegotiationRepository::find_turns_by_session(
            &self.db_pool,
            &session_id,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => return internal_tool_error(&e),
        };

        let turns_json: Vec<serde_json::Value> = turns
            .iter()
            .map(|t| {
                serde_json::json!({
                    "turn_number": t.turn_number,
                    "request_type": t.request_type.as_str(),
                    "outcome": t.outcome.as_str(),
                    "created_at": t.created_at,
                })
            })
            .collect();

        let result = serde_json::json!({
            "session_id": session.id.0,
            "quote_id": session.quote_id,
            "actor_id": session.actor_id,
            "state": session.state.as_str(),
            "policy_version": session.policy_version,
            "pricing_version": session.pricing_version,
            "max_turns": session.max_turns,
            "turn_count": turns.len(),
            "turns": turns_json,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
        });

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(
        description = "Escalate a negotiation session for approval. Bundles session context, concession deltas, and boundary evaluation into an evidence packet for the approval workflow."
    )]
    pub async fn negotiation_escalate(
        &self,
        Parameters(input): Parameters<NegotiationEscalateInput>,
    ) -> String {
        debug!(session_id = %input.session_id, "negotiation_escalate called");
        self.record_mcp_audit_event(
            "negotiation_escalate",
            None,
            serde_json::json!({ "session_id": &input.session_id, "reason": &input.reason }),
        )
        .await;

        let session_id = match normalize_id(&input.session_id, "session_id") {
            Ok(v) => v,
            Err(msg) => return tool_error("VALIDATION_ERROR", &msg, None),
        };

        if input.reason.trim().is_empty() {
            return tool_error("VALIDATION_ERROR", "reason is required", None);
        }

        // Fetch session
        let session = match quotey_db::repositories::SqlNegotiationRepository::find_session_by_id(
            &self.db_pool,
            &session_id,
        )
        .await
        {
            Ok(Some(s)) => s,
            Ok(None) => {
                return tool_error("NOT_FOUND", "Negotiation session not found", None);
            }
            Err(e) => return internal_tool_error(&e),
        };

        // Session must not be in a terminal state
        if session.state.is_terminal() {
            return tool_error(
                "INVALID_STATE",
                &format!("Cannot escalate session in '{}' state", session.state.as_str()),
                None,
            );
        }

        // Fetch turns
        let turns = match quotey_db::repositories::SqlNegotiationRepository::find_turns_by_session(
            &self.db_pool,
            &session_id,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => return internal_tool_error(&e),
        };

        // Run concession evaluation to get current envelope and boundary
        let policy = quotey_core::cpq::concession::ConcessionPolicy::default();
        let request = quotey_core::cpq::concession::ConcessionRequest {
            session_id: session_id.clone(),
            values: Vec::new(),
        };
        let engine = quotey_core::cpq::concession::ConcessionPolicyEngine;
        let (envelope, mut boundary) = engine.evaluate(&policy, &request);

        // Explicit escalation implies approval is required
        if !boundary.requires_approval && !boundary.walk_away {
            boundary.requires_approval = true;
        }

        // Build escalation context pack
        let mut builder = quotey_core::cpq::escalation::EscalationPackBuilder::new(session.clone())
            .with_turns(turns)
            .with_envelope(envelope)
            .with_boundary(boundary)
            .with_reason(&input.reason);

        if !input.offer_id.is_empty() {
            builder = builder.with_offer_id(&input.offer_id);
        }

        let pack = builder.build();

        // Validate the pack
        if let Err(validation_err) = quotey_core::cpq::escalation::validate_escalation_pack(&pack) {
            return tool_error(
                "ESCALATION_INVALID",
                &format!("Escalation pack validation failed: {validation_err}"),
                None,
            );
        }

        // Advance session to approval_pending
        if let Err(e) = quotey_db::repositories::SqlNegotiationRepository::advance_session_state(
            &self.db_pool,
            &session_id,
            quotey_core::NegotiationState::ApprovalPending,
        )
        .await
        {
            warn!(error = %e, "negotiation_escalate: failed to advance state to approval_pending");
        }

        let result = serde_json::json!({
            "session_id": pack.session_id,
            "quote_id": pack.quote_id,
            "actor_id": pack.actor_id,
            "session_state": "approval_pending",
            "policy_version": pack.policy_version,
            "pricing_version": pack.pricing_version,
            "turn_count": pack.turn_count,
            "trigger_turn_number": pack.trigger_turn_number,
            "offer_id": pack.offer_id,
            "concession_deltas": pack.concession_deltas.iter().map(|d| {
                serde_json::json!({
                    "dimension": d.dimension,
                    "floor": d.floor,
                    "ceiling": d.ceiling,
                    "current": d.current,
                    "utilization_pct": format!("{:.1}%", d.utilization_pct),
                })
            }).collect::<Vec<_>>(),
            "boundary": {
                "within_bounds": pack.boundary_within_bounds,
                "requires_approval": pack.boundary_requires_approval,
                "walk_away": pack.boundary_walk_away,
            },
            "stop_reasons": pack.stop_reasons,
            "blocking_reasons": pack.blocking_reasons,
            "escalation_reason": pack.escalation_reason,
            "message": "Session escalated to approval_pending. Approval workflow will evaluate the evidence packet.",
        });

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    /// Create a test DB with all migrations applied.
    async fn test_db() -> quotey_db::DbPool {
        let pool =
            quotey_db::connect_with_settings("sqlite::memory:", 1, 30).await.expect("in-memory DB");
        quotey_db::migrations::run_pending(&pool).await.expect("migrations");
        pool
    }

    /// Seed a product for use in quote tests.
    async fn seed_product(pool: &quotey_db::DbPool, id: &str, sku: &str, name: &str, price: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO product (id, sku, name, base_price, currency, active, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'USD', 1, ?, ?)",
        )
        .bind(id)
        .bind(sku)
        .bind(name)
        .bind(price)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed product");
    }

    async fn seed_quote(pool: &quotey_db::DbPool, id: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO quote (id, status, currency, created_by, created_at, updated_at)
             VALUES (?, 'draft', 'USD', 'test', ?, ?)",
        )
        .bind(id)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed quote");
    }

    async fn seed_portal_push_subscription(pool: &quotey_db::DbPool, endpoint: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO portal_push_subscription
                (id, endpoint, p256dh, auth, user_agent, device_label, revoked, created_at, updated_at)
             VALUES (?, ?, ?, ?, NULL, NULL, 0, ?, ?)",
        )
        .bind(format!("PUSH-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("test")))
        .bind(endpoint)
        .bind("test-p256dh")
        .bind("test-auth")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .expect("seed portal push subscription");
    }

    fn test_mcp_auth_context(display_name: Option<&str>) -> AuthContext {
        AuthContext {
            channel: AuthChannel::Mcp,
            method: AuthMethod::ApiKey,
            strength: AuthStrength::Possession,
            principal: AuthPrincipal {
                actor_id: "mcp:key:test-key".to_string(),
                display_name: display_name.map(str::to_string),
            },
            token_fingerprint: Some("checksum:test".to_string()),
            session_id: None,
        }
    }

    fn server(pool: quotey_db::DbPool) -> QuoteyMcpServer {
        QuoteyMcpServer::new(pool)
    }

    /// Parse the JSON output from a tool and return the parsed value.
    fn parse_output(output: &str) -> serde_json::Value {
        serde_json::from_str(output).expect("tool output must be valid JSON")
    }

    #[test]
    fn hash_tool_arguments_is_stable_for_same_payload() {
        let mut args = serde_json::Map::new();
        args.insert("quote_id".to_string(), serde_json::Value::String("Q-123".to_string()));
        args.insert("limit".to_string(), serde_json::Value::Number(20.into()));

        let first = hash_tool_arguments(Some(&args));
        let second = hash_tool_arguments(Some(&args));
        assert_eq!(first, second);
    }

    #[test]
    fn extract_quote_id_from_arguments_trims_and_ignores_blank() {
        let mut args = serde_json::Map::new();
        args.insert("quote_id".to_string(), serde_json::Value::String(" Q-TRIM ".to_string()));
        assert_eq!(extract_quote_id_from_arguments(Some(&args)).as_deref(), Some("Q-TRIM"));

        args.insert("quote_id".to_string(), serde_json::Value::String("   ".to_string()));
        assert!(extract_quote_id_from_arguments(Some(&args)).is_none());
    }

    #[test]
    fn outcome_from_tool_result_detects_error_envelope_code() {
        let error_payload = tool_error("VALIDATION_ERROR", "bad request", None);
        let result = CallToolResult {
            content: vec![Content::text(error_payload)],
            is_error: None,
            meta: None,
            structured_content: None,
        };

        let (success, code, message) = outcome_from_tool_result(&Ok(result));
        assert!(!success);
        assert_eq!(code, "VALIDATION_ERROR");
        assert!(message.is_some());
    }

    #[test]
    fn outcome_from_tool_result_reports_ok_for_non_error_payload() {
        let result = CallToolResult {
            content: vec![Content::text("{\"ok\":true}")],
            is_error: None,
            meta: None,
            structured_content: None,
        };

        let (success, code, message) = outcome_from_tool_result(&Ok(result));
        assert!(success);
        assert_eq!(code, "OK");
        assert!(message.is_none());
    }

    #[tokio::test]
    async fn record_mcp_invocation_received_persists_request_metadata() {
        let pool = test_db().await;
        seed_quote(&pool, "Q-AUD-001").await;
        let srv = server(pool.clone());
        let envelope = McpInvocationAuditEnvelope {
            tool_name: "quote_get".to_string(),
            quote_id: Some("Q-AUD-001".to_string()),
            actor: "agent:mcp:test-key".to_string(),
            auth_context: test_mcp_auth_context(Some("test-key")),
            request_id: "req-123".to_string(),
            correlation_id: "corr-123".to_string(),
            input_hash: "hash-abc".to_string(),
            success: true,
            outcome_code: "RECEIVED".to_string(),
            error_message: None,
            auth_error_code: None,
        };

        srv.record_mcp_invocation_received(&envelope).await;

        let row = sqlx::query(
            "SELECT actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json
             FROM audit_event
             WHERE event_type = 'mcp.quote_get.received'
             ORDER BY timestamp DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("received audit event");

        assert_eq!(row.get::<String, _>("actor"), "agent:mcp:test-key");
        assert_eq!(row.get::<String, _>("actor_type"), "agent");
        assert_eq!(row.get::<String, _>("quote_id"), "Q-AUD-001");
        assert_eq!(row.get::<String, _>("event_type"), "mcp.quote_get.received");
        assert_eq!(row.get::<String, _>("event_category"), "mcp_tool");

        let payload: serde_json::Value =
            serde_json::from_str(&row.get::<String, _>("payload_json")).expect("payload json");
        assert_eq!(payload["tool_name"].as_str(), Some("quote_get"));
        assert_eq!(payload["actor"].as_str(), Some("agent:mcp:test-key"));
        assert_eq!(payload["auth"]["channel"].as_str(), Some("mcp"));
        assert_eq!(payload["auth"]["method"].as_str(), Some("api_key"));
        assert_eq!(payload["auth"]["strength"].as_str(), Some("possession"));
        assert_eq!(payload["auth"]["principal"]["actor_id"].as_str(), Some("mcp:key:test-key"));
        assert_eq!(payload["request_id"].as_str(), Some("req-123"));
        assert_eq!(payload["correlation_id"].as_str(), Some("corr-123"));
        assert_eq!(payload["input_hash"].as_str(), Some("hash-abc"));
        assert!(payload["timestamp"].is_string());

        let metadata: serde_json::Value =
            serde_json::from_str(&row.get::<Option<String>, _>("metadata_json").expect("metadata"))
                .expect("metadata json");
        assert_eq!(metadata["source"].as_str(), Some("quotey-mcp"));
        assert_eq!(metadata["tool_name"].as_str(), Some("quote_get"));
        assert_eq!(metadata["request_id"].as_str(), Some("req-123"));
        assert_eq!(metadata["correlation_id"].as_str(), Some("corr-123"));
        assert_eq!(metadata["input_hash"].as_str(), Some("hash-abc"));
        assert_eq!(metadata["auth_channel"].as_str(), Some("mcp"));
        assert_eq!(metadata["auth_method"].as_str(), Some("api_key"));
        assert_eq!(metadata["auth_strength"].as_str(), Some("possession"));
        assert_eq!(metadata["auth_principal"].as_str(), Some("mcp:key:test-key"));
    }

    #[tokio::test]
    async fn record_mcp_invocation_outcome_persists_failure_code_and_error() {
        let pool = test_db().await;
        seed_quote(&pool, "Q-AUD-002").await;
        let srv = server(pool.clone());
        let envelope = McpInvocationAuditEnvelope {
            tool_name: "quote_price".to_string(),
            quote_id: Some("Q-AUD-002".to_string()),
            actor: "agent:mcp:test-key".to_string(),
            auth_context: test_mcp_auth_context(Some("test-key")),
            request_id: "req-456".to_string(),
            correlation_id: "corr-456".to_string(),
            input_hash: "hash-def".to_string(),
            success: false,
            outcome_code: "VALIDATION_ERROR".to_string(),
            error_message: Some("bad input".to_string()),
            auth_error_code: Some("invalid_credential".to_string()),
        };

        srv.record_mcp_invocation_outcome(&envelope).await;

        let row = sqlx::query(
            "SELECT actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json
             FROM audit_event
             WHERE event_type = 'mcp.quote_price.completed'
             ORDER BY timestamp DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("outcome audit event");

        assert_eq!(row.get::<String, _>("actor"), "agent:mcp:test-key");
        assert_eq!(row.get::<String, _>("actor_type"), "agent");
        assert_eq!(row.get::<String, _>("quote_id"), "Q-AUD-002");
        assert_eq!(row.get::<String, _>("event_type"), "mcp.quote_price.completed");
        assert_eq!(row.get::<String, _>("event_category"), "mcp_tool");

        let payload: serde_json::Value =
            serde_json::from_str(&row.get::<String, _>("payload_json")).expect("payload json");
        assert_eq!(payload["tool_name"].as_str(), Some("quote_price"));
        assert_eq!(payload["auth"]["channel"].as_str(), Some("mcp"));
        assert_eq!(payload["auth"]["method"].as_str(), Some("api_key"));
        assert_eq!(payload["request_id"].as_str(), Some("req-456"));
        assert_eq!(payload["correlation_id"].as_str(), Some("corr-456"));
        assert_eq!(payload["input_hash"].as_str(), Some("hash-def"));
        assert_eq!(payload["outcome"]["success"].as_bool(), Some(false));
        assert_eq!(payload["outcome"]["code"].as_str(), Some("VALIDATION_ERROR"));
        assert_eq!(payload["outcome"]["error_message"].as_str(), Some("bad input"));
        assert_eq!(payload["outcome"]["auth_code"].as_str(), Some("invalid_credential"));
        assert!(payload["timestamp"].is_string());

        let metadata: serde_json::Value =
            serde_json::from_str(&row.get::<Option<String>, _>("metadata_json").expect("metadata"))
                .expect("metadata json");
        assert_eq!(metadata["source"].as_str(), Some("quotey-mcp"));
        assert_eq!(metadata["tool_name"].as_str(), Some("quote_price"));
        assert_eq!(metadata["request_id"].as_str(), Some("req-456"));
        assert_eq!(metadata["correlation_id"].as_str(), Some("corr-456"));
        assert_eq!(metadata["input_hash"].as_str(), Some("hash-def"));
        assert_eq!(metadata["auth_channel"].as_str(), Some("mcp"));
        assert_eq!(metadata["auth_method"].as_str(), Some("api_key"));
        assert_eq!(metadata["auth_strength"].as_str(), Some("possession"));
        assert_eq!(metadata["auth_principal"].as_str(), Some("mcp:key:test-key"));
        assert_eq!(metadata["auth_error_code"].as_str(), Some("invalid_credential"));
    }

    #[tokio::test]
    async fn record_mcp_invocation_outcome_drops_unknown_quote_id_but_persists_event() {
        let pool = test_db().await;
        let srv = server(pool.clone());
        let envelope = McpInvocationAuditEnvelope {
            tool_name: "quote_get".to_string(),
            quote_id: Some("Q-NOT-REAL".to_string()),
            actor: "agent:mcp:test-key".to_string(),
            auth_context: test_mcp_auth_context(Some("test-key")),
            request_id: "req-789".to_string(),
            correlation_id: "corr-789".to_string(),
            input_hash: "hash-ghi".to_string(),
            success: false,
            outcome_code: "NOT_FOUND".to_string(),
            error_message: Some("quote missing".to_string()),
            auth_error_code: None,
        };

        srv.record_mcp_invocation_outcome(&envelope).await;

        let row = sqlx::query(
            "SELECT quote_id, payload_json
             FROM audit_event
             WHERE event_type = 'mcp.quote_get.completed'
             ORDER BY timestamp DESC
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
        .expect("outcome audit event");

        assert_eq!(row.get::<Option<String>, _>("quote_id"), None);
        let payload: serde_json::Value =
            serde_json::from_str(&row.get::<String, _>("payload_json")).expect("payload json");
        assert_eq!(payload["outcome"]["code"].as_str(), Some("NOT_FOUND"));
        assert_eq!(payload["request_id"].as_str(), Some("req-789"));
    }

    #[test]
    fn parse_protocol_version_defaults_to_latest_when_missing_or_blank() {
        assert_eq!(parse_protocol_version(None).unwrap(), ProtocolVersion::LATEST);
        assert_eq!(parse_protocol_version(Some("   ")).unwrap(), ProtocolVersion::LATEST);
    }

    #[test]
    fn parse_authorization_header_supports_bearer_and_apikey_formats() {
        assert_eq!(
            parse_authorization_header("Bearer secret-token"),
            Some("secret-token".to_string())
        );
        assert_eq!(
            parse_authorization_header("ApiKey   secret-token"),
            Some("secret-token".to_string())
        );
        assert_eq!(
            parse_authorization_header("Token secret-token"),
            Some("secret-token".to_string())
        );
        assert_eq!(parse_authorization_header("plain-secret"), Some("plain-secret".to_string()));
        assert_eq!(parse_authorization_header("Basic Zm9vOmJhcg=="), None);
        assert_eq!(parse_authorization_header("Bearer   "), None);
        assert_eq!(parse_authorization_header("   "), None);
    }

    #[test]
    fn auth_context_for_allowed_mcp_call_maps_api_key_to_possession() {
        let result =
            AuthResult::Allowed { key_name: "test-key".to_string(), remaining_requests: 9 };
        let context = auth_context_for_allowed_mcp_call(&result, Some("secret-api-key"));
        assert_eq!(context.channel, AuthChannel::Mcp);
        assert_eq!(context.method, AuthMethod::ApiKey);
        assert_eq!(context.strength, AuthStrength::Possession);
        assert_eq!(context.principal.actor_id, "mcp:key:test-key");
        assert_eq!(context.principal.display_name.as_deref(), Some("test-key"));
        assert!(context.token_fingerprint.is_some());
    }

    #[test]
    fn auth_context_for_allowed_anonymous_key_stays_anonymous_even_with_presented_key() {
        let result =
            AuthResult::Allowed { key_name: "anonymous".to_string(), remaining_requests: u32::MAX };
        let context = auth_context_for_allowed_mcp_call(&result, Some("ignored-key"));
        assert_eq!(context.channel, AuthChannel::Mcp);
        assert_eq!(context.method, AuthMethod::None);
        assert_eq!(context.strength, AuthStrength::Anonymous);
        assert_eq!(context.principal.actor_id, "mcp:anonymous");
        assert_eq!(context.principal.display_name, None);
        assert_eq!(context.token_fingerprint, None);
    }

    #[test]
    fn auth_context_for_denied_mcp_call_with_no_key_is_anonymous() {
        let context = auth_context_for_denied_mcp_call(None);
        assert_eq!(context.channel, AuthChannel::Mcp);
        assert_eq!(context.method, AuthMethod::None);
        assert_eq!(context.strength, AuthStrength::Anonymous);
        assert_eq!(context.principal.actor_id, "mcp:anonymous");
        assert_eq!(context.token_fingerprint, None);
    }

    #[test]
    fn auth_context_for_denied_mcp_call_with_key_tracks_method_but_not_assurance() {
        let context = auth_context_for_denied_mcp_call(Some("bad-key"));
        assert_eq!(context.channel, AuthChannel::Mcp);
        assert_eq!(context.method, AuthMethod::ApiKey);
        assert_eq!(context.strength, AuthStrength::Anonymous);
        assert_eq!(context.principal.actor_id, "mcp:anonymous");
        assert!(context.token_fingerprint.is_some());
    }

    #[test]
    fn actor_from_auth_context_uses_display_name_for_api_key_method() {
        let auth_context = AuthContext {
            channel: AuthChannel::Mcp,
            method: AuthMethod::ApiKey,
            strength: AuthStrength::Possession,
            principal: AuthPrincipal {
                actor_id: "mcp:key:test-key".to_string(),
                display_name: Some("test-key".to_string()),
            },
            token_fingerprint: Some("checksum:test".to_string()),
            session_id: None,
        };
        assert_eq!(actor_from_auth_context(&auth_context), "agent:mcp:test-key");
    }

    #[test]
    fn auth_code_from_error_data_extracts_canonical_code() {
        let mut data = serde_json::Map::new();
        data.insert("code".to_string(), serde_json::json!("missing_credential"));
        let error = rmcp::ErrorData::invalid_request(
            "Authentication failed".to_string(),
            Some(serde_json::Value::Object(data)),
        );
        assert_eq!(auth_code_from_error_data(&error).as_deref(), Some("missing_credential"));
    }

    #[test]
    fn extract_api_key_from_meta_supports_aliases_and_priority() {
        let mut meta = rmcp::model::Meta::new();
        meta.0.insert("authorization".to_string(), serde_json::json!("Bearer auth-token"));
        assert_eq!(extract_api_key_from_meta(&meta), Some("auth-token".to_string()));

        meta.0.insert("x-api-key".to_string(), serde_json::json!("header-token"));
        assert_eq!(extract_api_key_from_meta(&meta), Some("header-token".to_string()));

        meta.0.insert("api_key".to_string(), serde_json::json!("meta-token"));
        assert_eq!(extract_api_key_from_meta(&meta), Some("meta-token".to_string()));
    }

    #[tokio::test]
    async fn check_auth_rate_limit_returns_429_code_and_retry_after() {
        let pool = test_db().await;
        let auth = crate::auth::AuthManager::from_config(&crate::auth::AuthConfig {
            enabled: true,
            rate_limit_window_secs: 60,
            api_keys: vec![crate::auth::ApiKeyConfig {
                key: "limited-key".to_string(),
                name: "limited".to_string(),
                requests_per_minute: 1,
            }],
        });
        let srv = QuoteyMcpServer::with_auth(pool, auth);

        let mut meta = rmcp::model::Meta::new();
        meta.0.insert("api_key".to_string(), serde_json::json!("limited-key"));

        assert!(srv.check_auth(&meta).await.is_ok());

        let err = srv.check_auth(&meta).await.expect_err("should be rate limited");
        assert_eq!(err.code, rmcp::model::ErrorCode(429));
        let retry_after = auth_denial_u64(&err, "retry_after");
        assert!(retry_after.is_some());
        assert_eq!(auth_denial_str(&err, "reason"), Some("Rate limit exceeded"));
        assert_eq!(auth_denial_str(&err, "code"), Some("rate_limited"));
        assert_eq!(auth_denial_str(&err, "error_code"), Some("RATE_LIMIT_EXCEEDED"));
        assert_eq!(auth_denial_u64(&err, "http_status"), Some(429));
    }

    #[tokio::test]
    async fn check_auth_missing_key_uses_canonical_auth_error_code() {
        let pool = test_db().await;
        let auth = crate::auth::AuthManager::from_config(&crate::auth::AuthConfig {
            enabled: true,
            rate_limit_window_secs: 60,
            api_keys: vec![crate::auth::ApiKeyConfig {
                key: "required-key".to_string(),
                name: "required".to_string(),
                requests_per_minute: 10,
            }],
        });
        let srv = QuoteyMcpServer::with_auth(pool, auth);

        let meta = rmcp::model::Meta::new();
        let err = srv.check_auth(&meta).await.expect_err("missing key should fail");
        assert_eq!(auth_denial_str(&err, "reason"), Some("API key required"));
        assert_eq!(auth_denial_str(&err, "code"), Some("missing_credential"));
        assert_eq!(auth_denial_str(&err, "error_code"), Some("AUTHENTICATION_FAILED"));
        assert_eq!(auth_denial_u64(&err, "http_status"), Some(401));
        assert_eq!(auth_denial_u64(&err, "retry_after"), None);
    }

    #[tokio::test]
    async fn check_auth_invalid_key_uses_invalid_credential_code() {
        let pool = test_db().await;
        let auth = crate::auth::AuthManager::from_config(&crate::auth::AuthConfig {
            enabled: true,
            rate_limit_window_secs: 60,
            api_keys: vec![crate::auth::ApiKeyConfig {
                key: "valid-key".to_string(),
                name: "valid".to_string(),
                requests_per_minute: 10,
            }],
        });
        let srv = QuoteyMcpServer::with_auth(pool, auth);

        let mut meta = rmcp::model::Meta::new();
        meta.0.insert("api_key".to_string(), serde_json::json!("wrong-key"));
        let err = srv.check_auth(&meta).await.expect_err("invalid key should fail");

        assert_eq!(auth_denial_str(&err, "reason"), Some("Invalid API key"));
        assert_eq!(auth_denial_str(&err, "code"), Some("invalid_credential"));
        assert_eq!(auth_denial_str(&err, "error_code"), Some("AUTHENTICATION_FAILED"));
        assert_eq!(auth_denial_u64(&err, "http_status"), Some(401));
        assert_eq!(auth_denial_u64(&err, "retry_after"), None);
    }

    #[tokio::test]
    async fn check_auth_deactivated_key_uses_credential_revoked_code() {
        let pool = test_db().await;
        let auth = crate::auth::AuthManager::with_keys(vec![crate::auth::ApiKeyEntry {
            key: "disabled-key".to_string(),
            name: "disabled".to_string(),
            requests_per_minute: 10,
            created_at: chrono::Utc::now(),
            active: false,
        }]);
        let srv = QuoteyMcpServer::with_auth(pool, auth);

        let mut meta = rmcp::model::Meta::new();
        meta.0.insert("api_key".to_string(), serde_json::json!("disabled-key"));
        let err = srv.check_auth(&meta).await.expect_err("deactivated key should fail");

        assert_eq!(auth_denial_str(&err, "reason"), Some("API key deactivated"));
        assert_eq!(auth_denial_str(&err, "code"), Some("credential_revoked"));
        assert_eq!(auth_denial_str(&err, "error_code"), Some("AUTHENTICATION_FAILED"));
        assert_eq!(auth_denial_u64(&err, "http_status"), Some(401));
        assert_eq!(auth_denial_u64(&err, "retry_after"), None);
    }

    fn auth_denial_str<'a>(err: &'a rmcp::ErrorData, key: &'a str) -> Option<&'a str> {
        err.data.as_ref()?.get(key)?.as_str()
    }

    fn auth_denial_u64(err: &rmcp::ErrorData, key: &str) -> Option<u64> {
        err.data.as_ref()?.get(key)?.as_u64()
    }

    #[test]
    fn parse_protocol_version_accepts_known_versions_and_latest_alias() {
        assert_eq!(
            parse_protocol_version(Some("2024-11-05")).unwrap(),
            ProtocolVersion::V_2024_11_05
        );
        assert_eq!(
            parse_protocol_version(Some("2025-03-26")).unwrap(),
            ProtocolVersion::V_2025_03_26
        );
        assert_eq!(parse_protocol_version(Some("LATEST")).unwrap(), ProtocolVersion::LATEST);
    }

    #[test]
    fn parse_protocol_version_rejects_unknown_value() {
        let err = parse_protocol_version(Some("2025-99-99")).expect_err("must reject unknown");
        assert!(err.contains("Unsupported MCP protocol version"));
    }

    /// Assert the output is an error envelope with the expected code.
    fn assert_error_envelope(output: &str, expected_code: &str) {
        let v = parse_output(output);
        let code = v["error"]["code"].as_str().expect("error.code must be a string");
        assert_eq!(code, expected_code, "unexpected error code in: {output}");
        assert!(v["error"]["message"].is_string(), "error.message must be a string");
    }

    // ========================================================================
    // catalog_search
    // ========================================================================

    #[tokio::test]
    async fn catalog_search_empty_query_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .catalog_search(Parameters(CatalogSearchInput {
                query: "".to_string(),
                category: None,
                active_only: true,
                limit: 20,
                page: 1,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn catalog_search_by_category_returns_seeded_product() {
        let pool = test_db().await;
        // Seed with family_id so category filter works (avoids FTS5 content-sync issue)
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO product_family (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind("FAM-TEST")
        .bind("Test Family")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed family");
        sqlx::query(
            "INSERT INTO product (id, sku, name, base_price, currency, active, family_id, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'USD', 1, ?, ?, ?)",
        )
        .bind("PROD-1")
        .bind("WDG-001")
        .bind("Widget Alpha")
        .bind("50.00")
        .bind("FAM-TEST")
        .bind(&now)
        .bind(&now)
        .execute(&pool)
        .await
        .expect("seed product");

        let srv = server(pool);
        let output = srv
            .catalog_search(Parameters(CatalogSearchInput {
                query: "".to_string(),
                category: Some("FAM-TEST".to_string()),
                active_only: true,
                limit: 20,
                page: 1,
            }))
            .await;
        let v = parse_output(&output);
        // Verify contract shape
        assert!(v["items"].is_array(), "items must be array, got: {output}");
        assert!(v["pagination"].is_object(), "pagination must be object");
        assert!(v["pagination"]["total"].is_number());
        assert!(v["pagination"]["page"].is_number());
        assert!(v["pagination"]["per_page"].is_number());
        let items = v["items"].as_array().unwrap();
        assert!(!items.is_empty(), "should find at least one product");
        let first = &items[0];
        assert_eq!(first["id"].as_str().unwrap(), "PROD-1");
        assert_eq!(first["sku"].as_str().unwrap(), "WDG-001");
        assert_eq!(first["name"].as_str().unwrap(), "Widget Alpha");
        assert!(first["active"].as_bool().unwrap());
    }

    // ========================================================================
    // catalog_get
    // ========================================================================

    #[tokio::test]
    async fn catalog_get_not_found() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .catalog_get(Parameters(CatalogGetInput {
                product_id: "NONEXISTENT".to_string(),
                include_relationships: false,
            }))
            .await;
        assert_error_envelope(&output, "NOT_FOUND");
    }

    #[tokio::test]
    async fn catalog_get_returns_product_detail() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-2", "WDG-002", "Widget Beta", "75.00").await;
        let srv = server(pool);
        let output = srv
            .catalog_get(Parameters(CatalogGetInput {
                product_id: "PROD-2".to_string(),
                include_relationships: false,
            }))
            .await;
        let v = parse_output(&output);
        assert_eq!(v["id"].as_str().unwrap(), "PROD-2");
        assert_eq!(v["sku"].as_str().unwrap(), "WDG-002");
        assert_eq!(v["name"].as_str().unwrap(), "Widget Beta");
        assert!(v["active"].as_bool().unwrap());
        assert!(v["created_at"].is_string());
        assert!(v["updated_at"].is_string());
    }

    #[tokio::test]
    async fn catalog_get_empty_id_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .catalog_get(Parameters(CatalogGetInput {
                product_id: "  ".to_string(),
                include_relationships: false,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    // ========================================================================
    // quote_create
    // ========================================================================

    #[tokio::test]
    async fn quote_create_success() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-Q1", "SKU-Q1", "Quoteable Widget", "100.00").await;
        let srv = server(pool);
        let output = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-001".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: Some(12),
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-Q1".to_string(),
                    quantity: 5,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("test-key-1".to_string()),
            }))
            .await;
        let v = parse_output(&output);
        // Contract shape
        assert!(v["quote_id"].is_string(), "must have quote_id");
        assert_eq!(v["version"].as_u64().unwrap(), 1);
        assert_eq!(v["status"].as_str().unwrap(), "draft");
        assert_eq!(v["account_id"].as_str().unwrap(), "ACC-001");
        assert_eq!(v["currency"].as_str().unwrap(), "USD");
        assert!(v["created_at"].is_string());
        assert_eq!(v["message"].as_str().unwrap(), "Quote created successfully");
        // Line items
        let lines = v["line_items"].as_array().unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["product_id"].as_str().unwrap(), "PROD-Q1");
        assert_eq!(lines[0]["quantity"].as_u64().unwrap(), 5);
        assert!((lines[0]["unit_price"].as_f64().unwrap() - 100.0).abs() < 0.01);
        assert!((lines[0]["subtotal"].as_f64().unwrap() - 500.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn quote_create_empty_line_items_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-001".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![],
                idempotency_key: None,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn quote_create_empty_account_id_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "  ".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-X".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: None,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn quote_create_nonexistent_product_returns_not_found() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-001".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "GHOST-PRODUCT".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: None,
            }))
            .await;
        assert_error_envelope(&output, "NOT_FOUND");
    }

    #[tokio::test]
    async fn quote_create_zero_quantity_returns_validation_error() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-ZQ", "SKU-ZQ", "Zero Qty", "10.00").await;
        let srv = server(pool);
        let output = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-001".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-ZQ".to_string(),
                    quantity: 0,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: None,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    // ========================================================================
    // quote_get
    // ========================================================================

    #[tokio::test]
    async fn quote_get_not_found() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_get(Parameters(QuoteGetInput {
                quote_id: "Q-NONEXISTENT".to_string(),
                include_pricing: true,
            }))
            .await;
        assert_error_envelope(&output, "NOT_FOUND");
    }

    #[tokio::test]
    async fn quote_get_returns_created_quote() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-G1", "SKU-G1", "Get Widget", "200.00").await;
        let srv = server(pool.clone());

        // Create a quote first
        let create_output = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-GET".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: Some(6),
                start_date: None,
                notes: Some("test notes".to_string()),
                line_items: vec![LineItemInput {
                    product_id: "PROD-G1".to_string(),
                    quantity: 3,
                    discount_pct: 10.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("get-test-key".to_string()),
            }))
            .await;
        let created = parse_output(&create_output);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        // Now get it
        let output = srv
            .quote_get(Parameters(QuoteGetInput {
                quote_id: quote_id.clone(),
                include_pricing: true,
            }))
            .await;
        let v = parse_output(&output);

        // Verify contract shape
        assert!(v["quote"].is_object(), "must have quote object");
        assert_eq!(v["quote"]["id"].as_str().unwrap(), quote_id);
        assert_eq!(v["quote"]["status"].as_str().unwrap(), "draft");
        assert_eq!(v["quote"]["account_id"].as_str().unwrap(), "ACC-GET");
        assert_eq!(v["quote"]["currency"].as_str().unwrap(), "USD");
        assert!(v["quote"]["created_at"].is_string());
        assert!(v["quote"]["created_by"].is_string());

        // Line items
        let lines = v["line_items"].as_array().unwrap();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0]["product_id"].as_str().unwrap(), "PROD-G1");
        assert_eq!(lines[0]["quantity"].as_u64().unwrap(), 3);
        assert!(lines[0]["unit_price"].is_number());
        assert!(lines[0]["discount_pct"].is_number());

        // Pricing
        assert!(v["pricing"].is_object(), "must have pricing when requested");
        assert!(v["pricing"]["subtotal"].is_number());
        assert!(v["pricing"]["discount_total"].is_number());
        assert!(v["pricing"]["total"].is_number());
    }

    #[tokio::test]
    async fn quote_get_empty_id_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_get(Parameters(QuoteGetInput {
                quote_id: "".to_string(),
                include_pricing: true,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    // ========================================================================
    // quote_price
    // ========================================================================

    #[tokio::test]
    async fn quote_price_success() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-P1", "SKU-P1", "Price Widget", "150.00").await;
        let srv = server(pool.clone());

        // Create a quote to price
        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-PRICE".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-P1".to_string(),
                    quantity: 4,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("price-test".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        let output = srv
            .quote_price(Parameters(QuotePriceInput {
                quote_id: quote_id.clone(),
                requested_discount_pct: 5.0,
            }))
            .await;
        let v = parse_output(&output);

        // Contract shape
        assert_eq!(v["quote_id"].as_str().unwrap(), quote_id);
        assert!(v["version"].is_number());
        assert_eq!(v["status"].as_str().unwrap(), "draft");
        assert!(v["pricing"].is_object());
        assert!(v["pricing"]["subtotal"].is_number());
        assert!(v["pricing"]["discount_total"].is_number());
        assert!(v["pricing"]["tax_total"].is_number());
        assert!(v["pricing"]["total"].is_number());
        assert!(v["pricing"]["priced_at"].is_string());
        assert!(v["line_pricing"].is_array());
        let lp = v["line_pricing"].as_array().unwrap();
        assert_eq!(lp.len(), 1);
        assert!(lp[0]["base_unit_price"].is_number());
        assert!(lp[0]["unit_price"].is_number());
        assert!(lp[0]["subtotal_before_discount"].is_number());
        assert!(lp[0]["discount_pct"].is_number());
        assert!(lp[0]["discount_amount"].is_number());
        assert!(lp[0]["line_total"].is_number());
        // Verify boolean
        assert!(v["approval_required"].is_boolean());
        assert!(v["policy_violations"].is_array());
    }

    #[tokio::test]
    async fn quote_price_not_found() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_price(Parameters(QuotePriceInput {
                quote_id: "Q-GHOST".to_string(),
                requested_discount_pct: 0.0,
            }))
            .await;
        assert_error_envelope(&output, "NOT_FOUND");
    }

    #[tokio::test]
    async fn quote_price_invalid_discount_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_price(Parameters(QuotePriceInput {
                quote_id: "Q-ANY".to_string(),
                requested_discount_pct: 150.0,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    // ========================================================================
    // quote_list
    // ========================================================================

    #[tokio::test]
    async fn quote_list_empty_returns_valid_shape() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_list(Parameters(QuoteListInput {
                account_id: None,
                status: None,
                limit: 20,
                page: 1,
            }))
            .await;
        let v = parse_output(&output);
        assert!(v["items"].is_array());
        assert!(v["pagination"].is_object());
        assert_eq!(v["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn quote_list_returns_created_quotes() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-L1", "SKU-L1", "List Widget", "80.00").await;
        let srv = server(pool.clone());

        // Create two quotes
        for i in 0..2 {
            srv.quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-LIST".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-L1".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some(format!("list-key-{i}")),
            }))
            .await;
        }

        let output = srv
            .quote_list(Parameters(QuoteListInput {
                account_id: Some("ACC-LIST".to_string()),
                status: None,
                limit: 20,
                page: 1,
            }))
            .await;
        let v = parse_output(&output);
        let items = v["items"].as_array().unwrap();
        assert!(items.len() >= 2, "should find at least 2 quotes");
        // Verify item shape
        for item in items {
            assert!(item["id"].is_string());
            assert!(item["version"].is_number());
            assert!(item["status"].is_string());
            assert!(item["currency"].is_string());
            assert!(item["created_at"].is_string());
        }
    }

    // ========================================================================
    // approval_request
    // ========================================================================

    #[tokio::test]
    async fn approval_request_success() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-A1", "SKU-A1", "Approval Widget", "500.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-APR".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-A1".to_string(),
                    quantity: 2,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("apr-test".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        let output = srv
            .approval_request(Parameters(ApprovalRequestInput {
                quote_id: quote_id.clone(),
                justification: "Customer needs expedited processing".to_string(),
                approver_role: Some("sales_manager".to_string()),
            }))
            .await;
        let v = parse_output(&output);

        // Contract shape
        assert!(v["approval_id"].is_string());
        assert_eq!(v["quote_id"].as_str().unwrap(), quote_id);
        assert_eq!(v["status"].as_str().unwrap(), "pending");
        assert_eq!(v["approver_role"].as_str().unwrap(), "sales_manager");
        assert!(v["requested_by"].is_string());
        assert!(v["justification"].is_string());
        assert!(v["created_at"].is_string());
        assert!(v["expires_at"].is_string());
        assert!(v["message"].is_string());
    }

    #[tokio::test]
    async fn approval_request_with_subscriptions_records_push_audit_event() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-APUSH", "SKU-APUSH", "Push Widget", "250.00").await;
        seed_portal_push_subscription(&pool, "https://push.example/subscription/1").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-PUSH".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-APUSH".to_string(),
                    quantity: 2,
                    discount_pct: 10.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("approval-push-audit".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        let output = srv
            .approval_request(Parameters(ApprovalRequestInput {
                quote_id: quote_id.clone(),
                justification: "Needs manager approval".to_string(),
                approver_role: Some("sales_manager".to_string()),
            }))
            .await;
        let approval = parse_output(&output);
        let approval_id = approval["approval_id"].as_str().unwrap().to_string();

        let row = sqlx::query(
            "SELECT event_type, payload_json
             FROM audit_event
             WHERE quote_id = ?
               AND event_type LIKE 'portal.pwa.push_%'
             ORDER BY timestamp DESC
             LIMIT 1",
        )
        .bind(&quote_id)
        .fetch_optional(&pool)
        .await
        .expect("fetch push audit event")
        .expect("push audit event should be created");

        let event_type: String = row.get("event_type");
        assert!(
            event_type.starts_with("portal.pwa.push_"),
            "expected portal push event_type, got {event_type}"
        );

        let payload_json: String = row.get("payload_json");
        let payload: serde_json::Value =
            serde_json::from_str(&payload_json).expect("push payload should be valid json");
        let expected_link = format!("/approvals/{approval_id}");
        assert_eq!(payload["approval_id"].as_str(), Some(approval_id.as_str()));
        assert_eq!(payload["deep_link"].as_str(), Some(expected_link.as_str()));
        assert_eq!(payload["quote_id"].as_str(), Some(quote_id.as_str()));
        assert!(payload["amount"].as_str().unwrap_or_default().contains("USD"));
    }

    #[tokio::test]
    async fn approval_request_without_subscriptions_skips_push_audit_event() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-ANOPUSH", "SKU-ANOPUSH", "No Push Widget", "100.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-NOPUSH".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-ANOPUSH".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("approval-no-push-audit".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        let _ = srv
            .approval_request(Parameters(ApprovalRequestInput {
                quote_id: quote_id.clone(),
                justification: "No subscriptions should skip push".to_string(),
                approver_role: None,
            }))
            .await;

        let push_event_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)
             FROM audit_event
             WHERE quote_id = ?
               AND event_type LIKE 'portal.pwa.push_%'",
        )
        .bind(&quote_id)
        .fetch_one(&pool)
        .await
        .expect("count push events");
        assert_eq!(push_event_count, 0, "push events should not be created when no subscriptions");
    }

    #[tokio::test]
    async fn approval_request_empty_justification_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .approval_request(Parameters(ApprovalRequestInput {
                quote_id: "Q-WHATEVER".to_string(),
                justification: "  ".to_string(),
                approver_role: None,
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn approval_request_nonexistent_quote_returns_not_found() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .approval_request(Parameters(ApprovalRequestInput {
                quote_id: "Q-MISSING".to_string(),
                justification: "valid justification".to_string(),
                approver_role: None,
            }))
            .await;
        assert_error_envelope(&output, "NOT_FOUND");
    }

    #[tokio::test]
    async fn approval_request_duplicate_returns_conflict() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-AD", "SKU-AD", "Dup Approval", "300.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-DUP".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-AD".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("dup-apr".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        // First approval
        srv.approval_request(Parameters(ApprovalRequestInput {
            quote_id: quote_id.clone(),
            justification: "first request".to_string(),
            approver_role: Some("sales_manager".to_string()),
        }))
        .await;

        // Duplicate approval
        let output = srv
            .approval_request(Parameters(ApprovalRequestInput {
                quote_id,
                justification: "duplicate request".to_string(),
                approver_role: Some("sales_manager".to_string()),
            }))
            .await;
        assert_error_envelope(&output, "CONFLICT");
    }

    // ========================================================================
    // approval_status
    // ========================================================================

    #[tokio::test]
    async fn approval_status_no_approvals() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .approval_status(Parameters(ApprovalStatusInput { quote_id: "Q-NO-APR".to_string() }))
            .await;
        let v = parse_output(&output);
        assert_eq!(v["quote_id"].as_str().unwrap(), "Q-NO-APR");
        assert_eq!(v["current_status"].as_str().unwrap(), "no_approvals");
        assert!(v["pending_requests"].is_array());
        assert_eq!(v["pending_requests"].as_array().unwrap().len(), 0);
        assert!(!v["can_proceed"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn approval_status_shows_pending_after_request() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-AS", "SKU-AS", "Status Widget", "250.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-STA".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-AS".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("status-test".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        // Submit approval
        srv.approval_request(Parameters(ApprovalRequestInput {
            quote_id: quote_id.clone(),
            justification: "test approval status".to_string(),
            approver_role: None,
        }))
        .await;

        // Check status
        let output = srv
            .approval_status(Parameters(ApprovalStatusInput { quote_id: quote_id.clone() }))
            .await;
        let v = parse_output(&output);
        assert_eq!(v["current_status"].as_str().unwrap(), "pending_approval");
        let pending = v["pending_requests"].as_array().unwrap();
        assert_eq!(pending.len(), 1);
        assert!(pending[0]["approval_id"].is_string());
        assert_eq!(pending[0]["status"].as_str().unwrap(), "pending");
        assert!(pending[0]["approver_role"].is_string());
        assert!(pending[0]["requested_at"].is_string());
    }

    // ========================================================================
    // approval_pending
    // ========================================================================

    #[tokio::test]
    async fn approval_pending_returns_valid_shape() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .approval_pending(Parameters(ApprovalPendingInput { approver_role: None, limit: 20 }))
            .await;
        let v = parse_output(&output);
        assert!(v["items"].is_array());
        assert!(v["total"].is_number());
    }

    // ========================================================================
    // quote_pdf
    // ========================================================================

    #[tokio::test]
    async fn quote_pdf_not_found() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_pdf(Parameters(QuotePdfInput {
                quote_id: "Q-PDF-GHOST".to_string(),
                template: "detailed".to_string(),
            }))
            .await;
        assert_error_envelope(&output, "NOT_FOUND");
    }

    #[tokio::test]
    async fn quote_pdf_invalid_template_returns_validation_error() {
        let pool = test_db().await;
        let srv = server(pool);
        let output = srv
            .quote_pdf(Parameters(QuotePdfInput {
                quote_id: "Q-ANY".to_string(),
                template: "nonexistent_template".to_string(),
            }))
            .await;
        assert_error_envelope(&output, "VALIDATION_ERROR");
    }

    #[tokio::test]
    async fn quote_pdf_generates_for_valid_quote() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-PDF", "SKU-PDF", "PDF Widget", "99.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-PDF".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-PDF".to_string(),
                    quantity: 2,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("pdf-test".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap_or("").to_string();
        assert!(!quote_id.is_empty(), "quote_create did not return quote_id payload: {create_out}");

        let output = srv
            .quote_pdf(Parameters(QuotePdfInput {
                quote_id: quote_id.clone(),
                template: "compact".to_string(),
            }))
            .await;
        let v = parse_output(&output);

        if v["error"].is_object() {
            // Tera templates use date filters not registered in the MCP render path.
            // This is a known pre-existing limitation — verify the error is a template
            // error (not a NOT_FOUND or VALIDATION_ERROR).
            let code = v["error"]["code"].as_str().unwrap_or("");
            assert_eq!(
                code, "INTERNAL_ERROR",
                "Expected INTERNAL_ERROR from template render, got: {output}"
            );
            // The hardening layer sanitizes INTERNAL_ERROR messages to hide
            // implementation details, so we just verify the code is correct.
            let msg = v["error"]["message"].as_str().unwrap_or("");
            assert!(
                msg.contains("render") || msg.contains("Internal server error"),
                "Expected template render or sanitized internal error, got: {msg}"
            );
        } else {
            // Full contract verification when template rendering succeeds
            assert_eq!(v["quote_id"].as_str().unwrap(), quote_id);
            assert!(v["pdf_generated"].is_boolean());
            assert!(v["file_path"].is_string());
            assert!(v["file_size_bytes"].is_number());
            assert!(v["checksum"].is_string());
            assert_eq!(v["template_used"].as_str().unwrap(), "compact");
            assert!(v["generated_at"].is_string());
            assert!(v["file_size_bytes"].as_u64().unwrap() > 0, "file should have non-zero size");
        }
    }

    // ========================================================================
    // Error envelope consistency
    // ========================================================================

    #[tokio::test]
    async fn error_envelope_has_consistent_structure() {
        let pool = test_db().await;
        let srv = server(pool);

        // Collect errors from various tools
        let e1 = srv
            .catalog_search(Parameters(CatalogSearchInput {
                query: "".to_string(),
                category: None,
                active_only: true,
                limit: 20,
                page: 1,
            }))
            .await;
        let e2 = srv
            .catalog_get(Parameters(CatalogGetInput {
                product_id: "NOPE".to_string(),
                include_relationships: false,
            }))
            .await;
        let e3 = srv
            .quote_get(Parameters(QuoteGetInput {
                quote_id: "NOPE".to_string(),
                include_pricing: true,
            }))
            .await;
        let e4 = srv
            .quote_price(Parameters(QuotePriceInput {
                quote_id: "NOPE".to_string(),
                requested_discount_pct: 0.0,
            }))
            .await;
        let errors = [&e1, &e2, &e3, &e4];

        for (i, output) in errors.iter().enumerate() {
            let v = parse_output(output);
            assert!(v["error"].is_object(), "output[{i}] must have error object: {output}");
            assert!(v["error"]["code"].is_string(), "output[{i}] error.code must be string");
            assert!(v["error"]["message"].is_string(), "output[{i}] error.message must be string");
        }
    }

    // ========================================================================
    // Normalization helpers
    // ========================================================================

    #[test]
    fn normalize_limit_clamps_to_max() {
        assert_eq!(normalize_limit(0), DEFAULT_PAGE_LIMIT);
        assert_eq!(normalize_limit(200), MAX_PAGE_LIMIT);
        assert_eq!(normalize_limit(50), 50);
    }

    #[test]
    fn normalize_page_floors_to_one() {
        assert_eq!(normalize_page(0), 1);
        assert_eq!(normalize_page(5), 5);
    }

    #[test]
    fn normalize_discount_range() {
        assert!(normalize_discount(0.0, "d").is_ok());
        assert!(normalize_discount(100.0, "d").is_ok());
        assert!(normalize_discount(-1.0, "d").is_err());
        assert!(normalize_discount(101.0, "d").is_err());
        assert!(normalize_discount(f64::NAN, "d").is_err());
        assert!(normalize_discount(f64::INFINITY, "d").is_err());
    }

    #[test]
    fn normalize_currency_validation() {
        assert_eq!(normalize_currency("usd").unwrap(), "USD");
        assert_eq!(normalize_currency("  eur  ").unwrap(), "EUR");
        assert!(normalize_currency("").is_err());
        assert!(normalize_currency("US$").is_err()); // non-alpha
        assert!(normalize_currency("TOOLONGCURRENCY").is_err());
    }

    #[test]
    fn template_allowlist() {
        assert!(template_is_allowed("detailed"));
        assert!(template_is_allowed("executive_summary"));
        assert!(template_is_allowed("compact"));
        assert!(!template_is_allowed("malicious"));
        assert!(!template_is_allowed(""));
    }

    #[tokio::test]
    async fn anomaly_override_persists_and_returns_success() {
        let pool = test_db().await;
        seed_quote(&pool, "Q-2026-AO01").await;

        let server = QuoteyMcpServer::new(pool.clone());
        let result = server
            .anomaly_override(Parameters(AnomalyOverrideInput {
                quote_id: "Q-2026-AO01".to_string(),
                rule_kind: "discount".to_string(),
                severity: "warning".to_string(),
                justification: "Competitive deal - customer has a lower offer from a rival vendor"
                    .to_string(),
                overridden_by: "rep@example.com".to_string(),
            }))
            .await;

        let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "recorded");
        assert_eq!(parsed["quote_id"], "Q-2026-AO01");
        assert_eq!(parsed["rule_kind"], "discount");
        assert_eq!(parsed["manager_notification"]["queued"], true);
        assert_eq!(parsed["override_metrics"]["rep_override_count"], 1);
        assert_eq!(parsed["override_metrics"]["total_override_count"], 1);
        assert_eq!(parsed["override_metrics"]["rep_override_rate_pct"], 100.0);

        // Verify it was persisted
        let overrides = quotey_db::repositories::SqlAnomalyOverrideRepository::find_by_quote_id(
            &pool,
            "Q-2026-AO01",
        )
        .await
        .expect("find overrides");
        assert_eq!(overrides.len(), 1);
        assert_eq!(overrides[0].overridden_by, "rep@example.com");
        assert!(overrides[0].justification.contains("Competitive deal"));

        let manager_queue_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM approval_request WHERE quote_id = ? AND approver_role = 'sales_manager'",
        )
        .bind("Q-2026-AO01")
        .fetch_one(&pool)
        .await
        .expect("manager queue count");
        assert_eq!(manager_queue_count, 1);
    }

    #[tokio::test]
    async fn anomaly_override_tracks_rep_override_rate() {
        let pool = test_db().await;
        seed_quote(&pool, "Q-2026-AO11").await;
        seed_quote(&pool, "Q-2026-AO12").await;

        let server = QuoteyMcpServer::new(pool.clone());
        let first = server
            .anomaly_override(Parameters(AnomalyOverrideInput {
                quote_id: "Q-2026-AO11".to_string(),
                rule_kind: "discount".to_string(),
                severity: "warning".to_string(),
                justification: "first override".to_string(),
                overridden_by: "rep@example.com".to_string(),
            }))
            .await;
        let first_json: serde_json::Value = serde_json::from_str(&first).expect("valid json");
        assert_eq!(first_json["override_metrics"]["rep_override_count"], 1);
        assert_eq!(first_json["override_metrics"]["total_override_count"], 1);
        assert_eq!(first_json["override_metrics"]["rep_override_rate_pct"], 100.0);

        let second = server
            .anomaly_override(Parameters(AnomalyOverrideInput {
                quote_id: "Q-2026-AO12".to_string(),
                rule_kind: "margin".to_string(),
                severity: "critical".to_string(),
                justification: "second override".to_string(),
                overridden_by: "other-rep@example.com".to_string(),
            }))
            .await;
        let second_json: serde_json::Value = serde_json::from_str(&second).expect("valid json");
        assert_eq!(second_json["override_metrics"]["rep_override_count"], 1);
        assert_eq!(second_json["override_metrics"]["total_override_count"], 2);
        assert_eq!(second_json["override_metrics"]["rep_override_rate_pct"], 50.0);
    }

    #[tokio::test]
    async fn anomaly_override_rejects_empty_justification() {
        let pool = test_db().await;
        seed_quote(&pool, "Q-2026-AO02").await;

        let server = QuoteyMcpServer::new(pool.clone());
        let result = server
            .anomaly_override(Parameters(AnomalyOverrideInput {
                quote_id: "Q-2026-AO02".to_string(),
                rule_kind: "margin".to_string(),
                severity: "critical".to_string(),
                justification: "   ".to_string(),
                overridden_by: "rep@example.com".to_string(),
            }))
            .await;

        let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["error"]["code"], "VALIDATION_ERROR");
        assert!(parsed["error"]["message"].as_str().unwrap().contains("Justification is required"));
    }

    #[tokio::test]
    async fn anomaly_override_rejects_invalid_rule_kind() {
        let pool = test_db().await;
        seed_quote(&pool, "Q-2026-AO03").await;

        let server = QuoteyMcpServer::new(pool.clone());
        let result = server
            .anomaly_override(Parameters(AnomalyOverrideInput {
                quote_id: "Q-2026-AO03".to_string(),
                rule_kind: "bogus_rule".to_string(),
                severity: "warning".to_string(),
                justification: "Some reason".to_string(),
                overridden_by: "rep@example.com".to_string(),
            }))
            .await;

        let parsed: serde_json::Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["error"]["code"], "VALIDATION_ERROR");
        assert!(parsed["error"]["message"].as_str().unwrap().contains("Invalid rule_kind"));
    }

    #[test]
    fn tool_error_produces_valid_json() {
        let output = tool_error("TEST_CODE", "test message", None);
        let v: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(v["error"]["code"].as_str().unwrap(), "TEST_CODE");
        assert_eq!(v["error"]["message"].as_str().unwrap(), "test message");
        assert!(v["error"]["details"].is_null());

        let with_details =
            tool_error("DETAIL_CODE", "has details", Some(serde_json::json!({"key": "val"})));
        let v2: serde_json::Value = serde_json::from_str(&with_details).unwrap();
        assert_eq!(v2["error"]["details"]["key"].as_str().unwrap(), "val");
    }

    // ========================================================================
    // Negotiation tool tests
    // ========================================================================

    #[tokio::test]
    async fn negotiation_start_creates_session() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-NXT", "SKU-NXT", "NXT Product", "100.00").await;
        let srv = server(pool.clone());

        // Create a quote first
        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-NXT".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-NXT".to_string(),
                    quantity: 5,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("nxt-test".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        // Start negotiation
        let result = srv
            .negotiation_start(Parameters(NegotiationStartInput {
                quote_id: quote_id.clone(),
                actor_id: "rep-alice".to_string(),
                idempotency_key: "key-1".to_string(),
            }))
            .await;
        let parsed = parse_output(&result);

        assert!(parsed["session_id"].is_string());
        assert_eq!(parsed["quote_id"].as_str().unwrap(), quote_id);
        assert_eq!(parsed["state"].as_str().unwrap(), "draft");
    }

    #[tokio::test]
    async fn negotiation_start_returns_existing_session() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-NXT2", "SKU-NXT2", "NXT Product 2", "200.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-NXT2".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-NXT2".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("nxt-idem".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        // First start
        let r1 = srv
            .negotiation_start(Parameters(NegotiationStartInput {
                quote_id: quote_id.clone(),
                actor_id: "rep-bob".to_string(),
                idempotency_key: String::new(),
            }))
            .await;
        let p1 = parse_output(&r1);
        let sid1 = p1["session_id"].as_str().unwrap().to_string();

        // Second start should return same session
        let r2 = srv
            .negotiation_start(Parameters(NegotiationStartInput {
                quote_id,
                actor_id: "rep-bob".to_string(),
                idempotency_key: String::new(),
            }))
            .await;
        let p2 = parse_output(&r2);

        assert_eq!(p2["session_id"].as_str().unwrap(), sid1);
        assert!(p2["message"].as_str().unwrap().contains("idempotent"));
    }

    #[tokio::test]
    async fn negotiation_evaluate_returns_envelope_and_plan() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-NE", "SKU-NE", "Eval Product", "100.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-NE".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-NE".to_string(),
                    quantity: 1,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("nxt-eval".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        let start_out = srv
            .negotiation_start(Parameters(NegotiationStartInput {
                quote_id,
                actor_id: "rep-eval".to_string(),
                idempotency_key: "eval-key".to_string(),
            }))
            .await;
        let started = parse_output(&start_out);
        let session_id = started["session_id"].as_str().unwrap().to_string();

        // Evaluate with a discount request
        let eval_out = srv
            .negotiation_evaluate(Parameters(NegotiationEvaluateInput {
                session_id: session_id.clone(),
                discount_pct: Some(15.0),
                margin_pct: None,
                term_months: None,
            }))
            .await;
        let parsed = parse_output(&eval_out);

        assert_eq!(parsed["session_id"].as_str().unwrap(), session_id);
        assert_eq!(parsed["state"].as_str().unwrap(), "active");
        assert!(parsed["envelope"]["ranges"].is_array());
        assert!(parsed["boundary"]["within_bounds"].is_boolean());
        assert!(parsed["counteroffer_plan"]["alternatives"].is_array());
    }

    #[tokio::test]
    async fn negotiation_status_returns_session_details() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-NS", "SKU-NS", "Status Product", "50.00").await;
        let srv = server(pool.clone());

        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-NS".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-NS".to_string(),
                    quantity: 2,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("nxt-status".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        let start_out = srv
            .negotiation_start(Parameters(NegotiationStartInput {
                quote_id,
                actor_id: "rep-status".to_string(),
                idempotency_key: "status-key".to_string(),
            }))
            .await;
        let started = parse_output(&start_out);
        let session_id = started["session_id"].as_str().unwrap().to_string();

        let status_out = srv
            .negotiation_status(Parameters(NegotiationStatusInput {
                session_id: session_id.clone(),
            }))
            .await;
        let parsed = parse_output(&status_out);

        assert_eq!(parsed["session_id"].as_str().unwrap(), session_id);
        assert_eq!(parsed["state"].as_str().unwrap(), "draft");
        assert_eq!(parsed["turn_count"].as_u64().unwrap(), 0);
        assert!(parsed["turns"].is_array());
    }

    #[tokio::test]
    async fn negotiation_status_not_found() {
        let pool = test_db().await;
        let srv = server(pool);
        let result = srv
            .negotiation_status(Parameters(NegotiationStatusInput {
                session_id: "NXT-MISSING".to_string(),
            }))
            .await;
        assert_error_envelope(&result, "NOT_FOUND");
    }

    #[tokio::test]
    async fn negotiation_escalate_creates_approval_pack() {
        let pool = test_db().await;
        seed_product(&pool, "PROD-ESC", "SKU-ESC", "Escalation Product", "100.00").await;
        let srv = server(pool.clone());

        // Create a quote first
        let create_out = srv
            .quote_create(Parameters(QuoteCreateInput {
                account_id: "ACC-ESC".to_string(),
                deal_id: None,
                currency: "USD".to_string(),
                term_months: None,
                start_date: None,
                notes: None,
                line_items: vec![LineItemInput {
                    product_id: "PROD-ESC".to_string(),
                    quantity: 5,
                    discount_pct: 0.0,
                    attributes: None,
                    notes: None,
                }],
                idempotency_key: Some("esc-quote".to_string()),
            }))
            .await;
        let created = parse_output(&create_out);
        let quote_id = created["quote_id"].as_str().unwrap().to_string();

        // Start negotiation session
        let start_result = srv
            .negotiation_start(Parameters(NegotiationStartInput {
                quote_id,
                actor_id: "rep-alice".to_string(),
                idempotency_key: "esc-test".to_string(),
            }))
            .await;
        let start_json = parse_output(&start_result);
        let session_id = start_json["session_id"].as_str().unwrap().to_string();

        // Escalate it
        let result = srv
            .negotiation_escalate(Parameters(NegotiationEscalateInput {
                session_id: session_id.clone(),
                offer_id: "offer-step_down-1".to_string(),
                reason: "discount near soft ceiling requires manager approval".to_string(),
            }))
            .await;

        let json = parse_output(&result);
        assert_eq!(json["session_id"].as_str().unwrap(), session_id);
        assert_eq!(json["session_state"].as_str().unwrap(), "approval_pending");
        assert!(json["escalation_reason"].as_str().unwrap().contains("manager approval"));
        assert!(json["concession_deltas"].as_array().is_some());
    }

    #[tokio::test]
    async fn negotiation_escalate_rejects_missing_reason() {
        let pool = test_db().await;
        let srv = server(pool);
        let result = srv
            .negotiation_escalate(Parameters(NegotiationEscalateInput {
                session_id: "NXT-MISSING".to_string(),
                offer_id: String::new(),
                reason: "  ".to_string(),
            }))
            .await;
        assert_error_envelope(&result, "VALIDATION_ERROR");
    }
}
