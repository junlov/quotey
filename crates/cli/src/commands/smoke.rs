use std::time::Instant;

use crate::commands::CommandResult;
use quotey_core::config::{AppConfig, LoadOptions};
use quotey_db::{connect_with_settings, migrations};
use secrecy::ExposeSecret;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SmokeStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Serialize)]
struct SmokeCheck {
    name: &'static str,
    status: SmokeStatus,
    elapsed_ms: u64,
    message: String,
}

#[derive(Debug, Serialize)]
struct SmokeReport {
    command: &'static str,
    status: SmokeStatus,
    summary: String,
    total_elapsed_ms: u64,
    checks: Vec<SmokeCheck>,
}

pub fn run() -> CommandResult {
    let started = Instant::now();
    let mut checks = Vec::new();

    let config = match timed_check(|| AppConfig::load(LoadOptions::default())) {
        Ok((elapsed_ms, config)) => {
            checks.push(SmokeCheck {
                name: "config_validation",
                status: SmokeStatus::Pass,
                elapsed_ms,
                message: "configuration loaded and validated".to_string(),
            });
            config
        }
        Err((elapsed_ms, error)) => {
            checks.push(SmokeCheck {
                name: "config_validation",
                status: SmokeStatus::Fail,
                elapsed_ms,
                message: error.to_string(),
            });
            checks.push(skipped("slack_token_sanity"));
            checks.push(skipped("db_connectivity"));
            checks.push(skipped("migration_visibility"));
            return finalize_report(checks, started.elapsed().as_millis() as u64);
        }
    };

    let slack_check_started = Instant::now();
    let app_ok = config.slack.app_token.expose_secret().starts_with("xapp-");
    let bot_ok = config.slack.bot_token.expose_secret().starts_with("xoxb-");
    checks.push(SmokeCheck {
        name: "slack_token_sanity",
        status: if app_ok && bot_ok { SmokeStatus::Pass } else { SmokeStatus::Fail },
        elapsed_ms: slack_check_started.elapsed().as_millis() as u64,
        message: if app_ok && bot_ok {
            "token prefixes are valid".to_string()
        } else {
            "expected Slack credentials with valid prefixes (app xapp-*, bot xoxb-*)".to_string()
        },
    });

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            checks.push(SmokeCheck {
                name: "db_connectivity",
                status: SmokeStatus::Fail,
                elapsed_ms: 0,
                message: format!("failed to initialize async runtime: {error}"),
            });
            checks.push(skipped("migration_visibility"));
            return finalize_report(checks, started.elapsed().as_millis() as u64);
        }
    };

    let db_started = Instant::now();
    let db_result = runtime.block_on(async {
        connect_with_settings(
            &config.database.url,
            config.database.max_connections,
            config.database.timeout_secs,
        )
        .await
    });

    let pool = match db_result {
        Ok(pool) => {
            checks.push(SmokeCheck {
                name: "db_connectivity",
                status: SmokeStatus::Pass,
                elapsed_ms: db_started.elapsed().as_millis() as u64,
                message: format!("connected using `{}`", config.database.url),
            });
            pool
        }
        Err(error) => {
            checks.push(SmokeCheck {
                name: "db_connectivity",
                status: SmokeStatus::Fail,
                elapsed_ms: db_started.elapsed().as_millis() as u64,
                message: format!("failed to connect: {error}"),
            });
            checks.push(skipped("migration_visibility"));
            return finalize_report(checks, started.elapsed().as_millis() as u64);
        }
    };

    let migration_started = Instant::now();
    let migration_result = runtime.block_on(async { migrations::run_pending(&pool).await });
    runtime.block_on(async {
        pool.close().await;
    });

    match migration_result {
        Ok(()) => checks.push(SmokeCheck {
            name: "migration_visibility",
            status: SmokeStatus::Pass,
            elapsed_ms: migration_started.elapsed().as_millis() as u64,
            message: "migrations are visible and executable".to_string(),
        }),
        Err(error) => checks.push(SmokeCheck {
            name: "migration_visibility",
            status: SmokeStatus::Fail,
            elapsed_ms: migration_started.elapsed().as_millis() as u64,
            message: format!("migration execution failed: {error}"),
        }),
    }

    finalize_report(checks, started.elapsed().as_millis() as u64)
}

fn timed_check<T, E>(check: impl FnOnce() -> Result<T, E>) -> Result<(u64, T), (u64, E)> {
    let started = Instant::now();
    match check() {
        Ok(value) => Ok((started.elapsed().as_millis() as u64, value)),
        Err(error) => Err((started.elapsed().as_millis() as u64, error)),
    }
}

fn skipped(name: &'static str) -> SmokeCheck {
    SmokeCheck {
        name,
        status: SmokeStatus::Skipped,
        elapsed_ms: 0,
        message: "skipped due previous failure".to_string(),
    }
}

fn finalize_report(checks: Vec<SmokeCheck>, total_elapsed_ms: u64) -> CommandResult {
    let passed = checks.iter().filter(|check| check.status == SmokeStatus::Pass).count();
    let total = checks.len();
    let failed = checks.iter().any(|check| check.status == SmokeStatus::Fail);

    let report = SmokeReport {
        command: "smoke",
        status: if failed { SmokeStatus::Fail } else { SmokeStatus::Pass },
        summary: format!("smoke: {passed}/{total} checks passed in {total_elapsed_ms}ms"),
        total_elapsed_ms,
        checks,
    };

    let human = report.summary.clone();
    let machine = serde_json::to_string(&report).unwrap_or_else(|error| {
        format!(
            "{{\"command\":\"smoke\",\"status\":\"fail\",\"summary\":\"serialization failed\",\"error\":\"{}\"}}",
            error.to_string().replace('\\', "\\\\").replace('"', "\\\"")
        )
    });

    CommandResult { exit_code: if failed { 6 } else { 0 }, output: format!("{human}\n{machine}") }
}
