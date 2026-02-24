use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use quotey_core::config::{AppConfig, LoadOptions};
use secrecy::ExposeSecret;
use toml::Value;

pub fn run() -> String {
    let config = match AppConfig::load(LoadOptions::default()) {
        Ok(config) => config,
        Err(error) => return format!("config validation failed: {error}"),
    };

    let config_file_path = detect_config_path();
    let config_file_doc = load_config_file_doc(config_file_path.as_deref());

    let mut lines = vec!["effective config (source precedence: env > file > default):".to_string()];

    lines.push(render_line(
        "database.url",
        &config.database.url,
        field_source(
            "database.url",
            Some("QUOTEY_DATABASE_URL"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "database.max_connections",
        &config.database.max_connections.to_string(),
        field_source(
            "database.max_connections",
            Some("QUOTEY_DATABASE_MAX_CONNECTIONS"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "database.timeout_secs",
        &config.database.timeout_secs.to_string(),
        field_source(
            "database.timeout_secs",
            Some("QUOTEY_DATABASE_TIMEOUT_SECS"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    let app_token = redact_token(config.slack.app_token.expose_secret());
    let bot_token = redact_token(config.slack.bot_token.expose_secret());
    lines.push(render_line(
        "slack.app_token",
        &app_token,
        field_source(
            "slack.app_token",
            Some("QUOTEY_SLACK_APP_TOKEN"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "slack.bot_token",
        &bot_token,
        field_source(
            "slack.bot_token",
            Some("QUOTEY_SLACK_BOT_TOKEN"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    lines.push(render_line(
        "llm.provider",
        &format!("{:?}", config.llm.provider),
        field_source(
            "llm.provider",
            Some("QUOTEY_LLM_PROVIDER"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "llm.model",
        &config.llm.model,
        field_source(
            "llm.model",
            Some("QUOTEY_LLM_MODEL"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "llm.base_url",
        config.llm.base_url.as_deref().unwrap_or("<unset>"),
        field_source(
            "llm.base_url",
            Some("QUOTEY_LLM_BASE_URL"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    let llm_api_key = if config.llm.api_key.is_some() { "<redacted>" } else { "<unset>" };
    lines.push(render_line(
        "llm.api_key",
        llm_api_key,
        field_source(
            "llm.api_key",
            Some("QUOTEY_LLM_API_KEY"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    lines.push(render_line(
        "server.bind_address",
        &config.server.bind_address,
        field_source(
            "server.bind_address",
            Some("QUOTEY_SERVER_BIND_ADDRESS"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "server.health_check_port",
        &config.server.health_check_port.to_string(),
        field_source(
            "server.health_check_port",
            Some("QUOTEY_SERVER_HEALTH_CHECK_PORT"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    lines.push(render_line(
        "logging.level",
        &config.logging.level,
        field_source(
            "logging.level",
            Some("QUOTEY_LOGGING_LEVEL"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "logging.format",
        &format!("{:?}", config.logging.format),
        field_source(
            "logging.format",
            Some("QUOTEY_LOGGING_FORMAT"),
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    lines.join("\n")
}

fn detect_config_path() -> Option<PathBuf> {
    let root = PathBuf::from("quotey.toml");
    if root.exists() {
        return Some(root);
    }

    let nested = PathBuf::from("config/quotey.toml");
    if nested.exists() {
        return Some(nested);
    }

    None
}

fn load_config_file_doc(path: Option<&Path>) -> Option<Value> {
    let path = path?;
    let raw = fs::read_to_string(path).ok()?;
    raw.parse::<Value>().ok()
}

fn field_source(
    key_path: &str,
    env_key: Option<&str>,
    config_file_doc: Option<&Value>,
    config_file_path: Option<&Path>,
) -> String {
    if let Some(env_key) = env_key {
        if env::var_os(env_key).is_some() {
            return format!("env ({env_key})");
        }
    }

    if let Some(doc) = config_file_doc {
        if contains_path(doc, key_path) {
            let file_path = config_file_path
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "config file".to_string());
            return format!("file ({file_path})");
        }
    }

    "default".to_string()
}

fn contains_path(root: &Value, key_path: &str) -> bool {
    let mut current = root;
    for key in key_path.split('.') {
        let Some(next) = current.get(key) else {
            return false;
        };
        current = next;
    }
    true
}

fn render_line(key: &str, value: &str, source: String) -> String {
    format!("- {key} = {value} (source: {source})")
}

fn redact_token(token: &str) -> String {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return "<empty>".to_string();
    }

    if let Some((prefix, _)) = trimmed.split_once('-') {
        return format!("{prefix}-***");
    }

    "<redacted>".to_string()
}
