use quotey_core::{RequirementSourceType, REQUIREMENT_EXTRACTION_SCHEMA_VERSION};

pub fn build_requirement_extraction_prompt(
    source_type: RequirementSourceType,
    source_text: &str,
) -> String {
    let schema_version = REQUIREMENT_EXTRACTION_SCHEMA_VERSION;
    let source_label = match source_type {
        RequirementSourceType::Email => "email",
        RequirementSourceType::Rfp => "rfp",
        RequirementSourceType::SlackThread => "slack_thread",
    };

    format!(
        "You are Quotey's extraction translator.\n\
Return ONLY valid JSON.\n\
Do not include markdown fences.\n\
Never invent prices, discounts, approvals, or policy decisions.\n\
You may only extract requirement candidates and ambiguities from provided text.\n\n\
Output schema requirements:\n\
- schema_version must be \"{schema_version}\"\n\
- source_type must be \"{source_label}\"\n\
- sender_hint: optional sender identity (email/name/org) when detectable\n\
- context_hint: optional short context summary (deal type, urgency, renewal/new business)\n\
- requirements[]: each item includes requirement_type, name, quantity (optional), confidence (0.0-1.0), raw_excerpt (optional)\n\
- ambiguities[]: each item includes text, question, options (>=2), confidence (0.0-1.0)\n\
- missing_info[]: unresolved required fields as short strings\n\n\
Requirement typing guide:\n\
- product: specific product or add-on mention\n\
- feature: capability request (for example SSO, SOC2)\n\
- billing: term/billing cadence requirement\n\
- service: onboarding/support/implementation requirement\n\
- compliance: security or legal requirement\n\n\
Confidence rubric:\n\
- 0.90-1.00: explicit and unambiguous\n\
- 0.70-0.89: strong inference from context\n\
- 0.40-0.69: plausible but ambiguous\n\
- below 0.40: omit item and report as ambiguity or missing_info\n\n\
Text to extract from:\n\
{source_text}"
    )
}

#[cfg(test)]
mod tests {
    use super::build_requirement_extraction_prompt;
    use quotey_core::{RequirementSourceType, REQUIREMENT_EXTRACTION_SCHEMA_VERSION};

    #[test]
    fn prompt_contains_schema_and_source_constraints() {
        let prompt =
            build_requirement_extraction_prompt(RequirementSourceType::Email, "Need 100 seats.");

        assert!(prompt.contains(REQUIREMENT_EXTRACTION_SCHEMA_VERSION));
        assert!(prompt.contains("source_type must be \"email\""));
        assert!(prompt.contains("sender_hint: optional sender identity"));
        assert!(prompt.contains("context_hint: optional short context summary"));
        assert!(prompt.contains("Return ONLY valid JSON."));
    }

    #[test]
    fn prompt_forbids_pricing_or_policy_decisions() {
        let prompt = build_requirement_extraction_prompt(
            RequirementSourceType::Rfp,
            "Need enterprise and 15% discount approval.",
        );

        assert!(prompt.contains("Never invent prices, discounts, approvals, or policy decisions."));
        assert!(prompt.contains("Text to extract from"));
    }
}
