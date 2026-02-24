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
            &["QUOTEY_DATABASE_URL"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "database.max_connections",
        &config.database.max_connections.to_string(),
        field_source(
            "database.max_connections",
            &["QUOTEY_DATABASE_MAX_CONNECTIONS"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "database.timeout_secs",
        &config.database.timeout_secs.to_string(),
        field_source(
            "database.timeout_secs",
            &["QUOTEY_DATABASE_TIMEOUT_SECS"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    let app_token = redact_token(config.slack.app_token.expose_secret()); // ubs:ignore
    let bot_token = redact_token(config.slack.bot_token.expose_secret()); // ubs:ignore
    lines.push(render_line(
        "slack.app_token",
        &app_token,
        field_source(
            "slack.app_token",
            &["QUOTEY_SLACK_APP_TOKEN"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "slack.bot_token",
        &bot_token,
        field_source(
            "slack.bot_token",
            &["QUOTEY_SLACK_BOT_TOKEN"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    lines.push(render_line(
        "llm.provider",
        &format!("{:?}", config.llm.provider),
        field_source(
            "llm.provider",
            &["QUOTEY_LLM_PROVIDER"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "llm.model",
        &config.llm.model,
        field_source(
            "llm.model",
            &["QUOTEY_LLM_MODEL"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "llm.base_url",
        config.llm.base_url.as_deref().unwrap_or("<unset>"),
        field_source(
            "llm.base_url",
            &["QUOTEY_LLM_BASE_URL"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    let llm_api_key = if config.llm.api_key.is_some() { "<redacted>" } else { "<unset>" }; // ubs:ignore
    lines.push(render_line(
        "llm.api_key",
        llm_api_key,
        field_source(
            "llm.api_key",
            &["QUOTEY_LLM_API_KEY"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    lines.push(render_line(
        "server.bind_address",
        &config.server.bind_address,
        field_source(
            "server.bind_address",
            &["QUOTEY_SERVER_BIND_ADDRESS"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "server.health_check_port",
        &config.server.health_check_port.to_string(),
        field_source(
            "server.health_check_port",
            &["QUOTEY_SERVER_HEALTH_CHECK_PORT"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));

    lines.push(render_line(
        "logging.level",
        &config.logging.level,
        field_source(
            "logging.level",
            &["QUOTEY_LOGGING_LEVEL", "QUOTEY_LOG_LEVEL"],
            config_file_doc.as_ref(),
            config_file_path.as_deref(),
        ),
    ));
    lines.push(render_line(
        "logging.format",
        &format!("{:?}", config.logging.format),
        field_source(
            "logging.format",
            &["QUOTEY_LOGGING_FORMAT", "QUOTEY_LOG_FORMAT"],
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
    env_keys: &[&str],
    config_file_doc: Option<&Value>,
    config_file_path: Option<&Path>,
) -> String {
    for env_key in env_keys {
        if let Ok(value) = env::var(env_key) {
            if !value.trim().is_empty() {
                return format!("env ({env_key})");
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::sync::{Mutex, OnceLock};

    #[test]
    fn logs_level_prefers_env_over_file_and_default() {
        let output = run_with_env(
            &[
                ("QUOTEY_LOGGING_LEVEL", "warn"),
                ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
                ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
                ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ],
            run,
        );

        assert!(output.contains("logging.level = warn (source: env (QUOTEY_LOGGING_LEVEL))"));
    }

    #[test]
    fn logs_level_reports_alias() {
        let output = run_with_env(
            &[
                ("QUOTEY_LOG_LEVEL", "warn"),
                ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
                ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
                ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ],
            run,
        );

        assert!(
            output.contains("logging.level = warn (source: env (QUOTEY_LOG_LEVEL))"),
            "expected alias source attribution"
        );
    }

    #[test]
    fn empty_log_level_env_is_not_reported_as_source() {
        let output = run_with_env(
            &[
                ("QUOTEY_LOG_LEVEL", "   "),
                ("QUOTEY_LOGGING_LEVEL", ""),
                ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
                ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
                ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ],
            run,
        );

        assert!(output.contains("logging.level = info (source: default)"));
    }

    fn run_with_env(vars: &[(&str, &str)], test_fn: impl FnOnce() -> String) -> String {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        let _guard = ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env mutex should not be poisoned");

        let keys = [
            "QUOTEY_DATABASE_URL",
            "QUOTEY_DATABASE_MAX_CONNECTIONS",
            "QUOTEY_DATABASE_TIMEOUT_SECS",
            "QUOTEY_SLACK_APP_TOKEN",
            "QUOTEY_SLACK_BOT_TOKEN",
            "QUOTEY_LLM_PROVIDER",
            "QUOTEY_LLM_API_KEY",
            "QUOTEY_LLM_BASE_URL",
            "QUOTEY_LLM_MODEL",
            "QUOTEY_LLM_TIMEOUT_SECS",
            "QUOTEY_LLM_MAX_RETRIES",
            "QUOTEY_SERVER_BIND_ADDRESS",
            "QUOTEY_SERVER_HEALTH_CHECK_PORT",
            "QUOTEY_SERVER_GRACEFUL_SHUTDOWN_SECS",
            "QUOTEY_LOGGING_LEVEL",
            "QUOTEY_LOGGING_FORMAT",
            "QUOTEY_LOG_LEVEL",
            "QUOTEY_LOG_FORMAT",
        ];

        let previous_values: Vec<(&str, Option<String>)> =
            keys.iter().map(|key| (*key, env::var(key).ok())).collect();

        for key in &keys {
            env::remove_var(key);
        }
        for (key, value) in vars {
            env::set_var(key, value);
        }

        let result = test_fn();

        for (key, value) in previous_values {
            if let Some(value) = value {
                env::set_var(key, value);
            } else {
                env::remove_var(key);
            }
        }

        result
    }
}
