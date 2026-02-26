use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub slack: SlackConfig,
    pub llm: LlmConfig,
    pub server: ServerConfig,
    pub crm: CrmConfig,
    pub logging: LoggingConfig,
}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub timeout_secs: u64,
}

#[derive(Clone, Debug)]
pub struct SlackConfig {
    pub app_token: SecretString,
    pub bot_token: SecretString,
}

#[derive(Clone, Debug)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<SecretString>,
    pub base_url: Option<String>,
    pub model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind_address: String,
    pub health_check_port: u16,
    pub graceful_shutdown_secs: u64,
}

#[derive(Clone, Debug)]
pub struct CrmConfig {
    pub enabled: bool,
    pub webhook_secret: Option<String>,
    pub callback_base_url: Option<String>,
    pub salesforce_client_id: Option<String>,
    pub salesforce_client_secret: Option<String>,
    pub hubspot_client_id: Option<String>,
    pub hubspot_client_secret: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LoggingConfig {
    pub level: String,
    pub format: LogFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LlmProvider {
    OpenAi,
    Anthropic,
    Ollama,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    Compact,
    Pretty,
    Json,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigOverrides {
    pub database_url: Option<String>,
    pub log_level: Option<String>,
    pub llm_provider: Option<LlmProvider>,
    pub llm_model: Option<String>,
    pub slack_app_token: Option<String>,
    pub slack_bot_token: Option<String>,
    pub crm_enabled: Option<bool>,
    pub crm_webhook_secret: Option<String>,
    pub crm_callback_base_url: Option<String>,
    pub crm_salesforce_client_id: Option<String>,
    pub crm_salesforce_client_secret: Option<String>,
    pub crm_hubspot_client_id: Option<String>,
    pub crm_hubspot_client_secret: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct LoadOptions {
    pub config_path: Option<PathBuf>,
    pub require_file: bool,
    pub overrides: ConfigOverrides,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("could not read config file `{path}`: {source}")]
    ReadFile { path: PathBuf, source: std::io::Error },
    #[error("could not parse config file `{path}`: {source}")]
    ParseFile { path: PathBuf, source: toml::de::Error },
    #[error("required config file was not found: `{0}`")]
    MissingConfigFile(PathBuf),
    #[error("environment variable interpolation failed for `{var}`")]
    MissingEnvInterpolation { var: String },
    #[error("unterminated environment interpolation expression")]
    UnterminatedInterpolation,
    #[error("invalid environment override for `{key}`: `{value}`")]
    InvalidEnvOverride { key: String, value: String },
    #[error("configuration validation failed: {0}")]
    Validation(String),
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database: DatabaseConfig {
                url: "sqlite://quotey.db".to_string(),
                max_connections: 5,
                timeout_secs: 30,
            },
            slack: SlackConfig { app_token: String::new().into(), bot_token: String::new().into() },
            llm: LlmConfig {
                provider: LlmProvider::Ollama,
                api_key: None,
                base_url: Some("http://localhost:11434".to_string()),
                model: "llama3.1".to_string(),
                timeout_secs: 30,
                max_retries: 2,
            },
            server: ServerConfig {
                bind_address: "127.0.0.1".to_string(),
                health_check_port: 8080,
                graceful_shutdown_secs: 15,
            },
            crm: CrmConfig {
                enabled: false,
                webhook_secret: None,
                callback_base_url: None,
                salesforce_client_id: None,
                salesforce_client_secret: None,
                hubspot_client_id: None,
                hubspot_client_secret: None,
            },
            logging: LoggingConfig { level: "info".to_string(), format: LogFormat::Compact },
        }
    }
}

fn secret_value(value: String) -> SecretString {
    value.into()
}

impl std::str::FromStr for LlmProvider {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "openai" => Ok(Self::OpenAi),
            "anthropic" => Ok(Self::Anthropic),
            "ollama" => Ok(Self::Ollama),
            other => Err(ConfigError::Validation(format!(
                "unsupported llm provider `{other}` (expected openai|anthropic|ollama)"
            ))),
        }
    }
}

impl std::str::FromStr for LogFormat {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "compact" => Ok(Self::Compact),
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            other => Err(ConfigError::Validation(format!(
                "unsupported log format `{other}` (expected compact|pretty|json)"
            ))),
        }
    }
}

