use async_trait::async_trait;
use thiserror::Error;

use crate::blocks::{self, MessageTemplate};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlashCommandPayload {
    pub command: String,
    pub text: String,
    pub channel_id: String,
    pub user_id: String,
    pub trigger_ts: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandEnvelope {
    pub command: String,
    pub verb: String,
    pub quote_id: Option<String>,
    pub account_hint: Option<String>,
    pub freeform_args: String,
    pub channel_id: String,
    pub user_id: String,
    pub trigger_ts: String,
    pub request_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuoteCommand {
    New { customer_hint: Option<String>, freeform_args: String },
    Status { quote_id: Option<String>, freeform_args: String },
    List { filter: Option<String> },
    Help,
    Unknown { verb: String, freeform_args: String },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandParseError {
    #[error("unsupported slash command: {0}")]
    UnsupportedCommand(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandRouteError {
    #[error("command service failed: {0}")]
    Service(String),
}

pub fn normalize_quote_command(
    payload: SlashCommandPayload,
) -> Result<CommandEnvelope, CommandParseError> {
    if payload.command != "/quote" {
        return Err(CommandParseError::UnsupportedCommand(payload.command));
    }

    let text = payload.text.trim().to_owned();
    let mut parts = text.split_whitespace();
    let verb = parts.next().unwrap_or("help").to_ascii_lowercase();
    let freeform_args = parts.collect::<Vec<_>>().join(" ");
    let quote_id = freeform_args.split_whitespace().find_map(parse_quote_id_token);
    let account_hint = extract_account_hint(&verb, &freeform_args);

    Ok(CommandEnvelope {
        command: "quote".to_owned(),
        verb,
        quote_id,
        account_hint,
        freeform_args,
        channel_id: payload.channel_id,
        user_id: payload.user_id,
        trigger_ts: payload.trigger_ts,
        request_id: payload.request_id,
    })
}

pub fn parse_quote_command(input: &str) -> QuoteCommand {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return QuoteCommand::Help;
    }

    let mut parts = trimmed.split_whitespace();
    let verb = parts.next().unwrap_or_default().to_ascii_lowercase();
    let freeform_args = parts.collect::<Vec<_>>().join(" ");
    classify_quote_command(&verb, freeform_args)
}

pub struct CommandRouter<S> {
    service: S,
}

impl<S> CommandRouter<S>
where
    S: QuoteCommandService,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }

    pub async fn route(
        &self,
        envelope: CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        match classify_quote_command(&envelope.verb, envelope.freeform_args.clone()) {
            QuoteCommand::New { customer_hint, freeform_args } => {
                self.service.new_quote(customer_hint, freeform_args, &envelope).await
            }
            QuoteCommand::Status { quote_id, freeform_args } => {
                self.service.status_quote(quote_id, freeform_args, &envelope).await
            }
            QuoteCommand::List { filter } => self.service.list_quotes(filter, &envelope).await,
            QuoteCommand::Help => Ok(blocks::help_message()),
            QuoteCommand::Unknown { verb, .. } => Ok(blocks::error_message(
                &format!("Unsupported command `/quote {verb}`. Try `/quote help`."),
                &envelope.request_id,
            )),
        }
    }
}

#[async_trait]
pub trait QuoteCommandService: Send + Sync {
    async fn new_quote(
        &self,
        customer_hint: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    async fn status_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    async fn list_quotes(
        &self,
        filter: Option<String>,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;
}

#[derive(Default)]
pub struct NoopQuoteCommandService;

#[async_trait]
impl QuoteCommandService for NoopQuoteCommandService {
    async fn new_quote(
        &self,
        customer_hint: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let summary = customer_hint.unwrap_or_else(|| "unassigned account".to_owned());
        Ok(blocks::quote_status_message(
            "pending",
            &format!("initialized ({summary}; {freeform_args})"),
        ))
    }

    async fn status_quote(
        &self,
        quote_id: Option<String>,
        _freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        Ok(blocks::quote_status_message(&quote_id, "status requested"))
    }

    async fn list_quotes(
        &self,
        filter: Option<String>,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let filter = filter.unwrap_or_else(|| "all".to_owned());
        Ok(blocks::quote_status_message("list", &format!("filter={filter}")))
    }
}

fn classify_quote_command(verb: &str, freeform_args: String) -> QuoteCommand {
    match verb {
        "new" => QuoteCommand::New {
            customer_hint: extract_account_hint(verb, &freeform_args),
            freeform_args,
        },
        "status" => QuoteCommand::Status {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "list" => QuoteCommand::List {
            filter: if freeform_args.is_empty() { None } else { Some(freeform_args) },
        },
        "help" => QuoteCommand::Help,
        _ => QuoteCommand::Unknown { verb: verb.to_owned(), freeform_args },
    }
}

fn extract_account_hint(verb: &str, args: &str) -> Option<String> {
    if verb != "new" {
        return None;
    }

    let normalized = args.trim();
    if let Some(without_prefix) = normalized.strip_prefix("for ") {
        let first_segment = without_prefix.split(',').next().unwrap_or(without_prefix);
        let candidate = first_segment.trim();
        if !candidate.is_empty() {
            return Some(candidate.to_owned());
        }
    }

    None
}

fn parse_quote_id_token(token: &str) -> Option<String> {
    let trimmed = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
    let bytes = trimmed.as_bytes();

    if bytes.len() != 11 || !trimmed.starts_with("Q-") {
        return None;
    }
    if bytes[6] != b'-' {
        return None;
    }

    if bytes[2..6].iter().all(u8::is_ascii_digit) && bytes[7..11].iter().all(u8::is_ascii_digit) {
        Some(trimmed.to_owned())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{
        normalize_quote_command, parse_quote_command, CommandEnvelope, CommandRouteError,
        CommandRouter, NoopQuoteCommandService, QuoteCommand, QuoteCommandService,
        SlashCommandPayload,
    };
    use crate::blocks::MessageTemplate;

    #[tokio::test]
    async fn routes_new_status_list_help_commands() {
        let router = CommandRouter::new(NoopQuoteCommandService);

        let new_response = router
            .route(CommandEnvelope {
                command: "quote".to_owned(),
                verb: "new".to_owned(),
                quote_id: None,
                account_hint: Some("Acme".to_owned()),
                freeform_args: "for Acme".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-1".to_owned(),
            })
            .await
            .expect("new route");
        assert!(!new_response.blocks.is_empty());

        let status_response = router
            .route(CommandEnvelope {
                command: "quote".to_owned(),
                verb: "status".to_owned(),
                quote_id: Some("Q-2026-0042".to_owned()),
                account_hint: None,
                freeform_args: "Q-2026-0042".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-2".to_owned(),
            })
            .await
            .expect("status route");
        assert!(!status_response.blocks.is_empty());

        let list_response = router
            .route(CommandEnvelope {
                command: "quote".to_owned(),
                verb: "list".to_owned(),
                quote_id: None,
                account_hint: None,
                freeform_args: "mine".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-3".to_owned(),
            })
            .await
            .expect("list route");
        assert!(!list_response.blocks.is_empty());

        let help_response = router
            .route(CommandEnvelope {
                command: "quote".to_owned(),
                verb: "help".to_owned(),
                quote_id: None,
                account_hint: None,
                freeform_args: String::new(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-4".to_owned(),
            })
            .await
            .expect("help route");
        assert!(!help_response.blocks.is_empty());
    }

    #[test]
    fn parse_quote_command_preserves_known_verbs() {
        assert!(matches!(parse_quote_command("new for Acme"), QuoteCommand::New { .. }));
        assert!(matches!(parse_quote_command("status Q-2026-0001"), QuoteCommand::Status { .. }));
        assert!(matches!(parse_quote_command("list mine"), QuoteCommand::List { .. }));
        assert!(matches!(parse_quote_command("help"), QuoteCommand::Help));
        assert!(matches!(parse_quote_command("something-else"), QuoteCommand::Unknown { .. }));
    }

    #[test]
    fn normalize_quote_command_extracts_quote_id_and_account_hint() {
        let envelope = normalize_quote_command(SlashCommandPayload {
            command: "/quote".to_owned(),
            text: "new for Acme Corp, Pro plan".to_owned(),
            channel_id: "C123".to_owned(),
            user_id: "U123".to_owned(),
            trigger_ts: "1700000000.1".to_owned(),
            request_id: "req-123".to_owned(),
        })
        .expect("normalized");

        assert_eq!(envelope.command, "quote");
        assert_eq!(envelope.verb, "new");
        assert_eq!(envelope.account_hint.as_deref(), Some("Acme Corp"));
        assert_eq!(envelope.quote_id, None);
    }

    #[tokio::test]
    async fn router_calls_service_entrypoints() {
        #[derive(Default)]
        struct RecordingService {
            calls: Mutex<Vec<&'static str>>,
        }

        #[async_trait::async_trait]
        impl QuoteCommandService for RecordingService {
            async fn new_quote(
                &self,
                _customer_hint: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("new");
                Ok(crate::blocks::help_message())
            }

            async fn status_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("status");
                Ok(crate::blocks::help_message())
            }

            async fn list_quotes(
                &self,
                _filter: Option<String>,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("list");
                Ok(crate::blocks::help_message())
            }
        }

        let router = CommandRouter::new(RecordingService::default());
        for verb in ["new", "status", "list"] {
            router
                .route(CommandEnvelope {
                    command: "quote".to_owned(),
                    verb: verb.to_owned(),
                    quote_id: Some("Q-2026-1111".to_owned()),
                    account_hint: None,
                    freeform_args: "Q-2026-1111".to_owned(),
                    channel_id: "C1".to_owned(),
                    user_id: "U1".to_owned(),
                    trigger_ts: "1".to_owned(),
                    request_id: format!("req-{verb}"),
                })
                .await
                .expect("route");
        }

        let calls = router.service.calls.lock().expect("lock");
        assert_eq!(&*calls, &["new", "status", "list"]);
    }
}
