use std::cmp::Ordering;
use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::domain::product::ProductId;
use crate::domain::quote::{Quote, QuoteId, QuoteLine};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationType {
    Insert { line: QuoteLine },
    Update { product_id: ProductId, quantity: Option<u32>, unit_price: Option<Decimal> },
    Delete { product_id: ProductId },
}

impl OperationType {
    fn target_key(&self) -> String {
        match self {
            Self::Insert { line } => line.product_id.0.clone(),
            Self::Update { product_id, .. } | Self::Delete { product_id } => product_id.0.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationAuthority {
    pub role: String,
    pub rank: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuoteOperation {
    pub operation_id: String,
    pub quote_id: QuoteId,
    pub actor_user_id: String,
    pub authority: OperationAuthority,
    pub timestamp_ms: i64,
    pub operation: OperationType,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationStatus {
    Applied,
    Overridden,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OperationHistoryEntry {
    pub operation_id: String,
    pub target_key: String,
    pub actor_user_id: String,
    pub status: OperationStatus,
    pub reason: String,
    pub superseded_by: Option<String>,
    pub operation: OperationType,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct TransformResult {
    pub applied_operation_ids: Vec<String>,
    pub overridden_operation_ids: Vec<String>,
    pub rejected_operation_ids: Vec<String>,
    pub history_entries: Vec<OperationHistoryEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct OperationalTransform {
    history: Vec<OperationHistoryEntry>,
}

impl OperationalTransform {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn history(&self) -> &[OperationHistoryEntry] {
        &self.history
    }

    pub fn transform(
        &mut self,
        quote: &mut Quote,
        operations: Vec<QuoteOperation>,
    ) -> TransformResult {
        let mut result = TransformResult::default();
        let mut grouped_operations: HashMap<String, Vec<QuoteOperation>> = HashMap::new();
        let mut working_quote = quote.clone();

        for operation in operations {
            if operation.quote_id != quote.id {
                append_history(
                    &mut result,
                    &operation,
                    OperationStatus::Rejected,
                    "operation quote_id does not match target quote".to_string(),
                    None,
                );
                continue;
            }

            grouped_operations.entry(operation.operation.target_key()).or_default().push(operation);
        }

        let mut target_keys = grouped_operations.keys().cloned().collect::<Vec<_>>();
        target_keys.sort();

        for target_key in target_keys {
            let mut candidates = grouped_operations.remove(&target_key).unwrap_or_default();
            candidates.sort_by(operation_precedence_compare);

            let mut winning_operation_id: Option<String> = None;
            for candidate in candidates.into_iter().rev() {
                if winning_operation_id.is_some() {
                    append_history(
                        &mut result,
                        &candidate,
                        OperationStatus::Overridden,
                        "operation superseded by higher-authority valid edit".to_string(),
                        winning_operation_id.clone(),
                    );
                    continue;
                }

                if let Err(reason) = validate_operation(&working_quote, &candidate) {
                    append_history(
                        &mut result,
                        &candidate,
                        OperationStatus::Rejected,
                        reason,
                        None,
                    );
                    continue;
                }

                match apply_operation(&mut working_quote, &candidate.operation) {
                    Ok(()) => {
                        winning_operation_id = Some(candidate.operation_id.clone());
                        append_history(
                            &mut result,
                            &candidate,
                            OperationStatus::Applied,
                            "operation applied".to_string(),
                            None,
                        );
                    }
                    Err(reason) => {
                        append_history(
                            &mut result,
                            &candidate,
                            OperationStatus::Rejected,
                            reason,
                            None,
                        );
                    }
                }
            }
        }

        quote.lines = working_quote.lines;
        self.history.extend(result.history_entries.clone());
        result
    }
}

fn append_history(
    result: &mut TransformResult,
    operation: &QuoteOperation,
    status: OperationStatus,
    reason: String,
    superseded_by: Option<String>,
) {
    match status {
        OperationStatus::Applied => {
            result.applied_operation_ids.push(operation.operation_id.clone())
        }
        OperationStatus::Overridden => {
            result.overridden_operation_ids.push(operation.operation_id.clone())
        }
        OperationStatus::Rejected => {
            result.rejected_operation_ids.push(operation.operation_id.clone())
        }
    }

    result.history_entries.push(OperationHistoryEntry {
        operation_id: operation.operation_id.clone(),
        target_key: operation.operation.target_key(),
        actor_user_id: operation.actor_user_id.clone(),
        status,
        reason,
        superseded_by,
        operation: operation.operation.clone(),
    });
}

fn operation_precedence_compare(left: &QuoteOperation, right: &QuoteOperation) -> Ordering {
    left.authority
        .rank
        .cmp(&right.authority.rank)
        .then(left.timestamp_ms.cmp(&right.timestamp_ms))
        .then(left.operation_id.cmp(&right.operation_id))
}

fn validate_operation(quote: &Quote, operation: &QuoteOperation) -> Result<(), String> {
    match &operation.operation {
        OperationType::Insert { line } => {
            if line.quantity == 0 {
                return Err("insert quantity must be greater than zero".to_string());
            }
            Ok(())
        }
        OperationType::Update { product_id, quantity, unit_price } => {
            if quantity.is_none() && unit_price.is_none() {
                return Err("update operation must include at least one changed field".to_string());
            }
            if quantity.is_some_and(|value| value == 0) {
                return Err("update quantity must be greater than zero".to_string());
            }
            if !quote.lines.iter().any(|line| line.product_id == *product_id) {
                return Err(format!("cannot update missing product line {}", product_id.0));
            }
            Ok(())
        }
        OperationType::Delete { product_id } => {
            if !quote.lines.iter().any(|line| line.product_id == *product_id) {
                return Err(format!("cannot delete missing product line {}", product_id.0));
            }
            Ok(())
        }
    }
}

fn apply_operation(quote: &mut Quote, operation: &OperationType) -> Result<(), String> {
    match operation {
        OperationType::Insert { line } => {
            if let Some(position) =
                quote.lines.iter().position(|existing| existing.product_id == line.product_id)
            {
                quote.lines[position] = line.clone();
            } else {
                quote.lines.push(line.clone());
            }
            Ok(())
        }
        OperationType::Update { product_id, quantity, unit_price } => {
            let Some(line) = quote.lines.iter_mut().find(|line| line.product_id == *product_id)
            else {
                return Err(format!("cannot update missing product line {}", product_id.0));
            };

            if let Some(updated_quantity) = quantity {
                line.quantity = *updated_quantity;
            }
            if let Some(updated_price) = unit_price {
                line.unit_price = *updated_price;
            }
            Ok(())
        }
        OperationType::Delete { product_id } => {
            let Some(position) = quote.lines.iter().position(|line| line.product_id == *product_id)
            else {
                return Err(format!("cannot delete missing product line {}", product_id.0));
            };
            quote.lines.remove(position);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use super::{
        OperationAuthority, OperationStatus, OperationType, OperationalTransform, QuoteOperation,
    };
    use crate::domain::product::ProductId;
    use crate::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};

    #[test]
    fn higher_authority_update_wins_conflict() {
        let mut engine = OperationalTransform::new();
        let mut quote = quote_with_lines(vec![line("enterprise", 10, 100_000)]);
        let operations = vec![
            update_operation("op-ae", 1, 1000, "enterprise", Some(20), None),
            update_operation("op-vp", 3, 900, "enterprise", Some(25), None),
        ];

        let result = engine.transform(&mut quote, operations);
        assert_eq!(quote.lines[0].quantity, 25);
        assert_eq!(result.applied_operation_ids, vec!["op-vp".to_string()]);
        assert_eq!(result.overridden_operation_ids, vec!["op-ae".to_string()]);
    }

    #[test]
    fn same_authority_uses_latest_timestamp() {
        let mut engine = OperationalTransform::new();
        let mut quote = quote_with_lines(vec![line("premium", 5, 50_000)]);
        let operations = vec![
            update_operation("op-early", 2, 1000, "premium", Some(8), None),
            update_operation("op-late", 2, 2000, "premium", Some(12), None),
        ];

        let result = engine.transform(&mut quote, operations);
        assert_eq!(quote.lines[0].quantity, 12);
        assert_eq!(result.applied_operation_ids, vec!["op-late".to_string()]);
        assert_eq!(result.overridden_operation_ids, vec!["op-early".to_string()]);
    }

    #[test]
    fn falls_back_to_next_valid_operation_when_highest_is_invalid() {
        let mut engine = OperationalTransform::new();
        let mut quote = quote_with_lines(vec![line("starter", 3, 10_000)]);
        let operations = vec![
            update_operation("op-invalid-vp", 3, 2000, "starter", Some(0), None),
            update_operation("op-valid-ae", 1, 1000, "starter", Some(7), None),
        ];

        let result = engine.transform(&mut quote, operations);
        assert_eq!(quote.lines[0].quantity, 7);
        assert_eq!(result.applied_operation_ids, vec!["op-valid-ae".to_string()]);
        assert_eq!(result.rejected_operation_ids, vec!["op-invalid-vp".to_string()]);
    }

    #[test]
    fn applies_insert_update_and_delete_atomically_per_operation() {
        let mut engine = OperationalTransform::new();
        let mut quote =
            quote_with_lines(vec![line("starter", 2, 10_000), line("support", 1, 2_500)]);
        let operations = vec![
            insert_operation("op-insert", 1, 1000, line("analytics", 1, 5_000)),
            update_operation("op-update", 1, 1100, "starter", Some(4), None),
            delete_operation("op-delete", 1, 1200, "support"),
        ];

        let result = engine.transform(&mut quote, operations);
        assert_eq!(result.applied_operation_ids.len(), 3);
        assert!(quote
            .lines
            .iter()
            .any(|line| line.product_id == ProductId("analytics".to_string())));
        assert!(quote.lines.iter().any(|line| line.product_id == ProductId("starter".to_string())));
        assert!(!quote
            .lines
            .iter()
            .any(|line| line.product_id == ProductId("support".to_string())));
        assert_eq!(
            quote
                .lines
                .iter()
                .find(|line| line.product_id == ProductId("starter".to_string()))
                .map(|line| line.quantity),
            Some(4)
        );
    }

    #[test]
    fn maintains_history_across_batches() {
        let mut engine = OperationalTransform::new();
        let mut quote = quote_with_lines(vec![line("starter", 1, 10_000)]);

        let first_batch = vec![update_operation("op-1", 1, 1000, "starter", Some(2), None)];
        let second_batch = vec![update_operation("op-2", 1, 2000, "starter", Some(3), None)];

        let first_result = engine.transform(&mut quote, first_batch);
        let second_result = engine.transform(&mut quote, second_batch);

        assert_eq!(first_result.history_entries.len(), 1);
        assert_eq!(second_result.history_entries.len(), 1);
        assert_eq!(engine.history().len(), 2);
        assert_eq!(engine.history()[0].status, OperationStatus::Applied);
        assert_eq!(engine.history()[1].status, OperationStatus::Applied);
    }

    fn quote_with_lines(lines: Vec<QuoteLine>) -> Quote {
        let now = Utc::now();
        Quote {
            id: QuoteId("Q-collab-1".to_string()),
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
            created_by: "test".to_string(),
            lines,
            created_at: now,
            updated_at: now,
        }
    }

    fn line(product_id: &str, quantity: u32, unit_price_cents: i64) -> QuoteLine {
        let unit_price = cents_to_decimal(unit_price_cents);
        QuoteLine {
            product_id: ProductId(product_id.to_string()),
            quantity,
            unit_price,
            discount_pct: 0.0,
            notes: None,
        }
    }

    fn cents_to_decimal(cents: i64) -> Decimal {
        let sign = if cents < 0 { "-" } else { "" };
        let abs = cents.abs();
        let as_string = format!("{sign}{}.{:02}", abs / 100, abs % 100);
        as_string.parse().expect("valid decimal")
    }

    fn insert_operation(
        operation_id: &str,
        authority_rank: u8,
        timestamp_ms: i64,
        line: QuoteLine,
    ) -> QuoteOperation {
        QuoteOperation {
            operation_id: operation_id.to_string(),
            quote_id: QuoteId("Q-collab-1".to_string()),
            actor_user_id: "u-1".to_string(),
            authority: OperationAuthority { role: "sales_rep".to_string(), rank: authority_rank },
            timestamp_ms,
            operation: OperationType::Insert { line },
        }
    }

    fn update_operation(
        operation_id: &str,
        authority_rank: u8,
        timestamp_ms: i64,
        product_id: &str,
        quantity: Option<u32>,
        unit_price: Option<Decimal>,
    ) -> QuoteOperation {
        QuoteOperation {
            operation_id: operation_id.to_string(),
            quote_id: QuoteId("Q-collab-1".to_string()),
            actor_user_id: "u-1".to_string(),
            authority: OperationAuthority { role: "sales_rep".to_string(), rank: authority_rank },
            timestamp_ms,
            operation: OperationType::Update {
                product_id: ProductId(product_id.to_string()),
                quantity,
                unit_price,
            },
        }
    }

    fn delete_operation(
        operation_id: &str,
        authority_rank: u8,
        timestamp_ms: i64,
        product_id: &str,
    ) -> QuoteOperation {
        QuoteOperation {
            operation_id: operation_id.to_string(),
            quote_id: QuoteId("Q-collab-1".to_string()),
            actor_user_id: "u-1".to_string(),
            authority: OperationAuthority { role: "sales_rep".to_string(), rank: authority_rank },
            timestamp_ms,
            operation: OperationType::Delete { product_id: ProductId(product_id.to_string()) },
        }
    }
}
