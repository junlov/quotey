use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::product::ProductId;
use crate::errors::DomainError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuoteId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QuoteLineId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuoteStatus {
    Draft,
    Validated,
    Priced,
    Approval,
    Approved,
    Rejected,
    Finalized,
    Sent,
    Expired,
    Cancelled,
    Revised,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuoteLine {
    pub product_id: ProductId,
    pub quantity: u32,
    pub unit_price: Decimal,
    #[serde(default)]
    pub discount_pct: f64,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub id: QuoteId,
    pub version: u32,
    pub status: QuoteStatus,
    pub account_id: Option<String>,
    pub deal_id: Option<String>,
    pub currency: String,
    pub term_months: Option<u32>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub valid_until: Option<String>,
    pub notes: Option<String>,
    pub created_by: String,
    pub lines: Vec<QuoteLine>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Quote {
    pub fn can_transition_to(&self, next: QuoteStatus) -> bool {
        matches!(
            (&self.status, next),
            (QuoteStatus::Draft, QuoteStatus::Validated)
                | (QuoteStatus::Validated, QuoteStatus::Priced)
                | (QuoteStatus::Priced, QuoteStatus::Approval)
                | (QuoteStatus::Priced, QuoteStatus::Finalized)
                | (QuoteStatus::Approval, QuoteStatus::Approved)
                | (QuoteStatus::Approval, QuoteStatus::Rejected)
                | (QuoteStatus::Approved, QuoteStatus::Finalized)
                | (QuoteStatus::Finalized, QuoteStatus::Sent)
                | (QuoteStatus::Revised, QuoteStatus::Validated)
                | (_, QuoteStatus::Cancelled)
                | (_, QuoteStatus::Expired)
                | (_, QuoteStatus::Revised)
        )
    }

    pub fn transition_to(&mut self, next: QuoteStatus) -> Result<(), DomainError> {
        if self.can_transition_to(next.clone()) {
            self.status = next;
            return Ok(());
        }

        Err(DomainError::InvalidQuoteTransition { from: self.status.clone(), to: next })
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use crate::domain::product::ProductId;

    use super::{Quote, QuoteId, QuoteLine, QuoteStatus};

    fn quote(status: QuoteStatus) -> Quote {
        let now = Utc::now();
        Quote {
            id: QuoteId("Q-1".to_string()),
            version: 1,
            status,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "test".to_string(),
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_string()),
                quantity: 1,
                unit_price: Decimal::new(1000, 2),
                discount_pct: 0.0,
                notes: None,
            }],
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn allows_valid_lifecycle_transition() {
        let mut quote = quote(QuoteStatus::Draft);
        quote.transition_to(QuoteStatus::Validated).expect("draft->validated");
        assert_eq!(quote.status, QuoteStatus::Validated);
    }

    #[test]
    fn blocks_invalid_lifecycle_transition() {
        let mut quote = quote(QuoteStatus::Draft);
        let error = quote.transition_to(QuoteStatus::Sent).expect_err("draft->sent should fail");
        assert!(matches!(error, crate::errors::DomainError::InvalidQuoteTransition { .. }));
    }

    #[test]
    fn revised_quotes_can_reenter_validation() {
        let mut quote = quote(QuoteStatus::Rejected);
        quote.transition_to(QuoteStatus::Revised).expect("rejected -> revised");
        quote.transition_to(QuoteStatus::Validated).expect("revised -> validated");

        assert_eq!(quote.status, QuoteStatus::Validated);
    }
}
