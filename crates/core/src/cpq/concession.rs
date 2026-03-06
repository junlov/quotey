//! Deterministic concession policy engine for NXT negotiation autopilot.
//!
//! Evaluates negotiation requests against configurable boundaries to produce
//! a `ConcessionEnvelope` (allowed ranges per dimension) and a
//! `BoundaryEvaluation` (pass/fail with stop reasons).
//!
//! All outputs are deterministic given the same inputs and policy version.

use serde::{Deserialize, Serialize};

use crate::domain::negotiation::{BoundaryEvaluation, ConcessionEnvelope, ConcessionRange};

// ---------------------------------------------------------------------------
// Policy configuration (loaded from DB in production, inlined in tests)
// ---------------------------------------------------------------------------

/// Per-dimension concession boundaries. Each dimension has a hard floor,
/// a soft floor (warning threshold), and a ceiling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcessionDimensionPolicy {
    pub dimension: String,
    /// Absolute minimum — requests below this are blocked.
    pub hard_floor: f64,
    /// Soft floor — requests between hard and soft floor trigger approval.
    pub soft_floor: f64,
    /// Maximum allowed value (e.g., max discount %).
    pub ceiling: f64,
    /// Step size for deterministic counteroffer generation.
    pub step: f64,
}

/// Complete concession policy for a negotiation context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcessionPolicy {
    pub version: String,
    pub dimensions: Vec<ConcessionDimensionPolicy>,
    /// If true, any single dimension breaching hard floor triggers walk-away.
    pub walk_away_on_hard_breach: bool,
    /// Maximum total concession across all dimensions (0.0-1.0 normalized).
    pub max_total_concession: f64,
}

impl Default for ConcessionPolicy {
    fn default() -> Self {
        Self {
            version: "concession-v1".to_string(),
            dimensions: vec![
                ConcessionDimensionPolicy {
                    dimension: "discount_pct".to_string(),
                    hard_floor: 0.0,
                    soft_floor: 5.0,
                    ceiling: 40.0,
                    step: 2.5,
                },
                ConcessionDimensionPolicy {
                    dimension: "margin_pct".to_string(),
                    hard_floor: 15.0,
                    soft_floor: 25.0,
                    ceiling: 80.0,
                    step: 5.0,
                },
                ConcessionDimensionPolicy {
                    dimension: "term_months".to_string(),
                    hard_floor: 1.0,
                    soft_floor: 3.0,
                    ceiling: 36.0,
                    step: 3.0,
                },
            ],
            walk_away_on_hard_breach: true,
            max_total_concession: 0.60,
        }
    }
}

// ---------------------------------------------------------------------------
// Concession request (what the rep/customer is asking for)
// ---------------------------------------------------------------------------

/// A single dimension value in a negotiation request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcessionRequestValue {
    pub dimension: String,
    pub value: f64,
}

/// Full concession request to evaluate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConcessionRequest {
    pub session_id: String,
    pub values: Vec<ConcessionRequestValue>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Deterministic concession policy engine. Stateless — policy is provided at call time.
#[derive(Debug, Clone, Default)]
pub struct ConcessionPolicyEngine;

