use anyhow::Result;

use crate::guardrails::GuardrailPolicy;

#[derive(Default)]
pub struct AgentRuntime {
    guardrails: GuardrailPolicy,
}

impl AgentRuntime {
    pub fn new(guardrails: GuardrailPolicy) -> Self {
        Self { guardrails }
    }

    pub async fn handle_thread_message(&self, text: &str) -> Result<String> {
        let _guardrails = &self.guardrails;
        Ok(format!("received: {text}"))
    }
}
