use std::cmp::Ordering;
use std::fmt;

use thiserror::Error;

use crate::domain::precedent::{
    PrecedentDecisionStatus, PrecedentEvidence, PrecedentOutcomeStatus, PrecedentQuery,
    PrecedentResult,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PrecedentRankingInput {
    pub query: PrecedentQuery,
    pub min_similarity: f64,
    pub source_fingerprint_id: Option<String>,
    pub candidates: Vec<PrecedentCandidate>,
    pub correlation_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrecedentCandidate {
    pub candidate_quote_id: crate::domain::quote::QuoteId,
    pub candidate_fingerprint_id: String,
    pub similarity_score: f64,
    pub outcome_status: PrecedentOutcomeStatus,
    pub outcome_final_price: f64,
    pub approval_decision_status: Option<PrecedentDecisionStatus>,
    pub approval_route_version: Option<i32>,
    pub strategy_version: String,
    pub score_components_json: String,
    pub evidence_payload_json: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrecedentRankingOutput {
    pub results: Vec<PrecedentResult>,
    pub applied_limit: i32,
    pub applied_min_similarity: f64,
    pub degraded_reason_code: Option<&'static str>,
    pub degraded_user_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrecedentAuditEventType {
    QueryAllowed,
    QueryDenied,
    QueryDegraded,
    RankingCompleted,
    RankingFailed,
}

impl PrecedentAuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::QueryAllowed => "query_allowed",
            Self::QueryDenied => "query_denied",
            Self::QueryDegraded => "query_degraded",
            Self::RankingCompleted => "ranking_completed",
            Self::RankingFailed => "ranking_failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrecedentAuditEvent {
    pub event_type: PrecedentAuditEventType,
    pub quote_id: crate::domain::quote::QuoteId,
    pub correlation_id: String,
    pub reason_code: Option<&'static str>,
    pub user_message: Option<String>,
    pub fallback_path: Option<&'static str>,
    pub candidate_count: i32,
    pub selected_count: i32,
}

pub trait PrecedentAuditSink {
    fn record(&self, event: PrecedentAuditEvent);
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrecedentGuardrailPolicy {
    pub max_limit: i32,
    pub min_similarity_floor: f64,
}

impl Default for PrecedentGuardrailPolicy {
    fn default() -> Self {
        Self { max_limit: 5, min_similarity_floor: 0.7 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PrecedentGuardrailDecision {
    Allow {
        effective_limit: i32,
        effective_min_similarity: f64,
    },
    Deny {
        reason_code: &'static str,
        user_message: String,
        fallback_path: &'static str,
    },
    Degrade {
        reason_code: &'static str,
        user_message: String,
        fallback_path: &'static str,
        effective_limit: i32,
        effective_min_similarity: f64,
    },
}

impl PrecedentGuardrailPolicy {
    pub fn evaluate(&self, input: &PrecedentRankingInput) -> PrecedentGuardrailDecision {
        if input.source_fingerprint_id.as_deref().map_or(true, |value| value.trim().is_empty()) {
            return PrecedentGuardrailDecision::Deny {
                reason_code: "missing_source_fingerprint",
                user_message: "I do not have enough precedent evidence for this quote yet. Save the current configuration and try again."
                    .to_string(),
                fallback_path: "capture_quote_fingerprint",
            };
        }

        let clamped_limit = input.query.normalized_limit().min(self.max_limit.max(1));
        let clamped_similarity = if input.min_similarity.is_finite() {
            input.min_similarity.clamp(self.min_similarity_floor, 1.0)
        } else {
            self.min_similarity_floor
        };

        let similarity_adjusted = if input.min_similarity.is_finite() {
            (clamped_similarity - input.min_similarity).abs() > f64::EPSILON
        } else {
            true
        };
        let adjusted = clamped_limit != input.query.limit || similarity_adjusted;

        if adjusted {
            return PrecedentGuardrailDecision::Degrade {
                reason_code: "query_bounds_adjusted",
                user_message: format!(
                    "I adjusted the precedent query to safe deterministic bounds (limit <= {}, similarity >= {:.2}).",
                    self.max_limit, self.min_similarity_floor
                ),
                fallback_path: "deterministic_precedent_defaults",
                effective_limit: clamped_limit,
                effective_min_similarity: clamped_similarity,
            };
        }

        PrecedentGuardrailDecision::Allow {
            effective_limit: clamped_limit,
            effective_min_similarity: clamped_similarity,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeterministicPrecedentEngine {
    guardrails: PrecedentGuardrailPolicy,
}

impl Default for DeterministicPrecedentEngine {
    fn default() -> Self {
        Self::new(PrecedentGuardrailPolicy::default())
    }
}

impl DeterministicPrecedentEngine {
    pub fn new(guardrails: PrecedentGuardrailPolicy) -> Self {
        Self { guardrails }
    }

    pub fn rank_similar<S: PrecedentAuditSink>(
        &self,
        input: PrecedentRankingInput,
        audit: &S,
    ) -> Result<PrecedentRankingOutput, PrecedentEngineError> {
        let candidate_count = i32::try_from(input.candidates.len()).unwrap_or(i32::MAX);

        let decision = self.guardrails.evaluate(&input);
        let (applied_limit, applied_min_similarity, degraded_reason_code, degraded_user_message) =
            match decision {
                PrecedentGuardrailDecision::Allow { effective_limit, effective_min_similarity } => {
                    audit.record(PrecedentAuditEvent {
                        event_type: PrecedentAuditEventType::QueryAllowed,
                        quote_id: input.query.quote_id.clone(),
                        correlation_id: input.correlation_id.clone(),
                        reason_code: None,
                        user_message: None,
                        fallback_path: None,
                        candidate_count,
                        selected_count: 0,
                    });
                    (effective_limit, effective_min_similarity, None, None)
                }
                PrecedentGuardrailDecision::Deny { reason_code, user_message, fallback_path } => {
                    audit.record(PrecedentAuditEvent {
                        event_type: PrecedentAuditEventType::QueryDenied,
                        quote_id: input.query.quote_id.clone(),
                        correlation_id: input.correlation_id.clone(),
                        reason_code: Some(reason_code),
                        user_message: Some(user_message.clone()),
                        fallback_path: Some(fallback_path),
                        candidate_count,
                        selected_count: 0,
                    });

                    return Err(PrecedentEngineError::GuardrailDenied {
                        reason_code,
                        user_message,
                        fallback_path,
                    });
                }
                PrecedentGuardrailDecision::Degrade {
                    reason_code,
                    user_message,
                    fallback_path,
                    effective_limit,
                    effective_min_similarity,
                } => {
                    audit.record(PrecedentAuditEvent {
                        event_type: PrecedentAuditEventType::QueryDegraded,
                        quote_id: input.query.quote_id.clone(),
                        correlation_id: input.correlation_id.clone(),
                        reason_code: Some(reason_code),
                        user_message: Some(user_message.clone()),
                        fallback_path: Some(fallback_path),
                        candidate_count,
                        selected_count: 0,
                    });

                    (
                        effective_limit,
                        effective_min_similarity,
                        Some(reason_code),
                        Some(user_message),
                    )
                }
            };

        let mut filtered = Vec::new();
        for candidate in input.candidates {
            validate_candidate(&candidate).map_err(|error| {
                audit.record(PrecedentAuditEvent {
                    event_type: PrecedentAuditEventType::RankingFailed,
                    quote_id: input.query.quote_id.clone(),
                    correlation_id: input.correlation_id.clone(),
                    reason_code: Some(error.reason_code()),
                    user_message: Some(error.user_message()),
                    fallback_path: Some("inspect_precedent_data"),
                    candidate_count,
                    selected_count: 0,
                });
                error
            })?;

            if candidate.similarity_score >= applied_min_similarity {
                filtered.push(candidate);
            }
        }

        filtered.sort_by(|left, right| {
            right
                .similarity_score
                .partial_cmp(&left.similarity_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.candidate_quote_id.0.cmp(&right.candidate_quote_id.0))
                .then_with(|| left.candidate_fingerprint_id.cmp(&right.candidate_fingerprint_id))
        });

        if filtered.len() > applied_limit as usize {
            filtered.truncate(applied_limit as usize);
        }

        let results = filtered
            .into_iter()
            .map(|candidate| PrecedentResult {
                candidate_quote_id: candidate.candidate_quote_id,
                similarity_score: candidate.similarity_score,
                outcome_status: candidate.outcome_status,
                outcome_final_price: candidate.outcome_final_price,
                approval_decision_status: candidate.approval_decision_status,
                approval_route_version: candidate.approval_route_version,
                evidence: PrecedentEvidence {
                    source_fingerprint_id: input
                        .source_fingerprint_id
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    candidate_fingerprint_id: candidate.candidate_fingerprint_id,
                    strategy_version: candidate.strategy_version,
                    score_components_json: candidate.score_components_json,
                    evidence_payload_json: candidate.evidence_payload_json,
                },
            })
            .collect::<Vec<_>>();

        let selected_count = i32::try_from(results.len()).unwrap_or(i32::MAX);
        audit.record(PrecedentAuditEvent {
            event_type: PrecedentAuditEventType::RankingCompleted,
            quote_id: input.query.quote_id.clone(),
            correlation_id: input.correlation_id,
            reason_code: None,
            user_message: None,
            fallback_path: None,
            candidate_count,
            selected_count,
        });

        Ok(PrecedentRankingOutput {
            results,
            applied_limit,
            applied_min_similarity,
            degraded_reason_code,
            degraded_user_message,
        })
    }
}

fn validate_candidate(candidate: &PrecedentCandidate) -> Result<(), PrecedentEngineError> {
    if !candidate.similarity_score.is_finite() || !(0.0..=1.0).contains(&candidate.similarity_score)
    {
        return Err(PrecedentEngineError::InvalidSimilarityScore {
            candidate_quote_id: candidate.candidate_quote_id.0.clone(),
            similarity_score: candidate.similarity_score,
        });
    }

    if candidate.strategy_version.trim().is_empty() {
        return Err(PrecedentEngineError::MissingStrategyVersion {
            candidate_quote_id: candidate.candidate_quote_id.0.clone(),
        });
    }

    if !candidate.outcome_final_price.is_finite() || candidate.outcome_final_price < 0.0 {
        return Err(PrecedentEngineError::InvalidOutcomePrice {
            candidate_quote_id: candidate.candidate_quote_id.0.clone(),
            outcome_final_price: candidate.outcome_final_price,
        });
    }

    Ok(())
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PrecedentEngineError {
    #[error("precedent query denied by guardrail `{reason_code}`")]
    GuardrailDenied { reason_code: &'static str, user_message: String, fallback_path: &'static str },
    #[error("candidate `{candidate_quote_id}` has invalid similarity score `{similarity_score}`")]
    InvalidSimilarityScore { candidate_quote_id: String, similarity_score: f64 },
    #[error("candidate `{candidate_quote_id}` is missing strategy version")]
    MissingStrategyVersion { candidate_quote_id: String },
    #[error("candidate `{candidate_quote_id}` has invalid outcome price `{outcome_final_price}`")]
    InvalidOutcomePrice { candidate_quote_id: String, outcome_final_price: f64 },
}

impl PrecedentEngineError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::GuardrailDenied { reason_code, .. } => reason_code,
            Self::InvalidSimilarityScore { .. } => "invalid_similarity_score",
            Self::MissingStrategyVersion { .. } => "missing_strategy_version",
            Self::InvalidOutcomePrice { .. } => "invalid_outcome_price",
        }
    }

    pub fn user_message(&self) -> String {
        match self {
            Self::GuardrailDenied { user_message, .. } => user_message.clone(),
            Self::InvalidSimilarityScore { .. }
            | Self::MissingStrategyVersion { .. }
            | Self::InvalidOutcomePrice { .. } => {
                "I could not rank precedents due to invalid deterministic evidence data."
                    .to_string()
            }
        }
    }
}

#[derive(Default)]
pub struct InMemoryPrecedentAuditSink {
    events: std::sync::Mutex<Vec<PrecedentAuditEvent>>,
}

impl InMemoryPrecedentAuditSink {
    pub fn events(&self) -> Vec<PrecedentAuditEvent> {
        match self.events.lock() {
            Ok(events) => events.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl fmt::Debug for InMemoryPrecedentAuditSink {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InMemoryPrecedentAuditSink").finish_non_exhaustive()
    }
}

impl PrecedentAuditSink for InMemoryPrecedentAuditSink {
    fn record(&self, event: PrecedentAuditEvent) {
        match self.events.lock() {
            Ok(mut events) => events.push(event),
            Err(poisoned) => poisoned.into_inner().push(event),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cpq::precedent::{
        DeterministicPrecedentEngine, InMemoryPrecedentAuditSink, PrecedentCandidate,
        PrecedentEngineError, PrecedentGuardrailPolicy, PrecedentRankingInput,
    };
    use crate::domain::precedent::{
        PrecedentDecisionStatus, PrecedentOutcomeStatus, PrecedentQuery,
    };
    use crate::domain::quote::QuoteId;

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn allowed_flow_returns_ranked_precedents_with_audit_events() {
        let engine = DeterministicPrecedentEngine::default();
        let audit = InMemoryPrecedentAuditSink::default();

        let output = engine
            .rank_similar(
                PrecedentRankingInput {
                    query: PrecedentQuery {
                        quote_id: QuoteId("Q-PRE-SRC".to_string()),
                        customer_segment: None,
                        region: None,
                        product_family: None,
                        limit: 3,
                    },
                    min_similarity: 0.7,
                    source_fingerprint_id: Some("fp-src".to_string()),
                    candidates: vec![
                        candidate("Q-PRE-B", "fp-b", 0.82, 47_000.0),
                        candidate("Q-PRE-A", "fp-a", 0.91, 52_000.0),
                        candidate("Q-PRE-C", "fp-c", 0.64, 35_000.0),
                    ],
                    correlation_id: "corr-pre-allowed".to_string(),
                },
                &audit,
            )
            .expect("allowed flow should succeed");

        assert_eq!(output.results.len(), 2);
        assert_eq!(output.results[0].candidate_quote_id.0, "Q-PRE-A");
        assert_eq!(output.results[1].candidate_quote_id.0, "Q-PRE-B");
        assert!(output.degraded_reason_code.is_none());

        let events = audit.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type.as_str(), "query_allowed");
        assert_eq!(events[1].event_type.as_str(), "ranking_completed");
        assert_eq!(events[1].selected_count, 2);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn denied_flow_emits_user_safe_message_and_fallback_audit() {
        let engine = DeterministicPrecedentEngine::default();
        let audit = InMemoryPrecedentAuditSink::default();

        let error = engine
            .rank_similar(
                PrecedentRankingInput {
                    query: PrecedentQuery {
                        quote_id: QuoteId("Q-PRE-SRC".to_string()),
                        customer_segment: None,
                        region: None,
                        product_family: None,
                        limit: 5,
                    },
                    min_similarity: 0.8,
                    source_fingerprint_id: None,
                    candidates: vec![candidate("Q-PRE-A", "fp-a", 0.91, 52_000.0)],
                    correlation_id: "corr-pre-deny".to_string(),
                },
                &audit,
            )
            .expect_err("missing source fingerprint should deny");

        assert!(
            matches!(error, PrecedentEngineError::GuardrailDenied { .. }),
            "unexpected error: {error:?}"
        );
        if let PrecedentEngineError::GuardrailDenied { reason_code, user_message, fallback_path } =
            error
        {
            assert_eq!(reason_code, "missing_source_fingerprint");
            assert!(user_message.contains("not have enough precedent evidence"));
            assert_eq!(fallback_path, "capture_quote_fingerprint");
        }

        let events = audit.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type.as_str(), "query_denied");
        assert_eq!(events[0].reason_code, Some("missing_source_fingerprint"));
        assert_eq!(events[0].fallback_path, Some("capture_quote_fingerprint"));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn degraded_flow_clamps_query_bounds_and_records_degraded_audit() {
        let engine = DeterministicPrecedentEngine::new(PrecedentGuardrailPolicy {
            max_limit: 4,
            min_similarity_floor: 0.75,
        });
        let audit = InMemoryPrecedentAuditSink::default();

        let output = engine
            .rank_similar(
                PrecedentRankingInput {
                    query: PrecedentQuery {
                        quote_id: QuoteId("Q-PRE-SRC".to_string()),
                        customer_segment: None,
                        region: None,
                        product_family: None,
                        limit: 25,
                    },
                    min_similarity: 0.2,
                    source_fingerprint_id: Some("fp-src".to_string()),
                    candidates: vec![
                        candidate("Q-PRE-A", "fp-a", 0.91, 52_000.0),
                        candidate("Q-PRE-B", "fp-b", 0.72, 47_000.0),
                    ],
                    correlation_id: "corr-pre-degrade".to_string(),
                },
                &audit,
            )
            .expect("degraded flow should still succeed");

        assert_eq!(output.applied_limit, 4);
        assert!((output.applied_min_similarity - 0.75).abs() < f64::EPSILON);
        assert_eq!(output.results.len(), 1);
        assert_eq!(output.degraded_reason_code, Some("query_bounds_adjusted"));
        assert!(output
            .degraded_user_message
            .as_deref()
            .is_some_and(|msg| msg.contains("adjusted the precedent query")));

        let events = audit.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type.as_str(), "query_degraded");
        assert_eq!(events[1].event_type.as_str(), "ranking_completed");
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn non_finite_min_similarity_is_degraded_to_guardrail_floor() {
        let engine = DeterministicPrecedentEngine::new(PrecedentGuardrailPolicy {
            max_limit: 5,
            min_similarity_floor: 0.75,
        });
        let audit = InMemoryPrecedentAuditSink::default();

        let output = engine
            .rank_similar(
                PrecedentRankingInput {
                    query: PrecedentQuery {
                        quote_id: QuoteId("Q-PRE-SRC".to_string()),
                        customer_segment: None,
                        region: None,
                        product_family: None,
                        limit: 3,
                    },
                    min_similarity: f64::NAN,
                    source_fingerprint_id: Some("fp-src".to_string()),
                    candidates: vec![
                        candidate("Q-PRE-A", "fp-a", 0.91, 52_000.0),
                        candidate("Q-PRE-B", "fp-b", 0.72, 47_000.0),
                    ],
                    correlation_id: "corr-pre-nan-floor".to_string(),
                },
                &audit,
            )
            .expect("non-finite similarity should degrade and still succeed");

        assert_eq!(output.applied_limit, 3);
        assert!((output.applied_min_similarity - 0.75).abs() < f64::EPSILON);
        assert_eq!(output.results.len(), 1);
        assert_eq!(output.degraded_reason_code, Some("query_bounds_adjusted"));

        let events = audit.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type.as_str(), "query_degraded");
        assert_eq!(events[1].event_type.as_str(), "ranking_completed");
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn invalid_candidate_similarity_is_classified_and_audited_as_failure() {
        let engine = DeterministicPrecedentEngine::default();
        let audit = InMemoryPrecedentAuditSink::default();

        let error = engine
            .rank_similar(
                PrecedentRankingInput {
                    query: PrecedentQuery {
                        quote_id: QuoteId("Q-PRE-SRC".to_string()),
                        customer_segment: None,
                        region: None,
                        product_family: None,
                        limit: 5,
                    },
                    min_similarity: 0.7,
                    source_fingerprint_id: Some("fp-src".to_string()),
                    candidates: vec![candidate("Q-PRE-A", "fp-a", 1.2, 52_000.0)],
                    correlation_id: "corr-pre-invalid".to_string(),
                },
                &audit,
            )
            .expect_err("invalid candidate similarity should fail");

        assert!(
            matches!(error, PrecedentEngineError::InvalidSimilarityScore { .. }),
            "unexpected error: {error:?}"
        );
        if let PrecedentEngineError::InvalidSimilarityScore {
            candidate_quote_id,
            similarity_score,
        } = error
        {
            assert_eq!(candidate_quote_id, "Q-PRE-A");
            assert!((similarity_score - 1.2).abs() < f64::EPSILON);
        }

        let events = audit.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type.as_str(), "query_allowed");
        assert_eq!(events[1].event_type.as_str(), "ranking_failed");
        assert_eq!(events[1].reason_code, Some("invalid_similarity_score"));
        assert_eq!(events[1].fallback_path, Some("inspect_precedent_data"));
    }

    fn candidate(
        quote_id: &str,
        fingerprint_id: &str,
        similarity_score: f64,
        outcome_final_price: f64,
    ) -> PrecedentCandidate {
        PrecedentCandidate {
            candidate_quote_id: QuoteId(quote_id.to_string()),
            candidate_fingerprint_id: fingerprint_id.to_string(),
            similarity_score,
            outcome_status: PrecedentOutcomeStatus::Won,
            outcome_final_price,
            approval_decision_status: Some(PrecedentDecisionStatus::Approved),
            approval_route_version: Some(2),
            strategy_version: "simhash-v1".to_string(),
            score_components_json: "{\"hamming_distance\":11}".to_string(),
            evidence_payload_json: "{\"normalization\":\"v1\"}".to_string(),
        }
    }
}
