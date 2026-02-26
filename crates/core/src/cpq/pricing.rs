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
    let mut steps = Vec::new();
    let mut subtotal = Decimal::ZERO;
    let discount_total = Decimal::ZERO;

    for line in &quote.lines {
        let line_total = line.unit_price * Decimal::from(line.quantity);
        subtotal += line_total;
        steps.push(PricingTraceStep {
            stage: "line_item".to_string(),
            detail: format!("{} × {} @ {}", line.quantity, line.unit_price, line.product_id.0),
            amount: line_total,
        });
    }

    steps.push(PricingTraceStep {
        stage: "subtotal".to_string(),
        detail: "sum of line totals".to_string(),
        amount: subtotal,
    });

    steps.push(PricingTraceStep {
        stage: "discounts".to_string(),
        detail: "No explicit discount rules in baseline engine".to_string(),
        amount: discount_total,
    });

    let tax_total = Decimal::ZERO;
    steps.push(PricingTraceStep {
        stage: "tax".to_string(),
        detail: "Tax disabled in baseline engine".to_string(),
        amount: tax_total,
    });

    let total = subtotal - discount_total + tax_total;
    steps.push(PricingTraceStep {
        stage: "total".to_string(),
        detail: "subtotal - discounts + tax".to_string(),
        amount: total,
    });

    PricingResult {
        subtotal,
        discount_total,
        tax_total,
        total,
        approval_required: false,
        trace: PricingTrace { quote_id: quote.id.clone(), currency: currency.to_string(), steps },
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use super::price_quote_with_trace;
    use crate::domain::{
        product::ProductId,
        quote::{Quote, QuoteId, QuoteLine, QuoteStatus},
    };

    #[test]
    fn pricing_trace_includes_stepwise_calculation() {
        let now = Utc::now();
        let quote = Quote {
            id: QuoteId("Q-2026-4000".to_owned()),
            version: 1,
            status: QuoteStatus::Draft,
            account_id: None,
            deal_id: None,
            currency: "USD".to_string(),
            term_months: None,
            start_date: None,
            end_date: None,
            valid_until: None,
            notes: None,
            created_by: "system".to_string(),
            lines: vec![
                QuoteLine {
                    product_id: ProductId("plan-pro".to_owned()),
                    quantity: 2,
                    unit_price: Decimal::new(1000, 2),
                    discount_pct: 0.0,
                    notes: None,
                },
                QuoteLine {
                    product_id: ProductId("addon".to_owned()),
                    quantity: 1,
                    unit_price: Decimal::new(2500, 2),
                    discount_pct: 0.0,
                    notes: None,
                },
            ],
            created_at: now,
            updated_at: now,
        };

        let result = price_quote_with_trace(&quote, "USD");
        assert_eq!(result.subtotal, Decimal::new(4500, 2));
        assert_eq!(result.total, Decimal::new(4500, 2));
        assert_eq!(result.trace.steps.len(), 6);
        assert_eq!(result.trace.steps[0].stage, "line_item");
        assert_eq!(result.trace.steps.last().expect("trace has total").stage, "total");
    }
}
