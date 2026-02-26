//! MCP Tools for Quotey
//!
//! This module organizes the MCP tools into categories:
//! - Catalog: Product search and retrieval
//! - Quote: Quote creation, pricing, and management
//! - Approval: Approval workflow management
//! - PDF: Document generation

// Tool categories for organization
/// Catalog tools category
pub struct CatalogTools;

/// Quote tools category
pub struct QuoteTools;

/// Approval tools category
pub struct ApprovalTools;

/// PDF tools category
pub struct PdfTools;

/// Tool category trait
pub trait ToolCategory {
    /// Category name
    fn category_name() -> &'static str where Self: Sized;
    /// List of tool names in this category
    fn tool_names() -> &'static [&'static str] where Self: Sized;
}

impl ToolCategory for CatalogTools {
    fn category_name() -> &'static str { "catalog" }
    fn tool_names() -> &'static [&'static str] { &["catalog_search", "catalog_get"] }
}

impl ToolCategory for QuoteTools {
    fn category_name() -> &'static str { "quote" }
    fn tool_names() -> &'static [&'static str] { 
        &["quote_create", "quote_get", "quote_price", "quote_list"] 
    }
}

impl ToolCategory for ApprovalTools {
    fn category_name() -> &'static str { "approval" }
    fn tool_names() -> &'static [&'static str] { 
        &["approval_request", "approval_status", "approval_pending"] 
    }
}

impl ToolCategory for PdfTools {
    fn category_name() -> &'static str { "pdf" }
    fn tool_names() -> &'static [&'static str] { &["quote_pdf"] }
}

/// All tool names
pub const ALL_TOOL_NAMES: &[&str] = &[
    "catalog_search", "catalog_get",
    "quote_create", "quote_get", "quote_price", "quote_list",
    "approval_request", "approval_status", "approval_pending",
    "quote_pdf",
];

/// Total number of tools
pub const TOTAL_TOOLS: usize = ALL_TOOL_NAMES.len();

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_counts() {
        assert_eq!(CatalogTools::tool_names().len(), 2);
        assert_eq!(QuoteTools::tool_names().len(), 4);
        assert_eq!(ApprovalTools::tool_names().len(), 3);
        assert_eq!(PdfTools::tool_names().len(), 1);
        assert_eq!(TOTAL_TOOLS, 10);
    }
}
