use crate::commands::CommandResult;
use quotey_core::config::{AppConfig, LoadOptions};
use quotey_db::{connect_with_settings, migrations};

pub fn run() -> CommandResult {
    let config = match AppConfig::load(LoadOptions::default()) {
        Ok(config) => config,
        Err(error) => {
            return CommandResult::failure(
                "migrate",
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
                "migrate",
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
        pool.close().await;
        Ok::<(), (&'static str, String, u8)>(())
    });

    match result {
        Ok(()) => CommandResult::success("migrate", "applied pending migrations"),
        Err((error_class, message, exit_code)) => {
            CommandResult::failure("migrate", error_class, message, exit_code)
        }
    }
}