impl ConcessionPolicyEngine {
    /// Evaluate a request against the policy, producing the concession envelope
    /// (allowed ranges for each dimension) and boundary evaluation.
    pub fn evaluate(
        &self,
        policy: &ConcessionPolicy,
        request: &ConcessionRequest,
    ) -> (ConcessionEnvelope, BoundaryEvaluation) {
        let mut ranges = Vec::new();
        let mut blocking_reasons = Vec::new();
        let mut stop_reasons = Vec::new();

        let mut floor_breached = false;
        let mut ceiling_breached = false;
        let mut requires_approval = false;
        let mut walk_away = false;

        for dim_policy in &policy.dimensions {
            let current_value = request
                .values
                .iter()
                .find(|v| v.dimension == dim_policy.dimension)
                .map(|v| v.value);

            // Build the range regardless of whether a value was requested
            ranges.push(ConcessionRange {
                dimension: dim_policy.dimension.clone(),
                floor: dim_policy.hard_floor,
                ceiling: dim_policy.ceiling,
                current: current_value.unwrap_or(dim_policy.hard_floor),
            });

            if let Some(val) = current_value {
                // For "discount_pct": higher = worse for seller; check against ceiling
                // For "margin_pct": lower = worse for seller; check against floor
                // Generic logic: value below hard_floor or above ceiling = breach
                if val < dim_policy.hard_floor {
                    floor_breached = true;
                    let reason = format!(
                        "{}: value {:.2} below hard floor {:.2}",
                        dim_policy.dimension, val, dim_policy.hard_floor
                    );
                    blocking_reasons.push(reason.clone());
                    stop_reasons.push(reason);
                    if policy.walk_away_on_hard_breach {
                        walk_away = true;
                    }
                } else if val > dim_policy.ceiling {
                    ceiling_breached = true;
                    let reason = format!(
                        "{}: value {:.2} exceeds ceiling {:.2}",
                        dim_policy.dimension, val, dim_policy.ceiling
                    );
                    blocking_reasons.push(reason.clone());
                    stop_reasons.push(reason);
                } else if val < dim_policy.soft_floor || val > dim_policy.ceiling - dim_policy.step
                {
                    // In the approval zone (between hard and soft floor, or near ceiling)
                    requires_approval = true;
                    stop_reasons.push(format!(
                        "{}: value {:.2} requires approval (soft boundary zone)",
                        dim_policy.dimension, val
                    ));
                }
            }
        }

        // Check total concession across dimensions
        let total_concession = compute_total_concession(policy, request);
        if total_concession > policy.max_total_concession {
            requires_approval = true;
            stop_reasons.push(format!(
                "total concession {:.2} exceeds max {:.2}",
                total_concession, policy.max_total_concession
            ));
        }

        let within_bounds = !floor_breached && !ceiling_breached;

        let envelope = ConcessionEnvelope {
            session_id: crate::domain::negotiation::NegotiationSessionId(
                request.session_id.clone(),
            ),
            ranges,
            blocking_reasons,
        };

        let boundary = BoundaryEvaluation {
            within_bounds,
            floor_breached,
            ceiling_breached,
            walk_away,
            requires_approval,
            stop_reasons,
        };

        (envelope, boundary)
    }
}

