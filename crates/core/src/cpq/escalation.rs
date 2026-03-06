//! Escalation context pack serializer for NXT negotiation approval handoff.
//!
//! When a negotiation reaches an out-of-policy position that requires approval,
//! the escalation pack bundles all evidence (session, turns, boundary evaluation,
//! concession deltas, stop reasons) into a structured packet for the approval
//! workflow. Approvers see deterministic evidence, not free-text summaries.

use serde::{Deserialize, Serialize};

use crate::domain::negotiation::{
    BoundaryEvaluation, ConcessionEnvelope, NegotiationSession, NegotiationTurn,
};

// ---------------------------------------------------------------------------
// Escalation context pack
// ---------------------------------------------------------------------------

/// Complete evidence packet for an approval escalation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EscalationContextPack {
    /// Session being escalated.
    pub session_id: String,
    pub quote_id: String,
    pub actor_id: String,
    /// Current negotiation state.
    pub session_state: String,
    /// Policy version used for evaluation.
    pub policy_version: String,
    /// Pricing version used for evaluation.
    pub pricing_version: String,
    /// Number of turns in the session so far.
    pub turn_count: usize,
    /// The specific turn that triggered escalation (if any).
    pub trigger_turn_number: Option<u32>,
    /// The offer ID being escalated (if selected).
    pub offer_id: Option<String>,
    /// Concession deltas — how far each dimension has moved from initial.
    pub concession_deltas: Vec<ConcessionDelta>,
    /// Current boundary evaluation flags.
    pub boundary_within_bounds: bool,
    pub boundary_requires_approval: bool,
    pub boundary_walk_away: bool,
    /// Stop reasons from boundary evaluation.
    pub stop_reasons: Vec<String>,
    /// Blocking reasons from envelope evaluation.
    pub blocking_reasons: Vec<String>,
    /// Human-readable escalation summary.
    pub escalation_reason: String,
}

/// Delta for a single concession dimension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcessionDelta {
    pub dimension: String,
    pub floor: f64,
    pub ceiling: f64,
    pub current: f64,
    /// Distance from ceiling as percentage of range.
    pub utilization_pct: f64,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for escalation context packs.
pub struct EscalationPackBuilder {
    session: NegotiationSession,
    turns: Vec<NegotiationTurn>,
    envelope: Option<ConcessionEnvelope>,
    boundary: Option<BoundaryEvaluation>,
    trigger_turn_number: Option<u32>,
    offer_id: Option<String>,
    reason: String,
}

impl EscalationPackBuilder {
    pub fn new(session: NegotiationSession) -> Self {
        Self {
            session,
            turns: Vec::new(),
            envelope: None,
            boundary: None,
            trigger_turn_number: None,
            offer_id: None,
            reason: String::new(),
        }
    }

    pub fn with_turns(mut self, turns: Vec<NegotiationTurn>) -> Self {
        self.turns = turns;
        self
    }

    pub fn with_envelope(mut self, envelope: ConcessionEnvelope) -> Self {
        self.envelope = Some(envelope);
        self
    }

    pub fn with_boundary(mut self, boundary: BoundaryEvaluation) -> Self {
        self.boundary = Some(boundary);
        self
    }

    pub fn with_trigger_turn(mut self, turn_number: u32) -> Self {
        self.trigger_turn_number = Some(turn_number);
        self
    }

    pub fn with_offer_id(mut self, offer_id: impl Into<String>) -> Self {
        self.offer_id = Some(offer_id.into());
        self
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = reason.into();
        self
    }

