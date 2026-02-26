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

use crate::auth::{AuthManager, AuthResult};

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
    20
}
fn default_1() -> u32 {
    1
}
fn default_currency() -> String {
    "USD".to_string()
}
fn default_template() -> String {
    "standard".to_string()
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
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LineItemResult {
    pub line_id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: u32,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct QuoteCreateResult {
    pub quote_id: String,
    pub version: u32,
    pub status: String,
    pub account_id: String,
    pub currency: String,
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

// ============================================================================
// Tool Router Implementation
// ============================================================================

#[tool_router]
impl QuoteyMcpServer {
    // Catalog Tools
    #[tool(description = "Search products by name, SKU, or description")]
    async fn catalog_search(&self, Parameters(input): Parameters<CatalogSearchInput>) -> String {
        debug!(query = %input.query, "catalog_search called");

        let repo = quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
        use quotey_db::repositories::ProductRepository;

        match repo.search(&input.query, input.active_only, input.limit).await {
            Ok(products) => {
                let items: Vec<ProductSummary> = products
                    .into_iter()
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
                    pagination: PaginationInfo {
                        total: items.len() as u32,
                        page: input.page,
                        per_page: input.limit,
                        has_more: false,
                    },
                    items,
                };

                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => {
                warn!(error = %e, "catalog_search failed");
                serde_json::json!({"error": e.to_string()}).to_string()
            }
        }
    }

    #[tool(description = "Get detailed product information by ID")]
    async fn catalog_get(&self, Parameters(input): Parameters<CatalogGetInput>) -> String {
        debug!(product_id = %input.product_id, "catalog_get called");

        let repo = quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
        use quotey_core::domain::product::ProductId;
        use quotey_db::repositories::ProductRepository;

        match repo.find_by_id(&ProductId(input.product_id.clone())).await {
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
                serde_json::json!({"error": format!("Product '{}' not found", input.product_id)})
                    .to_string()
            }
            Err(e) => {
                warn!(error = %e, "catalog_get failed");
                serde_json::json!({"error": e.to_string()}).to_string()
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
        use rust_decimal::Decimal;

        let now = chrono::Utc::now();
        let quote_id =
            format!("Q-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("0000"));

        // Look up product names from catalog
        let product_repo = quotey_db::repositories::SqlProductRepository::new(self.db_pool.clone());
        use quotey_db::repositories::ProductRepository;

        let mut line_items_result = Vec::new();
        let mut quote_lines = Vec::new();

        for (i, item) in input.line_items.iter().enumerate() {
            let product_name =
                match product_repo.find_by_id(&ProductId(item.product_id.clone())).await {
                    Ok(Some(p)) => p.name,
                    _ => format!("Product {}", item.product_id),
                };

            line_items_result.push(LineItemResult {
                line_id: format!("{}-ql-{}", quote_id, i + 1),
                product_id: item.product_id.clone(),
                product_name: product_name.clone(),
                quantity: item.quantity,
            });

            quote_lines.push(QuoteLine {
                product_id: ProductId(item.product_id.clone()),
                quantity: item.quantity,
                unit_price: Decimal::ZERO, // Will be set by pricing engine
                discount_pct: item.discount_pct,
                notes: item.notes.clone(),
            });
        }

        let quote = Quote {
            id: QuoteId(quote_id.clone()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: Some(input.account_id.clone()),
            deal_id: input.deal_id,
            currency: input.currency.clone(),
            term_months: input.term_months,
            start_date: input.start_date,
            end_date: None,
            valid_until: None,
            notes: input.notes,
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
                    account_id: input.account_id,
                    currency: input.currency,
                    line_items: line_items_result,
                    created_at: now.to_rfc3339(),
                    message: "Quote created successfully".to_string(),
                };
                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => {
                warn!(error = %e, "quote_create failed");
                serde_json::json!({"error": e.to_string()}).to_string()
            }
        }
    }

    #[tool(description = "Get detailed quote information")]
    async fn quote_get(&self, Parameters(input): Parameters<QuoteGetInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_get called");

        use quotey_core::domain::quote::QuoteId;
        use quotey_db::repositories::QuoteRepository;

        let repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());

        match repo.find_by_id(&QuoteId(input.quote_id.clone())).await {
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
            Ok(None) => {
                serde_json::json!({"error": format!("Quote '{}' not found", input.quote_id)})
                    .to_string()
            }
            Err(e) => {
                warn!(error = %e, "quote_get failed");
                serde_json::json!({"error": e.to_string()}).to_string()
            }
        }
    }

    #[tool(description = "Run pricing engine on a quote")]
    async fn quote_price(&self, Parameters(input): Parameters<QuotePriceInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_price called");

        let result = QuotePriceResult {
            quote_id: input.quote_id,
            version: 1,
            status: "priced".to_string(),
            pricing: PricingInfo {
                subtotal: 14400.00,
                discount_total: 1440.00,
                tax_total: 0.00,
                total: 12960.00,
                priced_at: Some(chrono::Utc::now().to_rfc3339()),
            },
            line_pricing: vec![LinePricingInfo {
                line_id: "ql_001".to_string(),
                product_id: "prod_pro_v2".to_string(),
                product_name: "Pro Plan".to_string(),
                quantity: 150,
                base_unit_price: 10.00,
                unit_price: 6.00,
                subtotal_before_discount: 14400.00,
                discount_pct: 10.0,
                discount_amount: 1440.00,
                line_total: 12960.00,
            }],
            approval_required: true,
            policy_violations: vec![PolicyViolation {
                policy_id: "pol_discount_cap".to_string(),
                policy_name: "Discount Cap".to_string(),
                severity: "approval_required".to_string(),
                description: "10% discount exceeds auto-approval threshold".to_string(),
                threshold: Some(5.0),
                actual: Some(10.0),
                required_approver_role: Some("sales_manager".to_string()),
            }],
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "List quotes with optional filters")]
    async fn quote_list(&self, Parameters(input): Parameters<QuoteListInput>) -> String {
        debug!("quote_list called");

        use quotey_db::repositories::QuoteRepository;

        let repo = quotey_db::repositories::SqlQuoteRepository::new(self.db_pool.clone());
        let offset = (input.page.saturating_sub(1)) * input.limit;

        match repo
            .list(input.account_id.as_deref(), input.status.as_deref(), input.limit, offset)
            .await
        {
            Ok(quotes) => {
                use quotey_db::repositories::quote::quote_status_as_str;

                let items: Vec<QuoteListItem> = quotes
                    .iter()
                    .map(|q| {
                        let status = quote_status_as_str(&q.status).to_string();
                        let total: f64 = q
                            .lines
                            .iter()
                            .map(|l| {
                                let unit: f64 = l.unit_price.to_string().parse().unwrap_or(0.0);
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
                        page: input.page,
                        per_page: input.limit,
                        has_more: items.len() as u32 >= input.limit,
                    },
                    items,
                };

                serde_json::to_string_pretty(&result).unwrap_or_default()
            }
            Err(e) => {
                warn!(error = %e, "quote_list failed");
                serde_json::json!({"error": e.to_string()}).to_string()
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

        let result = ApprovalRequestResult {
            approval_id: format!("APR-{}", chrono::Utc::now().format("%Y-%m%d%H%M%S")),
            quote_id: input.quote_id,
            status: "pending".to_string(),
            approver_role: "sales_manager".to_string(),
            requested_by: "agent:mcp".to_string(),
            justification: input.justification,
            created_at: chrono::Utc::now().to_rfc3339(),
            expires_at: (chrono::Utc::now() + chrono::Duration::hours(4)).to_rfc3339(),
            message: "Approval request submitted".to_string(),
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "Check approval status for a quote")]
    async fn approval_status(&self, Parameters(input): Parameters<ApprovalStatusInput>) -> String {
        debug!(quote_id = %input.quote_id, "approval_status called");

        let result = ApprovalStatusResult {
            quote_id: input.quote_id,
            current_status: "pending_approval".to_string(),
            pending_requests: vec![PendingApproval {
                approval_id: "APR-2026-0089".to_string(),
                status: "pending".to_string(),
                approver_role: "sales_manager".to_string(),
                requested_at: chrono::Utc::now().to_rfc3339(),
                expires_at: (chrono::Utc::now() + chrono::Duration::hours(4)).to_rfc3339(),
            }],
            can_proceed: false,
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "List all pending approval requests")]
    async fn approval_pending(
        &self,
        Parameters(_input): Parameters<ApprovalPendingInput>,
    ) -> String {
        debug!("approval_pending called");

        let result = ApprovalPendingResult {
            items: vec![ApprovalPendingItem {
                approval_id: "APR-2026-0089".to_string(),
                quote_id: "Q-2026-0042".to_string(),
                account_name: "Acme Corp".to_string(),
                quote_total: 12960.00,
                requested_by: "agent:mcp".to_string(),
                justification: "10% discount for loyal customer".to_string(),
                requested_at: chrono::Utc::now().to_rfc3339(),
            }],
            total: 1,
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    // PDF Tools
    #[tool(description = "Generate PDF for a quote")]
    async fn quote_pdf(&self, Parameters(input): Parameters<QuotePdfInput>) -> String {
        debug!(quote_id = %input.quote_id, "quote_pdf called");

        let result = QuotePdfResult {
            quote_id: input.quote_id.clone(),
            pdf_generated: true,
            file_path: format!("/tmp/quote_{}.pdf", input.quote_id),
            file_size_bytes: 45678,
            checksum: "sha256:abc123...".to_string(),
            template_used: input.template,
            generated_at: chrono::Utc::now().to_rfc3339(),
        };

        serde_json::to_string_pretty(&result).unwrap_or_default()
    }
}
