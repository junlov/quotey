use std::env;
use std::sync::{Mutex, OnceLock};

use quotey_cli::commands::{migrate, seed, smoke, start};
use serde_json::Value;

#[test]
fn start_returns_success_with_valid_env() {
    with_env(
        &[
            ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
            ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
        ],
        || {
            let result = start::run();
            assert_eq!(result.exit_code, 0, "expected successful start preflight");

            let payload = parse_payload(&result.output);
            assert_eq!(payload["command"], "start");
            assert_eq!(payload["status"], "ok");
        },
    );
}

#[test]
fn start_returns_config_failure_without_tokens() {
    with_env(&[], || {
        let result = start::run();
        assert_eq!(result.exit_code, 2, "expected config validation failure code");

        let payload = parse_payload(&result.output);
        assert_eq!(payload["command"], "start");
        assert_eq!(payload["status"], "error");
        assert_eq!(payload["error_class"], "config_validation");
    });
}

#[test]
fn migrate_returns_success_with_valid_env() {
    with_env(
        &[
            ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
            ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
        ],
        || {
            let result = migrate::run();
            assert_eq!(result.exit_code, 0, "expected successful migrate run");

            let payload = parse_payload(&result.output);
            assert_eq!(payload["command"], "migrate");
            assert_eq!(payload["status"], "ok");
        },
    );
}

#[test]
fn seed_returns_noop_success_with_valid_env() {
    with_env(
        &[
            ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
            ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
        ],
        || {
            let result = seed::run();
            assert_eq!(result.exit_code, 0, "expected deterministic seed no-op success");

            let payload = parse_payload(&result.output);
            assert_eq!(payload["command"], "seed");
            assert_eq!(payload["status"], "ok");
        },
    );
}

#[test]
fn seed_returns_deterministic_flow_summary() {
    with_env(
        &[
            ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
            ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
        ],
        || {
            let result = seed::run();
            assert_eq!(result.exit_code, 0, "expected deterministic seed success");

            let payload = parse_payload(&result.output);
            assert_eq!(payload["command"], "seed");
            assert_eq!(payload["status"], "ok");

            let message = payload["message"].as_str().unwrap_or("");
            let net_new_line =
                "  - net_new: quote-netnew-001 (Enterprise new license - draft state)";
            let renewal_line =
                "  - renewal: quote-renewal-001 (Annual renewal with expansion - priced state)";
            let discount_line =
                "  - discount_exception: quote-discount-001 (25% discount requiring approval - approval state)";
            assert!(message.contains(net_new_line));
            assert!(message.contains(renewal_line));
            assert!(message.contains(discount_line));
        },
    );
}

#[test]
fn seed_is_idempotent_across_runs() {
    with_env(
        &[
            ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
            ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
        ],
        || {
            let first = seed::run();
            assert_eq!(first.exit_code, 0, "expected first seed invocation success");
            let first_payload = parse_payload(&first.output);
            assert_eq!(first_payload["command"], "seed");
            assert_eq!(first_payload["status"], "ok");

            let second = seed::run();
            assert_eq!(second.exit_code, 0, "expected second seed invocation success");
            let second_payload = parse_payload(&second.output);
            assert_eq!(second_payload["command"], "seed");
            assert_eq!(second_payload["status"], "ok");

            assert_eq!(first_payload["message"], second_payload["message"]);
        },
    );
}

#[test]
fn smoke_returns_success_report_with_valid_env() {
    with_env(
        &[
            ("QUOTEY_SLACK_APP_TOKEN", "xapp-test"),
            ("QUOTEY_SLACK_BOT_TOKEN", "xoxb-test"),
            ("QUOTEY_DATABASE_URL", "sqlite::memory:"),
        ],
        || {
            let result = smoke::run();
            assert_eq!(result.exit_code, 0, "expected successful smoke report");

            let payload = parse_payload(last_line(&result.output));
            assert_eq!(payload["command"], "smoke");
            assert_eq!(payload["status"], "pass");
        },
    );
}

#[test]
fn smoke_returns_failure_when_config_invalid() {
    with_env(&[], || {
        let result = smoke::run();
        assert_eq!(result.exit_code, 6, "expected smoke failure code");

        let payload = parse_payload(last_line(&result.output));
        assert_eq!(payload["command"], "smoke");
        assert_eq!(payload["status"], "fail");
    });
}

fn parse_payload(output: &str) -> Value {
    serde_json::from_str(output).expect("command output should be valid JSON")
}

fn last_line(output: &str) -> &str {
    output.lines().last().unwrap_or_default()
}

fn with_env(vars: &[(&str, &str)], test_fn: impl FnOnce()) {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard =
        ENV_LOCK.get_or_init(|| Mutex::new(())).lock().expect("env mutex should not be poisoned");

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

    test_fn();

    for (key, value) in previous_values {
        if let Some(value) = value {
            env::set_var(key, value);
        } else {
            env::remove_var(key);
        }
    }
}
