use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single org-level policy toggle or configuration knob.
///
/// Stored as a key-value pair where `value_json` holds a JSON-compatible
/// scalar (string, number, or boolean literal). Typed accessors provide
/// ergonomic conversion for the most common value shapes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OrgSetting {
    pub key: String,
    pub value_json: String,
    pub description: Option<String>,
    pub updated_at: DateTime<Utc>,
    pub updated_by: Option<String>,
}

impl OrgSetting {
    /// Parse `value_json` as an `f64` (e.g. discount thresholds).
    pub fn value_as_f64(&self) -> Option<f64> {
        self.value_json.trim().parse().ok()
    }

    /// Parse `value_json` as an `i64` (e.g. cent amounts, day counts).
    pub fn value_as_i64(&self) -> Option<i64> {
        self.value_json.trim().parse().ok()
    }

    /// Parse `value_json` as a `bool` (literal `"true"` / `"false"`).
    pub fn value_as_bool(&self) -> Option<bool> {
        match self.value_json.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }
}
