use crate::commands::CommandResult;
use quotey_core::autopsy::{
    AttributionGraphBuilder, AutopsyInput, CounterfactualEngine, CounterfactualRequest,
    DealAutopsyEngine, GenomeQueryRequest, RevenueGenomeQueryEngine,
};
use serde::Serialize;

pub fn run_autopsy(input_json: String) -> CommandResult {
    let input: AutopsyInput = match serde_json::from_str(&input_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "genome-autopsy",
                "input_parse",
                format!("invalid autopsy input json: {error}"),
                2,
            );
        }
    };

    let engine = DealAutopsyEngine::default();
    let report = match engine.perform(input) {
        Ok(report) => report,
        Err(error) => {
            return CommandResult::failure("genome-autopsy", "autopsy_error", error.to_string(), 3);
        }
    };

    #[derive(Serialize)]
    struct AutopsyOutput<'a> {
        command: &'static str,
        autopsy_id: &'a str,
        quote_id: &'a str,
        outcome: &'a str,
        fork_count: usize,
        score_count: usize,
        checksum: &'a str,
    }

    let payload = AutopsyOutput {
        command: "genome-autopsy",
        autopsy_id: &report.autopsy.id.0,
        quote_id: &report.autopsy.quote_id.0,
        outcome: report.autopsy.outcome_status.as_str(),
        fork_count: report.forks.len(),
        score_count: report.scores.len(),
        checksum: &report.checksum,
    };

    let output = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"genome-autopsy\",\"status\":\"error\",\"error\":\"{}\"}}",
            escape_json(&error.to_string())
        )
    });

    CommandResult { exit_code: 0, output }
}

pub fn run_query(query_json: String, graph_json: String) -> CommandResult {
    let request: GenomeQueryRequest = match serde_json::from_str(&query_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "genome-query",
                "query_parse",
                format!("invalid query json: {error}"),
                2,
            );
        }
    };

    let graph = match serde_json::from_str(&graph_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "genome-query",
                "graph_parse",
                format!("invalid graph json: {error}"),
                3,
            );
        }
    };

    let engine = RevenueGenomeQueryEngine;
    let response = match engine.query(&request, &graph) {
        Ok(response) => response,
        Err(error) => {
            return CommandResult::failure("genome-query", "query_error", error.to_string(), 4);
        }
    };

    #[derive(Serialize)]
    struct QueryOutput {
        command: &'static str,
        query_type: String,
        segments_analyzed: i32,
        evidence_count: i32,
        findings_count: usize,
        result_checksum: String,
        query_duration_ms: i64,
    }

    let payload = QueryOutput {
        command: "genome-query",
        query_type: response.query_type.as_str().to_string(),
        segments_analyzed: response.segments_analyzed,
        evidence_count: response.evidence_count,
        findings_count: response.findings.len(),
        result_checksum: response.result_checksum,
        query_duration_ms: response.query_duration_ms,
    };

    let output = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"genome-query\",\"status\":\"error\",\"error\":\"{}\"}}",
            escape_json(&error.to_string())
        )
    });

    CommandResult { exit_code: 0, output }
}

pub fn run_counterfactual(
    request_json: String,
    original_report_json: String,
    graph_json: String,
) -> CommandResult {
    let request: CounterfactualRequest = match serde_json::from_str(&request_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "genome-counterfactual",
                "request_parse",
                format!("invalid request json: {error}"),
                2,
            );
        }
    };

    let original_report = match serde_json::from_str(&original_report_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "genome-counterfactual",
                "report_parse",
                format!("invalid report json: {error}"),
                3,
            );
        }
    };

    let graph = match serde_json::from_str(&graph_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "genome-counterfactual",
                "graph_parse",
                format!("invalid graph json: {error}"),
                4,
            );
        }
    };

    let engine = CounterfactualEngine;
    let response = match engine.simulate(&request, &original_report, &graph) {
        Ok(response) => response,
        Err(error) => {
            return CommandResult::failure(
                "genome-counterfactual",
                "simulation_error",
                error.to_string(),
                5,
            );
        }
    };

    #[derive(Serialize)]
    struct CounterfactualOutput {
        command: &'static str,
        simulation_id: String,
        projected_outcome: String,
        projected_margin_delta_bps: i32,
        confidence_bps: i32,
        comparisons: usize,
        replay_checksum: String,
    }

    let payload = CounterfactualOutput {
        command: "genome-counterfactual",
        simulation_id: response.simulation.id.0,
        projected_outcome: response.simulation.projected_outcome_status.as_str().to_string(),
        projected_margin_delta_bps: response.simulation.projected_margin_delta_bps,
        confidence_bps: response.simulation.confidence_bps,
        comparisons: response.comparison.len(),
        replay_checksum: response.simulation.replay_checksum,
    };

    let output = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"genome-counterfactual\",\"status\":\"error\",\"error\":\"{}\"}}",
            escape_json(&error.to_string())
        )
    });

    CommandResult { exit_code: 0, output }
}

