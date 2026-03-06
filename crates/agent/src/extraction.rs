use anyhow::{anyhow, Context, Result};
use quotey_core::{ExtractedRequirements, RequirementSourceType};
use serde::Deserialize;
use std::time::Duration;
use tokio::time::timeout;

use crate::llm::LlmClient;
use crate::prompts::build_requirement_extraction_prompt;

/// Maximum size of LLM response to parse (10MB) - prevents DoS from malicious responses
const MAX_EXTRACTION_RESPONSE_SIZE: usize = 10 * 1024 * 1024;
/// Maximum time to wait for LLM extraction (60 seconds)
const EXTRACTION_TIMEOUT: Duration = Duration::from_secs(60);
/// Maximum iterations for markdown fence parsing (prevents infinite loops)
const MAX_FENCE_ITERATIONS: usize = 1000;

pub async fn parse_requirements(
    client: &dyn LlmClient,
    source_type: RequirementSourceType,
    source_text: &str,
) -> Result<ExtractedRequirements> {
    let prompt = build_requirement_extraction_prompt(source_type, source_text);
    let response = timeout(EXTRACTION_TIMEOUT, client.complete(&prompt))
        .await
        .context("LLM extraction timed out after 60 seconds")?
        .context("requirement extraction completion failed")?;

    // Check response size to prevent DoS
    if response.len() > MAX_EXTRACTION_RESPONSE_SIZE {
        return Err(anyhow!(
            "extraction response too large: {} bytes (max {})",
            response.len(),
            MAX_EXTRACTION_RESPONSE_SIZE
        ));
    }

    let mut extracted: ExtractedRequirements = parse_extracted_requirements(&response)
        .context("requirement extraction returned invalid JSON")?;

    extracted.sender_hint = normalize_optional_hint(extracted.sender_hint);
    extracted.context_hint = normalize_optional_hint(extracted.context_hint);

    if extracted.source_type != source_type {
        return Err(anyhow!(
            "requirement extraction source mismatch: expected {:?}, got {:?}",
            source_type,
            extracted.source_type
        ));
    }

    extracted.validate().context("requirement extraction payload failed validation")?;
    Ok(extracted)
}

pub async fn parse_email_requirements(
    client: &dyn LlmClient,
    source_text: &str,
) -> Result<ExtractedRequirements> {
    parse_requirements(client, RequirementSourceType::Email, source_text).await
}

pub async fn parse_rfp_requirements(
    client: &dyn LlmClient,
    source_text: &str,
) -> Result<ExtractedRequirements> {
    parse_requirements(client, RequirementSourceType::Rfp, source_text).await
}

fn parse_extracted_requirements(response: &str) -> Result<ExtractedRequirements> {
    if let Ok(parsed) = serde_json::from_str::<ExtractedRequirements>(response) {
        return Ok(parsed);
    }

    for payload in extract_markdown_code_fence_payloads(response) {
        if let Ok(parsed) = parse_first_json_value(payload) {
            return Ok(parsed);
        }
    }

    for (idx, ch) in response.char_indices() {
        if ch != '{' && ch != '[' {
            continue;
        }
        if let Ok(parsed) = parse_first_json_value(&response[idx..]) {
            return Ok(parsed);
        }
    }

    Err(anyhow!("response did not contain valid JSON object"))
}

fn parse_first_json_value(candidate: &str) -> Result<ExtractedRequirements, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(candidate);
    ExtractedRequirements::deserialize(&mut deserializer)
}

fn extract_markdown_code_fence_payloads(response: &str) -> Vec<&str> {
    let mut payloads = Vec::new();
    let mut remainder = response;
    let mut iterations = 0;

    while let Some(start_idx) = remainder.find("```") {
        // Prevent infinite loops from pathological input
        iterations += 1;
        if iterations > MAX_FENCE_ITERATIONS {
            break;
        }

        let after_start = &remainder[start_idx + 3..];
        let Some(end_idx) = after_start.find("```") else {
            break;
        };

        let mut fenced_body = after_start[..end_idx].trim_start();
        if !fenced_body.starts_with('{') && !fenced_body.starts_with('[') {
            fenced_body = fenced_body.split_once('\n').map(|(_, rest)| rest).unwrap_or("");
        }

        let payload = fenced_body.trim();
        if !payload.is_empty() {
            payloads.push(payload);
        }

        remainder = &after_start[end_idx + 3..];
    }

    payloads
}