impl AppConfig {
    pub fn load(options: LoadOptions) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        let maybe_path = resolve_config_path(options.config_path.as_deref());

        if let Some(path) = maybe_path {
            let patch = read_patch(&path)?;
            config.apply_patch(patch);
        } else if options.require_file {
            let expected = options.config_path.unwrap_or_else(|| PathBuf::from("quotey.toml"));
            return Err(ConfigError::MissingConfigFile(expected));
        }

        config.apply_env_overrides()?;
        config.apply_overrides(options.overrides);
        config.validate()?;

        Ok(config)
    }

    fn apply_patch(&mut self, patch: ConfigPatch) {
        if let Some(database) = patch.database {
            if let Some(url) = database.url {
                self.database.url = url;
            }
            if let Some(max_connections) = database.max_connections {
                self.database.max_connections = max_connections;
            }
            if let Some(timeout_secs) = database.timeout_secs {
                self.database.timeout_secs = timeout_secs;
            }
        }

        if let Some(slack) = patch.slack {
            if let Some(slack_app_token_value) = slack.app_token {
                self.slack.app_token = secret_value(slack_app_token_value); // ubs:ignore
            }
            if let Some(slack_bot_token_value) = slack.bot_token {
                self.slack.bot_token = secret_value(slack_bot_token_value); // ubs:ignore
            }
        }

        if let Some(llm) = patch.llm {
            if let Some(provider) = llm.provider {
                self.llm.provider = provider;
            }
            if let Some(llm_api_key_value) = llm.api_key {
                self.llm.api_key = Some(secret_value(llm_api_key_value)); // ubs:ignore
            }
            if let Some(base_url) = llm.base_url {
                self.llm.base_url = Some(base_url);
            }
            if let Some(model) = llm.model {
                self.llm.model = model;
            }
            if let Some(timeout_secs) = llm.timeout_secs {
                self.llm.timeout_secs = timeout_secs;
            }
            if let Some(max_retries) = llm.max_retries {
                self.llm.max_retries = max_retries;
            }
        }

        if let Some(server) = patch.server {
            if let Some(bind_address) = server.bind_address {
                self.server.bind_address = bind_address;
            }
            if let Some(health_check_port) = server.health_check_port {
                self.server.health_check_port = health_check_port;
            }
            if let Some(graceful_shutdown_secs) = server.graceful_shutdown_secs {
                self.server.graceful_shutdown_secs = graceful_shutdown_secs;
            }
        }

        if let Some(crm) = patch.crm {
            if let Some(enabled) = crm.enabled {
                self.crm.enabled = enabled;
            }
            if let Some(webhook_secret) = crm.webhook_secret {
                self.crm.webhook_secret = Some(webhook_secret);
            }
            if let Some(callback_base_url) = crm.callback_base_url {
                self.crm.callback_base_url = Some(callback_base_url);
            }
            if let Some(salesforce_client_id) = crm.salesforce_client_id {
                self.crm.salesforce_client_id = Some(salesforce_client_id);
            }
            if let Some(salesforce_client_secret) = crm.salesforce_client_secret {
                self.crm.salesforce_client_secret = Some(salesforce_client_secret);
            }
            if let Some(hubspot_client_id) = crm.hubspot_client_id {
                self.crm.hubspot_client_id = Some(hubspot_client_id);
            }
            if let Some(hubspot_client_secret) = crm.hubspot_client_secret {
                self.crm.hubspot_client_secret = Some(hubspot_client_secret);
            }
        }

        if let Some(logging) = patch.logging {
            if let Some(level) = logging.level {
                self.logging.level = level;
            }
            if let Some(format) = logging.format {
                self.logging.format = format;
            }
        }
    }

    fn apply_env_overrides(&mut self) -> Result<(), ConfigError> {
        if let Some(value) = read_env("QUOTEY_DATABASE_URL") {
            self.database.url = value;
        }
        if let Some(value) = read_env("QUOTEY_DATABASE_MAX_CONNECTIONS") {
            self.database.max_connections = parse_u32("QUOTEY_DATABASE_MAX_CONNECTIONS", &value)?;
        }
        if let Some(value) = read_env("QUOTEY_DATABASE_TIMEOUT_SECS") {
            self.database.timeout_secs = parse_u64("QUOTEY_DATABASE_TIMEOUT_SECS", &value)?;
        }

        if let Some(value) = read_env("QUOTEY_SLACK_APP_TOKEN") {
            self.slack.app_token = secret_value(value); // ubs:ignore
        }
        if let Some(value) = read_env("QUOTEY_SLACK_BOT_TOKEN") {
            self.slack.bot_token = secret_value(value); // ubs:ignore
        }

        if let Some(value) = read_env("QUOTEY_LLM_PROVIDER") {
            self.llm.provider = value.parse()?;
        }
        if let Some(value) = read_env("QUOTEY_LLM_API_KEY") {
            self.llm.api_key = Some(secret_value(value)); // ubs:ignore
        }
        if let Some(value) = read_env("QUOTEY_LLM_BASE_URL") {
            self.llm.base_url = Some(value);
        }
        if let Some(value) = read_env("QUOTEY_LLM_MODEL") {
            self.llm.model = value;
        }
        if let Some(value) = read_env("QUOTEY_LLM_TIMEOUT_SECS") {
            self.llm.timeout_secs = parse_u64("QUOTEY_LLM_TIMEOUT_SECS", &value)?;
        }
        if let Some(value) = read_env("QUOTEY_LLM_MAX_RETRIES") {
            self.llm.max_retries = parse_u32("QUOTEY_LLM_MAX_RETRIES", &value)?;
        }

        if let Some(value) = read_env("QUOTEY_SERVER_BIND_ADDRESS") {
            self.server.bind_address = value;
        }
        if let Some(value) = read_env("QUOTEY_SERVER_HEALTH_CHECK_PORT") {
            self.server.health_check_port = parse_u16("QUOTEY_SERVER_HEALTH_CHECK_PORT", &value)?;
        }
        if let Some(value) = read_env("QUOTEY_SERVER_GRACEFUL_SHUTDOWN_SECS") {
            self.server.graceful_shutdown_secs =
                parse_u64("QUOTEY_SERVER_GRACEFUL_SHUTDOWN_SECS", &value)?;
        }

        if let Some(value) = read_env("QUOTEY_CRM_ENABLED") {
            self.crm.enabled = parse_bool("QUOTEY_CRM_ENABLED", &value)?;
        }
        if let Some(value) = read_env("QUOTEY_CRM_WEBHOOK_SECRET") {
            self.crm.webhook_secret = Some(value);
        }
        if let Some(value) = read_env("QUOTEY_CRM_CALLBACK_BASE_URL") {
            self.crm.callback_base_url = Some(value);
        }
        if let Some(value) = read_env("QUOTEY_CRM_SALESFORCE_CLIENT_ID") {
            self.crm.salesforce_client_id = Some(value);
        }
        if let Some(value) = read_env("QUOTEY_CRM_SALESFORCE_CLIENT_SECRET") {
            self.crm.salesforce_client_secret = Some(value);
        }
        if let Some(value) = read_env("QUOTEY_CRM_HUBSPOT_CLIENT_ID") {
            self.crm.hubspot_client_id = Some(value);
        }
        if let Some(value) = read_env("QUOTEY_CRM_HUBSPOT_CLIENT_SECRET") {
            self.crm.hubspot_client_secret = Some(value);
        }

        let log_level = read_env("QUOTEY_LOGGING_LEVEL").or_else(|| read_env("QUOTEY_LOG_LEVEL"));
        if let Some(value) = log_level {
            self.logging.level = value;
        }
        let log_format =
            read_env("QUOTEY_LOGGING_FORMAT").or_else(|| read_env("QUOTEY_LOG_FORMAT"));
        if let Some(value) = log_format {
            self.logging.format = value.parse()?;
        }

        Ok(())
    }

    fn apply_overrides(&mut self, overrides: ConfigOverrides) {
        if let Some(database_url) = overrides.database_url {
            self.database.url = database_url;
        }
        if let Some(log_level) = overrides.log_level {
            self.logging.level = log_level;
        }
        if let Some(llm_provider) = overrides.llm_provider {
            self.llm.provider = llm_provider;
        }
        if let Some(llm_model) = overrides.llm_model {
            self.llm.model = llm_model;
        }
        if let Some(slack_app_token) = overrides.slack_app_token {
            self.slack.app_token = secret_value(slack_app_token); // ubs:ignore
        }
        if let Some(slack_bot_token) = overrides.slack_bot_token {
            self.slack.bot_token = secret_value(slack_bot_token); // ubs:ignore
        }

        if let Some(enabled) = overrides.crm_enabled {
            self.crm.enabled = enabled;
        }
        if let Some(webhook_secret) = overrides.crm_webhook_secret {
            self.crm.webhook_secret = Some(webhook_secret);
        }
        if let Some(callback_base_url) = overrides.crm_callback_base_url {
            self.crm.callback_base_url = Some(callback_base_url);
        }
        if let Some(salesforce_client_id) = overrides.crm_salesforce_client_id {
            self.crm.salesforce_client_id = Some(salesforce_client_id);
        }
        if let Some(salesforce_client_secret) = overrides.crm_salesforce_client_secret {
            self.crm.salesforce_client_secret = Some(salesforce_client_secret);
        }
        if let Some(hubspot_client_id) = overrides.crm_hubspot_client_id {
            self.crm.hubspot_client_id = Some(hubspot_client_id);
        }
        if let Some(hubspot_client_secret) = overrides.crm_hubspot_client_secret {
            self.crm.hubspot_client_secret = Some(hubspot_client_secret);
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        validate_database(&self.database)?;
        validate_slack(&self.slack)?;
        validate_llm(&self.llm)?;
        validate_server(&self.server)?;
        validate_crm(&self.crm)?;
        validate_logging(&self.logging)?;
        Ok(())
    }
}

