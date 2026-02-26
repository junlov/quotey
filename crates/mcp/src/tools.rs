//! MCP Tools for Quotey
//!
//! This module organizes the MCP tools into categories:
//! - Catalog: Product search and retrieval
//! - Quote: Quote creation, pricing, and management
//! - Approval: Approval workflow management
//! - PDF: Document generation

// Re-export types from server for convenience
pub use crate::server::{
    CatalogSearchInput, CatalogSearchResult, LineItemInput,
    PaginationInfo, ProductSummary, QuoteCreateInput, QuoteCreateResult,
};

/// Tool category trait for organization
pub trait ToolCategory {
    /// Category name
    fn category_name() -> &'static str;
    
    /// List of tool names in this category
    fn tool_names() -> &'static [&'static str];
}

/// Catalog tools category
pub struct CatalogTools;

impl ToolCategory for CatalogTools {
    fn category_name() -> &'static str {
        "catalog"
    }
    
    fn tool_names() -> &'static [&'static str] {
        &["catalog_search", "catalog_get"]
    }
}

/// Quote tools category
pub struct QuoteTools;

impl ToolCategory for QuoteTools {
    fn category_name() -> &'static str {
        "quote"
    }
    
    fn tool_names() -> &'static [&'static str] {
        &["quote_create", "quote_get", "quote_price", "quote_list"]
    }
}

/// Approval tools category
pub struct ApprovalTools;

impl ToolCategory for ApprovalTools {
    fn category_name() -> &'static str {
        "approval"
    }
    
    fn tool_names() -> &'static [&'static str] {
        &["approval_request", "approval_status", "approval_pending"]
    }
}

/// PDF tools category
pub struct PdfTools;

impl ToolCategory for PdfTools {
    fn category_name() -> &'static str {
        "pdf"
    }
    
    fn tool_names() -> &'static [&'static str] {
        &["quote_pdf"]
    }
}

/// All tool categories
pub const ALL_CATEGORIES: &[&dyn ToolCategory] = &[
    &CatalogTools,
    &QuoteTools,
    &ApprovalTools,
    &PdfTools,
];

/// Total number of tools
pub const TOTAL_TOOLS: usize = 10;

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tool_categories() {
        assert_eq!(CatalogTools::tool_names().len(), 2);
        assert_eq!(QuoteTools::tool_names().len(), 4);
        assert_eq!(ApprovalTools::tool_names().len(), 3);
        assert_eq!(PdfTools::tool_names().len(), 1);
        
        let total = CatalogTools::tool_names().len()
            + QuoteTools::tool_names().len()
            + ApprovalTools::tool_names().len()
            + PdfTools::tool_names().len();
        
        assert_eq!(total, TOTAL_TOOLS);
    }
}