    pub fn build(self) -> EscalationContextPack {
        let boundary = self.boundary.unwrap_or(BoundaryEvaluation {
            within_bounds: true,
            floor_breached: false,
            ceiling_breached: false,
            walk_away: false,
            requires_approval: false,
            stop_reasons: Vec::new(),
        });

        let concession_deltas = self.envelope.as_ref().map_or_else(Vec::new, |env| {
            env.ranges
                .iter()
                .map(|r| {
                    let range = r.ceiling - r.floor;
                    let utilization = if range > f64::EPSILON {
                        ((r.current - r.floor) / range * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    };
                    ConcessionDelta {
                        dimension: r.dimension.clone(),
                        floor: r.floor,
                        ceiling: r.ceiling,
                        current: r.current,
                        utilization_pct: utilization,
                    }
                })
                .collect()
        });

        let blocking_reasons =
            self.envelope.as_ref().map_or_else(Vec::new, |env| env.blocking_reasons.clone());

        EscalationContextPack {
            session_id: self.session.id.0,
            quote_id: self.session.quote_id,
            actor_id: self.session.actor_id,
            session_state: self.session.state.as_str().to_string(),
            policy_version: self.session.policy_version,
            pricing_version: self.session.pricing_version,
            turn_count: self.turns.len(),
            trigger_turn_number: self.trigger_turn_number,
            offer_id: self.offer_id,
            concession_deltas,
            boundary_within_bounds: boundary.within_bounds,
            boundary_requires_approval: boundary.requires_approval,
            boundary_walk_away: boundary.walk_away,
            stop_reasons: boundary.stop_reasons,
            blocking_reasons,
            escalation_reason: self.reason,
        }
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate that an escalation pack contains all required fields for approval.
/// Returns Ok(()) or an error describing the first missing field.
pub fn validate_escalation_pack(pack: &EscalationContextPack) -> Result<(), String> {
    if pack.session_id.is_empty() {
        return Err("missing session_id".to_string());
    }
    if pack.quote_id.is_empty() {
        return Err("missing quote_id".to_string());
    }
    if pack.actor_id.is_empty() {
        return Err("missing actor_id".to_string());
    }
    if pack.policy_version.is_empty() {
        return Err("missing policy_version".to_string());
    }
    if pack.escalation_reason.is_empty() {
        return Err("missing escalation_reason".to_string());
    }
    if !pack.boundary_requires_approval && !pack.boundary_walk_away {
        return Err("escalation pack should have requires_approval or walk_away set".to_string());
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
        BoundaryEvaluation, ConcessionEnvelope, ConcessionRange, NegotiationSession,
        NegotiationSessionId, NegotiationState, NegotiationTurn, NegotiationTurnId, TurnOutcome,
        TurnRequestType,
    };

    fn test_session() -> NegotiationSession {
        NegotiationSession {
            id: NegotiationSessionId("NXT-ESC-001".to_string()),
            quote_id: "Q-2026-0001".to_string(),
            actor_id: "rep-alice".to_string(),
            state: NegotiationState::ApprovalPending,
            policy_version: "policy-v1".to_string(),
            pricing_version: "pricing-v1".to_string(),
            idempotency_key: "key-1".to_string(),
            max_turns: 20,
            expires_at: None,
            created_at: "2026-03-06T00:00:00Z".to_string(),
            updated_at: "2026-03-06T00:00:00Z".to_string(),
        }
    }

    fn test_turn(n: u32) -> NegotiationTurn {
        NegotiationTurn {
            id: NegotiationTurnId(format!("T-{n}")),
            session_id: NegotiationSessionId("NXT-ESC-001".to_string()),
            turn_number: n,
            request_type: TurnRequestType::Counter,
            request_payload: "{}".to_string(),
            envelope_json: None,
            plan_json: None,
            chosen_offer_id: Some("offer-1".to_string()),
            outcome: TurnOutcome::Offered,
            boundary_json: None,
            transition_key: format!("txn-{n}"),
            created_at: "2026-03-06T00:01:00Z".to_string(),
        }
    }

    fn test_envelope() -> ConcessionEnvelope {
        ConcessionEnvelope {
            session_id: NegotiationSessionId("NXT-ESC-001".to_string()),
            ranges: vec![
                ConcessionRange {
                    dimension: "discount_pct".to_string(),
                    floor: 0.0,
                    ceiling: 40.0,
                    current: 35.0,
                },
                ConcessionRange {
                    dimension: "margin_pct".to_string(),
                    floor: 15.0,
                    ceiling: 80.0,
                    current: 20.0,
                },
            ],
            blocking_reasons: vec!["discount near ceiling".to_string()],
        }
    }

    fn test_boundary_approval() -> BoundaryEvaluation {
        BoundaryEvaluation {
            within_bounds: true,
            floor_breached: false,
            ceiling_breached: false,
            walk_away: false,
            requires_approval: true,
            stop_reasons: vec!["discount 35% exceeds soft ceiling".to_string()],
        }
    }

    #[test]
    fn build_escalation_pack_with_all_fields() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_turns(vec![test_turn(1), test_turn(2)])
            .with_envelope(test_envelope())
            .with_boundary(test_boundary_approval())
            .with_trigger_turn(2)
            .with_offer_id("offer-anchor-1")
            .with_reason("discount near soft ceiling requires manager approval")
            .build();

        assert_eq!(pack.session_id, "NXT-ESC-001");
        assert_eq!(pack.quote_id, "Q-2026-0001");
        assert_eq!(pack.turn_count, 2);
        assert_eq!(pack.trigger_turn_number, Some(2));
        assert_eq!(pack.offer_id.as_deref(), Some("offer-anchor-1"));
        assert!(pack.boundary_requires_approval);
        assert!(!pack.boundary_walk_away);
        assert_eq!(pack.concession_deltas.len(), 2);
        assert!(!pack.stop_reasons.is_empty());
    }

    #[test]
    fn concession_delta_utilization_computed_correctly() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_envelope(test_envelope())
            .with_boundary(test_boundary_approval())
            .with_reason("test")
            .build();

        // discount: (35 - 0) / (40 - 0) * 100 = 87.5%
        let discount_delta =
            pack.concession_deltas.iter().find(|d| d.dimension == "discount_pct").unwrap();
        assert!((discount_delta.utilization_pct - 87.5).abs() < 0.1);

        // margin: (20 - 15) / (80 - 15) * 100 ≈ 7.69%
        let margin_delta =
            pack.concession_deltas.iter().find(|d| d.dimension == "margin_pct").unwrap();
        assert!((margin_delta.utilization_pct - 7.69).abs() < 0.1);
    }

    #[test]
    fn validation_passes_for_complete_pack() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_boundary(test_boundary_approval())
            .with_reason("needs approval")
            .build();

        assert!(validate_escalation_pack(&pack).is_ok());
    }

