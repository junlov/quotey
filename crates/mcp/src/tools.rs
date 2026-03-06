//! MCP Tools for Quotey
//!
//! This module organizes the MCP tools into categories:
//! - Catalog: Product search and retrieval
//! - Quote: Quote creation, pricing, and management
//! - Approval: Approval workflow management
//! - PDF: Document generation
//! - Comment: Quote comments
//! - Lock: Quote locking
//! - Settings: Organization settings
//! - SalesRep: Sales representative management
//! - Negotiation: Negotiation autopilot
//! - Anomaly: Anomaly override management
//! - Cost: AI usage cost tracking

// Tool categories for organization
/// Catalog tools category
pub struct CatalogTools;

/// Quote tools category
pub struct QuoteTools;

/// Approval tools category
pub struct ApprovalTools;

/// PDF tools category
pub struct PdfTools;

/// Comment tools category
pub struct CommentTools;

/// Lock tools category
pub struct LockTools;

/// Settings tools category
pub struct SettingsTools;

/// Cost tools category
pub struct CostTools;

/// Tool category trait
pub trait ToolCategory {
    /// Category name
    fn category_name() -> &'static str
    where
        Self: Sized;
    /// List of tool names in this category
    fn tool_names() -> &'static [&'static str]
    where
        Self: Sized;
}

impl ToolCategory for CatalogTools {
    fn category_name() -> &'static str {
        "catalog"
    }
    fn tool_names() -> &'static [&'static str] {
        &["catalog_search", "catalog_get"]
    }
}

impl ToolCategory for QuoteTools {
    fn category_name() -> &'static str {
        "quote"
    }
    fn tool_names() -> &'static [&'static str] {
        &["quote_create", "quote_get", "quote_price", "quote_list"]
    }
}

impl ToolCategory for ApprovalTools {
    fn category_name() -> &'static str {
        "approval"
    }
    fn tool_names() -> &'static [&'static str] {
        &["approval_request", "approval_status", "approval_pending"]
    }
}

impl ToolCategory for PdfTools {
    fn category_name() -> &'static str {
        "pdf"
    }
    fn tool_names() -> &'static [&'static str] {
        &["quote_pdf"]
    }
}

impl ToolCategory for CommentTools {
    fn category_name() -> &'static str {
        "comment"
    }
    fn tool_names() -> &'static [&'static str] {
        &["comment_add", "comment_list"]
    }
}

impl ToolCategory for LockTools {
    fn category_name() -> &'static str {
        "lock"
    }
    fn tool_names() -> &'static [&'static str] {
        &[
            "quote_lock",
            "quote_unlock",
            "quote_force_unlock",
            "quote_lock_status",
        ]
    }
}

impl ToolCategory for SettingsTools {
    fn category_name() -> &'static str {
        "settings"
    }
    fn tool_names() -> &'static [&'static str] {
        &["settings_get", "settings_set", "settings_list"]
    }
}

impl ToolCategory for CostTools {
    fn category_name() -> &'static str {
        "cost"
    }
    fn tool_names() -> &'static [&'static str] {
        &["cost_summary", "cost_list"]
    }
}

/// All tool names (does not include negotiation, sales_rep, anomaly tools registered via #[tool_router])
pub const ALL_TOOL_NAMES: &[&str] = &[
    // Catalog
    "catalog_search",
    "catalog_get",
    // Quote
    "quote_create",
    "quote_get",
    "quote_price",
    "quote_list",
    // Approval
    "approval_request",
    "approval_status",
    "approval_pending",
    // PDF
    "quote_pdf",
    // Comment
    "comment_add",
    "comment_list",
    // Lock
    "quote_lock",
    "quote_unlock",
    "quote_force_unlock",
    "quote_lock_status",
    // Settings
    "settings_get",
    "settings_set",
    "settings_list",
    // Cost
    "cost_summary",
    "cost_list",
];

/// Total number of tools in the registry
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
        assert_eq!(CommentTools::tool_names().len(), 2);
        assert_eq!(LockTools::tool_names().len(), 4);
        assert_eq!(SettingsTools::tool_names().len(), 3);
        assert_eq!(CostTools::tool_names().len(), 2);
        assert_eq!(TOTAL_TOOLS, 21);
    }
}