fn resolve_config_path(explicit_path: Option<&Path>) -> Option<PathBuf> {
    if let Some(path) = explicit_path {
        return path.exists().then_some(path.to_path_buf());
    }

    [PathBuf::from("quotey.toml"), PathBuf::from("config/quotey.toml")]
        .into_iter()
        .find(|path| path.exists())
}

fn read_patch(path: &Path) -> Result<ConfigPatch, ConfigError> {
    let raw = fs::read_to_string(path)
        .map_err(|source| ConfigError::ReadFile { path: path.to_path_buf(), source })?;

    let interpolated = interpolate_env_vars(&raw)?;
    toml::from_str::<ConfigPatch>(&interpolated)
        .map_err(|source| ConfigError::ParseFile { path: path.to_path_buf(), source })
}

fn interpolate_env_vars(input: &str) -> Result<String, ConfigError> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' && matches!(chars.peek(), Some('{')) {
            chars.next();
            let mut key = String::new();

            loop {
                match chars.next() {
                    Some('}') => break,
                    Some(next) => key.push(next),
                    None => return Err(ConfigError::UnterminatedInterpolation),
                }
            }

            let value = env::var(&key)
                .map_err(|_| ConfigError::MissingEnvInterpolation { var: key.clone() })?;
            output.push_str(&value);
            continue;
        }

        output.push(ch);
    }

    Ok(output)
}

