use crate::commands::CommandResult;
use quotey_core::config::{AppConfig, LoadOptions};
use quotey_db::{connect_with_settings, migrations, E2ESeedDataset};

pub fn run() -> CommandResult {
    let config = match AppConfig::load(LoadOptions::default()) {
        Ok(config) => config,
        Err(error) => {
            return CommandResult::failure(
                "seed",
                "config_validation",
                format!("configuration issue: {error}"),
                2,
            );
        }
    };

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            return CommandResult::failure(
                "seed",
                "runtime_init",
                format!("failed to initialize async runtime: {error}"),
                3,
            );
        }
    };

    let result = runtime.block_on(async {
        let pool = connect_with_settings(
            &config.database.url,
            config.database.max_connections,
            config.database.timeout_secs,
        )
        .await
        .map_err(|error| ("db_connectivity", error.to_string(), 4u8))?;

        migrations::run_pending(&pool)
            .await
            .map_err(|error| ("migration", error.to_string(), 5u8))?;

        // Load E2E seed dataset for 3 core quote flows
        let seed_result = E2ESeedDataset::load(&pool)
            .await
            .map_err(|error| ("seed_execution", error.to_string(), 5u8))?;

        // Verify the seed data was loaded
        let verification = E2ESeedDataset::verify(&pool)
            .await
            .map_err(|error| ("seed_verification", error.to_string(), 6u8))?;

        let run_result: Result<SeedOutput, (&'static str, String, u8)> =
            if !verification.all_present {
                let failed_checks = verification
                    .checks
                    .iter()
                    .filter_map(|(check, passed)| (!passed).then_some(*check))
                    .collect::<Vec<_>>();
                let message = if failed_checks.is_empty() {
                    "Some seed data failed to load".to_string()
                } else {
                    format!("Seed verification failed for checks: {}", failed_checks.join(", "))
                };
                Err(("seed_verification", message, 6u8))
            } else {
                Ok(SeedOutput { flows: seed_result.flows_seeded })
            };

        pool.close().await;
        run_result
    });

    match result {
        Ok(output) => {
            let flow_descriptions: Vec<String> = output
                .flows
                .iter()
                .map(|f| format!("  - {}: {} ({})", f.flow_type, f.quote_id, f.description))
                .collect();
            let message = format!(
                "E2E seed dataset loaded successfully for 3 core quote flows:\n{}",
                flow_descriptions.join("\n")
            );
            CommandResult::success("seed", message)
        }
        Err((error_class, message, exit_code)) => {
            CommandResult::failure("seed", error_class, message, exit_code)
        }
    }
}

struct SeedOutput {
    flows: Vec<quotey_db::FlowSeedInfo>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn verification_error_message_targets_failed_checks() {
        let checks = [
            ("audit-events", true),
            ("quote-discount-state", false),
            ("audit-netnew-created", false),
        ];

        let failed_checks = checks
            .iter()
            .filter_map(|(check, passed)| (!passed).then_some(*check))
            .collect::<Vec<_>>();

        let message = if failed_checks.is_empty() {
            "Some seed data failed to load".to_string()
        } else {
            format!("Seed verification failed for checks: {}", failed_checks.join(", "))
        };

        assert_eq!(
            message,
            "Seed verification failed for checks: quote-discount-state, audit-netnew-created"
        );
    }

    #[test]
    fn verification_error_message_falls_back_to_generic_when_no_labels() {
        let checks = [("quote-state", true), ("audit-events", true)];

        let failed_checks = checks
            .iter()
            .filter_map(|(check, passed)| (!passed).then_some(*check))
            .collect::<Vec<_>>();
        let message = if failed_checks.is_empty() {
            "Some seed data failed to load".to_string()
        } else {
            format!("Seed verification failed for checks: {}", failed_checks.join(", "))
        };

        assert_eq!(message, "Some seed data failed to load");
    }
}
