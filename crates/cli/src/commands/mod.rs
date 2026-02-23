pub mod config;
pub mod doctor;
pub mod migrate;
pub mod seed;
pub mod start;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub exit_code: u8,
    pub output: String,
}

#[derive(Debug, Serialize)]
struct CommandOutcome {
    command: String,
    status: String,
    error_class: Option<String>,
    message: String,
}

impl CommandResult {
    pub fn success(command: &str, message: impl Into<String>) -> Self {
        let payload = CommandOutcome {
            command: command.to_string(),
            status: "ok".to_string(),
            error_class: None,
            message: message.into(),
        };
        Self { exit_code: 0, output: serialize_payload(payload) }
    }

    pub fn failure(
        command: &str,
        error_class: &str,
        message: impl Into<String>,
        exit_code: u8,
    ) -> Self {
        let payload = CommandOutcome {
            command: command.to_string(),
            status: "error".to_string(),
            error_class: Some(error_class.to_string()),
            message: message.into(),
        };
        Self { exit_code, output: serialize_payload(payload) }
    }
}

fn serialize_payload(payload: CommandOutcome) -> String {
    serde_json::to_string(&payload).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"unknown\",\"status\":\"error\",\"error_class\":\"serialization\",\"message\":\"{}\"}}",
            error.to_string().replace('\\', "\\\\").replace('"', "\\\"")
        )
    })
}