fn validate_database(database: &DatabaseConfig) -> Result<(), ConfigError> {
    let url = database.url.trim();
    let sqlite_url =
        url.starts_with("sqlite://") || url.starts_with("sqlite::") || url == ":memory:";
    if !sqlite_url {
        return Err(ConfigError::Validation(
            "database.url must be a sqlite URL (`sqlite://...`, `sqlite::...`, or `:memory:`)"
                .to_string(),
        ));
    }

    if database.max_connections == 0 {
        return Err(ConfigError::Validation(
            "database.max_connections must be greater than zero".to_string(),
        ));
    }

    if database.timeout_secs == 0 || database.timeout_secs > 300 {
        return Err(ConfigError::Validation(
            "database.timeout_secs must be in range 1..=300".to_string(),
        ));
    }

    Ok(())
}

fn validate_slack(slack: &SlackConfig) -> Result<(), ConfigError> {
    let app_token = slack.app_token.expose_secret(); // ubs:ignore
    if app_token.is_empty() {
        return Err(ConfigError::Validation(
            "slack.app_token is required. Get it from https://api.slack.com/apps > Your App > Basic Information > App-Level Tokens".to_string()
        ));
    }
    if !app_token.starts_with("xapp-") {
        let hint = if app_token.starts_with("xoxb-") {
            " (hint: you may have used the bot token instead of the app token)"
        } else {
            ""
        };
        return Err(ConfigError::Validation(format!(
            "slack.app_token must start with `xapp-`{hint}. Get it from https://api.slack.com/apps"
        )));
    }

    let bot_token = slack.bot_token.expose_secret(); // ubs:ignore
    if bot_token.is_empty() {
        return Err(ConfigError::Validation(
            "slack.bot_token is required. Get it from https://api.slack.com/apps > Your App > OAuth & Permissions > Bot User OAuth Token".to_string()
        ));
    }
    if !bot_token.starts_with("xoxb-") {
        let hint = if bot_token.starts_with("xapp-") {
            " (hint: you may have used the app token instead of the bot token)"
        } else {
            ""
        };
        return Err(ConfigError::Validation(format!(
            "slack.bot_token must start with `xoxb-`{hint}. Get it from https://api.slack.com/apps"
        )));
    }

    Ok(())
}

