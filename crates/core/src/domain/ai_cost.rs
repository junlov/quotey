use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single AI cost event recording token usage and estimated cost for one
/// MCP tool invocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AiCostEvent {
    pub id: String,
    pub quote_id: Option<String>,
    pub tool_name: String,
    pub model_name: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub total_tokens: i64,
    pub estimated_cost_cents: f64,
    pub actor_id: Option<String>,
    pub metadata_json: String,
    pub created_at: DateTime<Utc>,
}

/// Aggregated cost summary for a single quote across all tool invocations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuoteCostSummary {
    pub quote_id: String,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_tokens: i64,
    pub total_estimated_cost_cents: f64,
    pub invocation_count: i64,
}
