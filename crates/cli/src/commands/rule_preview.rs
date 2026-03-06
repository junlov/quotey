use crate::commands::CommandResult;
use quotey_core::{
    build_pricing_rule, preview_pricing_rule, PricingRulePreviewInput, VisualRuleDefinition,
    VisualRuleType,
};
use serde::Serialize;

pub fn run(rule_json: String, samples_json: String) -> CommandResult {
    let visual_rule: VisualRuleDefinition = match serde_json::from_str(&rule_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "rule-preview",
                "rule_parse",
                format!("invalid visual rule json: {error}"),
                2,
            );
        }
    };

    if visual_rule.rule_type != VisualRuleType::Pricing {
        return CommandResult::failure(
            "rule-preview",
            "unsupported_rule_type",
            format!(
                "rule preview currently supports pricing rules only; got {:?}",
                visual_rule.rule_type
            ),
            3,
        );
    }

    let rule_draft = match build_pricing_rule(&visual_rule) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "rule-preview",
                "rule_build",
                format!("failed to build pricing rule: {error}"),
                4,
            );
        }
    };

    let samples: Vec<PricingRulePreviewInput> = match serde_json::from_str(&samples_json) {
        Ok(value) => value,
        Err(error) => {
            return CommandResult::failure(
                "rule-preview",
                "samples_parse",
                format!("invalid samples json: {error}"),
                5,
            );
        }
    };

    let preview = preview_pricing_rule(&rule_draft, &samples);

    #[derive(Serialize)]
    struct RulePreviewOutput<'a> {
        command: &'static str,
        rule_id: &'a str,
        sample_count: usize,
        affected_count: usize,
        sql_preview: &'a str,
        affected_quote_ids: &'a [String],
        cases: &'a [quotey_core::PricingRulePreviewCase],
    }

    let payload = RulePreviewOutput {
        command: "rule-preview",
        rule_id: &preview.rule_id,
        sample_count: samples.len(),
        affected_count: preview.affected_quote_ids.len(),
        sql_preview: &preview.sql_preview,
        affected_quote_ids: &preview.affected_quote_ids,
        cases: &preview.cases,
    };

    let output = serde_json::to_string_pretty(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"rule-preview\",\"status\":\"error\",\"error\":\"{}\"}}",
            error.to_string().replace('\\', "\\\\").replace('"', "\\\"")
        )
    });

    CommandResult { exit_code: 0, output }
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn run_returns_preview_for_pricing_rule() {
        let rule_json = r#"{
  "schema_version":"visual_rule.v1",
  "id":"price-enterprise",
  "name":"Enterprise price",
  "description":null,
  "rule_type":"pricing",
  "enabled":true,
  "priority":10,
  "conditions":[
    {"field_key":"customer_segment","operator":"equals","value":"enterprise","connector":null}
  ],
  "actions":[
    {"action_type":"set_unit_price","parameters":{"amount":"90.00","currency":"USD"}}
  ],
  "metadata":{"created_by":"ops","updated_by":"ops","tags":[],"rationale":null}
}"#;

        let samples_json = r#"[
  {
    "quote_id":"Q-1",
    "context_fields":{"customer_segment":"enterprise"},
    "quantity":2,
    "unit_price":"100.00",
    "discount_pct":"10.0"
  },
  {
    "quote_id":"Q-2",
    "context_fields":{"customer_segment":"smb"},
    "quantity":1,
    "unit_price":"100.00",
    "discount_pct":"0"
  }
]"#;

        let result = run(rule_json.to_string(), samples_json.to_string());
        assert_eq!(result.exit_code, 0);
        assert!(result.output.contains("\"command\": \"rule-preview\""));
        assert!(result.output.contains("\"affected_count\": 1"));
        assert!(result.output.contains("UPDATE quote_line SET unit_price = 90"));
    }

    #[test]
    fn run_rejects_non_pricing_rule() {
        let rule_json = r#"{
  "schema_version":"visual_rule.v1",
  "id":"policy-smb",
  "name":"SMB discount",
  "description":null,
  "rule_type":"discount_policy",
  "enabled":true,
  "priority":10,
  "conditions":[
    {"field_key":"customer_segment","operator":"equals","value":"smb","connector":null}
  ],
  "actions":[
    {"action_type":"apply_discount_cap","parameters":{"max_discount_pct":"15"}}
  ],
  "metadata":{"created_by":"ops","updated_by":"ops","tags":[],"rationale":null}
}"#;

        let result = run(rule_json.to_string(), "[]".to_string());
        assert_eq!(result.exit_code, 3);
        assert!(result.output.contains("unsupported_rule_type"));
    }
}
