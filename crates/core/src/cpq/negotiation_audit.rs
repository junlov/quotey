//! Negotiation audit events and transcript reconstruction for NXT.
//!
//! Provides typed event constructors for every negotiation lifecycle transition,
//! plus a transcript reconstruction function that reassembles the full
//! negotiation story from audit events for replay and explainability.

use std::collections::BTreeMap;

use crate::audit::{AuditCategory, AuditEvent, AuditOutcome};
use crate::domain::negotiation::{NegotiationSession, NegotiationState, NegotiationTurn};
use crate::domain::quote::QuoteId;

// ---------------------------------------------------------------------------
// Negotiation audit event types
// ---------------------------------------------------------------------------

/// Negotiation audit schema version. Bump when event structure changes.
pub const NEGOTIATION_AUDIT_SCHEMA_VERSION: &str = "negotiation.v1";

pub mod event_types {
    pub const SESSION_CREATED: &str = "negotiation.session_created";
    pub const SESSION_STATE_CHANGED: &str = "negotiation.session_state_changed";
    pub const TURN_RECORDED: &str = "negotiation.turn_recorded";
    pub const ENVELOPE_EVALUATED: &str = "negotiation.envelope_evaluated";
    pub const COUNTEROFFER_PLANNED: &str = "negotiation.counteroffer_planned";
    pub const BOUNDARY_EVALUATED: &str = "negotiation.boundary_evaluated";
    pub const OFFER_SELECTED: &str = "negotiation.offer_selected";
    pub const ESCALATION_TRIGGERED: &str = "negotiation.escalation_triggered";
    pub const SESSION_EXPIRED: &str = "negotiation.session_expired";
    pub const SESSION_CANCELLED: &str = "negotiation.session_cancelled";
    pub const WALK_AWAY_TRIGGERED: &str = "negotiation.walk_away_triggered";
}

// ---------------------------------------------------------------------------
// Event constructors
// ---------------------------------------------------------------------------

fn base_event(session: &NegotiationSession, event_type: &str, outcome: AuditOutcome) -> AuditEvent {
    AuditEvent::new(
        Some(QuoteId(session.quote_id.clone())),
        None,
        session.id.0.clone(),
        event_type,
        AuditCategory::Flow,
        session.actor_id.clone(),
        outcome,
    )
    .with_metadata("schema_version", NEGOTIATION_AUDIT_SCHEMA_VERSION)
    .with_metadata("session_id", session.id.0.clone())
    .with_metadata("policy_version", session.policy_version.clone())
    .with_metadata("pricing_version", session.pricing_version.clone())
}

pub fn session_created(session: &NegotiationSession) -> AuditEvent {
    base_event(session, event_types::SESSION_CREATED, AuditOutcome::Success)
        .with_metadata("state", session.state.as_str())
        .with_metadata("idempotency_key", session.idempotency_key.clone())
}

pub fn session_state_changed(
    session: &NegotiationSession,
    from: &NegotiationState,
    to: &NegotiationState,
) -> AuditEvent {
    base_event(session, event_types::SESSION_STATE_CHANGED, AuditOutcome::Success)
        .with_metadata("from", from.as_str())
        .with_metadata("to", to.as_str())
}

pub fn turn_recorded(session: &NegotiationSession, turn: &NegotiationTurn) -> AuditEvent {
    base_event(session, event_types::TURN_RECORDED, AuditOutcome::Success)
        .with_metadata("turn_number", turn.turn_number.to_string())
        .with_metadata("request_type", turn.request_type.as_str())
        .with_metadata("outcome", turn.outcome.as_str())
        .with_metadata("transition_key", turn.transition_key.clone())
}

pub fn envelope_evaluated(
    session: &NegotiationSession,
    blocking_reasons_count: usize,
) -> AuditEvent {
    base_event(session, event_types::ENVELOPE_EVALUATED, AuditOutcome::Success)
        .with_metadata("blocking_reasons_count", blocking_reasons_count.to_string())
}

pub fn counteroffer_planned(
    session: &NegotiationSession,
    alternatives_count: usize,
    strategy: &str,
) -> AuditEvent {
    base_event(session, event_types::COUNTEROFFER_PLANNED, AuditOutcome::Success)
        .with_metadata("alternatives_count", alternatives_count.to_string())
        .with_metadata("strategy", strategy)
}

