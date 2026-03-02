use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::quote::QuoteId;
use crate::errors::DomainError;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DialogueSessionId(pub String);

impl DialogueSessionId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DialogueSessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogueSessionStatus {
    Active,
    Completed,
    Expired,
}

impl DialogueSessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DialogueSessionStatus::Active => "active",
            DialogueSessionStatus::Completed => "completed",
            DialogueSessionStatus::Expired => "expired",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s {
            "active" => Ok(DialogueSessionStatus::Active),
            "completed" => Ok(DialogueSessionStatus::Completed),
            "expired" => Ok(DialogueSessionStatus::Expired),
            _ => Err(DomainError::InvalidEnumValue {
                enum_name: "DialogueSessionStatus".to_string(),
                value: s.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlackQuoteState {
    IntentCapture,
    ContextCollection,
    AssumptionReview,
    PricingReady,
    PricedReview,
    ApprovalRequired,
    Approved,
    Finalized,
    Sent,
    BlockedError,
}

impl SlackQuoteState {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlackQuoteState::IntentCapture => "intent_capture",
            SlackQuoteState::ContextCollection => "context_collection",
            SlackQuoteState::AssumptionReview => "assumption_review",
            SlackQuoteState::PricingReady => "pricing_ready",
            SlackQuoteState::PricedReview => "priced_review",
            SlackQuoteState::ApprovalRequired => "approval_required",
            SlackQuoteState::Approved => "approved",
            SlackQuoteState::Finalized => "finalized",
            SlackQuoteState::Sent => "sent",
            SlackQuoteState::BlockedError => "blocked_error",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, DomainError> {
        match s {
            "intent_capture" => Ok(SlackQuoteState::IntentCapture),
            "context_collection" => Ok(SlackQuoteState::ContextCollection),
            "assumption_review" => Ok(SlackQuoteState::AssumptionReview),
            "pricing_ready" => Ok(SlackQuoteState::PricingReady),
            "priced_review" => Ok(SlackQuoteState::PricedReview),
            "approval_required" => Ok(SlackQuoteState::ApprovalRequired),
            "approved" => Ok(SlackQuoteState::Approved),
            "finalized" => Ok(SlackQuoteState::Finalized),
            "sent" => Ok(SlackQuoteState::Sent),
            "blocked_error" => Ok(SlackQuoteState::BlockedError),
            _ => Err(DomainError::InvalidEnumValue {
                enum_name: "SlackQuoteState".to_string(),
                value: s.to_string(),
            }),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, SlackQuoteState::Sent | SlackQuoteState::BlockedError)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueSession {
    pub id: DialogueSessionId,
    pub slack_thread_id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub current_state: SlackQuoteState,
    pub context_json: Option<String>,
    pub pending_clarifications_json: Option<String>,
    pub quote_draft_id: Option<QuoteId>,
    pub status: DialogueSessionStatus,
}

impl DialogueSession {
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    pub fn is_resumable(&self) -> bool {
        self.status == DialogueSessionStatus::Active && !self.is_expired()
    }

    pub fn mark_expired(&mut self) {
        self.status = DialogueSessionStatus::Expired;
    }

    pub fn transition_to(&mut self, next: SlackQuoteState) -> Result<(), DomainError> {
        if !self.is_resumable() {
            return Err(DomainError::InvalidStateTransition {
                from: self.current_state.as_str().to_string(),
                to: next.as_str().to_string(),
                reason: "Session is not resumable".to_string(),
            });
        }

        let valid = matches!(
            (&self.current_state, &next),
            (SlackQuoteState::IntentCapture, SlackQuoteState::ContextCollection)
                | (SlackQuoteState::ContextCollection, SlackQuoteState::AssumptionReview)
                | (SlackQuoteState::ContextCollection, SlackQuoteState::BlockedError)
                | (SlackQuoteState::AssumptionReview, SlackQuoteState::PricingReady)
                | (SlackQuoteState::PricingReady, SlackQuoteState::PricedReview)
                | (SlackQuoteState::PricedReview, SlackQuoteState::ApprovalRequired)
                | (SlackQuoteState::PricedReview, SlackQuoteState::Finalized)
                | (SlackQuoteState::ApprovalRequired, SlackQuoteState::Approved)
                | (SlackQuoteState::ApprovalRequired, SlackQuoteState::BlockedError)
                | (SlackQuoteState::Approved, SlackQuoteState::Finalized)
                | (SlackQuoteState::Finalized, SlackQuoteState::Sent)
                | (_, SlackQuoteState::BlockedError)
        );

        if valid {
            self.current_state = next;
            Ok(())
        } else {
            Err(DomainError::InvalidStateTransition {
                from: self.current_state.as_str().to_string(),
                to: next.as_str().to_string(),
                reason: "Invalid state transition".to_string(),
            })
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DialogueTurn {
    pub id: String,
    pub session_id: DialogueSessionId,
    pub turn_number: u32,
    pub user_message: String,
    pub bot_response: String,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_session() -> DialogueSession {
        DialogueSession {
            id: DialogueSessionId("session-1".to_string()),
            slack_thread_id: "thread-123".to_string(),
            user_id: "U12345".to_string(),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
            current_state: SlackQuoteState::IntentCapture,
            context_json: None,
            pending_clarifications_json: None,
            quote_draft_id: None,
            status: DialogueSessionStatus::Active,
        }
    }

    #[test]
    fn session_is_resumable_when_active_and_not_expired() {
        let session = new_session();
        assert!(session.is_resumable());
    }

    #[test]
    fn session_not_resumable_when_expired() {
        let mut session = new_session();
        session.expires_at = Utc::now() - chrono::Duration::hours(1);
        assert!(!session.is_resumable());
    }

    #[test]
    fn allows_valid_state_transition() {
        let mut session = new_session();
        session.transition_to(SlackQuoteState::ContextCollection).unwrap();
        assert_eq!(session.current_state, SlackQuoteState::ContextCollection);
    }

    #[test]
    fn blocks_invalid_state_transition() {
        let mut session = new_session();
        let result = session.transition_to(SlackQuoteState::Sent);
        assert!(result.is_err());
    }

    #[test]
    fn terminal_state_detection() {
        assert!(SlackQuoteState::Sent.is_terminal());
        assert!(SlackQuoteState::BlockedError.is_terminal());
        assert!(!SlackQuoteState::IntentCapture.is_terminal());
    }
}