fn validate_llm(llm: &LlmConfig) -> Result<(), ConfigError> {
    if llm.timeout_secs == 0 || llm.timeout_secs > 300 {
        return Err(ConfigError::Validation(
            "llm.timeout_secs must be in range 1..=300".to_string(),
        ));
    }

    match llm.provider {
        LlmProvider::OpenAi | LlmProvider::Anthropic => {
            let missing = llm
                .api_key
                .as_ref()
                .map(|value| value.expose_secret().trim().is_empty())
                .unwrap_or(true);
            if missing {
                return Err(ConfigError::Validation(
                    "llm.api_key is required for openai/anthropic providers".to_string(),
                ));
            }
        }
        LlmProvider::Ollama => {
            let missing =
                llm.base_url.as_ref().map(|value| value.trim().is_empty()).unwrap_or(true);
            if missing {
                return Err(ConfigError::Validation(
                    "llm.base_url is required for ollama provider".to_string(),
                ));
            }
        }
    }

    Ok(())
}

fn validate_server(server: &ServerConfig) -> Result<(), ConfigError> {
    if server.health_check_port == 0 {
        return Err(ConfigError::Validation(
            "server.health_check_port must be greater than zero".to_string(),
        ));
    }

    if server.graceful_shutdown_secs == 0 {
        return Err(ConfigError::Validation(
            "server.graceful_shutdown_secs must be greater than zero".to_string(),
        ));
    }

    Ok(())
}

fn validate_logging(logging: &LoggingConfig) -> Result<(), ConfigError> {
    let level = logging.level.trim().to_ascii_lowercase();
    match level.as_str() {
        "trace" | "debug" | "info" | "warn" | "error" => Ok(()),
        _ => Err(ConfigError::Validation(
            "logging.level must be one of trace|debug|info|warn|error".to_string(),
        )),
    }
}

fn validate_crm(crm: &CrmConfig) -> Result<(), ConfigError> {
    if crm.enabled {
        let has_provider = crm.salesforce_client_id.is_some() || crm.hubspot_client_id.is_some();
        if !has_provider {
            return Err(ConfigError::Validation(
                "crm.enabled is true but no CRM provider credentials are configured".to_string(),
            ));
        }

        let has_secret =
            crm.salesforce_client_secret.is_some() || crm.hubspot_client_secret.is_some();
        if !has_secret {
            return Err(ConfigError::Validation(
                "crm.enabled is true but provider client secrets are missing".to_string(),
            ));
        }
    }

    if let Some(base_url) = &crm.callback_base_url {
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(ConfigError::Validation(
                "crm.callback_base_url must start with http:// or https://".to_string(),
            ));
        }
    }

    Ok(())
}

fn read_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn parse_u16(key: &str, value: &str) -> Result<u16, ConfigError> {
    value.parse::<u16>().map_err(|_| ConfigError::InvalidEnvOverride {
        key: key.to_string(),
        value: value.to_string(),
    })
}

fn parse_u32(key: &str, value: &str) -> Result<u32, ConfigError> {
    value.parse::<u32>().map_err(|_| ConfigError::InvalidEnvOverride {
        key: key.to_string(),
        value: value.to_string(),
    })
}

fn parse_u64(key: &str, value: &str) -> Result<u64, ConfigError> {
    value.parse::<u64>().map_err(|_| ConfigError::InvalidEnvOverride {
        key: key.to_string(),
        value: value.to_string(),
    })
}

