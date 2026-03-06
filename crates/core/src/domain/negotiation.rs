use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// IDs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NegotiationSessionId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NegotiationTurnId(pub String);

// ---------------------------------------------------------------------------
// Lifecycle state (maps to spec section "Negotiation Lifecycle Contract")
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NegotiationState {
    Draft,
    Active,
    CounterPending,
    ApprovalPending,
    Approved,
    Accepted,
    Rejected,
    Expired,
    Cancelled,
}

impl NegotiationState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::CounterPending => "counter_pending",
            Self::ApprovalPending => "approval_pending",
            Self::Approved => "approved",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "counter_pending" => Some(Self::CounterPending),
            "approval_pending" => Some(Self::ApprovalPending),
            "approved" => Some(Self::Approved),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "expired" => Some(Self::Expired),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Accepted | Self::Rejected | Self::Expired | Self::Cancelled)
    }

    /// Valid state transitions per the NXT lifecycle contract.
    pub fn can_transition_to(&self, next: &NegotiationState) -> bool {
        matches!(
            (self, next),
            (Self::Draft, Self::Active)
                | (Self::Active, Self::CounterPending)
                | (Self::Active, Self::Accepted)
                | (Self::Active, Self::Rejected)
                | (Self::CounterPending, Self::Active)
                | (Self::CounterPending, Self::ApprovalPending)
                | (Self::CounterPending, Self::Accepted)
                | (Self::CounterPending, Self::Rejected)
                | (Self::ApprovalPending, Self::Approved)
                | (Self::ApprovalPending, Self::Rejected)
                | (Self::Approved, Self::Accepted)
                | (Self::Approved, Self::Active)
                | (_, Self::Expired)
                | (_, Self::Cancelled)
        )
    }
}

// ---------------------------------------------------------------------------
// Request / outcome enums for turns
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnRequestType {
    Open,
    Counter,
    Accept,
    Reject,
    Escalate,
    Cancel,
}

impl TurnRequestType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Counter => "counter",
            Self::Accept => "accept",
            Self::Reject => "reject",
            Self::Escalate => "escalate",
            Self::Cancel => "cancel",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "counter" => Some(Self::Counter),
            "accept" => Some(Self::Accept),
            "reject" => Some(Self::Reject),
            "escalate" => Some(Self::Escalate),
            "cancel" => Some(Self::Cancel),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnOutcome {
    Pending,
    Offered,
    Accepted,
    Rejected,
    Escalated,
    Expired,
    Cancelled,
}