pub fn run_build_graph(reports_json: String) -> CommandResult {
    let inputs: Vec<AutopsyInput> = match serde_json::from_str(&reports_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "genome-build-graph",
                "inputs_parse",
                format!("invalid autopsy inputs json: {error}"),
                2,
            );
        }
    };

    let engine = DealAutopsyEngine::default();
    let mut reports = Vec::with_capacity(inputs.len());
    for (idx, input) in inputs.into_iter().enumerate() {
        match engine.perform(input) {
            Ok(report) => reports.push(report),
            Err(error) => {
                return CommandResult::failure(
                    "genome-build-graph",
                    "autopsy_error",
                    format!("autopsy failed at index {}: {}", idx, error),
                    3,
                );
            }
        }
    }

    let builder = AttributionGraphBuilder;
    let graph = builder.build_from_reports(&reports);

    #[derive(Serialize)]
    struct GraphOutput {
        command: &'static str,
        total_autopsies: i32,
        node_count: usize,
        edge_count: usize,
        checksum: String,
    }

    let payload = GraphOutput {
        command: "genome-build-graph",
        total_autopsies: graph.total_autopsies,
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
        checksum: graph.checksum.clone(),
    };

    let output = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"genome-build-graph\",\"status\":\"error\",\"error\":\"{}\"}}",
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
    use super::*;
    use quotey_core::autopsy::AutopsyInput;
    use quotey_core::domain::autopsy::{AuditRefType, DealOutcomeType, DecisionStage};

    fn test_input_json() -> String {
        serde_json::to_string(&AutopsyInput {
            quote_id: "Q-2026-0042".to_string(),
            outcome_status: DealOutcomeType::Won,
            outcome_value_bps: 2500,
            outcome_revenue_cents: 1_296_000,
            audit_trail: vec![quotey_core::autopsy::AuditTrailEntry {
                entry_id: "audit-001".to_string(),
                entry_type: AuditRefType::PricingTrace,
                stage: DecisionStage::Pricing,
                action_summary: "Price book selection".to_string(),
                decision_data_json: r#"{"price_book":"enterprise_us"}"#.to_string(),
                alternatives_json: "[]".to_string(),
                timestamp: quotey_core::chrono::Utc::now(),
            }],
            segment_key: "enterprise".to_string(),
            idempotency_key: "idem-test-1".to_string(),
        })
        .expect("test input should serialize")
    }

    #[test]
    fn run_autopsy_succeeds_with_valid_input() {
        let result = run_autopsy(test_input_json());
        assert_eq!(result.exit_code, 0);
        assert!(result.output.contains("\"autopsy_id\""));
        assert!(result.output.contains("\"checksum\""));
    }

    #[test]
    fn run_autopsy_fails_with_invalid_json() {
        let result = run_autopsy("not-json".to_string());
        assert_ne!(result.exit_code, 0);
        assert!(result.output.contains("input_parse"));
    }

    #[test]
    fn run_build_graph_succeeds_with_valid_inputs() {
        let inputs = format!("[{}]", test_input_json());
        let result = run_build_graph(inputs);
        assert_eq!(result.exit_code, 0);
        assert!(result.output.contains("\"node_count\""));
        assert!(result.output.contains("\"edge_count\""));
    }
}
