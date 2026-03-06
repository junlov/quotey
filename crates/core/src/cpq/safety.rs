//! Safety + Red-Team harness for NXT negotiation autopilot.
//!
//! Provides an adversarial negotiation corpus that probes the edges of the
//! concession policy engine, counteroffer planner, and boundary calculator.
//! All tests are deterministic and designed to catch regressions where
//! safety guardrails could be bypassed.
//!
//! The adversarial corpus covers:
//! - Extreme numeric values (overflow, underflow, NaN-like)
//! - Boundary-straddling values (just above/below floors and ceilings)
//! - Multi-dimension combined attacks
//! - Repeated escalation attempts
//! - Empty/malformed inputs

use serde::{Deserialize, Serialize};

use crate::cpq::boundary::{BoundaryCalculator, BoundaryInput};
use crate::cpq::concession::{
    ConcessionPolicy, ConcessionPolicyEngine, ConcessionRequest, ConcessionRequestValue,
};
use crate::cpq::counteroffer::{CounterofferConfig, CounterofferPlanner};

// ---------------------------------------------------------------------------
// Adversarial corpus
// ---------------------------------------------------------------------------

/// A single adversarial test case.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdversarialCase {
    pub name: String,
    pub description: String,
    pub request_values: Vec<ConcessionRequestValue>,
    pub expected_within_bounds: bool,
    pub expected_walk_away: bool,
    pub must_not_produce_offers_below_floor: bool,
}

/// Result of running a single adversarial case.
#[derive(Debug, Clone, PartialEq)]
pub struct AdversarialResult {
    pub case_name: String,
    pub passed: bool,
    pub violations: Vec<String>,
}

/// Summary of running the full adversarial corpus.
#[derive(Debug, Clone, PartialEq)]
pub struct AdversarialReport {
    pub total_cases: usize,
    pub passed: usize,
    pub failed: usize,
    pub all_passed: bool,
    pub results: Vec<AdversarialResult>,
}

/// Build the standard adversarial negotiation corpus.
pub fn adversarial_corpus() -> Vec<AdversarialCase> {
    vec![
        // --- Extreme numeric values ---
        AdversarialCase {
            name: "extreme_negative_discount".to_string(),
            description: "Discount at extreme negative value should be blocked".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "discount_pct".to_string(),
                value: -1000.0,
            }],
            expected_within_bounds: false,
            expected_walk_away: true,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "extreme_positive_discount".to_string(),
            description: "Discount at extreme high value should be blocked".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "discount_pct".to_string(),
                value: 999_999.0,
            }],
            expected_within_bounds: false,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "extreme_negative_margin".to_string(),
            description: "Margin at extreme negative should trigger walk-away".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "margin_pct".to_string(),
                value: -500.0,
            }],
            expected_within_bounds: false,
            expected_walk_away: true,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "zero_all_dimensions".to_string(),
            description: "All dimensions at zero should hit floor for margin".to_string(),
            request_values: vec![
                ConcessionRequestValue { dimension: "discount_pct".to_string(), value: 0.0 },
                ConcessionRequestValue { dimension: "margin_pct".to_string(), value: 0.0 },
                ConcessionRequestValue { dimension: "term_months".to_string(), value: 0.0 },
            ],
            expected_within_bounds: false,
            expected_walk_away: true,
            must_not_produce_offers_below_floor: true,
        },
        // --- Boundary straddling ---
        AdversarialCase {
            name: "margin_just_above_hard_floor".to_string(),
            description: "Margin at hard_floor + epsilon should be within bounds".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "margin_pct".to_string(),
                value: 15.01,
            }],
            expected_within_bounds: true,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "margin_just_below_hard_floor".to_string(),
            description: "Margin at hard_floor - epsilon should be blocked".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "margin_pct".to_string(),
                value: 14.99,
            }],
            expected_within_bounds: false,
            expected_walk_away: true,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "discount_just_below_ceiling".to_string(),
            description: "Discount just below ceiling should be within bounds".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "discount_pct".to_string(),
                value: 39.99,
            }],
            expected_within_bounds: true,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "discount_just_above_ceiling".to_string(),
            description: "Discount just above ceiling should be blocked".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "discount_pct".to_string(),
                value: 40.01,
            }],
            expected_within_bounds: false,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        // --- Multi-dimension combined attacks ---
        AdversarialCase {
            name: "all_dimensions_at_ceiling".to_string(),
            description: "All dimensions at ceiling should require approval".to_string(),
            request_values: vec![
                ConcessionRequestValue { dimension: "discount_pct".to_string(), value: 40.0 },
                ConcessionRequestValue { dimension: "margin_pct".to_string(), value: 80.0 },
                ConcessionRequestValue { dimension: "term_months".to_string(), value: 36.0 },
            ],
            expected_within_bounds: true,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "mixed_extreme_values".to_string(),
            description: "One dimension extreme low, another extreme high".to_string(),
            request_values: vec![
                ConcessionRequestValue { dimension: "discount_pct".to_string(), value: 100.0 },
                ConcessionRequestValue { dimension: "margin_pct".to_string(), value: 5.0 },
            ],
            expected_within_bounds: false,
            expected_walk_away: true,
            must_not_produce_offers_below_floor: true,
        },
        // --- Empty/malformed inputs ---
        AdversarialCase {
            name: "empty_request".to_string(),
            description: "No dimensions specified should be safe".to_string(),
            request_values: vec![],
            expected_within_bounds: true,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "unknown_dimension".to_string(),
            description: "Unknown dimension should be ignored safely".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "nonexistent_dimension".to_string(),
                value: 999.0,
            }],
            expected_within_bounds: true,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        // --- Infinity-like values ---
        AdversarialCase {
            name: "infinity_discount".to_string(),
            description: "Infinity-like discount should be blocked".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "discount_pct".to_string(),
                value: f64::MAX,
            }],
            expected_within_bounds: false,
            expected_walk_away: false,
            must_not_produce_offers_below_floor: true,
        },
        AdversarialCase {
            name: "neg_infinity_margin".to_string(),
            description: "Negative infinity-like margin should trigger walk-away".to_string(),
            request_values: vec![ConcessionRequestValue {
                dimension: "margin_pct".to_string(),
                value: f64::MIN,
            }],
            expected_within_bounds: false,
            expected_walk_away: true,
            must_not_produce_offers_below_floor: true,
        },
    ]
}

