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

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use tera::{Context, Tera};

use crate::auth::{AuthManager, AuthResult};
use quotey_core::domain::quote::Quote;
use quotey_db::repositories::ApprovalRepository;

const MAX_PAGE_LIMIT: u32 = 100;
const DEFAULT_PAGE_LIMIT: u32 = 20;

fn tool_error(code: &str, message: &str, details: Option<serde_json::Value>) -> String {
    let payload = serde_json::json!({
        "error": {
            "code": code,
            "message": message,
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
    value.to_string().parse::<f64>().unwrap_or(0.0)
}

fn build_quote_id(account_id: &str, input: &QuoteCreateInput) -> String {
    if let Some(key) = input.idempotency_key.as_deref().filter(|v| !v.trim().is_empty()) {
        let mut hasher = DefaultHasher::new();
        account_id.hash(&mut hasher);
        key.hash(&mut hasher);
        input.currency.hash(&mut hasher);
        input.term_months.hash(&mut hasher);
        input.deal_id.hash(&mut hasher);
        for item in &input.line_items {
            item.product_id.hash(&mut hasher);
            item.quantity.hash(&mut hasher);
            item.discount_pct.to_bits().hash(&mut hasher);
        }
        format!("Q-{:016x}", hasher.finish())
    } else {
        format!("Q-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0000"))
    }
}

fn allowed_pdf_templates() -> &'static [&'static str] {
    &["detailed", "executive_summary", "compact"]
}

fn template_is_allowed(template: &str) -> bool {
    allowed_pdf_templates().contains(&template)
}

fn checksum_of(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("mock-checksum:{:016x}", hasher.finish())
}

fn checksum_of_bytes(value: &[u8]) -> String {
    use std::hash::Hasher as _;
    let mut hasher = DefaultHasher::new();
    hasher.write(value);
    format!("mock-checksum:{:016x}", hasher.finish())
}

fn sanitize_filename(value: &str) -> String {
    value
        .chars()
        .map(
            |c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' },
        )
        .collect()
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
}

impl QuoteyMcpServer {
    /// Create a new MCP server instance with database pool
    pub fn new(db_pool: quotey_db::DbPool) -> Self {
        info!("Initializing Quotey MCP Server (no auth)");
        let tool_router = Self::tool_router();
        let auth_manager = AuthManager::no_auth();
        Self { db_pool, tool_router, auth_manager }
    }

    /// Create a new MCP server with authentication
    pub fn with_auth(db_pool: quotey_db::DbPool, auth_manager: AuthManager) -> Self {
        info!("Initializing Quotey MCP Server (with auth)");
        let tool_router = Self::tool_router();
        Self { db_pool, tool_router, auth_manager }
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

    /// Validate the current request against the auth manager.
    ///
    /// Clients pass their key via the MCP `_meta` field on tool-call requests:
    /// ```json
    /// { "method": "tools/call", "params": {
    ///     "name": "catalog_search",
    ///     "arguments": { "query": "pro" },
    ///     "_meta": { "api_key": "your-secret-key" }
    /// }}
    /// ```
    async fn check_auth(&self, meta: &rmcp::model::Meta) -> Result<AuthResult, rmcp::ErrorData> {
        let api_key = meta.0.get("api_key").and_then(|v| v.as_str());

        let result = self.auth_manager.validate_request(api_key).await;

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
                let mut error = rmcp::ErrorData::invalid_request(
                    format!("Authentication failed: {}", reason),
                    None,
                );
                if let Some(retry) = retry_after {
                    error.data = Some(serde_json::json!({
                        "retry_after": retry
                    }));
                }
                Err(error)
            }
        }
    }
}

impl ServerHandler for QuoteyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
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
        // Enforce authentication when configured.
        // Clients pass their API key via `_meta.api_key` on each tool-call request.
        // When auth is not required the check is a no-op (returns Allowed).
        self.check_auth(&context.meta).await?;

        // Route to tool handler
        let tool_call_context = ToolCallContext::new(self, request, context);
        self.tool_router.call(tool_call_context).await
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
#[derive(Debug, Deserialize, JsonSchema)]
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

