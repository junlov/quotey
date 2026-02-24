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
    #[command(about = "Build and review deterministic policy approval packets")]
    PolicyPacket {
        #[command(subcommand)]
        command: PolicyPacketCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PolicyPacketCommand {
    #[command(
        about = "Build a deterministic approval packet from candidate-diff/replay JSON payloads"
    )]
    Build {
        #[arg(long, help = "Candidate diff JSON payload (PolicyCandidateDiffV1)")]
        candidate_diff_json: String,
        #[arg(long, help = "Replay report JSON payload (ReplayImpactReport)")]
        replay_report_json: String,
        #[arg(long, help = "Base policy version")]
        base_policy_version: i32,
        #[arg(long, help = "Proposed policy version")]
        proposed_policy_version: i32,
        #[arg(long, help = "Risk score in basis points (0-10000)")]
        risk_score_bps: i32,
        #[arg(long, help = "Fallback plan summary text")]
        fallback_plan: String,
    },
    #[command(about = "Create deterministic action payload for approve/reject/request_changes")]
    Action {
        #[arg(long, help = "Approval packet JSON payload")]
        packet_json: String,
        #[arg(long, help = "Decision: approve|reject|request_changes")]
        decision: String,
        #[arg(long, help = "Reason text (required for reject/request_changes)")]
        reason: Option<String>,
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
        Command::PolicyPacket { command } => match command {
            PolicyPacketCommand::Build {
                candidate_diff_json,
                replay_report_json,
                base_policy_version,
                proposed_policy_version,
                risk_score_bps,
                fallback_plan,
            } => commands::policy_packet::run_build(
                candidate_diff_json,
                replay_report_json,
                base_policy_version,
                proposed_policy_version,
                risk_score_bps,
                fallback_plan,
            ),
            PolicyPacketCommand::Action { packet_json, decision, reason } => {
                commands::policy_packet::run_action(packet_json, decision, reason)
            }
        },
    };

    println!("{}", result.output);
    ExitCode::from(result.exit_code)
}