// ---------------------------------------------------------------------------
// Safety runner
// ---------------------------------------------------------------------------

/// Run the full adversarial corpus against the concession policy engine.
pub fn run_adversarial_corpus(
    policy: &ConcessionPolicy,
    config: &CounterofferConfig,
) -> AdversarialReport {
    let engine = ConcessionPolicyEngine;
    let planner = CounterofferPlanner;
    let corpus = adversarial_corpus();
    let mut results = Vec::new();
    let mut passed_count = 0;

    for case in &corpus {
        let request = ConcessionRequest {
            session_id: format!("adversarial-{}", case.name),
            values: case.request_values.clone(),
        };

        let (envelope, boundary) = engine.evaluate(policy, &request);
        let plan = planner.plan(&envelope, config);

        let mut violations = Vec::new();

        // Check within_bounds expectation
        if boundary.within_bounds != case.expected_within_bounds {
            violations.push(format!(
                "within_bounds: expected {}, got {}",
                case.expected_within_bounds, boundary.within_bounds
            ));
        }

        // Check walk_away expectation
        if boundary.walk_away != case.expected_walk_away {
            violations.push(format!(
                "walk_away: expected {}, got {}",
                case.expected_walk_away, boundary.walk_away
            ));
        }

        // Safety invariant: no offers should have values below the policy floor
        if case.must_not_produce_offers_below_floor {
            for alt in &plan.alternatives {
                let discount_floor = policy
                    .dimensions
                    .iter()
                    .find(|d| d.dimension == "discount_pct")
                    .map(|d| d.hard_floor)
                    .unwrap_or(0.0);
                if alt.discount_pct < discount_floor {
                    violations.push(format!(
                        "offer {} has discount_pct {:.2} below floor {:.2}",
                        alt.offer_id, alt.discount_pct, discount_floor
                    ));
                }
            }
        }

        let case_passed = violations.is_empty();
        if case_passed {
            passed_count += 1;
        }

        results.push(AdversarialResult {
            case_name: case.name.clone(),
            passed: case_passed,
            violations,
        });
    }

    let total = corpus.len();
    AdversarialReport {
        total_cases: total,
        passed: passed_count,
        failed: total - passed_count,
        all_passed: passed_count == total,
        results,
    }
}

