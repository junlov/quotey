//! NXT Rollout — Demo checklist and go/no-go gate sheet.
//!
//! Provides a structured rollout readiness check that verifies all NXT
//! components are wired and all KPI gates pass. This is the final
//! verification before NXT is cleared for production rollout.

use serde::{Deserialize, Serialize};

use crate::cpq::boundary::BoundaryCalculator;
use crate::cpq::concession::ConcessionPolicy;
use crate::cpq::counteroffer::CounterofferConfig;
use crate::cpq::replay::{standard_fixture, ReplayHarness};
use crate::cpq::safety::{check_boundary_safety_invariants, run_adversarial_corpus};
use crate::cpq::telemetry::{GoNoGoDecision, KpiReport};

// ---------------------------------------------------------------------------
// Demo checklist
// ---------------------------------------------------------------------------

/// A single check in the demo/rollout checklist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub name: String,
    pub category: String,
    pub passed: bool,
    pub detail: String,
}

/// Full rollout readiness report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RolloutReadinessReport {
    pub checks: Vec<ChecklistItem>,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub ready: bool,
}

// ---------------------------------------------------------------------------
// Gate runner
// ---------------------------------------------------------------------------

/// Run the full rollout readiness gate.
///
/// This verifies:
/// 1. Replay harness: standard fixture replays deterministically
/// 2. Safety corpus: all adversarial cases pass
/// 3. Boundary invariants: extreme inputs never bypass guardrails
/// 4. KPI gate: if provided, go/no-go decision is "go"
pub fn run_rollout_gate(kpi_report: Option<&KpiReport>) -> RolloutReadinessReport {
    let mut checks = Vec::new();

    // 1. Replay determinism
    let harness = ReplayHarness;
    let fixture = standard_fixture();
    let replay_report = harness.replay_fixture(&fixture);
    checks.push(ChecklistItem {
        name: "replay_determinism".to_string(),
        category: "assurance".to_string(),
        passed: replay_report.deterministic,
        detail: format!(
            "{}/{} steps passed, {} drifts",
            replay_report.passed_steps,
            replay_report.total_steps,
            replay_report.all_drifts.len()
        ),
    });

    // 2. Adversarial safety corpus
    let policy = ConcessionPolicy::default();
    let config = CounterofferConfig::default();
    let adversarial_report = run_adversarial_corpus(&policy, &config);
    checks.push(ChecklistItem {
        name: "adversarial_safety_corpus".to_string(),
        category: "safety".to_string(),
        passed: adversarial_report.all_passed,
        detail: format!(
            "{}/{} cases passed",
            adversarial_report.passed, adversarial_report.total_cases
        ),
    });

    // 3. Boundary safety invariants
    let calc = BoundaryCalculator::default();
    let boundary_violations = check_boundary_safety_invariants(&calc);
    checks.push(ChecklistItem {
        name: "boundary_safety_invariants".to_string(),
        category: "safety".to_string(),
        passed: boundary_violations.is_empty(),
        detail: if boundary_violations.is_empty() {
            "all invariants hold".to_string()
        } else {
            format!("{} violations: {}", boundary_violations.len(), boundary_violations.join("; "))
        },
    });

    // 4. KPI go/no-go gate
    if let Some(report) = kpi_report {
        checks.push(ChecklistItem {
            name: "kpi_go_no_go".to_string(),
            category: "operations".to_string(),
            passed: report.go_no_go == GoNoGoDecision::Go,
            detail: format!(
                "decision: {}, {} sessions, {} measurements",
                report.go_no_go.as_str(),
                report.session_count,
                report.measurements.len()
            ),
        });
    }

    // 5. Engine component checks (structural)
    checks.push(ChecklistItem {
        name: "concession_policy_engine".to_string(),
        category: "engine".to_string(),
        passed: true,
        detail: format!("default policy has {} dimensions", policy.dimensions.len()),
    });
    checks.push(ChecklistItem {
        name: "counteroffer_planner".to_string(),
        category: "engine".to_string(),
        passed: true,
        detail: format!(
            "strategy: {}, max_alternatives: {}",
            config.strategy.as_str(),
            config.max_alternatives
        ),
    });
    checks.push(ChecklistItem {
        name: "boundary_calculator".to_string(),
        category: "engine".to_string(),
        passed: true,
        detail: "default policies for software + services categories".to_string(),
    });

    let total = checks.len();
    let passed = checks.iter().filter(|c| c.passed).count();

    RolloutReadinessReport {
        checks,
        total_checks: total,
        passed_checks: passed,
        failed_checks: total - passed,
        ready: passed == total,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpq::telemetry::{compute_kpis, GoNoGoDecision};
    use crate::domain::negotiation::{
        NegotiationSession, NegotiationSessionId, NegotiationState, NegotiationTurn,
        NegotiationTurnId, TurnOutcome, TurnRequestType,
    };

    #[test]
    fn rollout_gate_passes_without_kpi_report() {
        let report = run_rollout_gate(None);
        assert!(report.ready, "rollout gate must pass without KPI report");
        assert!(report.passed_checks >= 6);
        assert_eq!(report.failed_checks, 0);
    }

    #[test]
    fn rollout_gate_passes_with_healthy_kpis() {
        let sessions = vec![NegotiationSession {
            id: NegotiationSessionId("s1".to_string()),
            quote_id: "Q-2026-0001".to_string(),
            actor_id: "rep-alice".to_string(),
            state: NegotiationState::Accepted,
            policy_version: "v1".to_string(),
            pricing_version: "v1".to_string(),
            idempotency_key: "key-1".to_string(),
            max_turns: 20,
            expires_at: None,
            created_at: "2026-03-06T00:00:00Z".to_string(),
            updated_at: "2026-03-06T00:00:00Z".to_string(),
        }];
        let turns = vec![vec![NegotiationTurn {
            id: NegotiationTurnId("T-1".to_string()),
            session_id: NegotiationSessionId("s1".to_string()),
            turn_number: 1,
            request_type: TurnRequestType::Counter,
            request_payload: "{}".to_string(),
            envelope_json: None,
            plan_json: None,
            chosen_offer_id: Some("offer-1".to_string()),
            outcome: TurnOutcome::Accepted,
            boundary_json: None,
            transition_key: "txn-1".to_string(),
            created_at: "2026-03-06T00:01:00Z".to_string(),
        }]];

        let kpi_report = compute_kpis(&sessions, &turns, 100.0, 0);
        let report = run_rollout_gate(Some(&kpi_report));

        assert!(report.ready);
        assert!(report.checks.iter().any(|c| c.name == "kpi_go_no_go" && c.passed));
    }

    #[test]
    fn rollout_gate_fails_with_bad_kpis() {
        let sessions = vec![NegotiationSession {
            id: NegotiationSessionId("s1".to_string()),
            quote_id: "Q-2026-0001".to_string(),
            actor_id: "rep-alice".to_string(),
            state: NegotiationState::Active,
            policy_version: "v1".to_string(),
            pricing_version: "v1".to_string(),
            idempotency_key: "key-1".to_string(),
            max_turns: 20,
            expires_at: None,
            created_at: "2026-03-06T00:00:00Z".to_string(),
            updated_at: "2026-03-06T00:00:00Z".to_string(),
        }];
        let turns = vec![vec![]];

        // breach_incidents = 1 → NoGo
        let kpi_report = compute_kpis(&sessions, &turns, 100.0, 1);
        assert_eq!(kpi_report.go_no_go, GoNoGoDecision::NoGo);

        let report = run_rollout_gate(Some(&kpi_report));
        assert!(!report.ready);
        assert!(report.checks.iter().any(|c| c.name == "kpi_go_no_go" && !c.passed));
    }

    #[test]
    fn checklist_covers_all_categories() {
        let report = run_rollout_gate(None);
        let categories: Vec<&str> = report.checks.iter().map(|c| c.category.as_str()).collect();
        assert!(categories.contains(&"assurance"));
        assert!(categories.contains(&"safety"));
        assert!(categories.contains(&"engine"));
    }

    #[test]
    fn checklist_is_deterministic() {
        let r1 = run_rollout_gate(None);
        let r2 = run_rollout_gate(None);
        assert_eq!(r1.total_checks, r2.total_checks);
        assert_eq!(r1.passed_checks, r2.passed_checks);
        assert_eq!(r1.ready, r2.ready);
    }
}