impl TurnOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Offered => "offered",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Escalated => "escalated",
            Self::Expired => "expired",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "offered" => Some(Self::Offered),
            "accepted" => Some(Self::Accepted),
            "rejected" => Some(Self::Rejected),
            "escalated" => Some(Self::Escalated),
            "expired" => Some(Self::Expired),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Domain aggregates
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NegotiationSession {
    pub id: NegotiationSessionId,
    pub quote_id: String,
    pub actor_id: String,
    pub state: NegotiationState,
    pub policy_version: String,
    pub pricing_version: String,
    pub idempotency_key: String,
    pub max_turns: u32,
    pub expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NegotiationTurn {
    pub id: NegotiationTurnId,
    pub session_id: NegotiationSessionId,
    pub turn_number: u32,
    pub request_type: TurnRequestType,
    pub request_payload: String,
    pub envelope_json: Option<String>,
    pub plan_json: Option<String>,
    pub chosen_offer_id: Option<String>,
    pub outcome: TurnOutcome,
    pub boundary_json: Option<String>,
    pub transition_key: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Concession envelope (Slice C will flesh these out; shapes defined here)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConcessionRange {
    pub dimension: String,
    pub floor: f64,
    pub ceiling: f64,
    pub current: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConcessionEnvelope {
    pub session_id: NegotiationSessionId,
    pub ranges: Vec<ConcessionRange>,
    pub blocking_reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// Counteroffer plan (Slice C will flesh these out; shapes defined here)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CounterofferAlternative {
    pub offer_id: String,
    pub rank: u32,
    pub discount_pct: f64,
    pub term_months: Option<u32>,
    pub rationale: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CounterofferPlan {
    pub session_id: NegotiationSessionId,
    pub alternatives: Vec<CounterofferAlternative>,
    pub tie_break_field: String,
}

// ---------------------------------------------------------------------------
// Boundary evaluation (Slice C will flesh these out; shapes defined here)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundaryEvaluation {
    pub within_bounds: bool,
    pub floor_breached: bool,
    pub ceiling_breached: bool,
    pub walk_away: bool,
    pub requires_approval: bool,
    pub stop_reasons: Vec<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_roundtrip_through_str() {
        let states = [
            NegotiationState::Draft,
            NegotiationState::Active,
            NegotiationState::CounterPending,
            NegotiationState::ApprovalPending,
            NegotiationState::Approved,
            NegotiationState::Accepted,
            NegotiationState::Rejected,
            NegotiationState::Expired,
            NegotiationState::Cancelled,
        ];
        for s in &states {
            let label = s.as_str();
            let parsed = NegotiationState::parse_label(label).unwrap();
            assert_eq!(&parsed, s);
        }
    }

    #[test]
    fn terminal_states_are_correct() {
        assert!(!NegotiationState::Draft.is_terminal());
        assert!(!NegotiationState::Active.is_terminal());
        assert!(!NegotiationState::CounterPending.is_terminal());
        assert!(!NegotiationState::ApprovalPending.is_terminal());
        assert!(!NegotiationState::Approved.is_terminal());
        assert!(NegotiationState::Accepted.is_terminal());
        assert!(NegotiationState::Rejected.is_terminal());
        assert!(NegotiationState::Expired.is_terminal());
        assert!(NegotiationState::Cancelled.is_terminal());
    }

    #[test]
    fn valid_transitions_accepted() {
        assert!(NegotiationState::Draft.can_transition_to(&NegotiationState::Active));
        assert!(NegotiationState::Active.can_transition_to(&NegotiationState::CounterPending));
        assert!(
            NegotiationState::CounterPending.can_transition_to(&NegotiationState::ApprovalPending)
        );
        assert!(NegotiationState::ApprovalPending.can_transition_to(&NegotiationState::Approved));
        assert!(NegotiationState::Approved.can_transition_to(&NegotiationState::Accepted));
    }

    #[test]
    fn invalid_transitions_rejected() {
        assert!(!NegotiationState::Draft.can_transition_to(&NegotiationState::Approved));
        assert!(!NegotiationState::Active.can_transition_to(&NegotiationState::Draft));
        assert!(!NegotiationState::Accepted.can_transition_to(&NegotiationState::Active));
    }

    #[test]
    fn any_state_can_cancel_or_expire() {
        let non_terminal = [
            NegotiationState::Draft,
            NegotiationState::Active,
            NegotiationState::CounterPending,
            NegotiationState::ApprovalPending,
            NegotiationState::Approved,
        ];
        for s in &non_terminal {
            assert!(s.can_transition_to(&NegotiationState::Cancelled));
            assert!(s.can_transition_to(&NegotiationState::Expired));
        }
    }

    #[test]
    fn request_type_roundtrip() {
        let types = [
            TurnRequestType::Open,
            TurnRequestType::Counter,
            TurnRequestType::Accept,
            TurnRequestType::Reject,
            TurnRequestType::Escalate,
            TurnRequestType::Cancel,
        ];
        for t in &types {
            let parsed = TurnRequestType::parse_label(t.as_str()).unwrap();
            assert_eq!(&parsed, t);
        }
    }

    #[test]
    fn outcome_roundtrip() {
        let outcomes = [
            TurnOutcome::Pending,
            TurnOutcome::Offered,
            TurnOutcome::Accepted,
            TurnOutcome::Rejected,
            TurnOutcome::Escalated,
            TurnOutcome::Expired,
            TurnOutcome::Cancelled,
        ];
        for o in &outcomes {
            let parsed = TurnOutcome::parse_label(o.as_str()).unwrap();
            assert_eq!(&parsed, o);
        }
    }

    #[test]
    fn parse_label_returns_none_for_unknown() {
        assert!(NegotiationState::parse_label("bogus").is_none());
        assert!(TurnRequestType::parse_label("bogus").is_none());
        assert!(TurnOutcome::parse_label("bogus").is_none());
    }
}
