use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::quote::{Quote, QuoteId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingTraceStep {
    pub stage: String,
    pub detail: String,
    pub amount: Decimal,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingTrace {
    pub quote_id: QuoteId,
    pub currency: String,
    pub steps: Vec<PricingTraceStep>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PricingResult {
    pub subtotal: Decimal,
    pub discount_total: Decimal,
    pub tax_total: Decimal,
    pub total: Decimal,
    pub approval_required: bool,
    pub trace: PricingTrace,
}

pub trait PricingEngine: Send + Sync {
    fn price(&self, quote: &Quote, currency: &str) -> PricingResult;
}

#[derive(Default)]
pub struct DeterministicPricingEngine;

impl PricingEngine for DeterministicPricingEngine {
    fn price(&self, quote: &Quote, currency: &str) -> PricingResult {
        price_quote_with_trace(quote, currency)
    }
}

pub fn price_quote(quote: &Quote) -> Decimal {
    quote.lines.iter().map(|line| line.unit_price * Decimal::from(line.quantity)).sum()
}

pub fn price_quote_with_trace(quote: &Quote, currency: &str) -> PricingResult {
    let subtotal = price_quote(quote);
    let discount_total = Decimal::ZERO;
    let tax_total = Decimal::ZERO;
    let total = subtotal - discount_total + tax_total;

    PricingResult {
        subtotal,
        discount_total,
        tax_total,
        total,
        approval_required: false,
        trace: PricingTrace {
            quote_id: quote.id.clone(),
            currency: currency.to_string(),
            steps: vec![PricingTraceStep {
                stage: "subtotal".to_string(),
                detail: "sum(unit_price * quantity)".to_string(),
                amount: subtotal,
            }],
        },
    }
}
