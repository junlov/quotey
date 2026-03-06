use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Information about an active lock held on a quote.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LockInfo {
    pub quote_id: String,
    pub locked_by: String,
    pub locked_at: DateTime<Utc>,
    pub lock_expires_at: DateTime<Utc>,
}

/// Returned when a lock acquisition fails because the quote is already locked
/// by another actor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LockConflict {
    pub current_owner: String,
    pub locked_since: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