pub fn boundary_evaluated(
    session: &NegotiationSession,
    within_bounds: bool,
    walk_away: bool,
    requires_approval: bool,
) -> AuditEvent {
    base_event(session, event_types::BOUNDARY_EVALUATED, AuditOutcome::Success)
        .with_metadata("within_bounds", within_bounds.to_string())
        .with_metadata("walk_away", walk_away.to_string())
        .with_metadata("requires_approval", requires_approval.to_string())
}

pub fn offer_selected(
    session: &NegotiationSession,
    offer_id: &str,
    turn_number: u32,
) -> AuditEvent {
    base_event(session, event_types::OFFER_SELECTED, AuditOutcome::Success)
        .with_metadata("offer_id", offer_id)
        .with_metadata("turn_number", turn_number.to_string())
}

pub fn escalation_triggered(session: &NegotiationSession, reason: &str) -> AuditEvent {
    base_event(session, event_types::ESCALATION_TRIGGERED, AuditOutcome::Success)
        .with_metadata("reason", reason)
}

pub fn walk_away_triggered(session: &NegotiationSession, reason: &str) -> AuditEvent {
    base_event(session, event_types::WALK_AWAY_TRIGGERED, AuditOutcome::Rejected)
        .with_metadata("reason", reason)
}

// ---------------------------------------------------------------------------
// Transcript reconstruction
// ---------------------------------------------------------------------------

/// A single entry in a reconstructed negotiation transcript.
#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptEntry {
    pub sequence: usize,
    pub event_type: String,
    pub timestamp: String,
    pub metadata: BTreeMap<String, String>,
}

/// Reconstruct a negotiation transcript from audit events.
///
/// Filters events by session_id, sorts by timestamp, and produces a
/// deterministic ordered transcript. This is the foundation for
/// replay/simulation harness (Slice E).
pub fn reconstruct_transcript(events: &[AuditEvent], session_id: &str) -> Vec<TranscriptEntry> {
    let mut filtered: Vec<&AuditEvent> = events
        .iter()
        .filter(|e| e.metadata.get("session_id").map(|s| s.as_str()) == Some(session_id))
        .collect();

    // Stable sort by timestamp for deterministic ordering
    filtered.sort_by(|a, b| a.occurred_at.cmp(&b.occurred_at));

    filtered
        .iter()
        .enumerate()
        .map(|(i, e)| TranscriptEntry {
            sequence: i + 1,
            event_type: e.event_type.clone(),
            timestamp: e.occurred_at.to_rfc3339(),
            metadata: e.metadata.clone(),
        })
        .collect()
}

