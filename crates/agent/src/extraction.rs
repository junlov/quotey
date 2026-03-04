use anyhow::{anyhow, Context, Result};
use quotey_core::{ExtractedRequirements, RequirementSourceType};

use crate::llm::LlmClient;
use crate::prompts::build_requirement_extraction_prompt;

pub async fn parse_requirements(
    client: &dyn LlmClient,
    source_type: RequirementSourceType,
    source_text: &str,
) -> Result<ExtractedRequirements> {
    let prompt = build_requirement_extraction_prompt(source_type, source_text);
    let response =
        client.complete(&prompt).await.context("requirement extraction completion failed")?;

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

    if let Some(json_payload) = strip_markdown_code_fence(response) {
        return serde_json::from_str::<ExtractedRequirements>(&json_payload)
            .context("failed to parse fenced JSON payload");
    }

    Err(anyhow!("response did not contain valid JSON object"))
}

fn strip_markdown_code_fence(response: &str) -> Option<String> {
    let trimmed = response.trim();
    let fenced = trimmed.strip_prefix("```")?;
    let fenced = fenced.strip_suffix("```")?;
    let fenced = fenced.trim_start();
    let payload = if fenced.starts_with('{') || fenced.starts_with('[') {
        fenced
    } else {
        fenced.split_once('\n').map(|(_, rest)| rest).unwrap_or("")
    };
    let fenced = payload.trim();
    if fenced.is_empty() {
        None
    } else {
        Some(fenced.to_string())
    }
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
}