fn parse_bool(key: &str, value: &str) -> Result<bool, ConfigError> {
    value.parse::<bool>().map_err(|_| ConfigError::InvalidEnvOverride {
        key: key.to_string(),
        value: value.to_string(),
    })
}

#[derive(Debug, Default, Deserialize)]
struct ConfigPatch {
    database: Option<DatabasePatch>,
    slack: Option<SlackPatch>,
    llm: Option<LlmPatch>,
    server: Option<ServerPatch>,
    crm: Option<CrmPatch>,
    logging: Option<LoggingPatch>,
}

#[derive(Debug, Default, Deserialize)]
struct DatabasePatch {
    url: Option<String>,
    max_connections: Option<u32>,
    timeout_secs: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct SlackPatch {
    app_token: Option<String>,
    bot_token: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LlmPatch {
    provider: Option<LlmProvider>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    timeout_secs: Option<u64>,
    max_retries: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
struct ServerPatch {
    bind_address: Option<String>,
    health_check_port: Option<u16>,
    graceful_shutdown_secs: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct LoggingPatch {
    level: Option<String>,
    format: Option<LogFormat>,
}

#[derive(Debug, Default, Deserialize)]
struct CrmPatch {
    enabled: Option<bool>,
    webhook_secret: Option<String>,
    callback_base_url: Option<String>,
    salesforce_client_id: Option<String>,
    salesforce_client_secret: Option<String>,
    hubspot_client_id: Option<String>,
    hubspot_client_secret: Option<String>,
}

#[cfg(test)]
// ubs:ignore
mod tests {
    use std::env;
    use std::fs;
    use std::io;
    use std::sync::{Mutex, OnceLock};

    use secrecy::ExposeSecret;
    use tempfile::TempDir;

    use super::{AppConfig, ConfigError, ConfigOverrides, LoadOptions, LogFormat};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn clear_vars(vars: &[&str]) {
        for var in vars {
            env::remove_var(var);
        }
    }

    fn ensure(condition: bool, message: &'static str) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.to_string())
        }
    }

    #[test]
    fn file_load_supports_env_interpolation() -> Result<(), String> {
        let _guard = env_lock().lock().map_err(|_| "env lock is poisoned".to_string())?;

        env::set_var("TEST_SLACK_APP_TOKEN", "xapp-from-env");
        env::set_var("TEST_SLACK_BOT_TOKEN", "xoxb-from-env");

        let result = (|| -> Result<(), String> {
            let dir = TempDir::new().map_err(|err: io::Error| err.to_string())?;
            let path = dir.path().join("quotey.toml");
            fs::write(
                &path,
                r#"
[slack]
app_token = "${TEST_SLACK_APP_TOKEN}" # ubs:ignore
bot_token = "${TEST_SLACK_BOT_TOKEN}" # ubs:ignore
"#,
            )
            .map_err(|err| err.to_string())?;

            let config =
                AppConfig::load(LoadOptions { config_path: Some(path), ..LoadOptions::default() })
                    .map_err(|err| format!("config load failed: {err}"))?;

            ensure(
                config.slack.app_token.expose_secret() == "xapp-from-env",
                "app token should be loaded from environment",
            )?;
            ensure(
                config.slack.bot_token.expose_secret() == "xoxb-from-env",
                "bot token should be loaded from environment",
            )?;
            Ok(())
        })();

        clear_vars(&["TEST_SLACK_APP_TOKEN", "TEST_SLACK_BOT_TOKEN"]);
        result
    }

    #[test]
    fn logging_env_aliases_are_supported() -> Result<(), String> {
        let _guard = env_lock().lock().map_err(|_| "env lock is poisoned".to_string())?;

        env::set_var("QUOTEY_SLACK_APP_TOKEN", "xapp-test");
        env::set_var("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test");
        env::set_var("QUOTEY_LOG_LEVEL", "warn");
        env::set_var("QUOTEY_LOG_FORMAT", "pretty");

        let result = (|| -> Result<(), String> {
            let config = AppConfig::load(LoadOptions::default())
                .map_err(|err| format!("config load failed: {err}"))?;

            ensure(config.logging.level == "warn", "warning log level should be set from env var")?;
            ensure(
                matches!(config.logging.format, LogFormat::Pretty),
                "pretty logging format should be set from env var",
            )?;
            Ok(())
        })();

        clear_vars(&[
            "QUOTEY_SLACK_APP_TOKEN",
            "QUOTEY_SLACK_BOT_TOKEN",
            "QUOTEY_LOG_LEVEL",
            "QUOTEY_LOG_FORMAT",
        ]);
        result
    }

    #[test]
    fn precedence_defaults_file_env_overrides() -> Result<(), String> {
        let _guard = env_lock().lock().map_err(|_| "env lock is poisoned".to_string())?;

        env::set_var("QUOTEY_DATABASE_URL", "sqlite://from-env.db");
        env::set_var("QUOTEY_SLACK_APP_TOKEN", "xapp-from-env");
        env::set_var("QUOTEY_SLACK_BOT_TOKEN", "xoxb-from-env");

        let result = (|| -> Result<(), String> {
            let dir = TempDir::new().map_err(|err: io::Error| err.to_string())?;
            let path = dir.path().join("quotey.toml");
            fs::write(
                &path,
                r#"
[database]
url = "sqlite://from-file.db"

[slack]
app_token = "xapp-from-file" # ubs:ignore
bot_token = "xoxb-from-file" # ubs:ignore

[logging]
level = "warn"
"#,
            )
            .map_err(|err| err.to_string())?;

            let config = AppConfig::load(LoadOptions {
                config_path: Some(path),
                overrides: ConfigOverrides {
                    database_url: Some("sqlite://from-override.db".to_string()),
                    log_level: Some("debug".to_string()),
                    ..ConfigOverrides::default()
                },
                ..LoadOptions::default()
            })
            .map_err(|err| format!("config load failed: {err}"))?;

            ensure(
                config.database.url == "sqlite://from-override.db",
                "override database url should win",
            )?;
            ensure(config.logging.level == "debug", "overridden log level should be debug")?;
            ensure(
                config.slack.app_token.expose_secret() == "xapp-from-env",
                "env app token should win over file and defaults",
            )?;
            ensure(
                config.slack.bot_token.expose_secret() == "xoxb-from-env",
                "env bot token should win over file and defaults",
            )?;
            Ok(())
        })();

        clear_vars(&["QUOTEY_DATABASE_URL", "QUOTEY_SLACK_APP_TOKEN", "QUOTEY_SLACK_BOT_TOKEN"]);
        result
    }

    #[test]
    fn validation_fails_fast_with_actionable_error() -> Result<(), String> {
        let _guard = env_lock().lock().map_err(|_| "env lock is poisoned".to_string())?;

        env::set_var("QUOTEY_SLACK_APP_TOKEN", "bad");
        env::set_var("QUOTEY_SLACK_BOT_TOKEN", "xoxb-valid");

        let result = (|| -> Result<(), String> {
            let error = match AppConfig::load(LoadOptions::default()) {
                Ok(_) => {
                    return Err("expected validation failure but config load succeeded".to_string())
                }
                Err(error) => error,
            };
            let has_message = matches!(
                error,
                ConfigError::Validation(ref message) if message.contains("slack.app_token")
            );
            ensure(has_message, "validation failure should mention slack.app_token")
        })();

        clear_vars(&["QUOTEY_SLACK_APP_TOKEN", "QUOTEY_SLACK_BOT_TOKEN"]);
        result
    }

    #[test]
    fn secret_values_are_not_leaked_by_debug() -> Result<(), String> {
        let _guard = env_lock().lock().map_err(|_| "env lock is poisoned".to_string())?;

        env::set_var("QUOTEY_SLACK_APP_TOKEN", "xapp-secret-value");
        env::set_var("QUOTEY_SLACK_BOT_TOKEN", "xoxb-secret-value");

        let result = (|| -> Result<(), String> {
            let config = AppConfig::load(LoadOptions::default())
                .map_err(|err| format!("config load failed: {err}"))?;
            let debug = format!("{config:?}");

            ensure(
                !debug.contains("xapp-secret-value"),
                "debug output should not contain app token",
            )?;
            ensure(
                !debug.contains("xoxb-secret-value"),
                "debug output should not contain bot token",
            )?;
            ensure(
                matches!(config.logging.format, LogFormat::Compact),
                "default logging format should be compact",
            )?;
            Ok(())
        })();

        clear_vars(&["QUOTEY_SLACK_APP_TOKEN", "QUOTEY_SLACK_BOT_TOKEN"]);
        result
    }
}