#[derive(Debug, Deserialize, JsonSchema)]
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
        let discount_pct = line.discount_pct.max(0.0).min(100.0);
        let line_discount = line_subtotal * discount_pct / 100.0;
        let line_total = line_subtotal - line_discount;

        subtotal += line_subtotal;
        discount_total += line_discount;

        line_rows.push(serde_json::json!({
            "product_id": line.product_id.0,
            "product_name": line.product_id.0,
            "quantity": line.quantity,
            "unit_price": unit_price,
            "subtotal": line_total,
            "discount_pct": discount_pct,
            "discount_amount": line_discount,
            "total_price": line_total,
            "line_subtotal": line_total,
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
    context.insert(
        "company_name",
        &payload.get("company_name").cloned().unwrap_or(serde_json::json!("Quotey")),
    );
    context.insert(
        "primary_color",
        &payload.get("primary_color").cloned().unwrap_or(serde_json::json!("#2563eb")),
    );
    context.insert("white_label", &false);

    let template_name = format!("{}.html.tera", template);
    let html = tera.render(&template_name, &context).map_err(|e| e.to_string())?;

    let temp_dir = std::env::temp_dir();
    let html_path: PathBuf = temp_dir.join(format!("quotey-mcp-{}.html", uuid::Uuid::new_v4()));
    let pdf_path: PathBuf = temp_dir.join(format!("quotey-mcp-{}.pdf", uuid::Uuid::new_v4()));
    std::fs::write(&html_path, html.as_bytes()).map_err(|e| e.to_string())?;

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
        .arg(html_path.to_string_lossy().to_string())
        .arg(pdf_path.to_string_lossy().to_string())
        .output();

    let cleanup = |html_path: PathBuf, pdf_path: PathBuf| {
        let _ = std::fs::remove_file(html_path);
        let _ = std::fs::remove_file(pdf_path);
    };

    match output {
        Ok(result) if result.status.success() => {
            let pdf_bytes = match std::fs::read(&pdf_path) {
                Ok(bytes) => bytes,
                Err(read_err) => {
                    warn!(error = %read_err, "quote_pdf: failed to read generated PDF, returning HTML fallback");
                    cleanup(html_path, pdf_path);
                    return Ok(PdfRenderResult::Html(html));
                }
            };
            cleanup(html_path, pdf_path);
            Ok(PdfRenderResult::Pdf(pdf_bytes))
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            warn!(stderr = %stderr, "quote_pdf: wkhtmltopdf returned non-zero status, using HTML fallback");
            cleanup(html_path, pdf_path);
            Ok(PdfRenderResult::Html(html))
        }
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                warn!("quote_pdf: wkhtmltopdf not found, using HTML fallback");
            } else {
                warn!(error = %err, "quote_pdf: wkhtmltopdf command failed, using HTML fallback");
            }
            cleanup(html_path, pdf_path);
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
    async fn catalog_search(&self, Parameters(input): Parameters<CatalogSearchInput>) -> String {
        debug!(query = %input.query, "catalog_search called");

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
                        product.family_id.as_ref().is_none_or(|f| f.0 == category_filter)
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
                tool_error("INTERNAL_ERROR", &e.to_string(), None)
            }
        }
    }

    #[tool(description = "Get detailed product information by ID")]
    async fn catalog_get(&self, Parameters(input): Parameters<CatalogGetInput>) -> String {
        debug!(product_id = %input.product_id, "catalog_get called");

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
                tool_error("INTERNAL_ERROR", &e.to_string(), None)
            }
        }
    }

    // Quote Tools
    #[tool(description = "Create a new quote for a customer")]
    async fn quote_create(&self, Parameters(input): Parameters<QuoteCreateInput>) -> String {
        debug!(account_id = %input.account_id, "quote_create called");

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

            if item.quantity == 0 {
                return tool_error(
                    "VALIDATION_ERROR",
                    &format!("line_items[{}].quantity must be > 0", i),
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
                    return tool_error("INTERNAL_ERROR", &e.to_string(), None);
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

            let unit_price = product.base_price.unwrap_or_else(|| Decimal::ZERO);
            let subtotal = unit_price * Decimal::from(item.quantity as u32);
            let discount_rate = Decimal::from_f64(discount_pct).unwrap_or_else(|| Decimal::ZERO);
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
                tool_error("INTERNAL_ERROR", &e.to_string(), None)
            }
        }
    }

    #[tool(description = "Get detailed quote information")]
    async fn quote_get(&self, Parameters(input): Parameters<QuoteGetInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_get called");

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
                tool_error("INTERNAL_ERROR", &e.to_string(), None)
            }
        }
    }

    #[tool(description = "Run pricing engine on a quote")]
    async fn quote_price(&self, Parameters(input): Parameters<QuotePriceInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_price called");

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
                return tool_error("INTERNAL_ERROR", &e.to_string(), None);
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
    async fn quote_list(&self, Parameters(input): Parameters<QuoteListInput>) -> String {
        debug!("quote_list called");

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
                tool_error("INTERNAL_ERROR", &e.to_string(), None)
            }
        }
    }

    // Approval Tools
    #[tool(description = "Submit a quote for approval")]
    async fn approval_request(
        &self,
        Parameters(input): Parameters<ApprovalRequestInput>,
    ) -> String {
        debug!(quote_id = %input.quote_id, "approval_request called");

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
                return tool_error("INTERNAL_ERROR", &e.to_string(), None);
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
                    return tool_error("INTERNAL_ERROR", &e.to_string(), None);
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
            return tool_error("INTERNAL_ERROR", &e.to_string(), None);
        }

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
    async fn approval_status(&self, Parameters(input): Parameters<ApprovalStatusInput>) -> String {
        debug!(quote_id = %input.quote_id, "approval_status called");

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
                return tool_error("INTERNAL_ERROR", &e.to_string(), None);
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
    async fn approval_pending(
        &self,
        Parameters(input): Parameters<ApprovalPendingInput>,
    ) -> String {
        debug!("approval_pending called");

        let limit = normalize_limit(input.limit);
        let approver_role = normalize_optional_trimmed(&input.approver_role);

        use quotey_db::repositories::ApprovalRepository;
        use quotey_db::repositories::QuoteRepository;

        let repo = quotey_db::repositories::SqlApprovalRepository::new(self.db_pool.clone());
        let pending = match repo.list_pending(approver_role.as_deref(), limit).await {
            Ok(p) => p,
            Err(e) => {
                warn!(error = %e, "approval_pending: failed to list");
                return tool_error("INTERNAL_ERROR", &e.to_string(), None);
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
    async fn quote_pdf(&self, Parameters(input): Parameters<QuotePdfInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_pdf called");
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
                return tool_error("INTERNAL_ERROR", &e.to_string(), None);
            }
        };

        let payload = build_pdf_quote_payload(&quote);
        let render_result = match render_quote_pdf_html_to_bytes(&payload, &template).await {
            Ok(rendered) => rendered,
            Err(err) => {
                warn!(error = %err, "quote_pdf: failed to render PDF html");
                return tool_error("INTERNAL_ERROR", &err, None);
            }
        };

        let generated_at = chrono::Utc::now().to_rfc3339();
        let (dir, file_path, pdf_generated, file_size_bytes, checksum) = match &render_result {
            PdfRenderResult::Pdf(bytes) => {
                let (dir, file_path) = payload_to_output_path(&quote_id, &template, true);
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    warn!(error = %e, "quote_pdf: failed to create output directory");
                    return tool_error("INTERNAL_ERROR", &e.to_string(), None);
                }
                if let Err(e) = std::fs::write(&file_path, bytes) {
                    warn!(error = %e, "quote_pdf: failed to write artifact");
                    return tool_error("INTERNAL_ERROR", &e.to_string(), None);
                }
                let checksum = checksum_of_bytes(bytes);
                (dir, file_path, true, bytes.len() as u64, checksum)
            }
            PdfRenderResult::Html(html) => {
                let (dir, file_path) = payload_to_output_path(&quote_id, &template, false);
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    warn!(error = %e, "quote_pdf: failed to create output directory");
                    return tool_error("INTERNAL_ERROR", &e.to_string(), None);
                }
                if let Err(e) = std::fs::write(&file_path, html) {
                    warn!(error = %e, "quote_pdf: failed to write artifact");
                    return tool_error("INTERNAL_ERROR", &e.to_string(), None);
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
}
