pub mod commands;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "quotey",
    about = "Quotey operator CLI",
    long_about = "Operate Quotey runtime readiness, migrations, config inspection, and smoke validation.",
    after_help = "Examples:\n  quotey doctor --json\n  quotey config\n  quotey smoke"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(about = "Run startup preflight checks and return structured status output")]
    Start,
    #[command(about = "Apply pending database migrations and return structured status output")]
    Migrate,
    #[command(
        about = "Load deterministic demo fixtures (currently a deterministic no-op scaffold)"
    )]
    Seed,
    #[command(about = "Run end-to-end readiness checks with per-check timing details")]
    Smoke,
    #[command(
        about = "Inspect effective configuration values with source attribution and redaction"
    )]
    Config,
    #[command(about = "Validate config, Slack token readiness, and DB connectivity checks")]
    Doctor {
        #[arg(long, help = "Emit machine-readable JSON output")]
        json: bool,
    },
}

pub fn run() -> ExitCode {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Start => commands::start::run(),
        Command::Migrate => commands::migrate::run(),
        Command::Seed => commands::seed::run(),
        Command::Smoke => commands::smoke::run(),
        Command::Config => {
            commands::CommandResult { exit_code: 0, output: commands::config::run() }
        }
        Command::Doctor { json } => {
            commands::CommandResult { exit_code: 0, output: commands::doctor::run(json) }
        }
    };

    println!("{}", result.output);
    ExitCode::from(result.exit_code)
}
