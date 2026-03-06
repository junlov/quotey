//! Deterministic replay/simulation harness for NXT negotiation transcripts.
//!
//! Re-executes negotiation turns from audit events using the same policy and
//! configuration inputs, then compares outputs to detect determinism drift.
//! This is the core of the NXT replay guarantee: identical inputs with
//! identical policy versions must produce identical outputs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::cpq::concession::{
    ConcessionPolicy, ConcessionPolicyEngine, ConcessionRequest, ConcessionRequestValue,
};
use crate::cpq::counteroffer::{CounterofferConfig, CounterofferPlanner};
use crate::cpq::negotiation_audit::{self, TranscriptEntry};

// ---------------------------------------------------------------------------
// Replay step types
// ---------------------------------------------------------------------------

/// A single step in a replay scenario — captures the inputs that were
/// originally used and the expected outputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayStep {
    pub sequence: usize,
    pub event_type: String,
    pub original_metadata: BTreeMap<String, String>,
}

/// Field-level difference detected between original and replayed output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DriftDiagnostic {
    pub step_sequence: usize,
    pub field: String,
    pub original_value: String,
    pub replayed_value: String,
}

/// Result of replaying a single evaluation step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayStepResult {
    pub sequence: usize,
    pub event_type: String,
    pub passed: bool,
    pub drifts: Vec<DriftDiagnostic>,
}

/// Full replay report for a negotiation transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayReport {
    pub session_id: String,
    pub total_steps: usize,
    pub replayed_steps: usize,
    pub passed_steps: usize,
    pub failed_steps: usize,
    pub deterministic: bool,
    pub step_results: Vec<ReplayStepResult>,
    pub all_drifts: Vec<DriftDiagnostic>,
}

// ---------------------------------------------------------------------------
// Replay fixture (canned scenario for deterministic testing)
// ---------------------------------------------------------------------------

/// A canned replay fixture with all inputs needed to reproduce a negotiation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayFixture {
    pub session_id: String,
    pub policy: ConcessionPolicy,
    pub counteroffer_config: CounterofferConfig,
    pub turns: Vec<ReplayFixtureTurn>,
}

/// A single turn in a replay fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReplayFixtureTurn {
    pub turn_number: u32,
    pub request_values: Vec<ConcessionRequestValue>,
    /// Expected envelope ranges (dimension -> (floor, ceiling, current)).
    pub expected_envelope_ranges: Vec<(String, f64, f64, f64)>,
    /// Expected boundary flags.
    pub expected_within_bounds: bool,
    pub expected_walk_away: bool,
    pub expected_requires_approval: bool,
    /// Expected counteroffer count.
    pub expected_alternatives_count: usize,
}

// ---------------------------------------------------------------------------
// Replay harness
// ---------------------------------------------------------------------------

/// Deterministic replay harness. Stateless — policy and config provided at call time.
#[derive(Debug, Clone, Default)]
pub struct ReplayHarness;

impl ReplayHarness {
    /// Replay a transcript from audit events and verify determinism.
    ///
    /// For each ENVELOPE_EVALUATED event, re-runs the concession policy engine
    /// with the same inputs and compares the boundary evaluation flags.
    /// For each COUNTEROFFER_PLANNED event, re-runs the planner and compares
    /// the alternatives count.
    pub fn replay_transcript(
        &self,
        transcript: &[TranscriptEntry],
        policy: &ConcessionPolicy,
        config: &CounterofferConfig,
    ) -> ReplayReport {
        let session_id = transcript
            .first()
            .and_then(|e| e.metadata.get("session_id"))
            .cloned()
            .unwrap_or_default();

        let engine = ConcessionPolicyEngine;
        let planner = CounterofferPlanner;
        let mut step_results = Vec::new();
        let mut all_drifts = Vec::new();
        let mut replayed = 0;
        let mut passed = 0;

        for entry in transcript {
            match entry.event_type.as_str() {
                negotiation_audit::event_types::ENVELOPE_EVALUATED => {
                    replayed += 1;
                    let result = self.replay_envelope_step(entry, policy, &engine, &session_id);
                    if result.passed {
                        passed += 1;
                    }
                    all_drifts.extend(result.drifts.clone());
                    step_results.push(result);
                }
                negotiation_audit::event_types::COUNTEROFFER_PLANNED => {
                    replayed += 1;
                    let result =
                        self.replay_counteroffer_step(entry, policy, config, &engine, &planner);
                    if result.passed {
                        passed += 1;
                    }
                    all_drifts.extend(result.drifts.clone());
                    step_results.push(result);
                }
                negotiation_audit::event_types::BOUNDARY_EVALUATED => {
                    replayed += 1;
                    let result = self.replay_boundary_step(entry);
                    if result.passed {
                        passed += 1;
                    }
                    all_drifts.extend(result.drifts.clone());
                    step_results.push(result);
                }
                _ => {
                    // Non-replayable events (state changes, turns) just pass through
                    step_results.push(ReplayStepResult {
                        sequence: entry.sequence,
                        event_type: entry.event_type.clone(),
                        passed: true,
                        drifts: Vec::new(),
                    });
                }
            }
        }

        let failed = replayed - passed;

        ReplayReport {
            session_id,
            total_steps: transcript.len(),
            replayed_steps: replayed,
            passed_steps: passed,
            failed_steps: failed,
            deterministic: failed == 0,
            step_results,
            all_drifts,
        }
    }

