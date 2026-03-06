use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteId;
use crate::errors::DomainError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApprovalId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Rejected,
    Escalated,
    RevisionRequested,
}

impl ApprovalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Escalated => "escalated",
            Self::RevisionRequested => "revision_requested",
        }
    }
}

impl FromStr for ApprovalStatus {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "escalated" => Ok(Self::Escalated),
            "revision_requested" => Ok(Self::RevisionRequested),
            _ => Err(DomainError::InvalidEnumValue {
                enum_name: "ApprovalStatus".to_string(),
                value: value.to_string(),
            }),
        }
    }
}

/// The type of approval being requested
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalType {
    DiscountOverride,
    PriceException,
    NonStandardTerms,
    CustomBundle,
    CompetitorMatch,
    ExecutiveEscalation,
}

impl ApprovalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DiscountOverride => "discount_override",
            Self::PriceException => "price_exception",
            Self::NonStandardTerms => "non_standard_terms",
            Self::CustomBundle => "custom_bundle",
            Self::CompetitorMatch => "competitor_match",
            Self::ExecutiveEscalation => "executive_escalation",
        }
    }
}

impl FromStr for ApprovalType {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "discount_override" => Ok(Self::DiscountOverride),
            "price_exception" => Ok(Self::PriceException),
            "non_standard_terms" => Ok(Self::NonStandardTerms),
            "custom_bundle" => Ok(Self::CustomBundle),
            "competitor_match" => Ok(Self::CompetitorMatch),
            "executive_escalation" => Ok(Self::ExecutiveEscalation),
            _ => Err(DomainError::InvalidEnumValue {
                enum_name: "ApprovalType".to_string(),
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: ApprovalId,
    pub quote_id: QuoteId,
    pub approver_role: String,
    pub approval_type: ApprovalType,
    pub reason: String,
    pub justification: String,
    pub payload_json: String,
    pub status: ApprovalStatus,
    pub decision_note: Option<String>,
    pub requested_by: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