/// Compute normalized total concession across all dimensions.
/// Each dimension's concession is (value - floor) / (ceiling - floor), clamped to [0, 1].
fn compute_total_concession(policy: &ConcessionPolicy, request: &ConcessionRequest) -> f64 {
    let mut sum = 0.0;
    let mut count = 0;

    for dim_policy in &policy.dimensions {
        if let Some(val) = request.values.iter().find(|v| v.dimension == dim_policy.dimension) {
            let range = dim_policy.ceiling - dim_policy.hard_floor;
            if range > f64::EPSILON {
                let normalized = ((val.value - dim_policy.hard_floor) / range).clamp(0.0, 1.0);
                sum += normalized;
                count += 1;
            }
        }
    }

    if count > 0 {
        sum / count as f64
    } else {
        0.0
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> ConcessionPolicy {
        ConcessionPolicy::default()
    }

    fn request(values: Vec<(&str, f64)>) -> ConcessionRequest {
        ConcessionRequest {
            session_id: "test-session".to_string(),
            values: values
                .into_iter()
                .map(|(d, v)| ConcessionRequestValue { dimension: d.to_string(), value: v })
                .collect(),
        }
    }

    #[test]
    fn within_bounds_request_passes() {
        let engine = ConcessionPolicyEngine;
        let policy = default_policy();
        let req = request(vec![("discount_pct", 10.0), ("margin_pct", 40.0)]);

        let (envelope, boundary) = engine.evaluate(&policy, &req);

        assert!(boundary.within_bounds);
        assert!(!boundary.floor_breached);
        assert!(!boundary.ceiling_breached);
        assert!(!boundary.walk_away);
        assert!(envelope.blocking_reasons.is_empty());
    }

    #[test]
    fn hard_floor_breach_triggers_walk_away() {
        let engine = ConcessionPolicyEngine;
        let policy = default_policy();
        // margin_pct below hard floor of 15.0
        let req = request(vec![("margin_pct", 10.0)]);

        let (_envelope, boundary) = engine.evaluate(&policy, &req);

        assert!(!boundary.within_bounds);
        assert!(boundary.floor_breached);
        assert!(boundary.walk_away);
        assert!(boundary.stop_reasons.iter().any(|r| r.contains("below hard floor")));
    }

    #[test]
    fn ceiling_breach_blocks_without_walk_away() {
        let engine = ConcessionPolicyEngine;
        let policy = default_policy();
        // discount_pct above ceiling of 40.0
        let req = request(vec![("discount_pct", 50.0)]);

        let (_envelope, boundary) = engine.evaluate(&policy, &req);

        assert!(!boundary.within_bounds);
        assert!(boundary.ceiling_breached);
        // walk_away only on hard_floor breach when walk_away_on_hard_breach=true
        assert!(!boundary.walk_away);
        assert!(boundary.stop_reasons.iter().any(|r| r.contains("exceeds ceiling")));
    }

    #[test]
    fn soft_floor_zone_requires_approval() {
        let engine = ConcessionPolicyEngine;
        let policy = default_policy();
        // margin_pct between hard_floor (15) and soft_floor (25) → approval zone
        let req = request(vec![("margin_pct", 20.0)]);

        let (_envelope, boundary) = engine.evaluate(&policy, &req);

        assert!(boundary.within_bounds);
        assert!(boundary.requires_approval);
        assert!(boundary.stop_reasons.iter().any(|r| r.contains("requires approval")));
    }

    #[test]
    fn envelope_contains_all_dimensions() {
        let engine = ConcessionPolicyEngine;
        let policy = default_policy();
        let req = request(vec![("discount_pct", 10.0)]);

        let (envelope, _boundary) = engine.evaluate(&policy, &req);

        assert_eq!(envelope.ranges.len(), 3);
        let discount_range =
            envelope.ranges.iter().find(|r| r.dimension == "discount_pct").unwrap();
        assert_eq!(discount_range.floor, 0.0);
        assert_eq!(discount_range.ceiling, 40.0);
        assert_eq!(discount_range.current, 10.0);
    }

    #[test]
    fn total_concession_exceeding_max_requires_approval() {
        let engine = ConcessionPolicyEngine;
        let mut policy = default_policy();
        policy.max_total_concession = 0.30;
        // Request high values across all dimensions → high total concession
        let req = request(vec![
            ("discount_pct", 35.0), // near ceiling
            ("margin_pct", 70.0),   // near ceiling
            ("term_months", 30.0),  // near ceiling
        ]);

        let (_envelope, boundary) = engine.evaluate(&policy, &req);

        assert!(boundary.requires_approval);
        assert!(boundary.stop_reasons.iter().any(|r| r.contains("total concession")));
    }

    #[test]
    fn empty_request_produces_floor_defaults() {
        let engine = ConcessionPolicyEngine;
        let policy = default_policy();
        let req = request(vec![]);

        let (envelope, boundary) = engine.evaluate(&policy, &req);

        assert!(boundary.within_bounds);
        assert!(!boundary.requires_approval);
        // All ranges should have current = hard_floor
        for range in &envelope.ranges {
            let dim = policy.dimensions.iter().find(|d| d.dimension == range.dimension).unwrap();
            assert_eq!(range.current, dim.hard_floor);
        }
    }

    #[test]
    fn walk_away_disabled_does_not_trigger() {
        let engine = ConcessionPolicyEngine;
        let mut policy = default_policy();
        policy.walk_away_on_hard_breach = false;
        let req = request(vec![("margin_pct", 10.0)]); // below hard floor

        let (_envelope, boundary) = engine.evaluate(&policy, &req);

        assert!(boundary.floor_breached);
        assert!(!boundary.walk_away);
    }

    #[test]
    fn deterministic_same_inputs_same_outputs() {
        let engine = ConcessionPolicyEngine;
        let policy = default_policy();
        let req = request(vec![("discount_pct", 22.5), ("margin_pct", 30.0)]);

        let (env1, bnd1) = engine.evaluate(&policy, &req);
        let (env2, bnd2) = engine.evaluate(&policy, &req);

        assert_eq!(env1.ranges.len(), env2.ranges.len());
        for (a, b) in env1.ranges.iter().zip(env2.ranges.iter()) {
            assert_eq!(a.dimension, b.dimension);
            assert_eq!(a.floor, b.floor);
            assert_eq!(a.ceiling, b.ceiling);
            assert_eq!(a.current, b.current);
        }
        assert_eq!(bnd1.within_bounds, bnd2.within_bounds);
        assert_eq!(bnd1.requires_approval, bnd2.requires_approval);
        assert_eq!(bnd1.walk_away, bnd2.walk_away);
        assert_eq!(bnd1.stop_reasons, bnd2.stop_reasons);
    }
}