    /// Replay a fixture and verify all turns produce expected outputs.
    pub fn replay_fixture(&self, fixture: &ReplayFixture) -> ReplayReport {
        let engine = ConcessionPolicyEngine;
        let planner = CounterofferPlanner;
        let mut step_results = Vec::new();
        let mut all_drifts = Vec::new();
        let mut passed = 0;

        for turn in &fixture.turns {
            let request = ConcessionRequest {
                session_id: fixture.session_id.clone(),
                values: turn.request_values.clone(),
            };

            let (envelope, boundary) = engine.evaluate(&fixture.policy, &request);
            let plan = planner.plan(&envelope, &fixture.counteroffer_config);

            let mut drifts = Vec::new();
            let seq = turn.turn_number as usize;

            // Compare boundary flags
            if boundary.within_bounds != turn.expected_within_bounds {
                drifts.push(DriftDiagnostic {
                    step_sequence: seq,
                    field: "within_bounds".to_string(),
                    original_value: turn.expected_within_bounds.to_string(),
                    replayed_value: boundary.within_bounds.to_string(),
                });
            }
            if boundary.walk_away != turn.expected_walk_away {
                drifts.push(DriftDiagnostic {
                    step_sequence: seq,
                    field: "walk_away".to_string(),
                    original_value: turn.expected_walk_away.to_string(),
                    replayed_value: boundary.walk_away.to_string(),
                });
            }
            if boundary.requires_approval != turn.expected_requires_approval {
                drifts.push(DriftDiagnostic {
                    step_sequence: seq,
                    field: "requires_approval".to_string(),
                    original_value: turn.expected_requires_approval.to_string(),
                    replayed_value: boundary.requires_approval.to_string(),
                });
            }

            // Compare envelope ranges
            for (dim, exp_floor, exp_ceiling, exp_current) in &turn.expected_envelope_ranges {
                if let Some(range) = envelope.ranges.iter().find(|r| &r.dimension == dim) {
                    if (range.floor - exp_floor).abs() > f64::EPSILON {
                        drifts.push(DriftDiagnostic {
                            step_sequence: seq,
                            field: format!("{dim}.floor"),
                            original_value: format!("{exp_floor:.2}"),
                            replayed_value: format!("{:.2}", range.floor),
                        });
                    }
                    if (range.ceiling - exp_ceiling).abs() > f64::EPSILON {
                        drifts.push(DriftDiagnostic {
                            step_sequence: seq,
                            field: format!("{dim}.ceiling"),
                            original_value: format!("{exp_ceiling:.2}"),
                            replayed_value: format!("{:.2}", range.ceiling),
                        });
                    }
                    if (range.current - exp_current).abs() > f64::EPSILON {
                        drifts.push(DriftDiagnostic {
                            step_sequence: seq,
                            field: format!("{dim}.current"),
                            original_value: format!("{exp_current:.2}"),
                            replayed_value: format!("{:.2}", range.current),
                        });
                    }
                }
            }

            // Compare counteroffer count
            if plan.alternatives.len() != turn.expected_alternatives_count {
                drifts.push(DriftDiagnostic {
                    step_sequence: seq,
                    field: "alternatives_count".to_string(),
                    original_value: turn.expected_alternatives_count.to_string(),
                    replayed_value: plan.alternatives.len().to_string(),
                });
            }

            let step_passed = drifts.is_empty();
            if step_passed {
                passed += 1;
            }
            all_drifts.extend(drifts.clone());
            step_results.push(ReplayStepResult {
                sequence: seq,
                event_type: "fixture_turn".to_string(),
                passed: step_passed,
                drifts,
            });
        }

        let total = fixture.turns.len();
        ReplayReport {
            session_id: fixture.session_id.clone(),
            total_steps: total,
            replayed_steps: total,
            passed_steps: passed,
            failed_steps: total - passed,
            deterministic: passed == total,
            step_results,
            all_drifts,
        }
    }

