use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteId;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Escalated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: ApprovalId,
    pub quote_id: QuoteId,
    pub approver_role: String,
    pub reason: String,
    pub justification: String,
    pub status: ApprovalStatus,
    pub requested_by: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
