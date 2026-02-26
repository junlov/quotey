//! MCP Server Implementation
//!
//! Implements the Model Context Protocol server for Quotey.

use rmcp::{
    handler::server::ServerHandler,
    model::*,
    schemars::{self, JsonSchema},
    serde::{Deserialize, Serialize},
    tool, ServerHandlerExt,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::McpError;

/// Main MCP server for Quotey
#[derive(Debug, Clone)]
pub struct QuoteyMcpServer {
    // Database pool for queries
    // db_pool: sqlx::SqlitePool,
    
    // API key for authentication (if enabled)
    api_key: Option<String>,
}

impl QuoteyMcpServer {
    /// Create a new MCP server instance
    pub async fn new() -> anyhow::Result<Self> {
        info!("Initializing Quotey MCP Server");
        
        // TODO: Load database pool from configuration
        // TODO: Load API key from environment or config
        
        Ok(Self {
            // db_pool,
            api_key: None,
        })
    }

    /// Create server with API key authentication
    pub async fn with_api_key(api_key: String) -> anyhow::Result<Self> {
        let mut server = Self::new().await?;
        server.api_key = Some(api_key);
        Ok(server)
    }

    /// Run the server with stdio transport
    pub async fn run_stdio(self) -> anyhow::Result<()> {
        use tokio::io::{stdin, stdout};
        
        info!("Starting MCP server with stdio transport");
        
        let service = self.serve((stdin(), stdout())).await?;
        
        // Wait for shutdown
        let _quit = service.waiting().await?;
        
        info!("MCP server shutdown complete");
        Ok(())
    }

    /// Validate API key if authentication is enabled
    fn validate_auth(&self, _request: &Request) -> Result<(), McpError> {
        if let Some(ref _expected_key) = self.api_key {
            // TODO: Extract and validate API key from request headers
            // For now, allow all requests
        }
        Ok(())
    }
}

// Implement ServerHandler trait for MCP protocol
#[tool(tool_box)]
impl ServerHandler for QuoteyMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability::default()),
                resources: Some(ResourcesCapability::default()),
                prompts: Some(PromptsCapability::default()),
                logging: Some(LoggingCapability::default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: "quotey-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some(
                "Quotey MCP Server - CPQ automation for AI agents. \
                 Create quotes, check pricing, request approvals, and generate PDFs."
                    .to_string(),
            ),
        }
    }
}

// ============================================================================
// Catalog Tools
// ============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CatalogSearchInput {
    #[schemars(description = "Search query for product name, SKU, or description")]
    pub query: String,
    
    #[schemars(description = "Filter by product category")]
    #[serde(default)]
    pub category: Option<String>,
    
    #[schemars(description = "Only return active products", default = "default_true")]
    #[serde(default = "default_true")]
    pub active_only: bool,
    
    #[schemars(description = "Maximum results to return", default = "default_20")]
    #[serde(default = "default_20")]
    pub limit: u32,
    
    #[schemars(description = "Page number for pagination", default = "default_1")]
    #[serde(default = "default_1")]
    pub page: u32,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct CatalogSearchResult {
    pub items: Vec<ProductSummary>,
    pub pagination: PaginationInfo,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct ProductSummary {
    pub id: String,
    pub sku: String,
    pub name: String,
    pub description: String,
    pub product_type: String,
    pub category: String,
    pub active: bool,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct PaginationInfo {
    pub total: u32,
    pub page: u32,
    pub per_page: u32,
    pub has_more: bool,
}

#[tool(tool_box)]
impl QuoteyMcpServer {
    /// Search the product catalog
    #[tool(name = "catalog_search", description = "Search products by name, SKU, or description")]
    async fn catalog_search(
        &self,
        #[tool(aggr)] input: CatalogSearchInput,
    ) -> Result<CallToolResult, rmcp::Error> {
        debug!(query = %input.query, "catalog_search called");
        
        // TODO: Implement actual database query
        // For now, return mock data
        let result = CatalogSearchResult {
            items: vec![
                ProductSummary {
                    id: "prod_pro_v2".to_string(),
                    sku: "PLAN-PRO-001".to_string(),
                    name: "Pro Plan".to_string(),
                    description: "Professional tier".to_string(),
                    product_type: "configurable".to_string(),
                    category: "saas".to_string(),
                    active: true,
                },
            ],
            pagination: PaginationInfo {
                total: 1,
                page: input.page,
                per_page: input.limit,
                has_more: false,
            },
        };
        
        let content = serde_json::to_string_pretty(&result)
            .map_err(|e| rmcp::Error::internal_error(e.to_string(), None))?;
        
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: false,
            ..Default::default()
        })
    }
}

// ============================================================================
// Quote Tools
// ============================================================================

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QuoteCreateInput {
    #[schemars(description = "Account/Customer ID")]
    pub account_id: String,
    
    #[schemars(description = "Line items for the quote")]
    pub line_items: Vec<LineItemInput>,
    
    #[schemars(description = "Internal notes")]
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct LineItemInput {
    #[schemars(description = "Product ID")]
    pub product_id: String,
    
    #[schemars(description = "Quantity")]
    pub quantity: u32,
    
    #[schemars(description = "Requested discount percentage")]
    #[serde(default)]
    pub discount_pct: f64,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct QuoteCreateResult {
    pub quote_id: String,
    pub status: String,
    pub message: String,
}

#[tool(tool_box)]
impl QuoteyMcpServer {
    /// Create a new quote
    #[tool(name = "quote_create", description = "Create a new quote for a customer")]
    async fn quote_create(
        &self,
        #[tool(aggr)] input: QuoteCreateInput,
    ) -> Result<CallToolResult, rmcp::Error> {
        debug!(account_id = %input.account_id, "quote_create called");
        
        // TODO: Implement actual quote creation
        let result = QuoteCreateResult {
            quote_id: "Q-2026-0042".to_string(),
            status: "draft".to_string(),
            message: "Quote created successfully".to_string(),
        };
        
        let content = serde_json::to_string_pretty(&result)
            .map_err(|e| rmcp::Error::internal_error(e.to_string(), None))?;
        
        Ok(CallToolResult {
            content: vec![Content::text(content)],
            is_error: false,
            ..Default::default()
        })
    }
}

// ============================================================================
// Helper functions
// ============================================================================

fn default_true() -> bool {
    true
}

fn default_20() -> u32 {
    20
}

fn default_1() -> u32 {
    1
}