/// Safety invariant checker for the boundary calculator.
/// Verifies that extreme inputs never bypass walk-away guardrails.
pub fn check_boundary_safety_invariants(calc: &BoundaryCalculator) -> Vec<String> {
    let mut violations = Vec::new();

    // Test: margin at 0% for software must trigger walk-away
    let (eval, _) = calc.evaluate(&BoundaryInput {
        margin_pct: 0.0,
        discount_pct: 0.0,
        product_category: "software".to_string(),
        turn_count: 1,
        max_turns: 20,
    });
    if !eval.walk_away {
        violations.push("0% margin for software did not trigger walk-away".to_string());
    }

    // Test: discount at 100% for software must breach ceiling
    let (eval, _) = calc.evaluate(&BoundaryInput {
        margin_pct: 50.0,
        discount_pct: 100.0,
        product_category: "software".to_string(),
        turn_count: 1,
        max_turns: 20,
    });
    if !eval.ceiling_breached {
        violations.push("100% discount for software did not breach ceiling".to_string());
    }

    // Test: negative margin must trigger walk-away
    let (eval, _) = calc.evaluate(&BoundaryInput {
        margin_pct: -50.0,
        discount_pct: 0.0,
        product_category: "software".to_string(),
        turn_count: 1,
        max_turns: 20,
    });
    if !eval.walk_away {
        violations.push("-50% margin for software did not trigger walk-away".to_string());
    }

    // Test: max_turns exceeded must produce stop reason
    let (_, stops) = calc.evaluate(&BoundaryInput {
        margin_pct: 50.0,
        discount_pct: 10.0,
        product_category: "software".to_string(),
        turn_count: 100,
        max_turns: 20,
    });
    if stops.is_empty() {
        violations.push("turn 100 of 20 max did not produce stop reason".to_string());
    }

    violations
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpq::boundary::BoundaryCalculator;
    use crate::cpq::concession::ConcessionPolicy;
    use crate::cpq::counteroffer::CounterofferConfig;

    #[test]
    fn adversarial_corpus_passes_with_default_policy() {
        let policy = ConcessionPolicy::default();
        let config = CounterofferConfig::default();
        let report = run_adversarial_corpus(&policy, &config);

        for result in &report.results {
            if !result.passed {
                panic!("adversarial case '{}' failed: {:?}", result.case_name, result.violations);
            }
        }
        assert!(report.all_passed, "all adversarial cases must pass");
    }

    #[test]
    fn corpus_has_minimum_case_count() {
        let corpus = adversarial_corpus();
        assert!(
            corpus.len() >= 10,
            "adversarial corpus must have at least 10 cases, found {}",
            corpus.len()
        );
    }

    #[test]
    fn corpus_covers_extreme_values() {
        let corpus = adversarial_corpus();
        let names: Vec<&str> = corpus.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"extreme_negative_discount"));
        assert!(names.contains(&"extreme_positive_discount"));
        assert!(names.contains(&"infinity_discount"));
        assert!(names.contains(&"neg_infinity_margin"));
    }

    #[test]
    fn corpus_covers_boundary_straddling() {
        let corpus = adversarial_corpus();
        let names: Vec<&str> = corpus.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"margin_just_above_hard_floor"));
        assert!(names.contains(&"margin_just_below_hard_floor"));
        assert!(names.contains(&"discount_just_below_ceiling"));
        assert!(names.contains(&"discount_just_above_ceiling"));
    }

    #[test]
    fn corpus_covers_empty_and_unknown() {
        let corpus = adversarial_corpus();
        let names: Vec<&str> = corpus.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"empty_request"));
        assert!(names.contains(&"unknown_dimension"));
    }

    #[test]
    fn boundary_safety_invariants_pass() {
        let calc = BoundaryCalculator::default();
        let violations = check_boundary_safety_invariants(&calc);
        assert!(violations.is_empty(), "boundary safety invariants violated: {:?}", violations);
    }

    #[test]
    fn no_offer_below_floor_even_with_extreme_inputs() {
        let policy = ConcessionPolicy::default();
        let config = CounterofferConfig::default();
        let engine = ConcessionPolicyEngine;
        let planner = CounterofferPlanner;

        let extreme_requests = vec![
            vec![ConcessionRequestValue { dimension: "discount_pct".to_string(), value: -1000.0 }],
            vec![ConcessionRequestValue { dimension: "discount_pct".to_string(), value: f64::MAX }],
            vec![],
        ];

        for req_values in extreme_requests {
            let request =
                ConcessionRequest { session_id: "safety-test".to_string(), values: req_values };
            let (envelope, _) = engine.evaluate(&policy, &request);
            let plan = planner.plan(&envelope, &config);

            for alt in &plan.alternatives {
                assert!(
                    alt.discount_pct >= 0.0,
                    "offer {} has negative discount: {}",
                    alt.offer_id,
                    alt.discount_pct
                );
            }
        }
    }

    #[test]
    fn adversarial_corpus_is_deterministic() {
        let policy = ConcessionPolicy::default();
        let config = CounterofferConfig::default();

        let report1 = run_adversarial_corpus(&policy, &config);
        let report2 = run_adversarial_corpus(&policy, &config);

        assert_eq!(report1.total_cases, report2.total_cases);
        assert_eq!(report1.passed, report2.passed);
        assert_eq!(report1.failed, report2.failed);
        for (r1, r2) in report1.results.iter().zip(report2.results.iter()) {
            assert_eq!(r1.passed, r2.passed, "case {} determinism mismatch", r1.case_name);
        }
    }
}
