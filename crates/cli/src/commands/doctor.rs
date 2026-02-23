use quotey_core::config::{AppConfig, LoadOptions};
use quotey_db::connect_with_settings;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: &'static str,
    status: CheckStatus,
    details: String,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    overall_status: CheckStatus,
    summary: String,
    checks: Vec<DoctorCheck>,
}

pub fn run(json_output: bool) -> String {
    let report = build_report();

    if json_output {
        return serde_json::to_string_pretty(&report).unwrap_or_else(|error| {
            format!(
                "{{\"overall_status\":\"fail\",\"summary\":\"doctor serialization failed\",\"error\":\"{}\"}}",
                escape_json(&error.to_string())
            )
        });
    }

    render_human(&report)
}

fn build_report() -> DoctorReport {
    let mut checks = Vec::new();

    match AppConfig::load(LoadOptions::default()) {
        Ok(config) => {
            checks.push(DoctorCheck {
                name: "config_validation",
                status: CheckStatus::Pass,
                details: "configuration loaded and validated".to_string(),
            });
            checks.push(check_slack_tokens(&config));
            checks.push(check_database_connectivity(&config));
        }
        Err(error) => {
            checks.push(DoctorCheck {
                name: "config_validation",
                status: CheckStatus::Fail,
                details: error.to_string(),
            });
            checks.push(DoctorCheck {
                name: "slack_token_readiness",
                status: CheckStatus::Skipped,
                details: "skipped because configuration did not load".to_string(),
            });
            checks.push(DoctorCheck {
                name: "database_connectivity",
                status: CheckStatus::Skipped,
                details: "skipped because configuration did not load".to_string(),
            });
        }
    }

    let all_pass = checks.iter().all(|check| check.status == CheckStatus::Pass);
    let overall_status = if all_pass { CheckStatus::Pass } else { CheckStatus::Fail };
    let summary = if all_pass {
        "doctor: all readiness checks passed".to_string()
    } else {
        "doctor: one or more readiness checks failed".to_string()
    };

    DoctorReport { overall_status, summary, checks }
}

fn check_slack_tokens(config: &AppConfig) -> DoctorCheck {
    let _ = config;
    DoctorCheck {
        name: "slack_token_readiness",
        status: CheckStatus::Pass,
        details: "token format validated by config contract".to_string(),
    }
}

fn check_database_connectivity(config: &AppConfig) -> DoctorCheck {
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            return DoctorCheck {
                name: "database_connectivity",
                status: CheckStatus::Fail,
                details: format!("failed to initialize async runtime: {error}"),
            };
        }
    };

    let result = runtime.block_on(async {
        let pool = connect_with_settings(
            &config.database.url,
            config.database.max_connections,
            config.database.timeout_secs,
        )
        .await
        .map_err(|error| format!("failed to connect to database: {error}"))?;

        pool.close().await;
        Ok::<(), String>(())
    });

    match result {
        Ok(()) => DoctorCheck {
            name: "database_connectivity",
            status: CheckStatus::Pass,
            details: format!("connected using `{}`", config.database.url),
        },
        Err(error) => {
            DoctorCheck { name: "database_connectivity", status: CheckStatus::Fail, details: error }
        }
    }
}

fn render_human(report: &DoctorReport) -> String {
    let mut lines = Vec::new();
    lines.push(report.summary.clone());

    for check in &report.checks {
        let marker = match check.status {
            CheckStatus::Pass => "ok",
            CheckStatus::Fail => "fail",
            CheckStatus::Skipped => "skip",
        };
        lines.push(format!("- [{marker}] {}: {}", check.name, check.details));
    }

    lines.join("\n")
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