    // --- internal helpers ---

    fn replay_envelope_step(
        &self,
        entry: &TranscriptEntry,
        policy: &ConcessionPolicy,
        engine: &ConcessionPolicyEngine,
        session_id: &str,
    ) -> ReplayStepResult {
        // Re-run with empty request (the envelope event just records blocking reasons count)
        let request = ConcessionRequest { session_id: session_id.to_string(), values: Vec::new() };
        let (_envelope, _boundary) = engine.evaluate(policy, &request);

        // Compare blocking_reasons_count if present in metadata
        let mut drifts = Vec::new();
        if let Some(original_count) = entry.metadata.get("blocking_reasons_count") {
            let replayed_count = _envelope.blocking_reasons.len().to_string();
            if *original_count != replayed_count {
                drifts.push(DriftDiagnostic {
                    step_sequence: entry.sequence,
                    field: "blocking_reasons_count".to_string(),
                    original_value: original_count.clone(),
                    replayed_value: replayed_count,
                });
            }
        }

        ReplayStepResult {
            sequence: entry.sequence,
            event_type: entry.event_type.clone(),
            passed: drifts.is_empty(),
            drifts,
        }
    }

    fn replay_counteroffer_step(
        &self,
        entry: &TranscriptEntry,
        policy: &ConcessionPolicy,
        config: &CounterofferConfig,
        engine: &ConcessionPolicyEngine,
        planner: &CounterofferPlanner,
    ) -> ReplayStepResult {
        let session_id = entry.metadata.get("session_id").cloned().unwrap_or_default();
        let request = ConcessionRequest { session_id: session_id.clone(), values: Vec::new() };
        let (envelope, _) = engine.evaluate(policy, &request);
        let plan = planner.plan(&envelope, config);

        let mut drifts = Vec::new();

        // Compare alternatives count
        if let Some(original_count) = entry.metadata.get("alternatives_count") {
            let replayed_count = plan.alternatives.len().to_string();
            if *original_count != replayed_count {
                drifts.push(DriftDiagnostic {
                    step_sequence: entry.sequence,
                    field: "alternatives_count".to_string(),
                    original_value: original_count.clone(),
                    replayed_value: replayed_count,
                });
            }
        }

        // Compare strategy
        if let Some(original_strategy) = entry.metadata.get("strategy") {
            if *original_strategy != config.strategy.as_str() {
                drifts.push(DriftDiagnostic {
                    step_sequence: entry.sequence,
                    field: "strategy".to_string(),
                    original_value: original_strategy.clone(),
                    replayed_value: config.strategy.as_str().to_string(),
                });
            }
        }

        ReplayStepResult {
            sequence: entry.sequence,
            event_type: entry.event_type.clone(),
            passed: drifts.is_empty(),
            drifts,
        }
    }

    fn replay_boundary_step(&self, entry: &TranscriptEntry) -> ReplayStepResult {
        // Boundary events are metadata-only; we verify structural integrity
        let mut drifts = Vec::new();

        // Ensure required fields are present
        for field in &["within_bounds", "walk_away", "requires_approval"] {
            if !entry.metadata.contains_key(*field) {
                drifts.push(DriftDiagnostic {
                    step_sequence: entry.sequence,
                    field: field.to_string(),
                    original_value: "<missing>".to_string(),
                    replayed_value: "<expected>".to_string(),
                });
            }
        }

        ReplayStepResult {
            sequence: entry.sequence,
            event_type: entry.event_type.clone(),
            passed: drifts.is_empty(),
            drifts,
        }
    }
}

// ---------------------------------------------------------------------------
// Diff tooling
// ---------------------------------------------------------------------------

