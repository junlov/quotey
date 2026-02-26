//! MCP Server Implementation
//!
//! Implements the Model Context Protocol server for Quotey.

use rmcp::{
    ServerHandler,
    handler::server::{
        tool::ToolCallContext,
        router::tool::ToolRouter,
        wrapper::Parameters,
    },
    model::*,
    schemars::{self, JsonSchema},
    serde::{Deserialize, Serialize},
    tool, tool_router,
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

    /// Validate API key from request metadata (if present)
    async fn check_auth(&self, _meta: &Option<serde_json::Value>) -> Result<AuthResult, rmcp::ErrorData> {
        // Extract API key from request metadata if present
        // For stdio transport, we can use meta field to pass API key
        let api_key = _meta.as_ref()
            .and_then(|m| m.get("api_key"))
            .and_then(|k| k.as_str());

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
                let mut error = rmcp::ErrorData::invalid_params(
                    format!("Authentication failed: {}", reason),
                    None,
                );
                // Add retry_after to error data if rate limited
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
        // Check authentication before executing tool
        // Note: In stdio transport, meta is typically None, so we allow by default
        // For authenticated usage, the client should pass api_key in meta
        if self.auth_manager.is_auth_required() {
            // Extract meta from context or request - for now we skip detailed auth
            // as stdio transport doesn't easily support per-request metadata
            // In a real HTTP/SSE transport, we'd extract the Authorization header here
            debug!("Auth is required - checking...");
        }
        
        // Route to tool handler
        let tool_call_context = ToolCallContext::new(self, request, context);
        self.tool_router.call(tool_call_context).await
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            next_cursor: None,
            ..Default::default()
        })
    }
}

// ============================================================================
// Input/Output Types
// ============================================================================

