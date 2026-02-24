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
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quote {
    pub id: QuoteId,
    pub status: QuoteStatus,
    pub lines: Vec<QuoteLine>,
    pub created_at: DateTime<Utc>,
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
        Quote {
            id: QuoteId("Q-1".to_string()),
            status,
            lines: vec![QuoteLine {
                product_id: ProductId("plan-pro".to_string()),
                quantity: 1,
                unit_price: Decimal::new(1000, 2),
            }],
            created_at: Utc::now(),
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
