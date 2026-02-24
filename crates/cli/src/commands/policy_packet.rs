use crate::commands::CommandResult;
use quotey_core::chrono::Utc;
use quotey_core::policy::optimizer::{
    ApprovalPacket, ApprovalPacketActionPayload, ApprovalPacketDecision, PolicyCandidateDiffV1,
    ReplayImpactReport,
};
use serde::Serialize;

pub fn run_build(
    candidate_diff_json: String,
    replay_report_json: String,
    base_policy_version: i32,
    proposed_policy_version: i32,
    risk_score_bps: i32,
    fallback_plan: String,
) -> CommandResult {
    let candidate_diff: PolicyCandidateDiffV1 = match serde_json::from_str(&candidate_diff_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-build",
                "candidate_diff_parse",
                format!("invalid candidate diff json: {error}"),
                2,
            );
        }
    };

    let replay_report: ReplayImpactReport = match serde_json::from_str(&replay_report_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-build",
                "replay_report_parse",
                format!("invalid replay report json: {error}"),
                3,
            );
        }
    };

    let packet = match ApprovalPacket::build(
        candidate_diff,
        replay_report,
        base_policy_version,
        proposed_policy_version,
        risk_score_bps,
        fallback_plan,
    ) {
        Ok(packet) => packet,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-build",
                "packet_validation",
                error.to_string(),
                4,
            );
        }
    };

    let packet_json = match packet.canonical_json() {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-build",
                "packet_serialization",
                error.to_string(),
                5,
            );
        }
    };

    #[derive(Serialize)]
    struct BuildOutput<'a> {
        command: &'static str,
        packet_id: &'a str,
        packet_version: &'a str,
        packet_json: &'a str,
    }

    let payload = BuildOutput {
        command: "policy-packet-build",
        packet_id: &packet.packet_id,
        packet_version: &packet.schema_version,
        packet_json: &packet_json,
    };

    let output = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"policy-packet-build\",\"status\":\"error\",\"error\":\"{}\"}}",
            escape_json(&error.to_string())
        )
    });

    CommandResult { exit_code: 0, output }
}