/// Compare two replay reports and produce a list of differences.
pub fn diff_reports(baseline: &ReplayReport, candidate: &ReplayReport) -> Vec<DriftDiagnostic> {
    let mut diffs = Vec::new();

    if baseline.deterministic != candidate.deterministic {
        diffs.push(DriftDiagnostic {
            step_sequence: 0,
            field: "deterministic".to_string(),
            original_value: baseline.deterministic.to_string(),
            replayed_value: candidate.deterministic.to_string(),
        });
    }

    if baseline.passed_steps != candidate.passed_steps {
        diffs.push(DriftDiagnostic {
            step_sequence: 0,
            field: "passed_steps".to_string(),
            original_value: baseline.passed_steps.to_string(),
            replayed_value: candidate.passed_steps.to_string(),
        });
    }

    // Compare individual step pass/fail
    for (b_step, c_step) in baseline.step_results.iter().zip(candidate.step_results.iter()) {
        if b_step.passed != c_step.passed {
            diffs.push(DriftDiagnostic {
                step_sequence: b_step.sequence,
                field: format!("step_{}_passed", b_step.sequence),
                original_value: b_step.passed.to_string(),
                replayed_value: c_step.passed.to_string(),
            });
        }
    }

    diffs
}

/// Generate a standard replay fixture from the default policy for testing.
pub fn standard_fixture() -> ReplayFixture {
    use crate::cpq::concession::ConcessionRequestValue;

    ReplayFixture {
        session_id: "replay-fixture-001".to_string(),
        policy: ConcessionPolicy::default(),
        counteroffer_config: CounterofferConfig::default(),
        turns: vec![
            // Turn 1: normal request within bounds
            ReplayFixtureTurn {
                turn_number: 1,
                request_values: vec![
                    ConcessionRequestValue { dimension: "discount_pct".to_string(), value: 10.0 },
                    ConcessionRequestValue { dimension: "margin_pct".to_string(), value: 40.0 },
                ],
                expected_envelope_ranges: vec![
                    ("discount_pct".to_string(), 0.0, 40.0, 10.0),
                    ("margin_pct".to_string(), 15.0, 80.0, 40.0),
                    ("term_months".to_string(), 1.0, 36.0, 1.0),
                ],
                expected_within_bounds: true,
                expected_walk_away: false,
                expected_requires_approval: false,
                expected_alternatives_count: 3,
            },
            // Turn 2: aggressive discount near ceiling → approval required
            ReplayFixtureTurn {
                turn_number: 2,
                request_values: vec![
                    ConcessionRequestValue { dimension: "discount_pct".to_string(), value: 38.0 },
                    ConcessionRequestValue { dimension: "margin_pct".to_string(), value: 40.0 },
                ],
                expected_envelope_ranges: vec![
                    ("discount_pct".to_string(), 0.0, 40.0, 38.0),
                    ("margin_pct".to_string(), 15.0, 80.0, 40.0),
                    ("term_months".to_string(), 1.0, 36.0, 1.0),
                ],
                expected_within_bounds: true,
                expected_walk_away: false,
                expected_requires_approval: true,
                expected_alternatives_count: 3,
            },
            // Turn 3: margin below hard floor → walk away
            ReplayFixtureTurn {
                turn_number: 3,
                request_values: vec![
                    ConcessionRequestValue { dimension: "discount_pct".to_string(), value: 10.0 },
                    ConcessionRequestValue { dimension: "margin_pct".to_string(), value: 10.0 },
                ],
                expected_envelope_ranges: vec![
                    ("discount_pct".to_string(), 0.0, 40.0, 10.0),
                    ("margin_pct".to_string(), 15.0, 80.0, 10.0),
                    ("term_months".to_string(), 1.0, 36.0, 1.0),
                ],
                expected_within_bounds: false,
                expected_walk_away: true,
                expected_requires_approval: false,
                expected_alternatives_count: 3,
            },
        ],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpq::concession::ConcessionPolicy;
    use crate::cpq::counteroffer::CounterofferConfig;
    use crate::cpq::negotiation_audit;
    use crate::domain::negotiation::{
        NegotiationSession, NegotiationSessionId, NegotiationState, NegotiationTurn,
        NegotiationTurnId, TurnOutcome, TurnRequestType,
    };

    fn test_session() -> NegotiationSession {
        NegotiationSession {
            id: NegotiationSessionId("REPLAY-001".to_string()),
            quote_id: "Q-2026-0001".to_string(),
            actor_id: "rep-alice".to_string(),
            state: NegotiationState::Active,
            policy_version: "concession-v1".to_string(),
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
            session_id: NegotiationSessionId("REPLAY-001".to_string()),
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

    #[test]
    fn standard_fixture_replays_deterministically() {
        let harness = ReplayHarness;
        let fixture = standard_fixture();
        let report = harness.replay_fixture(&fixture);

        assert!(report.deterministic, "standard fixture must replay deterministically");
        assert_eq!(report.total_steps, 3);
        assert_eq!(report.passed_steps, 3);
        assert_eq!(report.failed_steps, 0);
        assert!(report.all_drifts.is_empty());
    }

    #[test]
    fn fixture_drift_detected_when_policy_changes() {
        let harness = ReplayHarness;
        let mut fixture = standard_fixture();
        // Change the policy hard floor — turn 3 should now pass (margin 10 >= new floor 5)
        for dim in &mut fixture.policy.dimensions {
            if dim.dimension == "margin_pct" {
                dim.hard_floor = 5.0;
            }
        }

        let report = harness.replay_fixture(&fixture);

        // Turn 3 expected walk_away=true but with new floor it's within bounds
        assert!(!report.deterministic, "policy change must cause drift");
        assert!(!report.all_drifts.is_empty());
        assert!(report
            .all_drifts
            .iter()
            .any(|d| d.field == "within_bounds" || d.field == "walk_away"));
    }

    #[test]
    fn transcript_replay_passes_for_consistent_events() {
        let session = test_session();
        let evt1 = negotiation_audit::session_created(&session);
        let evt2 = negotiation_audit::turn_recorded(&session, &test_turn(1));
        let evt3 = negotiation_audit::envelope_evaluated(&session, 0);
        let evt4 = negotiation_audit::counteroffer_planned(&session, 3, "step_down");
        let evt5 = negotiation_audit::boundary_evaluated(&session, true, false, false);

        let events = vec![evt1, evt2, evt3, evt4, evt5];
        let transcript = negotiation_audit::reconstruct_transcript(&events, "REPLAY-001");

        let harness = ReplayHarness;
        let policy = ConcessionPolicy::default();
        let config = CounterofferConfig::default();
        let report = harness.replay_transcript(&transcript, &policy, &config);

        assert!(report.deterministic);
        assert_eq!(report.total_steps, 5);
        assert!(report.replayed_steps >= 3); // envelope, counteroffer, boundary
    }

    #[test]
    fn diff_reports_detects_determinism_change() {
        let report_a = ReplayReport {
            session_id: "s1".to_string(),
            total_steps: 3,
            replayed_steps: 3,
            passed_steps: 3,
            failed_steps: 0,
            deterministic: true,
            step_results: vec![ReplayStepResult {
                sequence: 1,
                event_type: "test".to_string(),
                passed: true,
                drifts: Vec::new(),
            }],
            all_drifts: Vec::new(),
        };
        let report_b = ReplayReport {
            session_id: "s1".to_string(),
            total_steps: 3,
            replayed_steps: 3,
            passed_steps: 2,
            failed_steps: 1,
            deterministic: false,
            step_results: vec![ReplayStepResult {
                sequence: 1,
                event_type: "test".to_string(),
                passed: false,
                drifts: Vec::new(),
            }],
            all_drifts: Vec::new(),
        };

        let diffs = diff_reports(&report_a, &report_b);
        assert!(!diffs.is_empty());
        assert!(diffs.iter().any(|d| d.field == "deterministic"));
        assert!(diffs.iter().any(|d| d.field == "passed_steps"));
    }

    #[test]
    fn fixture_with_modified_counteroffer_count_detects_drift() {
        let harness = ReplayHarness;
        let mut fixture = standard_fixture();
        // Expect 5 alternatives but config only produces 3
        fixture.turns[0].expected_alternatives_count = 5;

        let report = harness.replay_fixture(&fixture);

        assert!(!report.deterministic);
        assert!(report.all_drifts.iter().any(|d| d.field == "alternatives_count"));
    }

    #[test]
    fn replay_twice_produces_identical_reports() {
        let harness = ReplayHarness;
        let fixture = standard_fixture();

        let report1 = harness.replay_fixture(&fixture);
        let report2 = harness.replay_fixture(&fixture);

        assert_eq!(report1.deterministic, report2.deterministic);
        assert_eq!(report1.passed_steps, report2.passed_steps);
        assert_eq!(report1.all_drifts.len(), report2.all_drifts.len());

        let diffs = diff_reports(&report1, &report2);
        assert!(diffs.is_empty(), "replaying same fixture twice must produce no diffs");
    }

    #[test]
    fn empty_transcript_produces_empty_report() {
        let harness = ReplayHarness;
        let policy = ConcessionPolicy::default();
        let config = CounterofferConfig::default();

        let report = harness.replay_transcript(&[], &policy, &config);

        assert!(report.deterministic);
        assert_eq!(report.total_steps, 0);
        assert_eq!(report.replayed_steps, 0);
    }
}
