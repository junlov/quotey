use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::autopsy::{
    AttributionEdge, AttributionEdgeId, AttributionNode, AttributionNodeId, AttributionScore,
    AttributionScoreId, AuditRefType, CounterfactualSimulation, CounterfactualSimulationId,
    DealAutopsy, DealAutopsyId, DealOutcomeType, DecisionFork, DecisionForkId, DecisionForkType,
    DecisionStage, GenomeQueryId, GenomeQueryType, ProjectedOutcomeStatus,
};
use crate::domain::quote::QuoteId;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutopsyError {
    #[error("quote id cannot be empty")]
    EmptyQuoteId,
    #[error("audit trail entry id cannot be empty at index {index}")]
    EmptyAuditEntryId { index: usize },
    #[error("autopsy requires at least one audit trail reference")]
    EmptyAuditTrail,
    #[error("decision fork extraction found no decision points")]
    NoDecisionForks,
    #[error("duplicate fork key detected: {key}")]
    DuplicateForkKey { key: String },
    #[error("attribution score sum exceeds 10000 bps: {total}")]
    AttributionOverflow { total: i32 },
    #[error("idempotency key cannot be empty")]
    EmptyIdempotencyKey,
    #[error("checksum mismatch: expected {expected}, actual {actual}")]
    ChecksumMismatch { expected: String, actual: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GenomeQueryError {
    #[error("insufficient data: need at least {required} samples, have {available}")]
    InsufficientData { required: i32, available: i32 },
    #[error("no matching nodes found for query filters")]
    NoMatchingNodes,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CounterfactualError {
    #[error("original autopsy not found: {0}")]
    AutopsyNotFound(String),
    #[error("no alternative decisions provided")]
    NoAlternatives,
    #[error("fork {fork_id} not found in autopsy {autopsy_id}")]
    ForkNotFound { fork_id: String, autopsy_id: String },
}

// ---------------------------------------------------------------------------
// Autopsy input / output contracts
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditTrailEntry {
    pub entry_id: String,
    pub entry_type: AuditRefType,
    pub stage: DecisionStage,
    pub action_summary: String,
    pub decision_data_json: String,
    pub alternatives_json: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopsyInput {
    pub quote_id: String,
    pub outcome_status: DealOutcomeType,
    pub outcome_value_bps: i32,
    pub outcome_revenue_cents: i64,
    pub audit_trail: Vec<AuditTrailEntry>,
    pub segment_key: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopsyReport {
    pub autopsy: DealAutopsy,
    pub forks: Vec<DecisionFork>,
    pub scores: Vec<AttributionScore>,
    pub checksum: String,
}

// ---------------------------------------------------------------------------
// Deal Autopsy Engine
// ---------------------------------------------------------------------------

pub struct DealAutopsyEngine {
    autopsy_version: String,
}

impl Default for DealAutopsyEngine {
    fn default() -> Self {
        Self::new("rgn_autopsy.v1".to_string())
    }
}

impl DealAutopsyEngine {
    pub fn new(autopsy_version: String) -> Self {
        Self { autopsy_version }
    }

    pub fn perform(&self, input: AutopsyInput) -> Result<AutopsyReport, AutopsyError> {
        validate_input(&input)?;

        let autopsy_id = DealAutopsyId(format!(
            "autopsy-{}",
            deterministic_id(&input.quote_id, &input.idempotency_key)
        ));
        let forks = extract_decision_forks(&autopsy_id, &input.audit_trail)?;
        let scores = score_attribution(&autopsy_id, &forks, &input)?;
        let checksum = compute_autopsy_checksum(&input, &forks, &scores);

        let now = Utc::now();
        let audit_refs: Vec<&str> = input.audit_trail.iter().map(|e| e.entry_id.as_str()).collect();
        let autopsy = DealAutopsy {
            id: autopsy_id,
            quote_id: QuoteId(input.quote_id),
            outcome_status: input.outcome_status,
            outcome_value_bps: input.outcome_value_bps,
            outcome_revenue_cents: input.outcome_revenue_cents,
            decision_fork_count: forks.len() as i32,
            attribution_checksum: checksum.clone(),
            audit_trail_refs_json: serde_json::to_string(&audit_refs).unwrap_or_default(),
            autopsy_version: self.autopsy_version.clone(),
            idempotency_key: input.idempotency_key,
            created_at: now,
            updated_at: now,
        };

        Ok(AutopsyReport { autopsy, forks, scores, checksum })
    }
}

// ---------------------------------------------------------------------------
// Fork extraction
// ---------------------------------------------------------------------------

fn extract_decision_forks(
    autopsy_id: &DealAutopsyId,
    trail: &[AuditTrailEntry],
) -> Result<Vec<DecisionFork>, AutopsyError> {
    let mut seen_keys = BTreeSet::new();
    let mut forks = Vec::with_capacity(trail.len());

    for (idx, entry) in trail.iter().enumerate() {
        let fork_type = classify_fork_type(&entry.entry_type, &entry.decision_data_json);
        let fork_id = DecisionForkId(format!("fork-{}-{}", autopsy_id.0, idx));

        let dedupe_key = entry.entry_id.trim().to_string();
        if dedupe_key.is_empty() {
            return Err(AutopsyError::EmptyAuditEntryId { index: idx });
        }
        if !seen_keys.insert(dedupe_key.clone()) {
            return Err(AutopsyError::DuplicateForkKey { key: dedupe_key });
        }

        forks.push(DecisionFork {
            id: fork_id,
            autopsy_id: autopsy_id.clone(),
            fork_type,
            stage: entry.stage.clone(),
            option_chosen_json: entry.decision_data_json.clone(),
            options_considered_json: entry.alternatives_json.clone(),
            audit_ref: entry.entry_id.clone(),
            audit_ref_type: entry.entry_type.clone(),
            sequence_order: idx as i32,
            created_at: entry.timestamp,
        });
    }

    if forks.is_empty() {
        return Err(AutopsyError::NoDecisionForks);
    }
    Ok(forks)
}

fn classify_fork_type(audit_type: &AuditRefType, decision_data: &str) -> DecisionForkType {
    match audit_type {
        AuditRefType::PricingTrace => {
            let lower = decision_data.to_ascii_lowercase();
            if lower.contains("discount") {
                DecisionForkType::DiscountLevel
            } else if lower.contains("bundle") {
                DecisionForkType::BundleChoice
            } else {
                DecisionForkType::PricingPath
            }
        }
        AuditRefType::LedgerEntry => {
            let lower = decision_data.to_ascii_lowercase();
            if lower.contains("product") || lower.contains("sku") {
                DecisionForkType::ProductSelection
            } else if lower.contains("term") || lower.contains("month") {
                DecisionForkType::TermSelection
            } else {
                DecisionForkType::PricingPath
            }
        }
        AuditRefType::AuditEvent => DecisionForkType::ConstraintResolution,
        AuditRefType::NegotiationTurn => DecisionForkType::NegotiationConcession,
        AuditRefType::ApprovalDecision => DecisionForkType::ApprovalException,
    }
}

// ---------------------------------------------------------------------------
// Attribution scoring
// ---------------------------------------------------------------------------

fn score_attribution(
    autopsy_id: &DealAutopsyId,
    forks: &[DecisionFork],
    input: &AutopsyInput,
) -> Result<Vec<AttributionScore>, AutopsyError> {
    if forks.is_empty() {
        return Err(AutopsyError::NoDecisionForks);
    }

    let raw_weights: Vec<i64> = forks
        .iter()
        .map(|f| stage_weight(&f.stage) as i64 * fork_type_weight(&f.fork_type) as i64)
        .collect();

    let total_weight: i64 = raw_weights.iter().sum();

    let contributions = if total_weight == 0 {
        let even = 10_000i32 / forks.len() as i32;
        let remainder = 10_000i32 - even * forks.len() as i32;
        forks
            .iter()
            .enumerate()
            .map(|(i, _)| if i == 0 { even + remainder } else { even })
            .collect::<Vec<_>>()
    } else {
        let mut c: Vec<i32> =
            raw_weights.iter().map(|w| ((w * 10_000) / total_weight) as i32).collect();
        let sum: i32 = c.iter().sum();
        let remainder = 10_000 - sum;
        if remainder != 0 {
            if let Some(max_idx) = c.iter().enumerate().max_by_key(|(_, v)| **v).map(|(i, _)| i) {
                c[max_idx] += remainder;
            }
        }
        c
    };

    let scores: Vec<AttributionScore> = forks
        .iter()
        .enumerate()
        .map(|(idx, fork)| build_score(autopsy_id, fork, contributions[idx], input))
        .collect();

    let total: i32 = scores.iter().map(|s| s.outcome_contribution_bps).sum();
    if total != 10_000 {
        return Err(AutopsyError::AttributionOverflow { total });
    }

    Ok(scores)
}

fn build_score(
    autopsy_id: &DealAutopsyId,
    fork: &DecisionFork,
    contribution_bps: i32,
    _input: &AutopsyInput,
) -> AttributionScore {
    AttributionScore {
        id: AttributionScoreId(format!("attr-{}-{}", autopsy_id.0, fork.id.0)),
        autopsy_id: autopsy_id.clone(),
        fork_id: fork.id.clone(),
        outcome_contribution_bps: contribution_bps,
        confidence_bps: compute_confidence(fork),
        evidence_count: 1,
        evidence_refs_json: format!("[\"{}\"]", fork.audit_ref),
        attribution_method: "deterministic_trace".to_string(),
        created_at: Utc::now(),
    }
}

fn stage_weight(stage: &DecisionStage) -> i32 {
    match stage {
        DecisionStage::Configuration => 10,
        DecisionStage::Pricing => 30,
        DecisionStage::Policy => 20,
        DecisionStage::Approval => 25,
        DecisionStage::Negotiation => 35,
        DecisionStage::Finalization => 15,
    }
}

fn fork_type_weight(fork_type: &DecisionForkType) -> i32 {
    match fork_type {
        DecisionForkType::PricingPath => 25,
        DecisionForkType::DiscountLevel => 35,
        DecisionForkType::ConstraintResolution => 15,
        DecisionForkType::ApprovalException => 20,
        DecisionForkType::NegotiationConcession => 30,
        DecisionForkType::ProductSelection => 20,
        DecisionForkType::TermSelection => 15,
        DecisionForkType::BundleChoice => 20,
    }
}

fn compute_confidence(fork: &DecisionFork) -> i32 {
    let base = 7_000;
    let alternatives_bonus =
        if fork.options_considered_json != "[]" && !fork.options_considered_json.is_empty() {
            1_500
        } else {
            0
        };
    let stage_bonus = match fork.stage {
        DecisionStage::Negotiation | DecisionStage::Approval => 1_500,
        DecisionStage::Pricing | DecisionStage::Policy => 1_000,
        _ => 0,
    };
    std::cmp::min(base + alternatives_bonus + stage_bonus, 10_000)
}

// ---------------------------------------------------------------------------
// Attribution Graph Builder
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttributionGraphSnapshot {
    pub nodes: Vec<AttributionNode>,
    pub edges: Vec<AttributionEdge>,
    pub total_autopsies: i32,
    pub checksum: String,
}

pub struct AttributionGraphBuilder;

impl Default for AttributionGraphBuilder {
    fn default() -> Self {
        Self
    }
}

impl AttributionGraphBuilder {
    pub fn build_from_reports(&self, reports: &[AutopsyReport]) -> AttributionGraphSnapshot {
        let mut node_map: BTreeMap<String, AttributionNode> = BTreeMap::new();
        let mut edge_map: BTreeMap<String, AttributionEdge> = BTreeMap::new();
        let now = Utc::now();

        for report in reports {
            let fork_keys: Vec<String> = report
                .forks
                .iter()
                .map(|f| {
                    format!(
                        "{}|{}|{}",
                        f.fork_type.as_str(),
                        f.stage.as_str(),
                        option_value_hash(&f.option_chosen_json),
                    )
                })
                .collect();

            for fork in &report.forks {
                let node_key = format!(
                    "{}|{}|{}",
                    fork.fork_type.as_str(),
                    fork.stage.as_str(),
                    option_value_hash(&fork.option_chosen_json),
                );

                let node = node_map.entry(node_key).or_insert_with(|| AttributionNode {
                    id: AttributionNodeId(format!(
                        "node-{}",
                        deterministic_hash(&format!(
                            "{}|{}|{}",
                            fork.fork_type.as_str(),
                            fork.stage.as_str(),
                            option_value_hash(&fork.option_chosen_json),
                        ))
                    )),
                    fork_type: fork.fork_type.clone(),
                    stage: fork.stage.clone(),
                    segment_key: "all".to_string(),
                    option_value_hash: option_value_hash(&fork.option_chosen_json),
                    option_value_summary: summarize_option(&fork.option_chosen_json),
                    sample_count: 0,
                    first_seen_at: now,
                    last_updated_at: now,
                });
                node.sample_count += 1;
                node.last_updated_at = now;
            }

            for window in fork_keys.windows(2) {
                let source_key = &window[0];
                let target_key = &window[1];
                let edge_key = format!("{}->{}", source_key, target_key);

                let is_won = report.autopsy.outcome_status == DealOutcomeType::Won;
                let win_increment: i64 = if is_won { 1 } else { 0 };

                let source_node_id = node_map
                    .get(source_key)
                    .map(|n| n.id.clone())
                    .unwrap_or_else(|| AttributionNodeId("unknown".to_string()));
                let target_node_id = node_map
                    .get(target_key)
                    .map(|n| n.id.clone())
                    .unwrap_or_else(|| AttributionNodeId("unknown".to_string()));

                let edge = edge_map.entry(edge_key.clone()).or_insert_with(|| AttributionEdge {
                    id: AttributionEdgeId(format!("edge-{}", deterministic_hash(&edge_key))),
                    source_node_id: source_node_id.clone(),
                    target_node_id: target_node_id.clone(),
                    outcome_weight_bps: 0,
                    sample_count: 0,
                    win_rate_bps: 0,
                    avg_margin_delta_bps: 0,
                    avg_revenue_cents: 0,
                    first_seen_at: now,
                    last_updated_at: now,
                });

                let old_count = edge.sample_count as i64;
                let new_count = old_count + 1;
                edge.sample_count = new_count as i32;

                let old_wins = (edge.win_rate_bps as i64 * old_count) / 10_000;
                let new_wins = old_wins + win_increment;
                edge.win_rate_bps =
                    if new_count > 0 { ((new_wins * 10_000) / new_count) as i32 } else { 0 };

                edge.avg_margin_delta_bps = ((edge.avg_margin_delta_bps as i64 * old_count
                    + report.autopsy.outcome_value_bps as i64)
                    / new_count) as i32;

                edge.avg_revenue_cents = (edge.avg_revenue_cents * old_count
                    + report.autopsy.outcome_revenue_cents)
                    / new_count;

                edge.outcome_weight_bps = edge.win_rate_bps;
                edge.last_updated_at = now;
            }
        }

        let nodes: Vec<_> = node_map.into_values().collect();
        let edges: Vec<_> = edge_map.into_values().collect();
        let checksum = compute_graph_checksum(&nodes, &edges);

        AttributionGraphSnapshot { nodes, edges, total_autopsies: reports.len() as i32, checksum }
    }
}

// ---------------------------------------------------------------------------
// Revenue Genome Query Engine
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomeQueryRequest {
    pub query_type: GenomeQueryType,
    pub segment_filter: Option<String>,
    pub fork_type_filter: Option<DecisionForkType>,
    pub stage_filter: Option<DecisionStage>,
    pub min_sample_count: i32,
    pub time_window_days: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomeFinding {
    pub finding_id: String,
    pub description: String,
    pub evidence_count: i32,
    pub confidence_bps: i32,
    pub win_rate_bps: i32,
    pub avg_margin_bps: i32,
    pub recommendation: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenomeQueryResponse {
    pub query_id: GenomeQueryId,
    pub query_type: GenomeQueryType,
    pub segments_analyzed: i32,
    pub evidence_count: i32,
    pub findings: Vec<GenomeFinding>,
    pub result_checksum: String,
    pub query_duration_ms: i64,
}

pub struct RevenueGenomeQueryEngine;

impl Default for RevenueGenomeQueryEngine {
    fn default() -> Self {
        Self
    }
}

impl RevenueGenomeQueryEngine {
    pub fn query(
        &self,
        request: &GenomeQueryRequest,
        graph: &AttributionGraphSnapshot,
    ) -> Result<GenomeQueryResponse, GenomeQueryError> {
        let start = std::time::Instant::now();
        let query_id = GenomeQueryId(uuid::Uuid::new_v4().to_string());

        let filtered_nodes: Vec<&AttributionNode> = graph
            .nodes
            .iter()
            .filter(|n| {
                let type_match =
                    request.fork_type_filter.as_ref().map_or(true, |ft| n.fork_type == *ft);
                let stage_match = request.stage_filter.as_ref().map_or(true, |s| n.stage == *s);
                let segment_match =
                    request.segment_filter.as_ref().map_or(true, |seg| n.segment_key == *seg);
                let sample_match = n.sample_count >= request.min_sample_count;
                type_match && stage_match && segment_match && sample_match
            })
            .collect();

        if filtered_nodes.is_empty() {
            return Err(GenomeQueryError::NoMatchingNodes);
        }

        let node_ids: BTreeSet<&str> = filtered_nodes.iter().map(|n| n.id.0.as_str()).collect();

        let relevant_edges: Vec<&AttributionEdge> = graph
            .edges
            .iter()
            .filter(|e| {
                node_ids.contains(e.source_node_id.0.as_str())
                    || node_ids.contains(e.target_node_id.0.as_str())
            })
            .collect();

        let mut findings: Vec<GenomeFinding> = filtered_nodes
            .iter()
            .map(|node| {
                let node_edges: Vec<&&AttributionEdge> =
                    relevant_edges.iter().filter(|e| e.source_node_id == node.id).collect();

                let avg_win_rate = if node_edges.is_empty() {
                    5_000
                } else {
                    let sum: i64 = node_edges.iter().map(|e| e.win_rate_bps as i64).sum();
                    (sum / node_edges.len() as i64) as i32
                };

                let avg_margin = if node_edges.is_empty() {
                    0
                } else {
                    let sum: i64 = node_edges.iter().map(|e| e.avg_margin_delta_bps as i64).sum();
                    (sum / node_edges.len() as i64) as i32
                };

                let recommendation = generate_recommendation(node, avg_win_rate, avg_margin);

                GenomeFinding {
                    finding_id: format!("finding-{}", deterministic_hash(&node.id.0)),
                    description: format!(
                        "{} at {} stage: {} (n={})",
                        node.fork_type.as_str(),
                        node.stage.as_str(),
                        node.option_value_summary,
                        node.sample_count,
                    ),
                    evidence_count: node.sample_count,
                    confidence_bps: std::cmp::min(node.sample_count * 500, 10_000),
                    win_rate_bps: avg_win_rate,
                    avg_margin_bps: avg_margin,
                    recommendation,
                }
            })
            .collect();

        findings.sort_by(|a, b| {
            b.evidence_count.cmp(&a.evidence_count).then_with(|| a.finding_id.cmp(&b.finding_id))
        });

        let evidence_total: i32 = findings.iter().map(|f| f.evidence_count).sum();
        let result_checksum = compute_findings_checksum(&findings);
        let elapsed_ms = start.elapsed().as_millis() as i64;

        Ok(GenomeQueryResponse {
            query_id,
            query_type: request.query_type.clone(),
            segments_analyzed: filtered_nodes.len() as i32,
            evidence_count: evidence_total,
            findings,
            result_checksum,
            query_duration_ms: elapsed_ms,
        })
    }
}

fn generate_recommendation(
    node: &AttributionNode,
    win_rate_bps: i32,
    avg_margin_bps: i32,
) -> Option<String> {
    if node.sample_count < 3 {
        return None;
    }

    if win_rate_bps >= 7_000 && avg_margin_bps >= 2_000 {
        Some(format!(
            "Strong pattern: {} at {} stage shows {:.1}% win rate with {:.1}% margin. Consider making this the default strategy.",
            node.fork_type.as_str(),
            node.stage.as_str(),
            win_rate_bps as f64 / 100.0,
            avg_margin_bps as f64 / 100.0
        ))
    } else if win_rate_bps >= 7_000 && avg_margin_bps < 1_000 {
        Some(format!(
            "High win rate ({:.1}%) but low margin ({:.1}%). Evaluate whether volume justifies margin trade-off.",
            win_rate_bps as f64 / 100.0,
            avg_margin_bps as f64 / 100.0
        ))
    } else if win_rate_bps < 3_000 {
        Some(format!(
            "Low win rate ({:.1}%). Consider alternative approaches at this decision point.",
            win_rate_bps as f64 / 100.0
        ))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Counterfactual Simulation Engine
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualRequest {
    pub original_quote_id: String,
    pub original_autopsy_id: DealAutopsyId,
    pub alternative_decisions: Vec<AlternativeDecision>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlternativeDecision {
    pub fork_id: DecisionForkId,
    pub alternative_option_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterfactualResponse {
    pub simulation: CounterfactualSimulation,
    pub comparison: Vec<ForkComparison>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForkComparison {
    pub fork_id: DecisionForkId,
    pub original_option: String,
    pub alternative_option: String,
    pub projected_impact_bps: i32,
}

pub struct CounterfactualEngine;

impl Default for CounterfactualEngine {
    fn default() -> Self {
        Self
    }
}

impl CounterfactualEngine {
    pub fn simulate(
        &self,
        request: &CounterfactualRequest,
        original_report: &AutopsyReport,
        graph: &AttributionGraphSnapshot,
    ) -> Result<CounterfactualResponse, CounterfactualError> {
        if request.alternative_decisions.is_empty() {
            return Err(CounterfactualError::NoAlternatives);
        }

        let mut comparisons = Vec::new();
        let mut total_projected_delta_bps: i64 = 0;

        for alt_decision in &request.alternative_decisions {
            let original_fork =
                original_report.forks.iter().find(|f| f.id == alt_decision.fork_id).ok_or_else(
                    || CounterfactualError::ForkNotFound {
                        fork_id: alt_decision.fork_id.0.clone(),
                        autopsy_id: request.original_autopsy_id.0.clone(),
                    },
                )?;

            let alt_hash = option_value_hash(&alt_decision.alternative_option_json);
            let alt_node = graph.nodes.iter().find(|n| {
                n.fork_type == original_fork.fork_type
                    && n.stage == original_fork.stage
                    && n.option_value_hash == alt_hash
            });

            let orig_hash = option_value_hash(&original_fork.option_chosen_json);
            let orig_node = graph.nodes.iter().find(|n| {
                n.fork_type == original_fork.fork_type
                    && n.stage == original_fork.stage
                    && n.option_value_hash == orig_hash
            });

            let alt_edges: Vec<&AttributionEdge> = alt_node
                .map(|n| graph.edges.iter().filter(|e| e.source_node_id == n.id).collect())
                .unwrap_or_default();

            let orig_edges: Vec<&AttributionEdge> = orig_node
                .map(|n| graph.edges.iter().filter(|e| e.source_node_id == n.id).collect())
                .unwrap_or_default();

            let alt_win_rate = if alt_edges.is_empty() {
                5_000
            } else {
                (alt_edges.iter().map(|e| e.win_rate_bps as i64).sum::<i64>()
                    / alt_edges.len() as i64) as i32
            };

            let orig_win_rate = if orig_edges.is_empty() {
                5_000
            } else {
                (orig_edges.iter().map(|e| e.win_rate_bps as i64).sum::<i64>()
                    / orig_edges.len() as i64) as i32
            };

            let projected_impact = alt_win_rate - orig_win_rate;
            total_projected_delta_bps += projected_impact as i64;

            comparisons.push(ForkComparison {
                fork_id: alt_decision.fork_id.clone(),
                original_option: original_fork.option_chosen_json.clone(),
                alternative_option: alt_decision.alternative_option_json.clone(),
                projected_impact_bps: projected_impact,
            });
        }

        let replay_checksum = compute_counterfactual_checksum(request, &comparisons);
        let projected_outcome = if total_projected_delta_bps > 1_000 {
            ProjectedOutcomeStatus::Won
        } else if total_projected_delta_bps < -1_000 {
            ProjectedOutcomeStatus::Lost
        } else {
            ProjectedOutcomeStatus::Unknown
        };
        let confidence = compute_counterfactual_confidence(&comparisons, graph);

        let simulation = CounterfactualSimulation {
            id: CounterfactualSimulationId(uuid::Uuid::new_v4().to_string()),
            original_quote_id: QuoteId(request.original_quote_id.clone()),
            original_autopsy_id: request.original_autopsy_id.clone(),
            alternative_decisions_json: serde_json::to_string(&request.alternative_decisions)
                .unwrap_or_default(),
            replay_checksum,
            projected_outcome_status: projected_outcome,
            projected_margin_delta_bps: total_projected_delta_bps as i32,
            projected_revenue_delta_cents: 0,
            delta_vs_actual_json: serde_json::to_string(&comparisons).unwrap_or_default(),
            evidence_chain_json: "[]".to_string(),
            confidence_bps: confidence,
            simulated_at: Utc::now(),
        };

        Ok(CounterfactualResponse { simulation, comparison: comparisons })
    }
}

fn compute_counterfactual_confidence(
    comparisons: &[ForkComparison],
    graph: &AttributionGraphSnapshot,
) -> i32 {
    if comparisons.is_empty() || graph.total_autopsies < 5 {
        return 2_000;
    }
    let data_confidence = std::cmp::min(graph.total_autopsies * 200, 6_000);
    let has_data_count = comparisons.iter().filter(|c| c.projected_impact_bps != 0).count();
    let data_ratio = if comparisons.is_empty() {
        0
    } else {
        (has_data_count as i32 * 4_000) / comparisons.len() as i32
    };
    std::cmp::min(data_confidence + data_ratio, 10_000)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn validate_input(input: &AutopsyInput) -> Result<(), AutopsyError> {
    if input.quote_id.trim().is_empty() {
        return Err(AutopsyError::EmptyQuoteId);
    }
    if input.idempotency_key.trim().is_empty() {
        return Err(AutopsyError::EmptyIdempotencyKey);
    }
    if input.audit_trail.is_empty() {
        return Err(AutopsyError::EmptyAuditTrail);
    }
    Ok(())
}

fn deterministic_id(quote_id: &str, idempotency_key: &str) -> String {
    deterministic_hash(&format!("{}|{}", quote_id, idempotency_key))
}

fn deterministic_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}

fn option_value_hash(json: &str) -> String {
    deterministic_hash(&canonicalize_json_str(json))
}

fn canonicalize_json_str(json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(json)
        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| json.to_string()))
        .unwrap_or_else(|_| json.to_string())
}

fn summarize_option(json: &str) -> String {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
        match &value {
            serde_json::Value::String(s) => return s.clone(),
            serde_json::Value::Object(map) => {
                let parts: Vec<String> =
                    map.iter().take(3).map(|(k, v)| format!("{}={}", k, v)).collect();
                return parts.join(", ");
            }
            _ => return value.to_string(),
        }
    }
    json.chars().take(50).collect()
}

fn compute_autopsy_checksum(
    input: &AutopsyInput,
    forks: &[DecisionFork],
    scores: &[AttributionScore],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.quote_id.as_bytes());
    hasher.update(input.outcome_status.as_str().as_bytes());
    hasher.update(input.outcome_value_bps.to_le_bytes());
    for fork in forks {
        hasher.update(fork.id.0.as_bytes());
        hasher.update(fork.fork_type.as_str().as_bytes());
    }
    for score in scores {
        hasher.update(score.outcome_contribution_bps.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn compute_graph_checksum(nodes: &[AttributionNode], edges: &[AttributionEdge]) -> String {
    let mut hasher = Sha256::new();
    for node in nodes {
        hasher.update(node.id.0.as_bytes());
        hasher.update(node.sample_count.to_le_bytes());
    }
    for edge in edges {
        hasher.update(edge.id.0.as_bytes());
        hasher.update(edge.sample_count.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn compute_findings_checksum(findings: &[GenomeFinding]) -> String {
    let mut hasher = Sha256::new();
    for finding in findings {
        hasher.update(finding.finding_id.as_bytes());
        hasher.update(finding.evidence_count.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn compute_counterfactual_checksum(
    request: &CounterfactualRequest,
    comparisons: &[ForkComparison],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(request.original_quote_id.as_bytes());
    hasher.update(request.original_autopsy_id.0.as_bytes());
    for comp in comparisons {
        hasher.update(comp.fork_id.0.as_bytes());
        hasher.update(comp.projected_impact_bps.to_le_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn pricing_entry(id: &str) -> AuditTrailEntry {
        AuditTrailEntry {
            entry_id: id.to_string(),
            entry_type: AuditRefType::PricingTrace,
            stage: DecisionStage::Pricing,
            action_summary: "Price book selection".to_string(),
            decision_data_json: r#"{"price_book":"enterprise_us","unit_price":800}"#.to_string(),
            alternatives_json: r#"[{"price_book":"standard_us","unit_price":1000}]"#.to_string(),
            timestamp: Utc::now(),
        }
    }

    fn discount_entry(id: &str) -> AuditTrailEntry {
        AuditTrailEntry {
            entry_id: id.to_string(),
            entry_type: AuditRefType::PricingTrace,
            stage: DecisionStage::Pricing,
            action_summary: "Discount applied".to_string(),
            decision_data_json: r#"{"discount_pct":10,"type":"volume"}"#.to_string(),
            alternatives_json: r#"[{"discount_pct":5},{"discount_pct":15}]"#.to_string(),
            timestamp: Utc::now(),
        }
    }

    fn negotiation_entry(id: &str) -> AuditTrailEntry {
        AuditTrailEntry {
            entry_id: id.to_string(),
            entry_type: AuditRefType::NegotiationTurn,
            stage: DecisionStage::Negotiation,
            action_summary: "Counteroffer accepted".to_string(),
            decision_data_json: r#"{"concession":"term_extension","months":24}"#.to_string(),
            alternatives_json: r#"[{"concession":"price_reduction"},{"concession":"bundle_add"}]"#
                .to_string(),
            timestamp: Utc::now(),
        }
    }

    #[allow(dead_code)]
    fn approval_entry(id: &str) -> AuditTrailEntry {
        AuditTrailEntry {
            entry_id: id.to_string(),
            entry_type: AuditRefType::ApprovalDecision,
            stage: DecisionStage::Approval,
            action_summary: "Discount exception approved".to_string(),
            decision_data_json: r#"{"approved":true,"approver":"sales_director"}"#.to_string(),
            alternatives_json: "[]".to_string(),
            timestamp: Utc::now(),
        }
    }

    fn test_input() -> AutopsyInput {
        AutopsyInput {
            quote_id: "Q-2026-0042".to_string(),
            outcome_status: DealOutcomeType::Won,
            outcome_value_bps: 2500,
            outcome_revenue_cents: 1_296_000,
            audit_trail: vec![
                pricing_entry("audit-001"),
                discount_entry("audit-002"),
                negotiation_entry("audit-003"),
            ],
            segment_key: "enterprise".to_string(),
            idempotency_key: "idem-q42-v1".to_string(),
        }
    }

    // === Autopsy Engine Tests ===

    #[test]
    fn autopsy_engine_performs_complete_autopsy() {
        let engine = DealAutopsyEngine::default();
        let report = engine.perform(test_input()).unwrap();

        assert!(!report.autopsy.id.0.is_empty());
        assert_eq!(report.autopsy.quote_id, QuoteId("Q-2026-0042".to_string()));
        assert_eq!(report.autopsy.outcome_status, DealOutcomeType::Won);
        assert_eq!(report.forks.len(), 3);
        assert_eq!(report.scores.len(), 3);
        assert!(!report.checksum.is_empty());
    }

    #[test]
    fn autopsy_is_deterministic_for_same_input() {
        let engine = DealAutopsyEngine::default();
        let input = test_input();
        let report1 = engine.perform(input.clone()).unwrap();
        let report2 = engine.perform(input).unwrap();

        assert_eq!(report1.autopsy.id, report2.autopsy.id);
        assert_eq!(report1.forks.len(), report2.forks.len());
        assert_eq!(report1.scores.len(), report2.scores.len());
        assert_eq!(report1.autopsy.attribution_checksum, report2.autopsy.attribution_checksum);
    }

    #[test]
    fn rejects_empty_quote_id() {
        let engine = DealAutopsyEngine::default();
        let mut input = test_input();
        input.quote_id = String::new();
        assert_eq!(engine.perform(input).unwrap_err(), AutopsyError::EmptyQuoteId);
    }

    #[test]
    fn rejects_empty_audit_trail() {
        let engine = DealAutopsyEngine::default();
        let mut input = test_input();
        input.audit_trail = vec![];
        assert_eq!(engine.perform(input).unwrap_err(), AutopsyError::EmptyAuditTrail);
    }

    #[test]
    fn rejects_empty_idempotency_key() {
        let engine = DealAutopsyEngine::default();
        let mut input = test_input();
        input.idempotency_key = String::new();
        assert_eq!(engine.perform(input).unwrap_err(), AutopsyError::EmptyIdempotencyKey);
    }

    #[test]
    fn rejects_empty_audit_entry_id() {
        let engine = DealAutopsyEngine::default();
        let mut input = test_input();
        input.audit_trail[0].entry_id = "   ".to_string();
        assert_eq!(
            engine.perform(input).unwrap_err(),
            AutopsyError::EmptyAuditEntryId { index: 0 }
        );
    }

    #[test]
    fn rejects_duplicate_audit_entry_ids() {
        let engine = DealAutopsyEngine::default();
        let mut input = test_input();
        input.audit_trail[1].entry_id = input.audit_trail[0].entry_id.clone();
        assert_eq!(
            engine.perform(input).unwrap_err(),
            AutopsyError::DuplicateForkKey { key: "audit-001".to_string() }
        );
    }

    #[test]
    fn attribution_scores_sum_to_10000_bps() {
        let engine = DealAutopsyEngine::default();
        let report = engine.perform(test_input()).unwrap();
        let total: i32 = report.scores.iter().map(|s| s.outcome_contribution_bps).sum();
        assert_eq!(total, 10_000);
    }

    #[test]
    fn single_fork_gets_full_attribution() {
        let engine = DealAutopsyEngine::default();
        let mut input = test_input();
        input.audit_trail = vec![pricing_entry("audit-001")];
        let report = engine.perform(input).unwrap();
        assert_eq!(report.scores.len(), 1);
        assert_eq!(report.scores[0].outcome_contribution_bps, 10_000);
    }

    // === Fork Classification Tests ===

    #[test]
    fn classifies_pricing_trace_as_pricing_path() {
        let ft = classify_fork_type(&AuditRefType::PricingTrace, r#"{"price": 100}"#);
        assert_eq!(ft, DecisionForkType::PricingPath);
    }

    #[test]
    fn classifies_pricing_trace_with_discount_as_discount_level() {
        let ft = classify_fork_type(&AuditRefType::PricingTrace, r#"{"discount": 15}"#);
        assert_eq!(ft, DecisionForkType::DiscountLevel);
    }

    #[test]
    fn classifies_negotiation_turn_as_negotiation_concession() {
        let ft = classify_fork_type(&AuditRefType::NegotiationTurn, "{}");
        assert_eq!(ft, DecisionForkType::NegotiationConcession);
    }

    #[test]
    fn classifies_approval_as_approval_exception() {
        let ft = classify_fork_type(&AuditRefType::ApprovalDecision, "{}");
        assert_eq!(ft, DecisionForkType::ApprovalException);
    }

    // === Graph Builder Tests ===

    #[test]
    fn graph_builder_creates_nodes_and_edges_from_reports() {
        let engine = DealAutopsyEngine::default();
        let builder = AttributionGraphBuilder;

        let reports = vec![
            engine.perform(test_input()).unwrap(),
            engine
                .perform({
                    let mut i = test_input();
                    i.idempotency_key = "idem-2".to_string();
                    i.outcome_status = DealOutcomeType::Lost;
                    i.outcome_value_bps = -500;
                    i
                })
                .unwrap(),
        ];

        let graph = builder.build_from_reports(&reports);
        assert!(!graph.nodes.is_empty());
        assert!(!graph.edges.is_empty());
        assert_eq!(graph.total_autopsies, 2);
        assert!(!graph.checksum.is_empty());
    }

    #[test]
    fn graph_builder_increments_sample_counts() {
        let engine = DealAutopsyEngine::default();
        let builder = AttributionGraphBuilder;

        let reports: Vec<_> = (0..5)
            .map(|i| {
                engine
                    .perform({
                        let mut inp = test_input();
                        inp.idempotency_key = format!("idem-{}", i);
                        inp
                    })
                    .unwrap()
            })
            .collect();

        let graph = builder.build_from_reports(&reports);
        assert!(graph.nodes.iter().all(|n| n.sample_count >= 1));
    }

    // === Genome Query Engine Tests ===

    #[test]
    fn genome_query_returns_findings_from_graph() {
        let engine = DealAutopsyEngine::default();
        let builder = AttributionGraphBuilder;
        let query_engine = RevenueGenomeQueryEngine;

        let reports: Vec<_> = (0..5)
            .map(|i| {
                engine
                    .perform({
                        let mut inp = test_input();
                        inp.idempotency_key = format!("idem-{}", i);
                        inp
                    })
                    .unwrap()
            })
            .collect();

        let graph = builder.build_from_reports(&reports);
        let request = GenomeQueryRequest {
            query_type: GenomeQueryType::StrategyRecommendation,
            segment_filter: None,
            fork_type_filter: None,
            stage_filter: None,
            min_sample_count: 1,
            time_window_days: None,
        };

        let response = query_engine.query(&request, &graph).unwrap();
        assert!(!response.findings.is_empty());
        assert!(response.evidence_count > 0);
        assert!(!response.result_checksum.is_empty());
    }

    #[test]
    fn genome_query_filters_by_fork_type() {
        let engine = DealAutopsyEngine::default();
        let builder = AttributionGraphBuilder;
        let query_engine = RevenueGenomeQueryEngine;

        let reports: Vec<_> = (0..3)
            .map(|i| {
                engine
                    .perform({
                        let mut inp = test_input();
                        inp.idempotency_key = format!("idem-{}", i);
                        inp
                    })
                    .unwrap()
            })
            .collect();

        let graph = builder.build_from_reports(&reports);
        let request = GenomeQueryRequest {
            query_type: GenomeQueryType::SegmentAnalysis,
            segment_filter: None,
            fork_type_filter: Some(DecisionForkType::DiscountLevel),
            stage_filter: None,
            min_sample_count: 1,
            time_window_days: None,
        };

        let response = query_engine.query(&request, &graph).unwrap();
        for finding in &response.findings {
            assert!(finding.description.contains("discount_level"));
        }
    }

    #[test]
    fn genome_query_returns_error_for_no_matching_nodes() {
        let query_engine = RevenueGenomeQueryEngine;
        let graph = AttributionGraphSnapshot {
            nodes: vec![],
            edges: vec![],
            total_autopsies: 0,
            checksum: "sha256:empty".to_string(),
        };

        let request = GenomeQueryRequest {
            query_type: GenomeQueryType::StrategyRecommendation,
            segment_filter: None,
            fork_type_filter: None,
            stage_filter: None,
            min_sample_count: 1,
            time_window_days: None,
        };

        assert_eq!(
            query_engine.query(&request, &graph).unwrap_err(),
            GenomeQueryError::NoMatchingNodes
        );
    }

    // === Counterfactual Engine Tests ===

    #[test]
    fn counterfactual_simulates_alternative_decisions() {
        let engine = DealAutopsyEngine::default();
        let builder = AttributionGraphBuilder;
        let cf_engine = CounterfactualEngine;

        let reports: Vec<_> = (0..5)
            .map(|i| {
                engine
                    .perform({
                        let mut inp = test_input();
                        inp.idempotency_key = format!("idem-{}", i);
                        inp
                    })
                    .unwrap()
            })
            .collect();

        let graph = builder.build_from_reports(&reports);
        let original = &reports[0];

        let request = CounterfactualRequest {
            original_quote_id: "Q-2026-0042".to_string(),
            original_autopsy_id: original.autopsy.id.clone(),
            alternative_decisions: vec![AlternativeDecision {
                fork_id: original.forks[0].id.clone(),
                alternative_option_json: r#"{"price": 200}"#.to_string(),
            }],
        };

        let response = cf_engine.simulate(&request, original, &graph).unwrap();
        assert!(!response.comparison.is_empty());
        assert!(!response.simulation.replay_checksum.is_empty());
    }

    #[test]
    fn counterfactual_rejects_empty_alternatives() {
        let engine = DealAutopsyEngine::default();
        let cf_engine = CounterfactualEngine;
        let original = engine.perform(test_input()).unwrap();
        let graph = AttributionGraphSnapshot {
            nodes: vec![],
            edges: vec![],
            total_autopsies: 0,
            checksum: "sha256:empty".to_string(),
        };

        let request = CounterfactualRequest {
            original_quote_id: "Q-2026-0042".to_string(),
            original_autopsy_id: original.autopsy.id.clone(),
            alternative_decisions: vec![],
        };

        assert_eq!(
            cf_engine.simulate(&request, &original, &graph).unwrap_err(),
            CounterfactualError::NoAlternatives
        );
    }

    #[test]
    fn counterfactual_rejects_unknown_fork() {
        let engine = DealAutopsyEngine::default();
        let cf_engine = CounterfactualEngine;
        let original = engine.perform(test_input()).unwrap();
        let graph = AttributionGraphSnapshot {
            nodes: vec![],
            edges: vec![],
            total_autopsies: 0,
            checksum: "sha256:empty".to_string(),
        };

        let request = CounterfactualRequest {
            original_quote_id: "Q-2026-0042".to_string(),
            original_autopsy_id: original.autopsy.id.clone(),
            alternative_decisions: vec![AlternativeDecision {
                fork_id: DecisionForkId("nonexistent".to_string()),
                alternative_option_json: "{}".to_string(),
            }],
        };

        assert!(matches!(
            cf_engine.simulate(&request, &original, &graph).unwrap_err(),
            CounterfactualError::ForkNotFound { .. }
        ));
    }

    // === Helper Tests ===

    #[test]
    fn deterministic_hash_is_consistent() {
        let h1 = deterministic_hash("test-input");
        let h2 = deterministic_hash("test-input");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn stage_weights_are_all_positive() {
        let stages = [
            DecisionStage::Configuration,
            DecisionStage::Pricing,
            DecisionStage::Policy,
            DecisionStage::Approval,
            DecisionStage::Negotiation,
            DecisionStage::Finalization,
        ];
        for stage in stages {
            assert!(stage_weight(&stage) > 0);
        }
    }

    #[test]
    fn fork_type_weights_are_all_positive() {
        let types = [
            DecisionForkType::PricingPath,
            DecisionForkType::DiscountLevel,
            DecisionForkType::ConstraintResolution,
            DecisionForkType::ApprovalException,
            DecisionForkType::NegotiationConcession,
            DecisionForkType::ProductSelection,
            DecisionForkType::TermSelection,
            DecisionForkType::BundleChoice,
        ];
        for ft in types {
            assert!(fork_type_weight(&ft) > 0);
        }
    }
}