/// Validate that a transcript contains the expected lifecycle sequence.
/// Returns Ok(()) if the transcript is valid, or an error describing the
/// first violation found.
pub fn validate_transcript_invariants(transcript: &[TranscriptEntry]) -> Result<(), String> {
    if transcript.is_empty() {
        return Err("empty transcript".to_string());
    }

    // First event must be session_created
    if transcript[0].event_type != event_types::SESSION_CREATED {
        return Err(format!(
            "transcript must start with session_created, found: {}",
            transcript[0].event_type
        ));
    }

    // Verify monotonic timestamps
    for window in transcript.windows(2) {
        if window[1].timestamp < window[0].timestamp {
            return Err(format!(
                "non-monotonic timestamps at sequence {} -> {}",
                window[0].sequence, window[1].sequence
            ));
        }
    }

    // Verify turn numbers are monotonically increasing (if turns are present)
    let turn_events: Vec<&TranscriptEntry> =
        transcript.iter().filter(|e| e.event_type == event_types::TURN_RECORDED).collect();

    for window in turn_events.windows(2) {
        let n1 =
            window[0].metadata.get("turn_number").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        let n2 =
            window[1].metadata.get("turn_number").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
        if n2 <= n1 {
            return Err(format!("turn numbers must be monotonically increasing: {} -> {}", n1, n2));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::negotiation::{
        NegotiationSession, NegotiationSessionId, NegotiationState, NegotiationTurn,
        NegotiationTurnId, TurnOutcome, TurnRequestType,
    };

    fn test_session() -> NegotiationSession {
        NegotiationSession {
            id: NegotiationSessionId("NXT-TEST-001".to_string()),
            quote_id: "Q-2026-0001".to_string(),
            actor_id: "rep-alice".to_string(),
            state: NegotiationState::Active,
            policy_version: "policy-v1".to_string(),
            pricing_version: "pricing-v1".to_string(),
            idempotency_key: "key-1".to_string(),
            max_turns: 20,
            expires_at: None,
            created_at: "2026-03-06T00:00:00Z".to_string(),
            updated_at: "2026-03-06T00:00:00Z".to_string(),
        }
    }

    fn test_turn(turn_number: u32) -> NegotiationTurn {
        NegotiationTurn {
            id: NegotiationTurnId(format!("T-{turn_number}")),
            session_id: NegotiationSessionId("NXT-TEST-001".to_string()),
            turn_number,
            request_type: TurnRequestType::Counter,
            request_payload: "{}".to_string(),
            envelope_json: None,
            plan_json: None,
            chosen_offer_id: Some("offer-1".to_string()),
            outcome: TurnOutcome::Offered,
            boundary_json: None,
            transition_key: format!("txn-{turn_number}"),
            created_at: "2026-03-06T00:01:00Z".to_string(),
        }
    }

    #[test]
    fn session_created_event_has_required_metadata() {
        let session = test_session();
        let event = session_created(&session);

        assert_eq!(event.event_type, event_types::SESSION_CREATED);
        assert_eq!(event.metadata.get("session_id").unwrap(), "NXT-TEST-001");
        assert_eq!(event.metadata.get("schema_version").unwrap(), NEGOTIATION_AUDIT_SCHEMA_VERSION);
        assert_eq!(event.metadata.get("policy_version").unwrap(), "policy-v1");
    }

    #[test]
    fn state_change_event_captures_from_to() {
        let session = test_session();
        let event =
            session_state_changed(&session, &NegotiationState::Draft, &NegotiationState::Active);

        assert_eq!(event.metadata.get("from").unwrap(), "draft");
        assert_eq!(event.metadata.get("to").unwrap(), "active");
    }

    #[test]
    fn turn_recorded_captures_turn_details() {
        let session = test_session();
        let turn = test_turn(1);
        let event = turn_recorded(&session, &turn);

        assert_eq!(event.metadata.get("turn_number").unwrap(), "1");
        assert_eq!(event.metadata.get("request_type").unwrap(), "counter");
        assert_eq!(event.metadata.get("outcome").unwrap(), "offered");
    }

    #[test]
    fn transcript_reconstruction_filters_by_session() {
        let session = test_session();
        let evt1 = session_created(&session);
        let evt2 = turn_recorded(&session, &test_turn(1));

        // An unrelated event (different session)
        let mut other_session = test_session();
        other_session.id = NegotiationSessionId("NXT-OTHER".to_string());
        let evt3 = session_created(&other_session);

        let events = vec![evt1, evt2, evt3];
        let transcript = reconstruct_transcript(&events, "NXT-TEST-001");

        assert_eq!(transcript.len(), 2);
        assert_eq!(transcript[0].sequence, 1);
        assert_eq!(transcript[0].event_type, event_types::SESSION_CREATED);
        assert_eq!(transcript[1].event_type, event_types::TURN_RECORDED);
    }

    #[test]
    fn transcript_validation_catches_missing_session_created() {
        let session = test_session();
        let evt = turn_recorded(&session, &test_turn(1));
        let events = vec![evt];
        let transcript = reconstruct_transcript(&events, "NXT-TEST-001");

        let result = validate_transcript_invariants(&transcript);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("session_created"));
    }

    #[test]
    fn valid_transcript_passes_invariants() {
        let session = test_session();
        let evt1 = session_created(&session);
        let evt2 = turn_recorded(&session, &test_turn(1));
        let evt3 = turn_recorded(&session, &test_turn(2));

        let events = vec![evt1, evt2, evt3];
        let transcript = reconstruct_transcript(&events, "NXT-TEST-001");

        assert!(validate_transcript_invariants(&transcript).is_ok());
    }

    #[test]
    fn boundary_event_captures_flags() {
        let session = test_session();
        let event = boundary_evaluated(&session, true, false, true);

        assert_eq!(event.metadata.get("within_bounds").unwrap(), "true");
        assert_eq!(event.metadata.get("walk_away").unwrap(), "false");
        assert_eq!(event.metadata.get("requires_approval").unwrap(), "true");
    }

    #[test]
    fn walk_away_event_has_rejected_outcome() {
        let session = test_session();
        let event = walk_away_triggered(&session, "margin below floor");

        assert_eq!(event.outcome, AuditOutcome::Rejected);
        assert_eq!(event.metadata.get("reason").unwrap(), "margin below floor");
    }

    #[test]
    fn empty_transcript_fails_validation() {
        let result = validate_transcript_invariants(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }
}