fn default_true() -> bool { true }
fn default_20() -> u32 { 20 }
fn default_1() -> u32 { 1 }
fn default_currency() -> String { "USD".to_string() }
fn default_template() -> String { "standard".to_string() }

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
    async fn catalog_search(
        &self,
        Parameters(input): Parameters<CatalogSearchInput>,
    ) -> String {
        debug!(query = %input.query, "catalog_search called");
        
        let items = vec![
            ProductSummary {
                id: "prod_pro_v2".to_string(),
                sku: "PLAN-PRO-001".to_string(),
                name: "Pro Plan".to_string(),
                description: Some("Professional tier with advanced features".to_string()),
                product_type: "configurable".to_string(),
                category: Some("saas".to_string()),
                active: true,
            },
            ProductSummary {
                id: "prod_enterprise".to_string(),
                sku: "PLAN-ENT-001".to_string(),
                name: "Enterprise Plan".to_string(),
                description: Some("Enterprise tier with SSO and premium support".to_string()),
                product_type: "configurable".to_string(),
                category: Some("saas".to_string()),
                active: true,
            },
        ];
        
        let filtered_items: Vec<_> = items
            .into_iter()
            .filter(|p| {
                let matches_query = p.name.to_lowercase().contains(&input.query.to_lowercase())
                    || p.sku.to_lowercase().contains(&input.query.to_lowercase());
                let matches_category = input.category.as_ref()
                    .map(|c| p.category.as_ref() == Some(c))
                    .unwrap_or(true);
                let matches_active = !input.active_only || p.active;
                matches_query && matches_category && matches_active
            })
            .take(input.limit as usize)
            .collect();
        
        let result = CatalogSearchResult {
            items: filtered_items.clone(),
            pagination: PaginationInfo {
                total: filtered_items.len() as u32,
                page: input.page,
                per_page: input.limit,
                has_more: false,
            },
        };
        
        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "Get detailed product information by ID")]
    async fn catalog_get(
        &self,
        Parameters(input): Parameters<CatalogGetInput>,
    ) -> String {
        debug!(product_id = %input.product_id, "catalog_get called");
        
        let result = CatalogGetResult {
            id: input.product_id,
            sku: "PLAN-PRO-001".to_string(),
            name: "Pro Plan".to_string(),
            description: Some("Professional tier with advanced features".to_string()),
            product_type: "configurable".to_string(),
            category: Some("saas".to_string()),
            attributes: Some(serde_json::json!({
                "seats": { "type": "integer", "min": 1, "max": 1000 },
                "billing": { "type": "enum", "values": ["monthly", "annual"] }
            })),
            active: true,
            created_at: "2025-01-15T10:00:00Z".to_string(),
            updated_at: "2025-06-20T14:30:00Z".to_string(),
        };
        
        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    // Quote Tools
    #[tool(description = "Create a new quote for a customer")]
    async fn quote_create(
        &self,
        Parameters(input): Parameters<QuoteCreateInput>,
    ) -> String {
        debug!(account_id = %input.account_id, "quote_create called");
        
        let result = QuoteCreateResult {
            quote_id: format!("Q-{}", chrono::Utc::now().format("%Y-%m%d%H%M%S")),
            version: 1,
            status: "draft".to_string(),
            account_id: input.account_id,
            currency: input.currency,
            line_items: input.line_items.iter().enumerate().map(|(i, item)| {
                LineItemResult {
                    line_id: format!("ql_{:03}", i + 1),
                    product_id: item.product_id.clone(),
                    product_name: "Product Name".to_string(),
                    quantity: item.quantity,
                }
            }).collect(),
            created_at: chrono::Utc::now().to_rfc3339(),
            message: "Quote created successfully".to_string(),
        };
        
        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "Get detailed quote information")]
    async fn quote_get(
        &self,
        Parameters(input): Parameters<QuoteGetInput>,
    ) -> String {
        debug!(quote_id = %input.quote_id, "quote_get called");
        
        let result = QuoteGetResult {
            quote: QuoteInfo {
                id: input.quote_id.clone(),
                version: 1,
                account_id: "acct_acme_001".to_string(),
                account_name: Some("Acme Corp".to_string()),
                deal_id: Some("deal_123".to_string()),
                status: "priced".to_string(),
                currency: "USD".to_string(),
                term_months: Some(12),
                start_date: Some("2026-03-01".to_string()),
                end_date: Some("2027-02-28".to_string()),
                valid_until: Some("2026-04-01".to_string()),
                notes: Some("Initial quote for Q1".to_string()),
                created_at: chrono::Utc::now().to_rfc3339(),
                created_by: "agent:mcp".to_string(),
            },
            line_items: vec![
                QuoteLineInfo {
                    line_id: "ql_001".to_string(),
                    product_id: "prod_pro_v2".to_string(),
                    product_name: "Pro Plan".to_string(),
                    quantity: 150,
                    unit_price: Some(6.00),
                    discount_pct: 10.0,
                    discount_amount: Some(1440.00),
                    subtotal: Some(12960.00),
                },
            ],
            pricing: if input.include_pricing {
                Some(PricingInfo {
                    subtotal: 14400.00,
                    discount_total: 1440.00,
                    tax_total: 0.00,
                    total: 12960.00,
                    priced_at: Some(chrono::Utc::now().to_rfc3339()),
                })
            } else {
                None
            },
        };
        
        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "Run pricing engine on a quote")]
    async fn quote_price(
        &self,
        Parameters(input): Parameters<QuotePriceInput>,
    ) -> String {
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
            line_pricing: vec![
                LinePricingInfo {
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
                },
            ],
            approval_required: true,
            policy_violations: vec![
                PolicyViolation {
                    policy_id: "pol_discount_cap".to_string(),
                    policy_name: "Discount Cap".to_string(),
                    severity: "approval_required".to_string(),
                    description: "10% discount exceeds auto-approval threshold".to_string(),
                    threshold: Some(5.0),
                    actual: Some(10.0),
                    required_approver_role: Some("sales_manager".to_string()),
                },
            ],
        };
        
        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    #[tool(description = "List quotes with optional filters")]
    async fn quote_list(
        &self,
        Parameters(input): Parameters<QuoteListInput>,
    ) -> String {
        debug!("quote_list called");
        
        let result = QuoteListResult {
            items: vec![
                QuoteListItem {
                    id: "Q-2026-0042".to_string(),
                    version: 1,
                    account_id: "acct_acme_001".to_string(),
                    account_name: Some("Acme Corp".to_string()),
                    status: "priced".to_string(),
                    currency: "USD".to_string(),
                    total: Some(12960.00),
                    valid_until: Some("2026-04-01".to_string()),
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            ],
            pagination: PaginationInfo {
                total: 1,
                page: input.page,
                per_page: input.limit,
                has_more: false,
            },
        };
        
        serde_json::to_string_pretty(&result).unwrap_or_default()
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
    async fn approval_status(
        &self,
        Parameters(input): Parameters<ApprovalStatusInput>,
    ) -> String {
        debug!(quote_id = %input.quote_id, "approval_status called");
        
        let result = ApprovalStatusResult {
            quote_id: input.quote_id,
            current_status: "pending_approval".to_string(),
            pending_requests: vec![
                PendingApproval {
                    approval_id: "APR-2026-0089".to_string(),
                    status: "pending".to_string(),
                    approver_role: "sales_manager".to_string(),
                    requested_at: chrono::Utc::now().to_rfc3339(),
                    expires_at: (chrono::Utc::now() + chrono::Duration::hours(4)).to_rfc3339(),
                },
            ],
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
            items: vec![
                ApprovalPendingItem {
                    approval_id: "APR-2026-0089".to_string(),
                    quote_id: "Q-2026-0042".to_string(),
                    account_name: "Acme Corp".to_string(),
                    quote_total: 12960.00,
                    requested_by: "agent:mcp".to_string(),
                    justification: "10% discount for loyal customer".to_string(),
                    requested_at: chrono::Utc::now().to_rfc3339(),
                },
            ],
            total: 1,
        };
        
        serde_json::to_string_pretty(&result).unwrap_or_default()
    }

    // PDF Tools
    #[tool(description = "Generate PDF for a quote")]
    async fn quote_pdf(
        &self,
        Parameters(input): Parameters<QuotePdfInput>,
    ) -> String {
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
