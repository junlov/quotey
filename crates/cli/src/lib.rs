pub mod commands;

use clap::{Parser, Subcommand};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(name = "quotey", about = "Quotey CLI scaffold")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Start,
    Migrate,
    Seed,
    Config,
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