    #[test]
    fn validation_fails_without_reason() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_boundary(test_boundary_approval())
            .build();

        let result = validate_escalation_pack(&pack);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("escalation_reason"));
    }

    #[test]
    fn validation_fails_without_approval_or_walk_away() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_boundary(BoundaryEvaluation {
                within_bounds: true,
                floor_breached: false,
                ceiling_breached: false,
                walk_away: false,
                requires_approval: false,
                stop_reasons: Vec::new(),
            })
            .with_reason("no reason to escalate actually")
            .build();

        let result = validate_escalation_pack(&pack);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires_approval"));
    }

    #[test]
    fn pack_serializes_to_json() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_turns(vec![test_turn(1)])
            .with_envelope(test_envelope())
            .with_boundary(test_boundary_approval())
            .with_trigger_turn(1)
            .with_reason("margin in approval zone")
            .build();

        let json = serde_json::to_string(&pack).unwrap();
        let deserialized: EscalationContextPack = serde_json::from_str(&json).unwrap();

        assert_eq!(pack, deserialized);
    }

    #[test]
    fn walk_away_pack_passes_validation() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_boundary(BoundaryEvaluation {
                within_bounds: false,
                floor_breached: true,
                ceiling_breached: false,
                walk_away: true,
                requires_approval: false,
                stop_reasons: vec!["margin below hard floor".to_string()],
            })
            .with_reason("walk-away: margin below hard floor")
            .build();

        assert!(validate_escalation_pack(&pack).is_ok());
    }

    #[test]
    fn no_envelope_produces_empty_deltas() {
        let pack = EscalationPackBuilder::new(test_session())
            .with_boundary(test_boundary_approval())
            .with_reason("test")
            .build();

        assert!(pack.concession_deltas.is_empty());
        assert!(pack.blocking_reasons.is_empty());
    }
}
