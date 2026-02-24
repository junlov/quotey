pub mod optimizer;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::cpq::policy::PolicyViolation;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplanationTemplate {
    pub rule_id: String,
    pub default_template: String,
    pub role_templates: HashMap<String, String>,
    pub resolution_paths: Vec<String>,
    pub documentation_url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneratedExplanation {
    pub rule_id: String,
    pub citation: String,
    pub role: String,
    pub summary: String,
    pub resolution_paths: Vec<String>,
    pub documentation_url: String,
}

#[derive(Clone, Debug, Default)]
pub struct ExplanationGenerator {
    templates: HashMap<String, ExplanationTemplate>,
}

impl ExplanationGenerator {
    pub fn new(templates: Vec<ExplanationTemplate>) -> Self {
        let templates = templates
            .into_iter()
            .map(|template| (normalize_key(&template.rule_id), template))
            .collect();
        Self { templates }
    }

    pub fn generate(
        &self,
        violation: &PolicyViolation,
        role: &str,
        variables: &HashMap<String, String>,
    ) -> GeneratedExplanation {
        let role_key = normalize_key(role);
        let template = self.templates.get(&normalize_key(&violation.policy_id));

        let mut merged_variables = HashMap::new();
        merged_variables.insert("rule_id".to_string(), violation.policy_id.clone());
        merged_variables.insert("reason".to_string(), violation.reason.clone());
        if let Some(required_approval) = &violation.required_approval {
            merged_variables.insert("required_approval".to_string(), required_approval.clone());
        }
        for (key, value) in variables {
            merged_variables.insert(key.clone(), value.clone());
        }

        let summary_template = template
            .and_then(|entry| entry.role_templates.get(&role_key))
            .cloned()
            .or_else(|| template.map(|entry| entry.default_template.clone()))
            .unwrap_or_else(|| "{{reason}}".to_string());

        let mut resolution_paths = template
            .map(|entry| {
                entry
                    .resolution_paths
                    .iter()
                    .map(|path| substitute_variables(path, &merged_variables))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if let Some(required_approval) = merged_variables.get("required_approval") {
            resolution_paths.push(format!("Request approval from {}", required_approval));
        }

        let documentation_url = template
            .map(|entry| entry.documentation_url.clone())
            .unwrap_or_else(|| format!("/policies/{}", violation.policy_id));

        GeneratedExplanation {
            rule_id: violation.policy_id.clone(),
            citation: format!("Rule {}", violation.policy_id),
            role: role.to_string(),
            summary: substitute_variables(&summary_template, &merged_variables),
            resolution_paths,
            documentation_url,
        }
    }

    pub fn generate_all(
        &self,
        violations: &[PolicyViolation],
        role: &str,
        variables: &HashMap<String, String>,
    ) -> Vec<GeneratedExplanation> {
        violations.iter().map(|violation| self.generate(violation, role, variables)).collect()
    }
}

fn substitute_variables(template: &str, variables: &HashMap<String, String>) -> String {
    let mut output = template.to_string();
    for (key, value) in variables {
        output = output.replace(&format!("{{{{{key}}}}}"), value);
    }
    output
}

fn normalize_key(input: &str) -> String {
    input.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::cpq::policy::PolicyViolation;

    use super::{ExplanationGenerator, ExplanationTemplate};

    #[test]
    fn generates_template_based_explanation_with_variable_substitution() {
        let generator = ExplanationGenerator::new(vec![discount_template()]);
        let violation = discount_violation();
        let variables = HashMap::from([
            ("discount_pct".to_string(), "25%".to_string()),
            ("limit_pct".to_string(), "20%".to_string()),
        ]);

        let explanation = generator.generate(&violation, "sales_rep", &variables);

        assert_eq!(explanation.rule_id, "discount-cap");
        assert!(explanation.summary.contains("25%"));
        assert!(explanation.summary.contains("20%"));
    }

    #[test]
    fn role_based_template_overrides_default_copy() {
        let generator = ExplanationGenerator::new(vec![discount_template()]);
        let violation = discount_violation();
        let variables = HashMap::new();

        let manager_explanation = generator.generate(&violation, "sales_manager", &variables);
        let rep_explanation = generator.generate(&violation, "sales_rep", &variables);

        assert!(manager_explanation.summary.starts_with("Manager guidance:"));
        assert!(!rep_explanation.summary.starts_with("Manager guidance:"));
    }

    #[test]
    fn includes_rule_citation_doc_link_and_resolution_paths() {
        let generator = ExplanationGenerator::new(vec![discount_template()]);
        let violation = discount_violation();
        let variables = HashMap::new();

        let explanation = generator.generate(&violation, "sales_rep", &variables);

        assert_eq!(explanation.citation, "Rule discount-cap");
        assert_eq!(explanation.documentation_url, "/policies/pricing#discount-cap");
        assert!(explanation.resolution_paths.len() >= 2);
        assert!(explanation.resolution_paths.iter().any(|path| path.contains("sales_manager")));
    }

    #[test]
    fn falls_back_to_violation_reason_when_template_missing() {
        let generator = ExplanationGenerator::default();
        let violation = PolicyViolation {
            policy_id: "margin-floor".to_string(),
            reason: "Margin floor breached".to_string(),
            required_approval: None,
        };
        let variables = HashMap::new();

        let explanation = generator.generate(&violation, "sales_rep", &variables);

        assert_eq!(explanation.summary, "Margin floor breached");
        assert_eq!(explanation.documentation_url, "/policies/margin-floor");
    }

    fn discount_template() -> ExplanationTemplate {
        ExplanationTemplate {
            rule_id: "discount-cap".to_string(),
            default_template:
                "Discount {{discount_pct}} exceeds allowed limit {{limit_pct}}. {{reason}}"
                    .to_string(),
            role_templates: HashMap::from([(
                "sales_manager".to_string(),
                "Manager guidance: review override rationale before approving {{discount_pct}}."
                    .to_string(),
            )]),
            resolution_paths: vec![
                "Reduce discount to {{limit_pct}}".to_string(),
                "Request escalation from {{required_approval}}".to_string(),
            ],
            documentation_url: "/policies/pricing#discount-cap".to_string(),
        }
    }

    fn discount_violation() -> PolicyViolation {
        PolicyViolation {
            policy_id: "discount-cap".to_string(),
            reason: "Requested discount exceeds auto-approval threshold".to_string(),
            required_approval: Some("sales_manager".to_string()),
        }
    }
}