pub fn run_action(packet_json: String, decision: String, reason: Option<String>) -> CommandResult {
    let packet: ApprovalPacket = match serde_json::from_str(&packet_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-action",
                "packet_parse",
                format!("invalid packet json: {error}"),
                2,
            );
        }
    };

    let Some(decision) = ApprovalPacketDecision::parse(&decision) else {
        return CommandResult::failure(
            "policy-packet-action",
            "decision_parse",
            format!("invalid decision `{}`; expected approve|reject|request_changes", decision),
            3,
        );
    };

    let action = match ApprovalPacketActionPayload::new(&packet, decision, reason) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-action",
                "action_validation",
                error.to_string(),
                4,
            );
        }
    };

    let action_json = match action.to_json() {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-action",
                "action_serialization",
                error.to_string(),
                5,
            );
        }
    };

    let audit_event = match action.to_audit_event("cli-reviewer", "cli-policy-packet", Utc::now()) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "policy-packet-action",
                "audit_event",
                error.to_string(),
                6,
            );
        }
    };

    #[derive(Serialize)]
    struct ActionOutput<'a> {
        command: &'static str,
        action_json: &'a str,
        target_status: &'a str,
        audit_event_type: &'a str,
        audit_event_idempotency_key: Option<&'a str>,
    }

    let target_status = action.target_status();
    let payload = ActionOutput {
        command: "policy-packet-action",
        action_json: &action_json,
        target_status: target_status.as_str(),
        audit_event_type: audit_event.event_type.as_str(),
        audit_event_idempotency_key: audit_event.idempotency_key.as_deref(),
    };

    let output = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"policy-packet-action\",\"status\":\"error\",\"error\":\"{}\"}}",
            escape_json(&error.to_string())
        )
    });

    CommandResult { exit_code: 0, output }
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{run_action, run_build};
    use quotey_core::policy::optimizer::{PolicyCandidateDiffV1, PolicyReplayEngine};

    #[test]
    fn run_build_returns_packet_json() {
        let replay_report = replay_report_fixture();
        let candidate_diff =
            serde_json::to_string(&candidate_diff_fixture(&replay_report.input_checksum))
                .expect("candidate diff fixture should serialize");
        let replay_report =
            serde_json::to_string(&replay_report).expect("replay report fixture should serialize");

        let result = run_build(candidate_diff, replay_report, 41, 42, 1200, "fallback".to_string());
        assert_eq!(result.exit_code, 0);
        assert!(result.output.contains("\"packet_id\""));
        assert!(result.output.contains("\"packet_json\""));
    }

    #[test]
    fn run_action_requires_reason_for_reject() {
        let replay_report = replay_report_fixture();
        let candidate_diff =
            serde_json::to_string(&candidate_diff_fixture(&replay_report.input_checksum))
                .expect("candidate diff fixture should serialize");
        let replay_report =
            serde_json::to_string(&replay_report).expect("replay report fixture should serialize");
        let build = run_build(candidate_diff, replay_report, 41, 42, 1200, "fallback".to_string());
        assert_eq!(build.exit_code, 0);

        let payload: serde_json::Value =
            serde_json::from_str(&build.output).expect("build output should be json");
        let packet_json = payload
            .get("packet_json")
            .and_then(serde_json::Value::as_str)
            .expect("packet_json field should be present")
            .to_string();

        let reject_without_reason = run_action(packet_json, "reject".to_string(), None);
        assert_ne!(reject_without_reason.exit_code, 0);
        assert!(reject_without_reason.output.contains("reason"));
    }

    fn candidate_diff_fixture(replay_checksum: &str) -> PolicyCandidateDiffV1 {
        let mut payload: PolicyCandidateDiffV1 = serde_json::from_str(
            r#"{
                "schema_version":"clo_candidate_diff.v1",
                "candidate_id":"cand-101",
                "rule_diffs":[
                    {
                        "rule_id":"discount-cap",
                        "operation":"update",
                        "field":"threshold",
                        "from_value_json":"{\"value\":20}",
                        "to_value_json":"{\"value\":18}",
                        "rationale":"tighten discount cap"
                    }
                ],
                "cohort_scope":{
                    "segment_keys":["smb"],
                    "region_keys":["na"],
                    "quote_ids":[],
                    "time_window_days":90
                },
                "projected_impact":{
                    "replay_checksum":"sha256:placeholder",
                    "replay_deterministic_pass":true,
                    "projected_margin_delta_bps":55,
                    "projected_win_rate_proxy_delta_bps":20,
                    "projected_approval_load_delta_bps":0,
                    "projected_hard_violation_delta":0
                },
                "confidence_bounds":{
                    "lower_bps":6200,
                    "point_estimate_bps":7100,
                    "upper_bps":7800
                },
                "provenance":{
                    "source_replay_evaluation_ids":["replay-1"],
                    "source_outcome_window":"2025-Q4",
                    "generated_by":"cli-test"
                },
                "rationale_summary":"deterministic fixture"
            }"#,
        )
        .expect("candidate diff fixture should deserialize");
        payload.projected_impact.replay_checksum = replay_checksum.to_string();
        payload
    }

    fn replay_report_fixture() -> quotey_core::policy::optimizer::ReplayImpactReport {
        PolicyReplayEngine::default()
            .evaluate(quotey_core::policy::optimizer::ReplayImpactRequest {
                candidate_id: quotey_core::domain::optimizer::PolicyCandidateId(
                    "cand-101".to_string(),
                ),
                base_policy_version: 41,
                proposed_policy_version: 42,
                policy_diff_json: "{\"rule\":\"discount-cap\"}".to_string(),
                cohort_scope_json: "{\"segments\":[\"smb\"]}".to_string(),
                engine_version: "optimizer-v1".to_string(),
                expected_input_checksum: None,
                snapshots: vec![quotey_core::policy::optimizer::ReplayQuoteSnapshot {
                    quote_id: "q-1".to_string(),
                    cohort_id: "cohort-a".to_string(),
                    segment_key: "smb".to_string(),
                    impacted_rule_ids: vec!["discount-cap".to_string()],
                    baseline_margin_bps: 3000,
                    candidate_margin_bps: 3050,
                    baseline_win_rate_proxy_bps: 5200,
                    candidate_win_rate_proxy_bps: 5220,
                    baseline_approval_required: false,
                    candidate_approval_required: false,
                    baseline_hard_violation_count: 0,
                    candidate_hard_violation_count: 0,
                }],
            })
            .expect("replay fixture should evaluate")
    }
}