fn normalize_optional_hint(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_trait::async_trait;
    use quotey_core::{RequirementSourceType, REQUIREMENT_EXTRACTION_SCHEMA_VERSION};

    use super::{parse_email_requirements, parse_requirements, parse_rfp_requirements};
    use crate::llm::LlmClient;

    struct MockLlmClient {
        response: String,
    }

    #[async_trait]
    impl LlmClient for MockLlmClient {
        async fn complete(&self, _prompt: &str) -> Result<String> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn parse_requirements_accepts_valid_email_payload() {
        let client = MockLlmClient {
            response: format!(
                r#"{{
  "schema_version":"{}",
  "source_type":"email",
  "sender_hint":"morgan@acme.com",
  "context_hint":"Net-new enterprise expansion request",
  "requirements":[{{"requirement_type":"product","name":"Enterprise Plan","quantity":150,"confidence":0.95,"raw_excerpt":"Need enterprise for 150 users"}}],
  "ambiguities":[{{"text":"enterprise tier","question":"Pro or Enterprise?","options":["Pro Plan","Enterprise Plan"],"confidence":0.62}}],
  "missing_info":["start date"]
}}"#,
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let parsed = parse_requirements(&client, RequirementSourceType::Email, "Need enterprise")
            .await
            .expect("parse should succeed");

        assert_eq!(parsed.source_type, RequirementSourceType::Email);
        assert_eq!(parsed.requirements.len(), 1);
        assert_eq!(parsed.ambiguities.len(), 1);
        assert_eq!(parsed.sender_hint.as_deref(), Some("morgan@acme.com"));
        assert_eq!(parsed.context_hint.as_deref(), Some("Net-new enterprise expansion request"));
    }

    #[tokio::test]
    async fn parse_requirements_rejects_source_mismatch() {
        let client = MockLlmClient {
            response: format!(
                r#"{{
  "schema_version":"{}",
  "source_type":"rfp",
  "requirements":[],
  "ambiguities":[],
  "missing_info":[]
}}"#,
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let error = parse_requirements(&client, RequirementSourceType::Email, "hello")
            .await
            .expect_err("source mismatch should fail");

        assert!(error.to_string().contains("source mismatch"));
    }

    #[tokio::test]
    async fn parse_rfp_requirements_accepts_valid_rfp_payload() {
        let client = MockLlmClient {
            response: format!(
                r#"{{
  "schema_version":"{}",
  "source_type":"rfp",
  "context_hint":"Security and compliance procurement",
  "requirements":[{{"requirement_type":"compliance","name":"SOC2 Type II","quantity":null,"confidence":0.91,"raw_excerpt":"Vendor must provide SOC2 Type II report"}}],
  "ambiguities":[{{"text":"enterprise support tier","question":"Standard or premium support?","options":["Standard","Premium"],"confidence":0.58}}],
  "missing_info":["contract start date"]
}}"#,
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let parsed =
            parse_rfp_requirements(&client, "RFP content").await.expect("rfp parse should succeed");

        assert_eq!(parsed.source_type, RequirementSourceType::Rfp);
        assert_eq!(parsed.requirements.len(), 1);
        assert_eq!(parsed.context_hint.as_deref(), Some("Security and compliance procurement"));
    }

    #[tokio::test]
    async fn parse_email_requirements_uses_email_source_type() {
        let client = MockLlmClient {
            response: format!(
                r#"{{
  "schema_version":"{}",
  "source_type":"email",
  "sender_hint":"buyer@example.com",
  "context_hint":"Urgent replacement quote",
  "requirements":[],
  "ambiguities":[],
  "missing_info":[]
}}"#,
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let parsed = parse_email_requirements(&client, "From: buyer@example.com")
            .await
            .expect("email parse should succeed");

        assert_eq!(parsed.source_type, RequirementSourceType::Email);
        assert_eq!(parsed.sender_hint.as_deref(), Some("buyer@example.com"));
        assert_eq!(parsed.context_hint.as_deref(), Some("Urgent replacement quote"));
    }

    #[tokio::test]
    async fn parse_requirements_accepts_fenced_json_response() {
        let client = MockLlmClient {
            response: format!(
                "```json\n{{\n  \"schema_version\":\"{}\",\n  \"source_type\":\"email\",\n  \"sender_hint\":\"  buyer@example.com  \",\n  \"context_hint\":\"  Urgent quote request  \",\n  \"requirements\":[],\n  \"ambiguities\":[],\n  \"missing_info\":[]\n}}\n```",
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let parsed = parse_requirements(&client, RequirementSourceType::Email, "raw")
            .await
            .expect("fenced payload should parse");

        assert_eq!(parsed.source_type, RequirementSourceType::Email);
        assert_eq!(parsed.sender_hint.as_deref(), Some("buyer@example.com"));
        assert_eq!(parsed.context_hint.as_deref(), Some("Urgent quote request"));
    }

    #[tokio::test]
    async fn parse_requirements_accepts_embedded_json_with_preface_and_suffix() {
        let client = MockLlmClient {
            response: format!(
                "I extracted these requirements:\n{{\"schema_version\":\"{}\",\"source_type\":\"email\",\"sender_hint\":\"buyer@example.com\",\"context_hint\":\"Renewal\",\"requirements\":[],\"ambiguities\":[],\"missing_info\":[\"start date\"]}}\nPlease validate before sending.",
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let parsed = parse_requirements(&client, RequirementSourceType::Email, "raw")
            .await
            .expect("embedded JSON should parse");

        assert_eq!(parsed.source_type, RequirementSourceType::Email);
        assert_eq!(parsed.missing_info, vec!["start date".to_string()]);
    }

    #[tokio::test]
    async fn parse_requirements_accepts_nested_payload_inside_wrapper_json() {
        let client = MockLlmClient {
            response: format!(
                "{{\"draft_warnings\":[\"Need clarification\"],\"extracted\":{{\"schema_version\":\"{}\",\"source_type\":\"email\",\"sender_hint\":\"buyer@example.com\",\"context_hint\":\"Expansion\",\"requirements\":[],\"ambiguities\":[],\"missing_info\":[]}}}}",
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let parsed = parse_requirements(&client, RequirementSourceType::Email, "raw")
            .await
            .expect("nested payload should parse");

        assert_eq!(parsed.source_type, RequirementSourceType::Email);
        assert_eq!(parsed.context_hint.as_deref(), Some("Expansion"));
    }

    #[tokio::test]
    async fn parse_requirements_accepts_fenced_payload_with_surrounding_text() {
        let client = MockLlmClient {
            response: format!(
                "Parsed output follows.\n```json\n{{\"schema_version\":\"{}\",\"source_type\":\"rfp\",\"requirements\":[],\"ambiguities\":[],\"missing_info\":[]}}\n```\nEnd of output.",
                REQUIREMENT_EXTRACTION_SCHEMA_VERSION
            ),
        };

        let parsed = parse_requirements(&client, RequirementSourceType::Rfp, "raw")
            .await
            .expect("fenced payload with surrounding text should parse");

        assert_eq!(parsed.source_type, RequirementSourceType::Rfp);
    }
}
