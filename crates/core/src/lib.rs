pub mod approvals;
pub mod audit;
pub mod config;
pub mod cpq;
pub mod domain;
pub mod errors;
pub mod flows;

pub use approvals::{
    ApprovalValidationFailure, ApprovalValidationInput, ApprovalValidationResult,
    ApprovalValidator, ApproverAuthority,
};
pub use domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
pub use domain::product::{Product, ProductId};
pub use domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
pub use errors::{ApplicationError, DomainError, InterfaceError};
