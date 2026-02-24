use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::optimizer::{
    PolicyApplyRecord, PolicyApplyRecordId, PolicyApprovalDecisionId, PolicyCandidateId,
    PolicyCandidateStatus, PolicyLifecycleAuditEvent, PolicyLifecycleAuditEventType,
    PolicyLifecycleAuditId, PolicyRollbackRecord, PolicyRollbackRecordId,
};

const BASIS_POINTS_SCALE: i64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayQuoteSnapshot {
    pub quote_id: String,
    pub cohort_id: String,
    pub segment_key: String,
    pub impacted_rule_ids: Vec<String>,
    pub baseline_margin_bps: i32,
    pub candidate_margin_bps: i32,
    pub baseline_win_rate_proxy_bps: i32,
    pub candidate_win_rate_proxy_bps: i32,
    pub baseline_approval_required: bool,
    pub candidate_approval_required: bool,
    pub baseline_hard_violation_count: i32,
    pub candidate_hard_violation_count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayImpactRequest {
    pub candidate_id: PolicyCandidateId,
    pub base_policy_version: i32,
    pub proposed_policy_version: i32,
    pub policy_diff_json: String,
    pub cohort_scope_json: String,
    pub engine_version: String,
    pub expected_input_checksum: Option<String>,
    pub snapshots: Vec<ReplayQuoteSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayGuardrailCode {
    MarginDeltaTooLow,
    WinRateDeltaTooLow,
    ApprovalLoadDeltaTooHigh,
    HardViolationDeltaTooHigh,
}

impl ReplayGuardrailCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MarginDeltaTooLow => "margin_delta_too_low",
            Self::WinRateDeltaTooLow => "win_rate_delta_too_low",
            Self::ApprovalLoadDeltaTooHigh => "approval_load_delta_too_high",
            Self::HardViolationDeltaTooHigh => "hard_violation_delta_too_high",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayGuardrailBlock {
    pub code: ReplayGuardrailCode,
    pub measured_value: i32,
    pub threshold_value: i32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayGuardrailThresholds {
    pub min_margin_delta_bps: i32,
    pub min_win_rate_proxy_delta_bps: i32,
    pub max_approval_load_delta_bps: i32,
    pub max_hard_violation_delta: i32,
}

impl Default for ReplayGuardrailThresholds {
    fn default() -> Self {
        Self {
            min_margin_delta_bps: -50,
            min_win_rate_proxy_delta_bps: -25,
            max_approval_load_delta_bps: 400,
            max_hard_violation_delta: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayGuardrailEvaluation {
    pub passed: bool,
    pub blocks: Vec<ReplayGuardrailBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlastRadiusSummary {
    pub impacted_quote_count: i32,
    pub impacted_quote_ratio_bps: i32,
    pub impacted_segment_keys: Vec<String>,
    pub impacted_rule_ids: Vec<String>,
    pub impacted_cohort_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayImpactReport {
    pub candidate_id: PolicyCandidateId,
    pub engine_version: String,
    pub input_checksum: String,
    pub cohort_size: i32,
    pub projected_margin_delta_bps: i32,
    pub projected_win_rate_proxy_delta_bps: i32,
    pub projected_approval_load_delta_bps: i32,
    pub projected_hard_violation_delta: i32,
    pub blast_radius: BlastRadiusSummary,
    pub guardrails: ReplayGuardrailEvaluation,
    pub risk_flags: Vec<String>,
    pub deterministic_pass: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplayImpactError {
    #[error("replay requires at least one historical snapshot")]
    EmptyReplayCohort,
    #[error("candidate id cannot be empty")]
    EmptyCandidateId,
    #[error("engine version cannot be empty")]
    EmptyEngineVersion,
    #[error("snapshot {index} field `{field}` cannot be empty")]
    EmptySnapshotField { index: usize, field: &'static str },
    #[error("snapshot {index} has a negative hard-violation count")]
    NegativeHardViolationCount { index: usize },
    #[error("duplicate replay snapshot key detected: {key}")]
    DuplicateSnapshotKey { key: String },
    #[error("input checksum mismatch: expected {expected}, actual {actual}")]
    InputChecksumMismatch { expected: String, actual: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct NormalizedSnapshot {
    quote_id: String,
    cohort_id: String,
    segment_key: String,
    impacted_rule_ids: Vec<String>,
    baseline_margin_bps: i32,
    candidate_margin_bps: i32,
    baseline_win_rate_proxy_bps: i32,
    candidate_win_rate_proxy_bps: i32,
    baseline_approval_required: bool,
    candidate_approval_required: bool,
    baseline_hard_violation_count: i32,
    candidate_hard_violation_count: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct CanonicalReplayInput {
    candidate_id: String,
    base_policy_version: i32,
    proposed_policy_version: i32,
    policy_diff_json: String,
    cohort_scope_json: String,
    engine_version: String,
    snapshots: Vec<NormalizedSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ReplayMetrics {
    projected_margin_delta_bps: i32,
    projected_win_rate_proxy_delta_bps: i32,
    projected_approval_load_delta_bps: i32,
    projected_hard_violation_delta: i32,
}

pub struct PolicyReplayEngine {
    guardrails: ReplayGuardrailThresholds,
}

impl Default for PolicyReplayEngine {
    fn default() -> Self {
        Self::new(ReplayGuardrailThresholds::default())
    }
}

impl PolicyReplayEngine {
    pub fn new(guardrails: ReplayGuardrailThresholds) -> Self {
        Self { guardrails }
    }

    pub fn evaluate(
        &self,
        request: ReplayImpactRequest,
    ) -> Result<ReplayImpactReport, ReplayImpactError> {
        if request.candidate_id.0.trim().is_empty() {
            return Err(ReplayImpactError::EmptyCandidateId);
        }

        if request.engine_version.trim().is_empty() {
            return Err(ReplayImpactError::EmptyEngineVersion);
        }

        let snapshots = normalize_snapshots(request.snapshots)?;
        if snapshots.is_empty() {
            return Err(ReplayImpactError::EmptyReplayCohort);
        }

        let canonical_input = CanonicalReplayInput {
            candidate_id: request.candidate_id.0.clone(),
            base_policy_version: request.base_policy_version,
            proposed_policy_version: request.proposed_policy_version,
            policy_diff_json: canonicalize_json(&request.policy_diff_json),
            cohort_scope_json: canonicalize_json(&request.cohort_scope_json),
            engine_version: request.engine_version.trim().to_string(),
            snapshots,
        };

        let input_checksum = checksum_for_input(&canonical_input);
        if let Some(expected_checksum) = request.expected_input_checksum {
            let normalized_expected = expected_checksum.trim().to_ascii_lowercase();
            if normalized_expected != input_checksum {
                return Err(ReplayImpactError::InputChecksumMismatch {
                    expected: normalized_expected,
                    actual: input_checksum,
                });
            }
        }

        let metrics = compute_metrics(&canonical_input.snapshots);
        let blast_radius = compute_blast_radius(&canonical_input.snapshots);
        let guardrails = evaluate_guardrails(&metrics, &self.guardrails);
        let risk_flags = derive_risk_flags(&guardrails, &blast_radius, &metrics);

        Ok(ReplayImpactReport {
            candidate_id: request.candidate_id,
            engine_version: canonical_input.engine_version,
            input_checksum,
            cohort_size: saturating_i64_to_i32(canonical_input.snapshots.len() as i64),
            projected_margin_delta_bps: metrics.projected_margin_delta_bps,
            projected_win_rate_proxy_delta_bps: metrics.projected_win_rate_proxy_delta_bps,
            projected_approval_load_delta_bps: metrics.projected_approval_load_delta_bps,
            projected_hard_violation_delta: metrics.projected_hard_violation_delta,
            blast_radius,
            guardrails,
            risk_flags,
            deterministic_pass: true,
        })
    }
}

fn normalize_snapshots(
    snapshots: Vec<ReplayQuoteSnapshot>,
) -> Result<Vec<NormalizedSnapshot>, ReplayImpactError> {
    if snapshots.is_empty() {
        return Err(ReplayImpactError::EmptyReplayCohort);
    }

    let mut seen = BTreeSet::new();
    let mut normalized = Vec::with_capacity(snapshots.len());

    for (index, snapshot) in snapshots.into_iter().enumerate() {
        let quote_id = snapshot.quote_id.trim().to_string();
        if quote_id.is_empty() {
            return Err(ReplayImpactError::EmptySnapshotField { index, field: "quote_id" });
        }

        let cohort_id = snapshot.cohort_id.trim().to_string();
        if cohort_id.is_empty() {
            return Err(ReplayImpactError::EmptySnapshotField { index, field: "cohort_id" });
        }

        let segment_key = snapshot.segment_key.trim().to_string();
        if segment_key.is_empty() {
            return Err(ReplayImpactError::EmptySnapshotField { index, field: "segment_key" });
        }

        if snapshot.baseline_hard_violation_count < 0 || snapshot.candidate_hard_violation_count < 0
        {
            return Err(ReplayImpactError::NegativeHardViolationCount { index });
        }

        let dedupe_key = format!("{quote_id}|{cohort_id}");
        if !seen.insert(dedupe_key.clone()) {
            return Err(ReplayImpactError::DuplicateSnapshotKey { key: dedupe_key });
        }

        normalized.push(NormalizedSnapshot {
            quote_id,
            cohort_id,
            segment_key,
            impacted_rule_ids: normalize_rule_ids(snapshot.impacted_rule_ids),
            baseline_margin_bps: snapshot.baseline_margin_bps,
            candidate_margin_bps: snapshot.candidate_margin_bps,
            baseline_win_rate_proxy_bps: snapshot.baseline_win_rate_proxy_bps,
            candidate_win_rate_proxy_bps: snapshot.candidate_win_rate_proxy_bps,
            baseline_approval_required: snapshot.baseline_approval_required,
            candidate_approval_required: snapshot.candidate_approval_required,
            baseline_hard_violation_count: snapshot.baseline_hard_violation_count,
            candidate_hard_violation_count: snapshot.candidate_hard_violation_count,
        });
    }

    normalized.sort_by(|left, right| {
        (&left.quote_id, &left.cohort_id, &left.segment_key).cmp(&(
            &right.quote_id,
            &right.cohort_id,
            &right.segment_key,
        ))
    });
    Ok(normalized)
}

fn normalize_rule_ids(rule_ids: Vec<String>) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for rule_id in rule_ids {
        let normalized = rule_id.trim().to_ascii_lowercase();
        if !normalized.is_empty() {
            unique.insert(normalized);
        }
    }
    unique.into_iter().collect()
}

fn checksum_for_input(input: &CanonicalReplayInput) -> String {
    let canonical =
        serde_json::to_string(input).unwrap_or_else(|error| format!("serialization_error:{error}"));
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn compute_metrics(snapshots: &[NormalizedSnapshot]) -> ReplayMetrics {
    let cohort_size = snapshots.len() as i64;
    let mut margin_delta_sum = 0_i64;
    let mut win_delta_sum = 0_i64;
    let mut baseline_approval_count = 0_i64;
    let mut candidate_approval_count = 0_i64;
    let mut hard_violation_delta_sum = 0_i64;

    for snapshot in snapshots {
        margin_delta_sum += i64::from(snapshot.candidate_margin_bps - snapshot.baseline_margin_bps);
        win_delta_sum +=
            i64::from(snapshot.candidate_win_rate_proxy_bps - snapshot.baseline_win_rate_proxy_bps);
        baseline_approval_count += if snapshot.baseline_approval_required { 1 } else { 0 };
        candidate_approval_count += if snapshot.candidate_approval_required { 1 } else { 0 };
        hard_violation_delta_sum += i64::from(
            snapshot.candidate_hard_violation_count - snapshot.baseline_hard_violation_count,
        );
    }

    let baseline_approval_rate_bps = ratio_to_basis_points(baseline_approval_count, cohort_size);
    let candidate_approval_rate_bps = ratio_to_basis_points(candidate_approval_count, cohort_size);

    ReplayMetrics {
        projected_margin_delta_bps: round_divide_i64(margin_delta_sum, cohort_size),
        projected_win_rate_proxy_delta_bps: round_divide_i64(win_delta_sum, cohort_size),
        projected_approval_load_delta_bps: candidate_approval_rate_bps - baseline_approval_rate_bps,
        projected_hard_violation_delta: saturating_i64_to_i32(hard_violation_delta_sum),
    }
}

fn compute_blast_radius(snapshots: &[NormalizedSnapshot]) -> BlastRadiusSummary {
    let cohort_size = snapshots.len() as i64;
    let mut impacted_count = 0_i64;
    let mut impacted_segments = BTreeSet::new();
    let mut impacted_rules = BTreeSet::new();
    let mut impacted_cohorts = BTreeSet::new();

    for snapshot in snapshots {
        if snapshot_is_impacted(snapshot) {
            impacted_count += 1;
            impacted_segments.insert(snapshot.segment_key.clone());
            impacted_cohorts.insert(snapshot.cohort_id.clone());
            for rule_id in &snapshot.impacted_rule_ids {
                impacted_rules.insert(rule_id.clone());
            }
        }
    }

    BlastRadiusSummary {
        impacted_quote_count: saturating_i64_to_i32(impacted_count),
        impacted_quote_ratio_bps: ratio_to_basis_points(impacted_count, cohort_size),
        impacted_segment_keys: impacted_segments.into_iter().collect(),
        impacted_rule_ids: impacted_rules.into_iter().collect(),
        impacted_cohort_ids: impacted_cohorts.into_iter().collect(),
    }
}

fn snapshot_is_impacted(snapshot: &NormalizedSnapshot) -> bool {
    snapshot.baseline_margin_bps != snapshot.candidate_margin_bps
        || snapshot.baseline_win_rate_proxy_bps != snapshot.candidate_win_rate_proxy_bps
        || snapshot.baseline_approval_required != snapshot.candidate_approval_required
        || snapshot.baseline_hard_violation_count != snapshot.candidate_hard_violation_count
}

fn evaluate_guardrails(
    metrics: &ReplayMetrics,
    thresholds: &ReplayGuardrailThresholds,
) -> ReplayGuardrailEvaluation {
    let mut blocks = Vec::new();

    if metrics.projected_margin_delta_bps < thresholds.min_margin_delta_bps {
        blocks.push(ReplayGuardrailBlock {
            code: ReplayGuardrailCode::MarginDeltaTooLow,
            measured_value: metrics.projected_margin_delta_bps,
            threshold_value: thresholds.min_margin_delta_bps,
            reason: format!(
                "Projected margin delta {}bps is below minimum {}bps",
                metrics.projected_margin_delta_bps, thresholds.min_margin_delta_bps
            ),
        });
    }

    if metrics.projected_win_rate_proxy_delta_bps < thresholds.min_win_rate_proxy_delta_bps {
        blocks.push(ReplayGuardrailBlock {
            code: ReplayGuardrailCode::WinRateDeltaTooLow,
            measured_value: metrics.projected_win_rate_proxy_delta_bps,
            threshold_value: thresholds.min_win_rate_proxy_delta_bps,
            reason: format!(
                "Projected win-rate proxy delta {}bps is below minimum {}bps",
                metrics.projected_win_rate_proxy_delta_bps, thresholds.min_win_rate_proxy_delta_bps
            ),
        });
    }

    if metrics.projected_approval_load_delta_bps > thresholds.max_approval_load_delta_bps {
        blocks.push(ReplayGuardrailBlock {
            code: ReplayGuardrailCode::ApprovalLoadDeltaTooHigh,
            measured_value: metrics.projected_approval_load_delta_bps,
            threshold_value: thresholds.max_approval_load_delta_bps,
            reason: format!(
                "Projected approval-load delta {}bps exceeds maximum {}bps",
                metrics.projected_approval_load_delta_bps, thresholds.max_approval_load_delta_bps
            ),
        });
    }

    if metrics.projected_hard_violation_delta > thresholds.max_hard_violation_delta {
        blocks.push(ReplayGuardrailBlock {
            code: ReplayGuardrailCode::HardViolationDeltaTooHigh,
            measured_value: metrics.projected_hard_violation_delta,
            threshold_value: thresholds.max_hard_violation_delta,
            reason: format!(
                "Projected hard-violation delta {} exceeds maximum {}",
                metrics.projected_hard_violation_delta, thresholds.max_hard_violation_delta
            ),
        });
    }

    ReplayGuardrailEvaluation { passed: blocks.is_empty(), blocks }
}

fn derive_risk_flags(
    guardrails: &ReplayGuardrailEvaluation,
    blast_radius: &BlastRadiusSummary,
    metrics: &ReplayMetrics,
) -> Vec<String> {
    let mut flags = BTreeSet::new();

    for block in &guardrails.blocks {
        flags.insert(format!("guardrail:{}", block.code.as_str()));
    }

    if blast_radius.impacted_quote_ratio_bps >= 5_000 {
        flags.insert("blast_radius:high".to_string());
    }

    if metrics.projected_approval_load_delta_bps > 0 {
        flags.insert("approval_load:increased".to_string());
    }

    if metrics.projected_hard_violation_delta > 0 {
        flags.insert("hard_violations:increased".to_string());
    }

    flags.into_iter().collect()
}

fn ratio_to_basis_points(numerator: i64, denominator: i64) -> i32 {
    if denominator <= 0 {
        return 0;
    }

    let scaled = numerator.saturating_mul(BASIS_POINTS_SCALE);
    round_divide_i64(scaled, denominator)
}

fn round_divide_i64(numerator: i64, denominator: i64) -> i32 {
    if denominator == 0 {
        return 0;
    }

    let quotient = numerator / denominator;
    let remainder = numerator % denominator;
    if remainder == 0 {
        return saturating_i64_to_i32(quotient);
    }

    let needs_increment = (remainder.abs() * 2) >= denominator.abs();
    if needs_increment {
        if numerator.is_negative() {
            saturating_i64_to_i32(quotient - 1)
        } else {
            saturating_i64_to_i32(quotient + 1)
        }
    } else {
        saturating_i64_to_i32(quotient)
    }
}

fn saturating_i64_to_i32(value: i64) -> i32 {
    if value > i64::from(i32::MAX) {
        i32::MAX
    } else if value < i64::from(i32::MIN) {
        i32::MIN
    } else {
        value as i32
    }
}

fn canonicalize_json(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "{}".to_string();
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => canonicalize_json_value(&value),
        Err(_) => trimmed.to_string(),
    }
}

fn canonicalize_json_value(value: &Value) -> String {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.to_string(),
        Value::Array(entries) => {
            let mut output = String::from("[");
            for (index, entry) in entries.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&canonicalize_json_value(entry));
            }
            output.push(']');
            output
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();

            let mut output = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&Value::String((*key).clone()).to_string());
                output.push(':');
                if let Some(entry) = map.get(*key) {
                    output.push_str(&canonicalize_json_value(entry));
                }
            }
            output.push('}');
            output
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRuleOperation {
    Add,
    Update,
    Remove,
}

impl CandidateRuleOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Update => "update",
            Self::Remove => "remove",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateRuleSignal {
    pub rule_id: String,
    pub operation: CandidateRuleOperation,
    pub field: String,
    pub from_value_json: Option<String>,
    pub to_value_json: Option<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateRuleDiff {
    pub rule_id: String,
    pub operation: CandidateRuleOperation,
    pub field: String,
    pub from_value_json: Option<String>,
    pub to_value_json: Option<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateCohortScope {
    pub segment_keys: Vec<String>,
    pub region_keys: Vec<String>,
    pub quote_ids: Vec<String>,
    pub time_window_days: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateProjectedImpact {
    pub replay_checksum: String,
    pub replay_deterministic_pass: bool,
    pub projected_margin_delta_bps: i32,
    pub projected_win_rate_proxy_delta_bps: i32,
    pub projected_approval_load_delta_bps: i32,
    pub projected_hard_violation_delta: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateConfidenceBounds {
    pub lower_bps: i32,
    pub point_estimate_bps: i32,
    pub upper_bps: i32,
}

impl CandidateConfidenceBounds {
    pub fn confidence_score(&self) -> f64 {
        f64::from(self.point_estimate_bps) / f64::from(BASIS_POINTS_SCALE as i32)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateProvenance {
    pub source_replay_evaluation_ids: Vec<String>,
    pub source_outcome_window: String,
    pub generated_by: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyCandidateDiffV1 {
    pub schema_version: String,
    pub candidate_id: PolicyCandidateId,
    pub rule_diffs: Vec<CandidateRuleDiff>,
    pub cohort_scope: CandidateCohortScope,
    pub projected_impact: CandidateProjectedImpact,
    pub confidence_bounds: CandidateConfidenceBounds,
    pub provenance: CandidateProvenance,
    pub rationale_summary: String,
}

impl PolicyCandidateDiffV1 {
    pub const SCHEMA_VERSION: &'static str = "clo_candidate_diff.v1";

    pub fn validate(&self) -> Result<(), CandidateDiffValidationError> {
        if self.schema_version.trim() != Self::SCHEMA_VERSION {
            return Err(CandidateDiffValidationError::UnsupportedSchemaVersion {
                received: self.schema_version.trim().to_string(),
            });
        }

        if self.candidate_id.0.trim().is_empty() {
            return Err(CandidateDiffValidationError::EmptyCandidateId);
        }

        if self.rule_diffs.is_empty() {
            return Err(CandidateDiffValidationError::NoRuleDiffs);
        }

        let mut seen_rule_diffs = BTreeSet::new();
        for (index, rule_diff) in self.rule_diffs.iter().enumerate() {
            if rule_diff.rule_id.trim().is_empty() {
                return Err(CandidateDiffValidationError::InvalidRuleDiff {
                    index,
                    reason: "rule_id cannot be empty".to_string(),
                });
            }

            if rule_diff.field.trim().is_empty() {
                return Err(CandidateDiffValidationError::InvalidRuleDiff {
                    index,
                    reason: "field cannot be empty".to_string(),
                });
            }

            if rule_diff.rationale.trim().is_empty() {
                return Err(CandidateDiffValidationError::InvalidRuleDiff {
                    index,
                    reason: "rationale cannot be empty".to_string(),
                });
            }

            match rule_diff.operation {
                CandidateRuleOperation::Add => {
                    if rule_diff.to_value_json.as_deref().unwrap_or("").trim().is_empty() {
                        return Err(CandidateDiffValidationError::InvalidRuleDiff {
                            index,
                            reason: "add operation requires to_value_json".to_string(),
                        });
                    }
                }
                CandidateRuleOperation::Update => {
                    if rule_diff.from_value_json.as_deref().unwrap_or("").trim().is_empty()
                        || rule_diff.to_value_json.as_deref().unwrap_or("").trim().is_empty()
                    {
                        return Err(CandidateDiffValidationError::InvalidRuleDiff {
                            index,
                            reason:
                                "update operation requires both from_value_json and to_value_json"
                                    .to_string(),
                        });
                    }
                }
                CandidateRuleOperation::Remove => {
                    if rule_diff.from_value_json.as_deref().unwrap_or("").trim().is_empty() {
                        return Err(CandidateDiffValidationError::InvalidRuleDiff {
                            index,
                            reason: "remove operation requires from_value_json".to_string(),
                        });
                    }
                }
            }

            let dedupe_key = format!(
                "{}|{}|{}",
                rule_diff.rule_id.trim().to_ascii_lowercase(),
                rule_diff.field.trim().to_ascii_lowercase(),
                rule_diff.operation.as_str()
            );
            if !seen_rule_diffs.insert(dedupe_key.clone()) {
                return Err(CandidateDiffValidationError::DuplicateRuleDiff { key: dedupe_key });
            }
        }

        if self.cohort_scope.time_window_days <= 0 {
            return Err(CandidateDiffValidationError::InvalidCohortScope(
                "time_window_days must be greater than zero".to_string(),
            ));
        }

        if self.cohort_scope.segment_keys.is_empty()
            && self.cohort_scope.region_keys.is_empty()
            && self.cohort_scope.quote_ids.is_empty()
        {
            return Err(CandidateDiffValidationError::InvalidCohortScope(
                "at least one segment, region, or quote id must be present".to_string(),
            ));
        }

        if self.projected_impact.replay_checksum.trim().is_empty() {
            return Err(CandidateDiffValidationError::MissingReplayChecksum);
        }

        if !self.projected_impact.replay_checksum.starts_with("sha256:") {
            return Err(CandidateDiffValidationError::MissingReplayChecksum);
        }

        if !self.projected_impact.replay_deterministic_pass {
            return Err(CandidateDiffValidationError::ReplayEvidenceNotDeterministic);
        }

        if self.projected_impact.projected_hard_violation_delta > 0 {
            return Err(CandidateDiffValidationError::UnsafeHardViolationDelta {
                value: self.projected_impact.projected_hard_violation_delta,
            });
        }

        let bounds = &self.confidence_bounds;
        if bounds.lower_bps < 0 || bounds.point_estimate_bps < 0 || bounds.upper_bps < 0 {
            return Err(CandidateDiffValidationError::InvalidConfidenceBounds(
                "confidence bounds must be non-negative".to_string(),
            ));
        }
        if bounds.upper_bps > 10_000 {
            return Err(CandidateDiffValidationError::InvalidConfidenceBounds(
                "confidence bounds must be <= 10000 bps".to_string(),
            ));
        }
        if !(bounds.lower_bps <= bounds.point_estimate_bps
            && bounds.point_estimate_bps <= bounds.upper_bps)
        {
            return Err(CandidateDiffValidationError::InvalidConfidenceBounds(
                "confidence bounds must satisfy lower <= point_estimate <= upper".to_string(),
            ));
        }

        if self.provenance.generated_by.trim().is_empty() {
            return Err(CandidateDiffValidationError::MissingProvenance(
                "generated_by is required".to_string(),
            ));
        }
        if self.provenance.source_outcome_window.trim().is_empty() {
            return Err(CandidateDiffValidationError::MissingProvenance(
                "source_outcome_window is required".to_string(),
            ));
        }
        if self.provenance.source_replay_evaluation_ids.is_empty() {
            return Err(CandidateDiffValidationError::MissingProvenance(
                "source_replay_evaluation_ids is required".to_string(),
            ));
        }

        if self.rationale_summary.trim().is_empty() {
            return Err(CandidateDiffValidationError::EmptyRationaleSummary);
        }

        Ok(())
    }

    pub fn canonical_json(&self) -> Result<String, CandidateDiffValidationError> {
        let normalized = self.normalized();
        normalized.validate()?;
        serde_json::to_string(&normalized).map_err(|err| {
            CandidateDiffValidationError::SerializationError { details: err.to_string() }
        })
    }

    fn normalized(&self) -> Self {
        let mut rule_diffs = self
            .rule_diffs
            .iter()
            .map(|rule_diff| CandidateRuleDiff {
                rule_id: rule_diff.rule_id.trim().to_ascii_lowercase(),
                operation: rule_diff.operation.clone(),
                field: rule_diff.field.trim().to_ascii_lowercase(),
                from_value_json: rule_diff
                    .from_value_json
                    .as_deref()
                    .map(canonicalize_json)
                    .map(Some)
                    .unwrap_or(None),
                to_value_json: rule_diff
                    .to_value_json
                    .as_deref()
                    .map(canonicalize_json)
                    .map(Some)
                    .unwrap_or(None),
                rationale: rule_diff.rationale.trim().to_string(),
            })
            .collect::<Vec<_>>();
        rule_diffs.sort_by(|left, right| {
            (&left.rule_id, &left.field, left.operation.as_str()).cmp(&(
                &right.rule_id,
                &right.field,
                right.operation.as_str(),
            ))
        });

        Self {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            candidate_id: PolicyCandidateId(self.candidate_id.0.trim().to_string()),
            rule_diffs,
            cohort_scope: CandidateCohortScope {
                segment_keys: normalize_string_list(&self.cohort_scope.segment_keys, true, false),
                region_keys: normalize_string_list(&self.cohort_scope.region_keys, true, false),
                quote_ids: normalize_string_list(&self.cohort_scope.quote_ids, false, false),
                time_window_days: self.cohort_scope.time_window_days,
            },
            projected_impact: CandidateProjectedImpact {
                replay_checksum: self.projected_impact.replay_checksum.trim().to_ascii_lowercase(),
                replay_deterministic_pass: self.projected_impact.replay_deterministic_pass,
                projected_margin_delta_bps: self.projected_impact.projected_margin_delta_bps,
                projected_win_rate_proxy_delta_bps: self
                    .projected_impact
                    .projected_win_rate_proxy_delta_bps,
                projected_approval_load_delta_bps: self
                    .projected_impact
                    .projected_approval_load_delta_bps,
                projected_hard_violation_delta: self
                    .projected_impact
                    .projected_hard_violation_delta,
            },
            confidence_bounds: self.confidence_bounds.clone(),
            provenance: CandidateProvenance {
                source_replay_evaluation_ids: normalize_string_list(
                    &self.provenance.source_replay_evaluation_ids,
                    true,
                    false,
                ),
                source_outcome_window: self.provenance.source_outcome_window.trim().to_string(),
                generated_by: self.provenance.generated_by.trim().to_string(),
            },
            rationale_summary: self.rationale_summary.trim().to_string(),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CandidateDiffValidationError {
    #[error("unsupported schema version `{received}`")]
    UnsupportedSchemaVersion { received: String },
    #[error("candidate id cannot be empty")]
    EmptyCandidateId,
    #[error("at least one rule diff is required")]
    NoRuleDiffs,
    #[error("rule diff at index {index} is invalid: {reason}")]
    InvalidRuleDiff { index: usize, reason: String },
    #[error("duplicate rule diff key `{key}`")]
    DuplicateRuleDiff { key: String },
    #[error("cohort scope is invalid: {0}")]
    InvalidCohortScope(String),
    #[error("projected impact is missing a valid replay checksum")]
    MissingReplayChecksum,
    #[error("replay evidence is not deterministic")]
    ReplayEvidenceNotDeterministic,
    #[error("unsafe projected hard-violation delta `{value}`")]
    UnsafeHardViolationDelta { value: i32 },
    #[error("confidence bounds are invalid: {0}")]
    InvalidConfidenceBounds(String),
    #[error("provenance is incomplete: {0}")]
    MissingProvenance(String),
    #[error("rationale summary cannot be empty")]
    EmptyRationaleSummary,
    #[error("candidate diff serialization failed: {details}")]
    SerializationError { details: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateGenerationRequest {
    pub candidate_id: PolicyCandidateId,
    pub replay_report: ReplayImpactReport,
    pub rule_signals: Vec<CandidateRuleSignal>,
    pub cohort_scope: CandidateCohortScope,
    pub confidence_bounds: CandidateConfidenceBounds,
    pub provenance: CandidateProvenance,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GeneratedCandidatePackage {
    pub candidate_diff: PolicyCandidateDiffV1,
    pub candidate_diff_json: String,
    pub cohort_scope_json: String,
    pub provenance_json: String,
    pub confidence_score: f64,
    pub rationale_summary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CandidateGenerationError {
    #[error("candidate generation requires at least one rule signal")]
    EmptyRuleSignals,
    #[error("unsafe replay evidence blocks candidate generation: {reasons:?}")]
    UnsafeReplayEvidence { reasons: Vec<String> },
    #[error(transparent)]
    InvalidCandidateDiff(#[from] CandidateDiffValidationError),
}

#[derive(Default)]
pub struct PolicyCandidateGenerator;

impl PolicyCandidateGenerator {
    pub fn generate(
        &self,
        request: CandidateGenerationRequest,
    ) -> Result<GeneratedCandidatePackage, CandidateGenerationError> {
        if request.rule_signals.is_empty() {
            return Err(CandidateGenerationError::EmptyRuleSignals);
        }

        if !request.replay_report.guardrails.passed {
            let reasons = request
                .replay_report
                .guardrails
                .blocks
                .iter()
                .map(|block| block.reason.clone())
                .collect::<Vec<_>>();
            return Err(CandidateGenerationError::UnsafeReplayEvidence { reasons });
        }

        let mut rule_diffs = request
            .rule_signals
            .into_iter()
            .map(|signal| CandidateRuleDiff {
                rule_id: signal.rule_id,
                operation: signal.operation,
                field: signal.field,
                from_value_json: signal.from_value_json,
                to_value_json: signal.to_value_json,
                rationale: signal.rationale,
            })
            .collect::<Vec<_>>();
        rule_diffs.sort_by(|left, right| {
            (&left.rule_id, &left.field, left.operation.as_str()).cmp(&(
                &right.rule_id,
                &right.field,
                right.operation.as_str(),
            ))
        });

        let rationale_summary =
            build_rationale_summary(&request.candidate_id, &rule_diffs, &request.replay_report);

        let candidate_diff = PolicyCandidateDiffV1 {
            schema_version: PolicyCandidateDiffV1::SCHEMA_VERSION.to_string(),
            candidate_id: request.candidate_id,
            rule_diffs,
            cohort_scope: request.cohort_scope,
            projected_impact: CandidateProjectedImpact {
                replay_checksum: request.replay_report.input_checksum.clone(),
                replay_deterministic_pass: request.replay_report.deterministic_pass,
                projected_margin_delta_bps: request.replay_report.projected_margin_delta_bps,
                projected_win_rate_proxy_delta_bps: request
                    .replay_report
                    .projected_win_rate_proxy_delta_bps,
                projected_approval_load_delta_bps: request
                    .replay_report
                    .projected_approval_load_delta_bps,
                projected_hard_violation_delta: request
                    .replay_report
                    .projected_hard_violation_delta,
            },
            confidence_bounds: request.confidence_bounds,
            provenance: request.provenance,
            rationale_summary: rationale_summary.clone(),
        };

        let candidate_diff_json = candidate_diff.canonical_json()?;
        let normalized_candidate = candidate_diff.normalized();
        let cohort_scope_json =
            serde_json::to_string(&normalized_candidate.cohort_scope).map_err(|err| {
                CandidateDiffValidationError::SerializationError { details: err.to_string() }
            })?;
        let provenance_json =
            serde_json::to_string(&normalized_candidate.provenance).map_err(|err| {
                CandidateDiffValidationError::SerializationError { details: err.to_string() }
            })?;
        let confidence_score = normalized_candidate.confidence_bounds.confidence_score();

        Ok(GeneratedCandidatePackage {
            candidate_diff: normalized_candidate,
            candidate_diff_json,
            cohort_scope_json,
            provenance_json,
            confidence_score,
            rationale_summary,
        })
    }
}

fn normalize_string_list(values: &[String], lowercase: bool, allow_empty: bool) -> Vec<String> {
    let mut unique = BTreeSet::new();
    for value in values {
        let mut normalized = value.trim().to_string();
        if lowercase {
            normalized = normalized.to_ascii_lowercase();
        }

        if normalized.is_empty() && !allow_empty {
            continue;
        }
        unique.insert(normalized);
    }
    unique.into_iter().collect()
}

fn build_rationale_summary(
    candidate_id: &PolicyCandidateId,
    rule_diffs: &[CandidateRuleDiff],
    replay_report: &ReplayImpactReport,
) -> String {
    let touched_rules = rule_diffs
        .iter()
        .map(|diff| format!("{}:{}", diff.rule_id.trim(), diff.field.trim()))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "Candidate {} proposes {} rule changes [{}] across {} replayed quotes; projected deltas: margin {:+}bps, win-rate proxy {:+}bps, approval-load {:+}bps, hard-violations {:+}.",
        candidate_id.0.trim(),
        rule_diffs.len(),
        touched_rules,
        replay_report.cohort_size,
        replay_report.projected_margin_delta_bps,
        replay_report.projected_win_rate_proxy_delta_bps,
        replay_report.projected_approval_load_delta_bps,
        replay_report.projected_hard_violation_delta,
    )
}

const APPROVAL_PACKET_SCHEMA_VERSION: &str = "clo_approval_packet.v1";
const APPROVAL_PACKET_ACTION_VERSION: &str = "clo_approval_packet_action.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalPacket {
    pub packet_id: String,
    pub schema_version: String,
    pub candidate_id: PolicyCandidateId,
    pub base_policy_version: i32,
    pub proposed_policy_version: i32,
    pub candidate_diff: PolicyCandidateDiffV1,
    pub replay_report: ReplayImpactReport,
    pub risk_score_bps: i32,
    pub blast_radius: BlastRadiusSummary,
    pub fallback_plan: String,
}

impl ApprovalPacket {
    pub fn build(
        candidate_diff: PolicyCandidateDiffV1,
        replay_report: ReplayImpactReport,
        base_policy_version: i32,
        proposed_policy_version: i32,
        risk_score_bps: i32,
        fallback_plan: impl Into<String>,
    ) -> Result<Self, ApprovalPacketValidationError> {
        let candidate_diff = candidate_diff.normalized();
        candidate_diff.validate().map_err(ApprovalPacketValidationError::InvalidCandidateDiff)?;

        if candidate_diff.candidate_id != replay_report.candidate_id {
            return Err(ApprovalPacketValidationError::CandidateMismatch {
                candidate_id: candidate_diff.candidate_id.0.clone(),
                replay_candidate_id: replay_report.candidate_id.0.clone(),
            });
        }
        if candidate_diff.projected_impact.replay_checksum != replay_report.input_checksum {
            return Err(ApprovalPacketValidationError::ReplayChecksumMismatch {
                candidate_checksum: candidate_diff.projected_impact.replay_checksum.clone(),
                replay_checksum: replay_report.input_checksum.clone(),
            });
        }

        let packet_id = build_packet_id(
            &candidate_diff.candidate_id,
            base_policy_version,
            proposed_policy_version,
            &replay_report.input_checksum,
        );

        let packet = Self {
            packet_id,
            schema_version: APPROVAL_PACKET_SCHEMA_VERSION.to_string(),
            candidate_id: candidate_diff.candidate_id.clone(),
            base_policy_version,
            proposed_policy_version,
            candidate_diff,
            replay_report: replay_report.clone(),
            risk_score_bps,
            blast_radius: replay_report.blast_radius,
            fallback_plan: fallback_plan.into(),
        };

        packet.validate()?;
        Ok(packet)
    }

    pub fn validate(&self) -> Result<(), ApprovalPacketValidationError> {
        if self.schema_version.trim() != APPROVAL_PACKET_SCHEMA_VERSION {
            return Err(ApprovalPacketValidationError::UnsupportedSchemaVersion {
                received: self.schema_version.trim().to_string(),
            });
        }

        if self.packet_id.trim().is_empty() {
            return Err(ApprovalPacketValidationError::MissingPacketId);
        }

        if self.candidate_id.0.trim().is_empty() {
            return Err(ApprovalPacketValidationError::MissingCandidateId);
        }

        if self.base_policy_version <= 0 || self.proposed_policy_version <= 0 {
            return Err(ApprovalPacketValidationError::InvalidPolicyVersion);
        }

        self.candidate_diff
            .validate()
            .map_err(ApprovalPacketValidationError::InvalidCandidateDiff)?;

        if self.candidate_diff.candidate_id != self.candidate_id {
            return Err(ApprovalPacketValidationError::CandidateSectionMismatch);
        }

        if self.replay_report.candidate_id != self.candidate_id {
            return Err(ApprovalPacketValidationError::ReplaySectionMismatch);
        }
        if self.candidate_diff.projected_impact.replay_checksum != self.replay_report.input_checksum
        {
            return Err(ApprovalPacketValidationError::ReplayChecksumMismatch {
                candidate_checksum: self.candidate_diff.projected_impact.replay_checksum.clone(),
                replay_checksum: self.replay_report.input_checksum.clone(),
            });
        }

        if self.risk_score_bps < 0 || self.risk_score_bps > 10_000 {
            return Err(ApprovalPacketValidationError::InvalidRiskScoreBps(self.risk_score_bps));
        }

        if self.fallback_plan.trim().is_empty() {
            return Err(ApprovalPacketValidationError::MissingFallbackPlan);
        }

        Ok(())
    }

    pub fn canonical_json(&self) -> Result<String, ApprovalPacketValidationError> {
        self.validate()?;
        serde_json::to_string(self)
            .map_err(|err| ApprovalPacketValidationError::SerializationError(err.to_string()))
    }
}

fn build_packet_id(
    candidate_id: &PolicyCandidateId,
    base_policy_version: i32,
    proposed_policy_version: i32,
    replay_checksum: &str,
) -> String {
    let payload = serde_json::json!({
        "schema_version": APPROVAL_PACKET_SCHEMA_VERSION,
        "candidate_id": candidate_id.0.trim(),
        "base_policy_version": base_policy_version,
        "proposed_policy_version": proposed_policy_version,
        "replay_checksum": replay_checksum.trim(),
    });
    let mut hasher = Sha256::new();
    hasher.update(payload.to_string().as_bytes());
    format!("pktv1:{:x}", hasher.finalize())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalPacketValidationError {
    #[error("unsupported packet schema version `{received}`")]
    UnsupportedSchemaVersion { received: String },
    #[error("packet id is required")]
    MissingPacketId,
    #[error("candidate id is required")]
    MissingCandidateId,
    #[error("base/proposed policy versions must be positive")]
    InvalidPolicyVersion,
    #[error("invalid candidate diff section: {0}")]
    InvalidCandidateDiff(CandidateDiffValidationError),
    #[error("candidate section does not match packet candidate id")]
    CandidateSectionMismatch,
    #[error("replay section does not match packet candidate id")]
    ReplaySectionMismatch,
    #[error(
        "candidate mismatch between diff and replay ({candidate_id} vs {replay_candidate_id})"
    )]
    CandidateMismatch { candidate_id: String, replay_candidate_id: String },
    #[error(
        "candidate diff replay checksum `{candidate_checksum}` does not match replay checksum `{replay_checksum}`"
    )]
    ReplayChecksumMismatch { candidate_checksum: String, replay_checksum: String },
    #[error("risk score must be between 0 and 10000 bps, got {0}")]
    InvalidRiskScoreBps(i32),
    #[error("fallback plan section is required")]
    MissingFallbackPlan,
    #[error("approval packet serialization failed: {0}")]
    SerializationError(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPacketDecision {
    Approve,
    Reject,
    RequestChanges,
}

impl ApprovalPacketDecision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::RequestChanges => "request_changes",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "approve" => Some(Self::Approve),
            "reject" => Some(Self::Reject),
            "request_changes" => Some(Self::RequestChanges),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalPacketActionPayload {
    pub action_version: String,
    pub packet_id: String,
    pub candidate_id: PolicyCandidateId,
    pub proposed_policy_version: i32,
    pub decision: ApprovalPacketDecision,
    pub reason: Option<String>,
    pub idempotency_key: String,
}

impl ApprovalPacketActionPayload {
    pub fn new(
        packet: &ApprovalPacket,
        decision: ApprovalPacketDecision,
        reason: Option<String>,
    ) -> Result<Self, ApprovalPacketActionError> {
        packet.validate().map_err(ApprovalPacketActionError::InvalidPacket)?;

        let reason = reason.map(|value| value.trim().to_string()).filter(|value| !value.is_empty());
        if matches!(
            decision,
            ApprovalPacketDecision::Reject | ApprovalPacketDecision::RequestChanges
        ) && reason.is_none()
        {
            return Err(ApprovalPacketActionError::MissingDecisionReason);
        }

        let idempotency_key = build_action_idempotency_key(
            &packet.packet_id,
            &packet.candidate_id,
            packet.proposed_policy_version,
            &decision,
            reason.as_deref(),
        );

        Ok(Self {
            action_version: APPROVAL_PACKET_ACTION_VERSION.to_string(),
            packet_id: packet.packet_id.clone(),
            candidate_id: packet.candidate_id.clone(),
            proposed_policy_version: packet.proposed_policy_version,
            decision,
            reason,
            idempotency_key,
        })
    }

    pub fn parse_json(raw: &str) -> Result<Self, ApprovalPacketActionError> {
        let payload: Self = serde_json::from_str(raw)
            .map_err(|err| ApprovalPacketActionError::Parse(err.to_string()))?;
        payload.validate()?;
        Ok(payload)
    }

    pub fn validate(&self) -> Result<(), ApprovalPacketActionError> {
        if self.action_version.trim() != APPROVAL_PACKET_ACTION_VERSION {
            return Err(ApprovalPacketActionError::UnsupportedActionVersion {
                received: self.action_version.trim().to_string(),
            });
        }

        if self.packet_id.trim().is_empty() {
            return Err(ApprovalPacketActionError::MissingPacketId);
        }

        if self.candidate_id.0.trim().is_empty() {
            return Err(ApprovalPacketActionError::MissingCandidateId);
        }

        if self.proposed_policy_version <= 0 {
            return Err(ApprovalPacketActionError::InvalidProposedPolicyVersion(
                self.proposed_policy_version,
            ));
        }

        if matches!(
            self.decision,
            ApprovalPacketDecision::Reject | ApprovalPacketDecision::RequestChanges
        ) && self.reason.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(ApprovalPacketActionError::MissingDecisionReason);
        }

        if self.idempotency_key.trim().is_empty() {
            return Err(ApprovalPacketActionError::MissingIdempotencyKey);
        }

        Ok(())
    }

    pub fn to_json(&self) -> Result<String, ApprovalPacketActionError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|err| ApprovalPacketActionError::Parse(err.to_string()))
    }

    pub fn target_status(&self) -> PolicyCandidateStatus {
        match self.decision {
            ApprovalPacketDecision::Approve => PolicyCandidateStatus::Approved,
            ApprovalPacketDecision::Reject => PolicyCandidateStatus::Rejected,
            ApprovalPacketDecision::RequestChanges => PolicyCandidateStatus::ChangesRequested,
        }
    }

    pub fn to_audit_event(
        &self,
        actor_id: &str,
        correlation_id: &str,
        occurred_at: DateTime<Utc>,
    ) -> Result<PolicyLifecycleAuditEvent, ApprovalPacketActionError> {
        self.validate()?;
        let event_type = match self.decision {
            ApprovalPacketDecision::Approve => PolicyLifecycleAuditEventType::Approved,
            ApprovalPacketDecision::Reject => PolicyLifecycleAuditEventType::Rejected,
            ApprovalPacketDecision::RequestChanges => {
                PolicyLifecycleAuditEventType::ChangesRequested
            }
        };
        let payload_json = self.to_json()?;

        Ok(PolicyLifecycleAuditEvent {
            id: PolicyLifecycleAuditId(format!("audit:{}", self.idempotency_key)),
            candidate_id: self.candidate_id.clone(),
            replay_evaluation_id: None,
            approval_decision_id: None,
            apply_record_id: None,
            rollback_record_id: None,
            event_type,
            event_payload_json: payload_json,
            actor_type: "human_reviewer".to_string(),
            actor_id: actor_id.trim().to_string(),
            correlation_id: correlation_id.trim().to_string(),
            idempotency_key: Some(self.idempotency_key.clone()),
            occurred_at,
        })
    }
}

fn build_action_idempotency_key(
    packet_id: &str,
    candidate_id: &PolicyCandidateId,
    proposed_policy_version: i32,
    decision: &ApprovalPacketDecision,
    reason: Option<&str>,
) -> String {
    let payload = serde_json::json!({
        "action_version": APPROVAL_PACKET_ACTION_VERSION,
        "packet_id": packet_id.trim(),
        "candidate_id": candidate_id.0.trim(),
        "proposed_policy_version": proposed_policy_version,
        "decision": decision.as_str(),
        "reason": reason.map(str::trim).filter(|value| !value.is_empty()),
    });

    let mut hasher = Sha256::new();
    hasher.update(payload.to_string().as_bytes());
    format!("pktactv1:{:x}", hasher.finalize())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalPacketActionError {
    #[error("packet contract is invalid: {0}")]
    InvalidPacket(ApprovalPacketValidationError),
    #[error("unable to parse action payload: {0}")]
    Parse(String),
    #[error("unsupported action payload version `{received}`")]
    UnsupportedActionVersion { received: String },
    #[error("packet id is required")]
    MissingPacketId,
    #[error("candidate id is required")]
    MissingCandidateId,
    #[error("proposed policy version must be positive, got {0}")]
    InvalidProposedPolicyVersion(i32),
    #[error("reject/request_changes actions require a reason")]
    MissingDecisionReason,
    #[error("idempotency key is required")]
    MissingIdempotencyKey,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyApplyRequest {
    pub packet: ApprovalPacket,
    pub action: ApprovalPacketActionPayload,
    pub actor_id: String,
    pub signature_key_id: String,
    pub signing_secret: String,
    pub idempotency_key_override: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyApplyOutcome {
    pub apply_record: PolicyApplyRecord,
    pub audit_event: PolicyLifecycleAuditEvent,
    pub current_policy_version: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRollbackRequest {
    pub apply_record_id: PolicyApplyRecordId,
    pub candidate_id: PolicyCandidateId,
    pub rollback_reason: String,
    pub actor_id: String,
    pub signature_key_id: String,
    pub signing_secret: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyRollbackOutcome {
    pub rollback_record: PolicyRollbackRecord,
    pub audit_event: PolicyLifecycleAuditEvent,
    pub current_policy_version: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RollbackDrillReport {
    pub candidate_id: PolicyCandidateId,
    pub apply_duration_ms: u64,
    pub rollback_duration_ms: u64,
    pub reapply_duration_ms: u64,
    pub first_apply_verification_checksum: String,
    pub rollback_verification_checksum: String,
    pub reapply_verification_checksum: String,
    pub safety_passed: bool,
    pub final_policy_version: i32,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyLifecycleError {
    #[error("invalid approval packet: {0}")]
    InvalidApprovalPacket(ApprovalPacketValidationError),
    #[error("invalid reviewer action payload: {0}")]
    InvalidActionPayload(ApprovalPacketActionError),
    #[error("apply requires an approved action")]
    ActionNotApproved,
    #[error("action packet id does not match packet contract")]
    ActionPacketMismatch,
    #[error("action candidate/version does not match packet contract")]
    ActionCandidateMismatch,
    #[error("packet replay checksum evidence mismatch")]
    ReplayChecksumMismatch,
    #[error("actor id is required")]
    MissingActorId,
    #[error("signature key id is required")]
    MissingSignatureKeyId,
    #[error("signing secret is required")]
    MissingSigningSecret,
    #[error("idempotency key is required")]
    MissingIdempotencyKey,
    #[error("apply idempotency conflict for key `{idempotency_key}`")]
    ApplyIdempotencyConflict { idempotency_key: String },
    #[error("rollback idempotency conflict for key `{idempotency_key}`")]
    RollbackIdempotencyConflict { idempotency_key: String },
    #[error("current policy version {actual} does not match packet base version {expected}")]
    BaseVersionMismatch { expected: i32, actual: i32 },
    #[error("apply record not found: {apply_record_id}")]
    ApplyRecordNotFound { apply_record_id: String },
    #[error("rollback candidate mismatch: expected {expected}, got {actual}")]
    RollbackCandidateMismatch { expected: String, actual: String },
    #[error(
        "current policy version {actual} does not match apply version {expected} for rollback"
    )]
    RollbackVersionMismatch { expected: i32, actual: i32 },
    #[error("rollback reason is required")]
    MissingRollbackReason,
}

impl PolicyLifecycleError {
    pub fn user_safe_message(&self) -> String {
        match self {
            Self::InvalidApprovalPacket(_) => {
                "Cannot apply this candidate because the approval packet is invalid.".to_string()
            }
            Self::InvalidActionPayload(_) => {
                "Reviewer action payload is invalid or stale. Regenerate the packet action."
                    .to_string()
            }
            Self::ActionNotApproved => {
                "Apply is blocked: reviewer decision is not approved.".to_string()
            }
            Self::ActionPacketMismatch | Self::ActionCandidateMismatch => {
                "Apply is blocked: reviewer action does not match the approval packet."
                    .to_string()
            }
            Self::ReplayChecksumMismatch => {
                "Apply is blocked: replay evidence checksum mismatch detected.".to_string()
            }
            Self::MissingActorId => {
                "Operation blocked: actor identity is required.".to_string()
            }
            Self::MissingSignatureKeyId => {
                "Operation blocked: signature key identity is required.".to_string()
            }
            Self::MissingSigningSecret => {
                "Operation blocked: signing secret is required.".to_string()
            }
            Self::MissingIdempotencyKey => {
                "Operation blocked: idempotency key is required.".to_string()
            }
            Self::ApplyIdempotencyConflict { .. } => {
                "Apply blocked: idempotency key is already bound to a different apply payload."
                    .to_string()
            }
            Self::RollbackIdempotencyConflict { .. } => {
                "Rollback blocked: idempotency key is already bound to a different rollback payload."
                    .to_string()
            }
            Self::BaseVersionMismatch { expected, actual } => format!(
                "Apply blocked: expected base policy version {expected}, but current version is {actual}."
            ),
            Self::ApplyRecordNotFound { .. } => {
                "Rollback failed: apply record was not found.".to_string()
            }
            Self::RollbackCandidateMismatch { .. } => {
                "Rollback blocked: candidate identity does not match apply record.".to_string()
            }
            Self::RollbackVersionMismatch { expected, actual } => format!(
                "Rollback blocked: expected active policy version {expected}, found {actual}."
            ),
            Self::MissingRollbackReason => {
                "Rollback blocked: provide a rollback reason for audit.".to_string()
            }
        }
    }
}

#[derive(Default)]
pub struct InMemoryPolicyLifecycleEngine {
    current_policy_version: i32,
    apply_records_by_id: BTreeMap<String, PolicyApplyRecord>,
    apply_records_by_idempotency: BTreeMap<String, PolicyApplyRecord>,
    apply_record_id_by_applied_version: BTreeMap<i32, String>,
    rollback_records_by_id: BTreeMap<String, PolicyRollbackRecord>,
    rollback_records_by_idempotency: BTreeMap<String, PolicyRollbackRecord>,
    rollback_record_ids_by_target_version: BTreeMap<i32, Vec<String>>,
    rollback_chain_by_apply_id: BTreeMap<String, Vec<PolicyRollbackRecord>>,
    lifecycle_events: Vec<PolicyLifecycleAuditEvent>,
}

impl InMemoryPolicyLifecycleEngine {
    pub fn new(initial_policy_version: i32) -> Self {
        Self { current_policy_version: initial_policy_version, ..Self::default() }
    }

    pub fn current_policy_version(&self) -> i32 {
        self.current_policy_version
    }

    pub fn list_lifecycle_events(&self) -> &[PolicyLifecycleAuditEvent] {
        &self.lifecycle_events
    }

    pub fn find_apply_record_by_applied_version(
        &self,
        policy_version: i32,
    ) -> Option<&PolicyApplyRecord> {
        self.apply_record_id_by_applied_version
            .get(&policy_version)
            .and_then(|record_id| self.apply_records_by_id.get(record_id))
    }

    pub fn list_rollbacks_by_target_version(
        &self,
        target_policy_version: i32,
    ) -> Vec<PolicyRollbackRecord> {
        self.rollback_record_ids_by_target_version
            .get(&target_policy_version)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.rollback_records_by_id.get(id))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub fn apply(
        &mut self,
        request: PolicyApplyRequest,
    ) -> Result<PolicyApplyOutcome, PolicyLifecycleError> {
        request.packet.validate().map_err(PolicyLifecycleError::InvalidApprovalPacket)?;
        request.action.validate().map_err(PolicyLifecycleError::InvalidActionPayload)?;

        if request.action.decision != ApprovalPacketDecision::Approve {
            return Err(PolicyLifecycleError::ActionNotApproved);
        }
        if request.action.packet_id != request.packet.packet_id {
            return Err(PolicyLifecycleError::ActionPacketMismatch);
        }
        if request.action.candidate_id != request.packet.candidate_id
            || request.action.proposed_policy_version != request.packet.proposed_policy_version
        {
            return Err(PolicyLifecycleError::ActionCandidateMismatch);
        }
        if request.packet.candidate_diff.projected_impact.replay_checksum
            != request.packet.replay_report.input_checksum
        {
            return Err(PolicyLifecycleError::ReplayChecksumMismatch);
        }
        let actor_id = request.actor_id.trim().to_string();
        if actor_id.is_empty() {
            return Err(PolicyLifecycleError::MissingActorId);
        }
        let signature_key_id = request.signature_key_id.trim().to_string();
        if signature_key_id.is_empty() {
            return Err(PolicyLifecycleError::MissingSignatureKeyId);
        }
        let signing_secret = request.signing_secret.clone();
        if signing_secret.trim().is_empty() {
            return Err(PolicyLifecycleError::MissingSigningSecret);
        }
        let expected_approval_decision_id =
            PolicyApprovalDecisionId(format!("decision:{}", request.action.idempotency_key));

        let idempotency_key = request
            .idempotency_key_override
            .clone()
            .unwrap_or_else(|| request.action.idempotency_key.clone())
            .trim()
            .to_string();
        if idempotency_key.is_empty() {
            return Err(PolicyLifecycleError::MissingIdempotencyKey);
        }
        if let Some(existing) = self.apply_records_by_idempotency.get(&idempotency_key) {
            if existing.candidate_id != request.packet.candidate_id
                || existing.prior_policy_version != request.packet.base_policy_version
                || existing.applied_policy_version != request.packet.proposed_policy_version
                || existing.replay_checksum != request.packet.replay_report.input_checksum
                || existing.approval_decision_id != expected_approval_decision_id
                || existing.actor_id != actor_id
                || existing.signature_key_id != signature_key_id
            {
                return Err(PolicyLifecycleError::ApplyIdempotencyConflict {
                    idempotency_key: idempotency_key.clone(),
                });
            }

            let audit_event = self
                .lifecycle_events
                .iter()
                .find(|event| {
                    event.event_type == PolicyLifecycleAuditEventType::Applied
                        && event.idempotency_key.as_deref() == Some(idempotency_key.as_str())
                })
                .cloned()
                .unwrap_or_else(|| build_apply_audit_event(existing, &request, &idempotency_key));

            return Ok(PolicyApplyOutcome {
                apply_record: existing.clone(),
                audit_event,
                current_policy_version: self.current_policy_version,
            });
        }

        if self.current_policy_version != request.packet.base_policy_version {
            return Err(PolicyLifecycleError::BaseVersionMismatch {
                expected: request.packet.base_policy_version,
                actual: self.current_policy_version,
            });
        }

        let apply_signature_payload = format!(
            "{}|{}|{}|{}|{}",
            request.packet.packet_id,
            request.packet.base_policy_version,
            request.packet.proposed_policy_version,
            request.packet.replay_report.input_checksum,
            idempotency_key
        );
        let apply_signature =
            sign_lifecycle_payload(&signature_key_id, &signing_secret, &apply_signature_payload);
        let candidate_diff_json =
            request.packet.candidate_diff.canonical_json().map_err(|error| {
                PolicyLifecycleError::InvalidApprovalPacket(
                    ApprovalPacketValidationError::InvalidCandidateDiff(error),
                )
            })?;
        let proposed_policy_version = request.packet.proposed_policy_version.to_string();
        let verification_checksum = checksum_from_parts(&[
            request.packet.packet_id.as_str(),
            request.packet.replay_report.input_checksum.as_str(),
            candidate_diff_json.as_str(),
            proposed_policy_version.as_str(),
        ]);

        let apply_record = PolicyApplyRecord {
            id: PolicyApplyRecordId(format!("apply:{idempotency_key}")),
            candidate_id: request.packet.candidate_id.clone(),
            approval_decision_id: PolicyApprovalDecisionId(format!(
                "decision:{}",
                request.action.idempotency_key
            )),
            prior_policy_version: request.packet.base_policy_version,
            applied_policy_version: request.packet.proposed_policy_version,
            replay_checksum: request.packet.replay_report.input_checksum.clone(),
            apply_signature,
            signature_key_id,
            actor_id,
            idempotency_key: idempotency_key.clone(),
            verification_checksum,
            apply_audit_json: request
                .action
                .to_json()
                .map_err(PolicyLifecycleError::InvalidActionPayload)?,
            applied_at: Utc::now(),
        };

        self.current_policy_version = apply_record.applied_policy_version;
        self.apply_record_id_by_applied_version
            .insert(apply_record.applied_policy_version, apply_record.id.0.clone());
        self.apply_records_by_id.insert(apply_record.id.0.clone(), apply_record.clone());
        self.apply_records_by_idempotency.insert(idempotency_key.clone(), apply_record.clone());

        let audit_event = build_apply_audit_event(&apply_record, &request, &idempotency_key);
        self.lifecycle_events.push(audit_event.clone());

        Ok(PolicyApplyOutcome {
            apply_record,
            audit_event,
            current_policy_version: self.current_policy_version,
        })
    }

    pub fn rollback(
        &mut self,
        request: PolicyRollbackRequest,
    ) -> Result<PolicyRollbackOutcome, PolicyLifecycleError> {
        if request.rollback_reason.trim().is_empty() {
            return Err(PolicyLifecycleError::MissingRollbackReason);
        }

        let idempotency_key = request.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() {
            return Err(PolicyLifecycleError::MissingIdempotencyKey);
        }
        let actor_id = request.actor_id.trim().to_string();
        if actor_id.is_empty() {
            return Err(PolicyLifecycleError::MissingActorId);
        }
        let signature_key_id = request.signature_key_id.trim().to_string();
        if signature_key_id.is_empty() {
            return Err(PolicyLifecycleError::MissingSignatureKeyId);
        }
        let signing_secret = request.signing_secret.clone();
        if signing_secret.trim().is_empty() {
            return Err(PolicyLifecycleError::MissingSigningSecret);
        }
        if let Some(existing) = self.rollback_records_by_idempotency.get(&idempotency_key) {
            if existing.apply_record_id != request.apply_record_id
                || existing.candidate_id != request.candidate_id
                || existing.rollback_reason != request.rollback_reason.trim()
                || existing.actor_id != actor_id
                || existing.signature_key_id != signature_key_id
            {
                return Err(PolicyLifecycleError::RollbackIdempotencyConflict {
                    idempotency_key: idempotency_key.clone(),
                });
            }

            let audit_event = self
                .lifecycle_events
                .iter()
                .find(|event| {
                    event.event_type == PolicyLifecycleAuditEventType::RolledBack
                        && event.idempotency_key.as_deref() == Some(idempotency_key.as_str())
                })
                .cloned()
                .unwrap_or_else(|| build_rollback_audit_event(existing, &request));

            return Ok(PolicyRollbackOutcome {
                rollback_record: existing.clone(),
                audit_event,
                current_policy_version: self.current_policy_version,
            });
        }

        let Some(apply_record) = self.apply_records_by_id.get(&request.apply_record_id.0) else {
            return Err(PolicyLifecycleError::ApplyRecordNotFound {
                apply_record_id: request.apply_record_id.0.clone(),
            });
        };
        if apply_record.candidate_id != request.candidate_id {
            return Err(PolicyLifecycleError::RollbackCandidateMismatch {
                expected: apply_record.candidate_id.0.clone(),
                actual: request.candidate_id.0.clone(),
            });
        }
        if self.current_policy_version != apply_record.applied_policy_version {
            return Err(PolicyLifecycleError::RollbackVersionMismatch {
                expected: apply_record.applied_policy_version,
                actual: self.current_policy_version,
            });
        }

        let existing_chain =
            self.rollback_chain_by_apply_id.get(&apply_record.id.0).cloned().unwrap_or_default();
        let parent = existing_chain.last().cloned();
        let rollback_depth = parent.as_ref().map(|record| record.rollback_depth + 1).unwrap_or(1);

        let rollback_signature_payload = format!(
            "{}|{}|{}|{}|{}",
            apply_record.id.0,
            apply_record.applied_policy_version,
            apply_record.prior_policy_version,
            request.rollback_reason.trim(),
            idempotency_key
        );
        let rollback_signature =
            sign_lifecycle_payload(&signature_key_id, &signing_secret, &rollback_signature_payload);
        let verification_checksum = checksum_from_parts(&[
            apply_record.verification_checksum.as_str(),
            request.rollback_reason.trim(),
            idempotency_key.as_str(),
            &rollback_depth.to_string(),
        ]);

        let rollback_record = PolicyRollbackRecord {
            id: PolicyRollbackRecordId(format!("rollback:{idempotency_key}")),
            candidate_id: request.candidate_id.clone(),
            apply_record_id: apply_record.id.clone(),
            rollback_target_version: apply_record.prior_policy_version,
            rollback_reason: request.rollback_reason.trim().to_string(),
            verification_checksum,
            rollback_signature,
            signature_key_id,
            actor_id,
            idempotency_key: idempotency_key.clone(),
            parent_rollback_id: parent.as_ref().map(|record| record.id.clone()),
            rollback_depth,
            rollback_metadata_json: format!(
                "{{\"apply_record_id\":\"{}\",\"rollback_depth\":{}}}",
                apply_record.id.0, rollback_depth
            ),
            rolled_back_at: Utc::now(),
        };

        self.current_policy_version = rollback_record.rollback_target_version;
        self.rollback_records_by_id.insert(rollback_record.id.0.clone(), rollback_record.clone());
        self.rollback_records_by_idempotency.insert(idempotency_key, rollback_record.clone());
        self.rollback_record_ids_by_target_version
            .entry(rollback_record.rollback_target_version)
            .or_default()
            .push(rollback_record.id.0.clone());
        self.rollback_chain_by_apply_id
            .entry(apply_record.id.0.clone())
            .or_default()
            .push(rollback_record.clone());

        let audit_event = build_rollback_audit_event(&rollback_record, &request);
        self.lifecycle_events.push(audit_event.clone());

        Ok(PolicyRollbackOutcome {
            rollback_record,
            audit_event,
            current_policy_version: self.current_policy_version,
        })
    }

    pub fn run_rollback_drill(
        &mut self,
        packet: ApprovalPacket,
        action: ApprovalPacketActionPayload,
        actor_id: String,
        signature_key_id: String,
        signing_secret: String,
        rollback_reason: String,
    ) -> Result<RollbackDrillReport, PolicyLifecycleError> {
        let apply_started = Instant::now();
        let first_apply = self.apply(PolicyApplyRequest {
            packet: packet.clone(),
            action: action.clone(),
            actor_id: actor_id.clone(),
            signature_key_id: signature_key_id.clone(),
            signing_secret: signing_secret.clone(),
            idempotency_key_override: None,
        })?;
        let apply_duration_ms = apply_started.elapsed().as_millis() as u64;

        let rollback_started = Instant::now();
        let rollback = self.rollback(PolicyRollbackRequest {
            apply_record_id: first_apply.apply_record.id.clone(),
            candidate_id: packet.candidate_id.clone(),
            rollback_reason,
            actor_id: actor_id.clone(),
            signature_key_id: signature_key_id.clone(),
            signing_secret: signing_secret.clone(),
            idempotency_key: format!("{}:rollback", action.idempotency_key),
        })?;
        let rollback_duration_ms = rollback_started.elapsed().as_millis() as u64;

        let reapply_started = Instant::now();
        let reapply = self.apply(PolicyApplyRequest {
            packet: packet.clone(),
            action,
            actor_id,
            signature_key_id,
            signing_secret,
            idempotency_key_override: Some(format!(
                "{}:reapply",
                first_apply.apply_record.idempotency_key
            )),
        })?;
        let reapply_duration_ms = reapply_started.elapsed().as_millis() as u64;

        let safety_passed = rollback.rollback_record.rollback_target_version
            == packet.base_policy_version
            && reapply.apply_record.applied_policy_version == packet.proposed_policy_version
            && first_apply.apply_record.verification_checksum
                == reapply.apply_record.verification_checksum;

        Ok(RollbackDrillReport {
            candidate_id: packet.candidate_id,
            apply_duration_ms,
            rollback_duration_ms,
            reapply_duration_ms,
            first_apply_verification_checksum: first_apply.apply_record.verification_checksum,
            rollback_verification_checksum: rollback.rollback_record.verification_checksum,
            reapply_verification_checksum: reapply.apply_record.verification_checksum,
            safety_passed,
            final_policy_version: self.current_policy_version,
        })
    }
}

fn build_apply_audit_event(
    apply_record: &PolicyApplyRecord,
    request: &PolicyApplyRequest,
    idempotency_key: &str,
) -> PolicyLifecycleAuditEvent {
    PolicyLifecycleAuditEvent {
        id: PolicyLifecycleAuditId(format!("audit:apply:{idempotency_key}")),
        candidate_id: apply_record.candidate_id.clone(),
        replay_evaluation_id: None,
        approval_decision_id: Some(apply_record.approval_decision_id.clone()),
        apply_record_id: Some(apply_record.id.clone()),
        rollback_record_id: None,
        event_type: PolicyLifecycleAuditEventType::Applied,
        event_payload_json: format!(
            "{{\"packet_id\":\"{}\",\"replay_checksum\":\"{}\",\"applied_policy_version\":{}}}",
            request.packet.packet_id,
            apply_record.replay_checksum,
            apply_record.applied_policy_version
        ),
        actor_type: "human_reviewer".to_string(),
        actor_id: request.actor_id.trim().to_string(),
        correlation_id: format!("apply:{}", request.packet.packet_id),
        idempotency_key: Some(idempotency_key.to_string()),
        occurred_at: apply_record.applied_at,
    }
}

fn build_rollback_audit_event(
    rollback_record: &PolicyRollbackRecord,
    request: &PolicyRollbackRequest,
) -> PolicyLifecycleAuditEvent {
    PolicyLifecycleAuditEvent {
        id: PolicyLifecycleAuditId(format!("audit:rollback:{}", rollback_record.idempotency_key)),
        candidate_id: rollback_record.candidate_id.clone(),
        replay_evaluation_id: None,
        approval_decision_id: None,
        apply_record_id: Some(rollback_record.apply_record_id.clone()),
        rollback_record_id: Some(rollback_record.id.clone()),
        event_type: PolicyLifecycleAuditEventType::RolledBack,
        event_payload_json: format!(
            "{{\"rollback_target_version\":{},\"reason\":\"{}\"}}",
            rollback_record.rollback_target_version,
            rollback_record.rollback_reason.replace('"', "\\\"")
        ),
        actor_type: "human_reviewer".to_string(),
        actor_id: request.actor_id.trim().to_string(),
        correlation_id: format!("rollback:{}", rollback_record.apply_record_id.0),
        idempotency_key: Some(rollback_record.idempotency_key.clone()),
        occurred_at: rollback_record.rolled_back_at,
    }
}

fn checksum_from_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update(b"|");
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn sign_lifecycle_payload(signature_key_id: &str, signing_secret: &str, payload: &str) -> String {
    checksum_from_parts(&[signature_key_id, signing_secret, payload])
}

#[cfg(test)]
mod tests {
    use super::{
        sign_lifecycle_payload, ApprovalPacket, ApprovalPacketActionError,
        ApprovalPacketActionPayload, ApprovalPacketDecision, ApprovalPacketValidationError,
        CandidateCohortScope, CandidateConfidenceBounds, CandidateDiffValidationError,
        CandidateGenerationError, CandidateGenerationRequest, CandidateProvenance,
        CandidateRuleOperation, CandidateRuleSignal, InMemoryPolicyLifecycleEngine,
        PolicyApplyRequest, PolicyCandidateDiffV1, PolicyCandidateGenerator, PolicyCandidateStatus,
        PolicyLifecycleError, PolicyReplayEngine, PolicyRollbackRequest, ReplayGuardrailBlock,
        ReplayGuardrailCode, ReplayGuardrailEvaluation, ReplayGuardrailThresholds,
        ReplayImpactError, ReplayImpactReport, ReplayImpactRequest, ReplayQuoteSnapshot,
    };
    use crate::domain::optimizer::{PolicyCandidateId, PolicyLifecycleAuditEventType};
    use chrono::Utc;

    #[test]
    fn identical_input_yields_identical_outputs() {
        let engine = PolicyReplayEngine::default();
        let request = replay_request();

        let first = engine.evaluate(request.clone()).expect("first replay should pass");
        let second = engine.evaluate(request).expect("second replay should pass");

        assert_eq!(first, second);
        assert!(first.guardrails.passed);
    }

    #[test]
    fn checksum_mismatch_is_detected_and_blocked() {
        let engine = PolicyReplayEngine::default();
        let baseline_report =
            engine.evaluate(replay_request()).expect("baseline replay should pass");

        let mut request = replay_request();
        request.expected_input_checksum = Some("sha256:deadbeef".to_string());
        let error = engine.evaluate(request).expect_err("mismatch must be blocked");

        assert!(
            matches!(error, ReplayImpactError::InputChecksumMismatch { .. }),
            "unexpected error variant: {error:?}"
        );
        if let ReplayImpactError::InputChecksumMismatch { expected, actual } = error {
            assert_eq!(expected, "sha256:deadbeef");
            assert_eq!(actual, baseline_report.input_checksum);
        }
    }

    #[test]
    fn blast_radius_is_order_independent() {
        let engine = PolicyReplayEngine::default();
        let request_a = replay_request();
        let mut request_b = replay_request();
        request_b.snapshots.reverse();
        request_b.snapshots[0].impacted_rule_ids =
            vec!["discount-cap".to_string(), "margin-floor".to_string()];
        request_b.snapshots[1].impacted_rule_ids = vec![
            "discount-cap".to_string(),
            "margin-floor".to_string(),
            "discount-cap".to_string(),
        ];

        let report_a = engine.evaluate(request_a).expect("replay a should pass");
        let report_b = engine.evaluate(request_b).expect("replay b should pass");

        assert_eq!(report_a.blast_radius, report_b.blast_radius);
        assert_eq!(report_a.input_checksum, report_b.input_checksum);
    }

    #[test]
    fn hard_thresholds_block_candidate_promotion() {
        let guardrails = ReplayGuardrailThresholds {
            min_margin_delta_bps: -10,
            min_win_rate_proxy_delta_bps: -10,
            max_approval_load_delta_bps: 100,
            max_hard_violation_delta: 0,
        };
        let engine = PolicyReplayEngine::new(guardrails);

        let request = ReplayImpactRequest {
            candidate_id: PolicyCandidateId("cand-risk".to_string()),
            base_policy_version: 8,
            proposed_policy_version: 9,
            policy_diff_json:
                r#"{"mutations":[{"rule":"discount-cap","op":"increase","value":28}]}"#.to_string(),
            cohort_scope_json: r#"{"segments":["smb","enterprise"]}"#.to_string(),
            engine_version: "optimizer-v1".to_string(),
            expected_input_checksum: None,
            snapshots: vec![
                ReplayQuoteSnapshot {
                    quote_id: "q-1".to_string(),
                    cohort_id: "cohort-b".to_string(),
                    segment_key: "smb".to_string(),
                    impacted_rule_ids: vec!["discount-cap".to_string()],
                    baseline_margin_bps: 2_900,
                    candidate_margin_bps: 2_100,
                    baseline_win_rate_proxy_bps: 5_400,
                    candidate_win_rate_proxy_bps: 4_900,
                    baseline_approval_required: false,
                    candidate_approval_required: true,
                    baseline_hard_violation_count: 0,
                    candidate_hard_violation_count: 1,
                },
                ReplayQuoteSnapshot {
                    quote_id: "q-2".to_string(),
                    cohort_id: "cohort-b".to_string(),
                    segment_key: "enterprise".to_string(),
                    impacted_rule_ids: vec!["margin-floor".to_string()],
                    baseline_margin_bps: 3_100,
                    candidate_margin_bps: 2_300,
                    baseline_win_rate_proxy_bps: 5_800,
                    candidate_win_rate_proxy_bps: 5_200,
                    baseline_approval_required: false,
                    candidate_approval_required: true,
                    baseline_hard_violation_count: 0,
                    candidate_hard_violation_count: 1,
                },
            ],
        };

        let report = engine.evaluate(request).expect("replay should compute");
        assert!(!report.guardrails.passed);

        let codes =
            report.guardrails.blocks.iter().map(|block| block.code.as_str()).collect::<Vec<_>>();
        assert!(codes.contains(&ReplayGuardrailCode::MarginDeltaTooLow.as_str()));
        assert!(codes.contains(&ReplayGuardrailCode::WinRateDeltaTooLow.as_str()));
        assert!(codes.contains(&ReplayGuardrailCode::ApprovalLoadDeltaTooHigh.as_str()));
        assert!(codes.contains(&ReplayGuardrailCode::HardViolationDeltaTooHigh.as_str()));
    }

    #[test]
    fn candidate_diff_canonical_json_is_order_independent() {
        let mut candidate_a = candidate_diff_fixture();
        let mut candidate_b = candidate_diff_fixture();

        candidate_a.rule_diffs.reverse();
        candidate_a.cohort_scope.segment_keys.reverse();
        candidate_b.rule_diffs[1].rule_id = "Discount-Cap".to_string();
        candidate_b.rule_diffs[1].field = "Threshold".to_string();

        let json_a = candidate_a.canonical_json().expect("candidate diff should serialize");
        let json_b = candidate_b.canonical_json().expect("candidate diff should serialize");

        assert_eq!(json_a, json_b);
    }

    #[test]
    fn candidate_diff_validation_rejects_incomplete_or_unsafe_payloads() {
        let mut invalid = candidate_diff_fixture();
        invalid.rule_diffs.clear();
        let error = invalid.validate().expect_err("empty rule diff payload must fail");
        assert_eq!(error, CandidateDiffValidationError::NoRuleDiffs);

        let mut unsafe_payload = candidate_diff_fixture();
        unsafe_payload.projected_impact.projected_hard_violation_delta = 1;
        let unsafe_error = unsafe_payload.validate().expect_err("unsafe payload must fail");
        assert_eq!(
            unsafe_error,
            CandidateDiffValidationError::UnsafeHardViolationDelta { value: 1 }
        );
    }

    #[test]
    fn candidate_generator_is_deterministic_and_links_replay_evidence() {
        let generator = PolicyCandidateGenerator;
        let replay_report =
            PolicyReplayEngine::default().evaluate(replay_request()).expect("replay should pass");

        let request = CandidateGenerationRequest {
            candidate_id: PolicyCandidateId("cand-101".to_string()),
            replay_report: replay_report.clone(),
            rule_signals: vec![
                CandidateRuleSignal {
                    rule_id: "discount-cap".to_string(),
                    operation: CandidateRuleOperation::Update,
                    field: "threshold".to_string(),
                    from_value_json: Some("20".to_string()),
                    to_value_json: Some("18".to_string()),
                    rationale: "Reduce discretionary discount ceiling to improve margin"
                        .to_string(),
                },
                CandidateRuleSignal {
                    rule_id: "margin-floor".to_string(),
                    operation: CandidateRuleOperation::Update,
                    field: "min_margin_pct".to_string(),
                    from_value_json: Some("25".to_string()),
                    to_value_json: Some("27".to_string()),
                    rationale: "Align floor with observed high-win healthy-margin cohort"
                        .to_string(),
                },
            ],
            cohort_scope: CandidateCohortScope {
                segment_keys: vec!["smb".to_string(), "enterprise".to_string()],
                region_keys: vec!["na".to_string()],
                quote_ids: vec![],
                time_window_days: 90,
            },
            confidence_bounds: CandidateConfidenceBounds {
                lower_bps: 6200,
                point_estimate_bps: 7100,
                upper_bps: 7800,
            },
            provenance: CandidateProvenance {
                source_replay_evaluation_ids: vec![
                    "replay-cand-101-v1".to_string(),
                    "replay-cand-101-v2".to_string(),
                ],
                source_outcome_window: "2025-Q4".to_string(),
                generated_by: "policy-candidate-generator-v1".to_string(),
            },
        };

        let first = generator.generate(request.clone()).expect("first generation should pass");
        let second = generator.generate(request).expect("second generation should pass");

        assert_eq!(first, second);
        assert!(first.candidate_diff_json.contains("\"schema_version\":\"clo_candidate_diff.v1\""));
        assert_eq!(
            first.candidate_diff.projected_impact.replay_checksum,
            replay_report.input_checksum
        );
    }

    #[test]
    fn candidate_generator_rejects_guardrail_failed_replay() {
        let generator = PolicyCandidateGenerator;
        let mut replay_report =
            PolicyReplayEngine::default().evaluate(replay_request()).expect("replay should pass");
        replay_report.guardrails = ReplayGuardrailEvaluation {
            passed: false,
            blocks: vec![ReplayGuardrailBlock {
                code: ReplayGuardrailCode::HardViolationDeltaTooHigh,
                measured_value: 1,
                threshold_value: 0,
                reason: "Projected hard-violation delta 1 exceeds maximum 0".to_string(),
            }],
        };

        let request = CandidateGenerationRequest {
            candidate_id: PolicyCandidateId("cand-unsafe".to_string()),
            replay_report,
            rule_signals: vec![CandidateRuleSignal {
                rule_id: "discount-cap".to_string(),
                operation: CandidateRuleOperation::Update,
                field: "threshold".to_string(),
                from_value_json: Some("20".to_string()),
                to_value_json: Some("30".to_string()),
                rationale: "Aggressive discount to push win rate".to_string(),
            }],
            cohort_scope: CandidateCohortScope {
                segment_keys: vec!["smb".to_string()],
                region_keys: vec!["na".to_string()],
                quote_ids: vec![],
                time_window_days: 30,
            },
            confidence_bounds: CandidateConfidenceBounds {
                lower_bps: 3000,
                point_estimate_bps: 5000,
                upper_bps: 6000,
            },
            provenance: CandidateProvenance {
                source_replay_evaluation_ids: vec!["replay-unsafe".to_string()],
                source_outcome_window: "2026-Q1".to_string(),
                generated_by: "policy-candidate-generator-v1".to_string(),
            },
        };

        let error =
            generator.generate(request).expect_err("guardrail failures must block generation");
        assert!(
            matches!(error, CandidateGenerationError::UnsafeReplayEvidence { .. }),
            "unexpected error variant: {error:?}"
        );
        if let CandidateGenerationError::UnsafeReplayEvidence { reasons } = error {
            assert_eq!(reasons.len(), 1);
            assert!(reasons[0].contains("hard-violation"));
        }
    }

    #[test]
    fn approval_packet_id_is_idempotent_and_versioned() {
        let candidate_diff = candidate_diff_fixture();
        let replay_report =
            PolicyReplayEngine::default().evaluate(replay_request()).expect("replay should pass");

        let first = ApprovalPacket::build(
            candidate_diff.clone(),
            replay_report.clone(),
            41,
            42,
            1800,
            "Fallback to policy version 41 and notify approvers.",
        )
        .expect("packet build should pass");

        let second = ApprovalPacket::build(
            candidate_diff,
            replay_report,
            41,
            42,
            1800,
            "Fallback to policy version 41 and notify approvers.",
        )
        .expect("packet build should pass");

        assert_eq!(first.packet_id, second.packet_id);
        assert_eq!(first.schema_version, "clo_approval_packet.v1");
        assert!(first.packet_id.starts_with("pktv1:"));
    }

    #[test]
    fn approval_packet_validation_enforces_required_sections() {
        let candidate_diff = candidate_diff_fixture();
        let replay_report =
            PolicyReplayEngine::default().evaluate(replay_request()).expect("replay should pass");

        let mut packet = ApprovalPacket::build(
            candidate_diff,
            replay_report,
            41,
            42,
            1200,
            "Fallback to policy version 41.",
        )
        .expect("packet should build");
        packet.fallback_plan = "   ".to_string();

        let error = packet.validate().expect_err("missing fallback section must fail");
        assert_eq!(error, ApprovalPacketValidationError::MissingFallbackPlan);
    }

    #[test]
    fn reviewer_actions_map_to_deterministic_transitions_and_idempotency() {
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");
        let approve_again =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");
        assert_eq!(approve.idempotency_key, approve_again.idempotency_key);
        assert_eq!(approve.target_status(), PolicyCandidateStatus::Approved);

        let reject = ApprovalPacketActionPayload::new(
            &packet,
            ApprovalPacketDecision::Reject,
            Some("Margin regression too high for this cohort".to_string()),
        )
        .expect("reject action should build");
        assert_eq!(reject.target_status(), PolicyCandidateStatus::Rejected);

        let request_changes = ApprovalPacketActionPayload::new(
            &packet,
            ApprovalPacketDecision::RequestChanges,
            Some("Narrow scope to enterprise segment only".to_string()),
        )
        .expect("request-changes action should build");
        assert_eq!(request_changes.target_status(), PolicyCandidateStatus::ChangesRequested);

        let missing_reason =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Reject, None)
                .expect_err("reject without reason must fail");
        assert_eq!(missing_reason, ApprovalPacketActionError::MissingDecisionReason);
    }

    #[test]
    fn action_payload_is_version_aware_and_generates_append_only_audit_event() {
        let packet = approval_packet_fixture();
        let action = ApprovalPacketActionPayload::new(
            &packet,
            ApprovalPacketDecision::RequestChanges,
            Some("Need explicit rollback blast-radius note.".to_string()),
        )
        .expect("action should build");

        let raw_json = action.to_json().expect("payload serialization should pass");
        let parsed =
            ApprovalPacketActionPayload::parse_json(&raw_json).expect("payload parse should pass");
        assert_eq!(parsed, action);

        let mut wrong_version = parsed.clone();
        wrong_version.action_version = "clo_approval_packet_action.v0".to_string();
        let version_error = wrong_version.validate().expect_err("unsupported version must fail");
        assert_eq!(
            version_error,
            ApprovalPacketActionError::UnsupportedActionVersion {
                received: "clo_approval_packet_action.v0".to_string()
            }
        );

        let event = parsed
            .to_audit_event("u-reviewer-1", "corr-abc-123", Utc::now())
            .expect("audit event should build");
        assert!(event.id.0.starts_with("audit:"));
        assert_eq!(event.candidate_id, packet.candidate_id);
        assert_eq!(event.event_type, PolicyLifecycleAuditEventType::ChangesRequested);
        assert_eq!(event.idempotency_key, Some(parsed.idempotency_key));
    }

    #[test]
    fn apply_requires_approved_action_and_matching_replay_checksum() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let reject_action = ApprovalPacketActionPayload::new(
            &packet,
            ApprovalPacketDecision::Reject,
            Some("requires human review".to_string()),
        )
        .expect("reject action should build");

        let reject_error = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: reject_action,
                actor_id: "reviewer-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: None,
            })
            .expect_err("non-approved action must be blocked");
        assert_eq!(reject_error, PolicyLifecycleError::ActionNotApproved);
        assert_eq!(
            reject_error.user_safe_message(),
            "Apply is blocked: reviewer decision is not approved."
        );

        let replay_report = replay_report_fixture();
        let mut mismatched_candidate = candidate_diff_fixture();
        mismatched_candidate.projected_impact.replay_checksum = "sha256:tampered".to_string();
        let packet_error = ApprovalPacket::build(
            mismatched_candidate,
            replay_report,
            41,
            42,
            1200,
            "Fallback to policy version 41.",
        )
        .expect_err("checksum mismatch must fail packet contract");
        assert!(matches!(
            packet_error,
            ApprovalPacketValidationError::ReplayChecksumMismatch { .. }
        ));
    }

    #[test]
    fn apply_and_rollback_are_idempotent_with_queryable_audit_history() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");

        let apply_request = PolicyApplyRequest {
            packet: packet.clone(),
            action: approve.clone(),
            actor_id: "approver-1".to_string(),
            signature_key_id: "key-1".to_string(),
            signing_secret: "secret-1".to_string(),
            idempotency_key_override: None,
        };

        let first_apply = engine.apply(apply_request.clone()).expect("first apply should pass");
        assert_eq!(engine.current_policy_version(), 42);
        assert!(first_apply.apply_record.apply_signature.starts_with("sha256:"));
        assert_eq!(engine.list_lifecycle_events().len(), 1);

        let second_apply = engine.apply(apply_request).expect("idempotent apply should pass");
        assert_eq!(first_apply.apply_record.id, second_apply.apply_record.id);
        assert_eq!(engine.list_lifecycle_events().len(), 1);

        let rollback_request = PolicyRollbackRequest {
            apply_record_id: first_apply.apply_record.id.clone(),
            candidate_id: packet.candidate_id.clone(),
            rollback_reason: "drift detected in production cohort".to_string(),
            actor_id: "approver-1".to_string(),
            signature_key_id: "key-1".to_string(),
            signing_secret: "secret-1".to_string(),
            idempotency_key: "rollback-1".to_string(),
        };
        let first_rollback =
            engine.rollback(rollback_request.clone()).expect("rollback should pass");
        assert_eq!(engine.current_policy_version(), 41);
        assert!(first_rollback.rollback_record.rollback_signature.starts_with("sha256:"));
        assert_eq!(first_rollback.rollback_record.rollback_depth, 1);
        assert_eq!(engine.list_lifecycle_events().len(), 2);

        let second_rollback =
            engine.rollback(rollback_request).expect("idempotent rollback should pass");
        assert_eq!(first_rollback.rollback_record.id, second_rollback.rollback_record.id);
        assert_eq!(engine.list_lifecycle_events().len(), 2);

        let apply_lookup = engine
            .find_apply_record_by_applied_version(42)
            .expect("apply record should be queryable by applied version");
        assert_eq!(apply_lookup.id, first_apply.apply_record.id);

        let rollback_lookup = engine.list_rollbacks_by_target_version(41);
        assert_eq!(rollback_lookup.len(), 1);
        assert_eq!(rollback_lookup[0].id, first_rollback.rollback_record.id);

        let event_types = engine
            .list_lifecycle_events()
            .iter()
            .map(|event| event.event_type.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![PolicyLifecycleAuditEventType::Applied, PolicyLifecycleAuditEventType::RolledBack]
        );
    }

    #[test]
    fn rollback_drill_report_contains_timing_checksum_and_safety_artifacts() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");

        let report = engine
            .run_rollback_drill(
                packet,
                approve,
                "approver-1".to_string(),
                "key-1".to_string(),
                "secret-1".to_string(),
                "scheduled rollback drill".to_string(),
            )
            .expect("rollback drill should pass");

        assert_eq!(report.candidate_id, PolicyCandidateId("cand-101".to_string()));
        assert!(report.first_apply_verification_checksum.starts_with("sha256:"));
        assert!(report.rollback_verification_checksum.starts_with("sha256:"));
        assert!(report.reapply_verification_checksum.starts_with("sha256:"));
        let total_duration_ms = report
            .apply_duration_ms
            .saturating_add(report.rollback_duration_ms)
            .saturating_add(report.reapply_duration_ms);
        assert!(total_duration_ms < 60_000);
        assert!(report.safety_passed);
        assert_eq!(report.final_policy_version, 42);
        assert_eq!(engine.current_policy_version(), 42);
        assert_eq!(engine.list_lifecycle_events().len(), 3);
    }

    #[test]
    fn lifecycle_apply_idempotency_conflict_is_blocked() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");

        engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve.clone(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("shared-idempotency".to_string()),
            })
            .expect("first apply should pass");

        let replay_report = replay_report_fixture();
        let next_packet = ApprovalPacket::build(
            candidate_diff_with_replay_checksum(&replay_report.input_checksum),
            replay_report,
            42,
            43,
            1000,
            "Fallback to policy version 42.",
        )
        .expect("next packet should build");
        let next_approve =
            ApprovalPacketActionPayload::new(&next_packet, ApprovalPacketDecision::Approve, None)
                .expect("second approve action should build");

        let error = engine
            .apply(PolicyApplyRequest {
                packet: next_packet,
                action: next_approve,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("shared-idempotency".to_string()),
            })
            .expect_err("idempotency conflict must fail");
        assert_eq!(
            error,
            PolicyLifecycleError::ApplyIdempotencyConflict {
                idempotency_key: "shared-idempotency".to_string()
            }
        );
    }

    #[test]
    fn lifecycle_apply_idempotency_conflict_is_blocked_for_action_identity_change() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve_without_reason =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");

        engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve_without_reason,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("shared-idempotency".to_string()),
            })
            .expect("first apply should pass");

        let approve_with_reason = ApprovalPacketActionPayload::new(
            &packet,
            ApprovalPacketDecision::Approve,
            Some("human reviewer note".to_string()),
        )
        .expect("approve action with reason should build");
        let error = engine
            .apply(PolicyApplyRequest {
                packet,
                action: approve_with_reason,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("shared-idempotency".to_string()),
            })
            .expect_err("idempotency conflict must fail");
        assert_eq!(
            error,
            PolicyLifecycleError::ApplyIdempotencyConflict {
                idempotency_key: "shared-idempotency".to_string()
            }
        );
    }

    #[test]
    fn lifecycle_rollback_idempotency_conflict_is_blocked() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");
        let first_apply = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: None,
            })
            .expect("first apply should pass");

        engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: first_apply.apply_record.id.clone(),
                candidate_id: packet.candidate_id.clone(),
                rollback_reason: "first reason".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: "shared-rollback".to_string(),
            })
            .expect("first rollback should pass");

        let error = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: first_apply.apply_record.id,
                candidate_id: packet.candidate_id,
                rollback_reason: "different reason".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: "shared-rollback".to_string(),
            })
            .expect_err("rollback idempotency conflict must fail");
        assert_eq!(
            error,
            PolicyLifecycleError::RollbackIdempotencyConflict {
                idempotency_key: "shared-rollback".to_string()
            }
        );
    }

    #[test]
    fn lifecycle_rollback_uses_trimmed_idempotency_key_for_record_identity() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");
        let first_apply = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: None,
            })
            .expect("first apply should pass");

        let first_rollback = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: first_apply.apply_record.id.clone(),
                candidate_id: packet.candidate_id.clone(),
                rollback_reason: "manual".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: " rollback-key ".to_string(),
            })
            .expect("first rollback should pass");
        assert_eq!(first_rollback.rollback_record.id.0, "rollback:rollback-key");
        assert_eq!(first_rollback.rollback_record.idempotency_key, "rollback-key");

        let second_rollback = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: first_apply.apply_record.id,
                candidate_id: packet.candidate_id,
                rollback_reason: "manual".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: "rollback-key".to_string(),
            })
            .expect("second rollback should be idempotent");
        assert_eq!(second_rollback.rollback_record.id.0, "rollback:rollback-key");
    }

    #[test]
    fn lifecycle_missing_idempotency_key_is_rejected() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");

        let apply_error = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("   ".to_string()),
            })
            .expect_err("blank apply idempotency key must fail");
        assert_eq!(apply_error, PolicyLifecycleError::MissingIdempotencyKey);

        let rollback_error = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: super::PolicyApplyRecordId("apply:any".to_string()),
                candidate_id: packet.candidate_id,
                rollback_reason: "manual".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: "  ".to_string(),
            })
            .expect_err("blank rollback idempotency key must fail");
        assert_eq!(rollback_error, PolicyLifecycleError::MissingIdempotencyKey);
    }

    #[test]
    fn lifecycle_missing_signing_metadata_is_rejected() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");

        let missing_actor_error = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve.clone(),
                actor_id: "   ".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("apply-missing-actor".to_string()),
            })
            .expect_err("blank actor id must fail");
        assert_eq!(missing_actor_error, PolicyLifecycleError::MissingActorId);

        let missing_key_error = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve.clone(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "  ".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("apply-missing-key".to_string()),
            })
            .expect_err("blank signature key id must fail");
        assert_eq!(missing_key_error, PolicyLifecycleError::MissingSignatureKeyId);

        let missing_secret_error = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve.clone(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: " \n\t ".to_string(),
                idempotency_key_override: Some("apply-missing-secret".to_string()),
            })
            .expect_err("blank signing secret must fail");
        assert_eq!(missing_secret_error, PolicyLifecycleError::MissingSigningSecret);

        let first_apply = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: Some("apply-valid-for-rollback".to_string()),
            })
            .expect("valid apply should pass");

        let rollback_missing_actor_error = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: first_apply.apply_record.id.clone(),
                candidate_id: packet.candidate_id.clone(),
                rollback_reason: "manual".to_string(),
                actor_id: " ".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: "rollback-missing-actor".to_string(),
            })
            .expect_err("blank rollback actor id must fail");
        assert_eq!(rollback_missing_actor_error, PolicyLifecycleError::MissingActorId);

        let rollback_missing_key_error = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: first_apply.apply_record.id.clone(),
                candidate_id: packet.candidate_id.clone(),
                rollback_reason: "manual".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: " ".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: "rollback-missing-key".to_string(),
            })
            .expect_err("blank rollback signature key id must fail");
        assert_eq!(rollback_missing_key_error, PolicyLifecycleError::MissingSignatureKeyId);

        let rollback_missing_secret_error = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: first_apply.apply_record.id,
                candidate_id: packet.candidate_id,
                rollback_reason: "manual".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "\t ".to_string(),
                idempotency_key: "rollback-missing-secret".to_string(),
            })
            .expect_err("blank rollback signing secret must fail");
        assert_eq!(rollback_missing_secret_error, PolicyLifecycleError::MissingSigningSecret);
    }

    #[test]
    fn lifecycle_apply_signature_preserves_signing_secret_bytes() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(41);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");
        let signing_secret = " secret-with-boundary-space ".to_string();
        let idempotency_key = "signature-secret-idempotency".to_string();

        let apply_outcome = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: signing_secret.clone(),
                idempotency_key_override: Some(idempotency_key.clone()),
            })
            .expect("apply should pass");

        let signature_payload = format!(
            "{}|{}|{}|{}|{}",
            packet.packet_id,
            packet.base_policy_version,
            packet.proposed_policy_version,
            packet.replay_report.input_checksum,
            idempotency_key
        );
        let exact_secret_signature =
            sign_lifecycle_payload("key-1", &signing_secret, &signature_payload);
        let trimmed_secret_signature =
            sign_lifecycle_payload("key-1", signing_secret.trim(), &signature_payload);

        assert_eq!(apply_outcome.apply_record.apply_signature, exact_secret_signature);
        assert_ne!(apply_outcome.apply_record.apply_signature, trimmed_secret_signature);
    }

    #[test]
    fn lifecycle_failure_paths_return_user_safe_remediation_messages() {
        let mut engine = InMemoryPolicyLifecycleEngine::new(40);
        let packet = approval_packet_fixture();
        let approve =
            ApprovalPacketActionPayload::new(&packet, ApprovalPacketDecision::Approve, None)
                .expect("approve action should build");

        let apply_error = engine
            .apply(PolicyApplyRequest {
                packet: packet.clone(),
                action: approve,
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key_override: None,
            })
            .expect_err("base policy version mismatch must fail");
        assert!(matches!(
            apply_error,
            PolicyLifecycleError::BaseVersionMismatch { expected: 41, actual: 40 }
        ));
        assert_eq!(
            apply_error.user_safe_message(),
            "Apply blocked: expected base policy version 41, but current version is 40."
        );

        let rollback_error = engine
            .rollback(PolicyRollbackRequest {
                apply_record_id: super::PolicyApplyRecordId("apply:missing".to_string()),
                candidate_id: packet.candidate_id,
                rollback_reason: "   ".to_string(),
                actor_id: "approver-1".to_string(),
                signature_key_id: "key-1".to_string(),
                signing_secret: "secret-1".to_string(),
                idempotency_key: "rollback-missing".to_string(),
            })
            .expect_err("missing rollback reason must fail");
        assert_eq!(rollback_error, PolicyLifecycleError::MissingRollbackReason);
        assert_eq!(
            rollback_error.user_safe_message(),
            "Rollback blocked: provide a rollback reason for audit."
        );

        let key_error = PolicyLifecycleError::ApplyIdempotencyConflict {
            idempotency_key: "shared-idempotency".to_string(),
        };
        assert_eq!(
            key_error.user_safe_message(),
            "Apply blocked: idempotency key is already bound to a different apply payload."
        );
    }

    fn replay_request() -> ReplayImpactRequest {
        ReplayImpactRequest {
            candidate_id: PolicyCandidateId("cand-101".to_string()),
            base_policy_version: 41,
            proposed_policy_version: 42,
            policy_diff_json:
                r#"{"mutations":[{"op":"adjust","rule":"discount-cap","from":20,"to":18}]}"#
                    .to_string(),
            cohort_scope_json:
                r#"{"date_window":"90d","segments":["enterprise","smb"],"regions":["na"]}"#
                    .to_string(),
            engine_version: "optimizer-v1".to_string(),
            expected_input_checksum: None,
            snapshots: vec![
                ReplayQuoteSnapshot {
                    quote_id: "quote-2".to_string(),
                    cohort_id: "cohort-a".to_string(),
                    segment_key: "enterprise".to_string(),
                    impacted_rule_ids: vec![
                        "margin-floor".to_string(),
                        "discount-cap".to_string(),
                        "discount-cap".to_string(),
                    ],
                    baseline_margin_bps: 2_900,
                    candidate_margin_bps: 2_950,
                    baseline_win_rate_proxy_bps: 5_400,
                    candidate_win_rate_proxy_bps: 5_420,
                    baseline_approval_required: true,
                    candidate_approval_required: true,
                    baseline_hard_violation_count: 1,
                    candidate_hard_violation_count: 1,
                },
                ReplayQuoteSnapshot {
                    quote_id: "quote-1".to_string(),
                    cohort_id: "cohort-a".to_string(),
                    segment_key: "smb".to_string(),
                    impacted_rule_ids: vec!["discount-cap".to_string(), "margin-floor".to_string()],
                    baseline_margin_bps: 3_100,
                    candidate_margin_bps: 3_125,
                    baseline_win_rate_proxy_bps: 5_200,
                    candidate_win_rate_proxy_bps: 5_260,
                    baseline_approval_required: false,
                    candidate_approval_required: false,
                    baseline_hard_violation_count: 0,
                    candidate_hard_violation_count: 0,
                },
            ],
        }
    }

    fn candidate_diff_fixture() -> PolicyCandidateDiffV1 {
        let replay_report = replay_report_fixture();
        candidate_diff_with_replay_checksum(&replay_report.input_checksum)
    }

    fn candidate_diff_with_replay_checksum(replay_checksum: &str) -> PolicyCandidateDiffV1 {
        PolicyCandidateDiffV1 {
            schema_version: PolicyCandidateDiffV1::SCHEMA_VERSION.to_string(),
            candidate_id: PolicyCandidateId("cand-101".to_string()),
            rule_diffs: vec![
                super::CandidateRuleDiff {
                    rule_id: "margin-floor".to_string(),
                    operation: CandidateRuleOperation::Update,
                    field: "min_margin_pct".to_string(),
                    from_value_json: Some("{\"value\":25}".to_string()),
                    to_value_json: Some("{\"value\":27}".to_string()),
                    rationale: "raise floor for risky segments".to_string(),
                },
                super::CandidateRuleDiff {
                    rule_id: "discount-cap".to_string(),
                    operation: CandidateRuleOperation::Update,
                    field: "threshold".to_string(),
                    from_value_json: Some("{\"value\":20}".to_string()),
                    to_value_json: Some("{\"value\":18}".to_string()),
                    rationale: "tighten discretionary cap".to_string(),
                },
            ],
            cohort_scope: CandidateCohortScope {
                segment_keys: vec!["enterprise".to_string(), "smb".to_string()],
                region_keys: vec!["na".to_string()],
                quote_ids: vec![],
                time_window_days: 90,
            },
            projected_impact: super::CandidateProjectedImpact {
                replay_checksum: replay_checksum.to_string(),
                replay_deterministic_pass: true,
                projected_margin_delta_bps: 55,
                projected_win_rate_proxy_delta_bps: 20,
                projected_approval_load_delta_bps: 0,
                projected_hard_violation_delta: 0,
            },
            confidence_bounds: CandidateConfidenceBounds {
                lower_bps: 6200,
                point_estimate_bps: 7100,
                upper_bps: 7800,
            },
            provenance: CandidateProvenance {
                source_replay_evaluation_ids: vec!["replay-cand-101-v1".to_string()],
                source_outcome_window: "2025-Q4".to_string(),
                generated_by: "policy-candidate-generator-v1".to_string(),
            },
            rationale_summary:
                "Candidate cand-101 proposes 2 rule changes with positive margin/win projections."
                    .to_string(),
        }
    }

    fn replay_report_fixture() -> ReplayImpactReport {
        PolicyReplayEngine::default()
            .evaluate(replay_request())
            .expect("replay fixture should pass")
    }

    fn approval_packet_fixture() -> ApprovalPacket {
        let replay_report = replay_report_fixture();
        ApprovalPacket::build(
            candidate_diff_with_replay_checksum(&replay_report.input_checksum),
            replay_report,
            41,
            42,
            1250,
            "Fallback to version 41, pause apply jobs, and notify #deal-desk.",
        )
        .expect("packet fixture should build")
    }
}
