use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

use crate::blocks::{self, MessageTemplate};

const SUPPORTED_QUOTE_VERBS: [&str; 11] = [
    "help", "new", "status", "list", "audit", "edit", "add-line", "discount", "send", "clone",
    "simulate",
];

fn suggest_supported_verb(input: &str) -> Option<&'static str> {
    let input = input.trim().to_ascii_lowercase();
    let mut best: Option<(usize, &'static str)> = None;

    for &candidate in &SUPPORTED_QUOTE_VERBS {
        let distance = levenshtein_distance(&input, candidate);
        match best {
            Some((current_best, _)) if distance >= current_best => {}
            _ => {
                if distance <= 2 {
                    best = Some((distance, candidate));
                }
            }
        }
    }

    best.map(|(_, candidate)| candidate)
}

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.bytes().enumerate() {
        let mut current = vec![0usize; b.len() + 1];
        current[0] = i + 1;
        for (j, cb) in b.bytes().enumerate() {
            let insertion = current[j] + 1;
            let deletion = previous[j + 1] + 1;
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = insertion.min(deletion).min(substitution);
        }
        previous = current;
    }
    previous[b.len()]
}

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
    Audit { quote_id: Option<String>, freeform_args: String },
    Edit { quote_id: Option<String>, freeform_args: String },
    AddLine { quote_id: Option<String>, freeform_args: String },
    Discount { quote_id: Option<String>, freeform_args: String },
    Send { quote_id: Option<String>, freeform_args: String },
    Clone { quote_id: Option<String>, freeform_args: String },
    Simulate { request: SimulationRequest },
    Help,
    Unknown { verb: String, freeform_args: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationLineAdjustment {
    pub product_id: String,
    pub quantity_delta: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationRequest {
    pub quote_id: Option<String>,
    pub variant_key: String,
    pub requested_discount_pct: Option<Decimal>,
    pub minimum_margin_pct: Option<Decimal>,
    pub deal_value: Option<Decimal>,
    pub line_adjustments: Vec<SimulationLineAdjustment>,
    pub raw_args: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationPromotionAction {
    pub quote_id: String,
    pub variant_key: String,
    pub action: String,
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
    #[error("unsupported interactive action: {0}")]
    UnsupportedInteractiveAction(String),
    #[error("invalid simulation action payload")]
    InvalidSimulationActionPayload,
}

pub fn normalize_quote_command(
    payload: SlashCommandPayload,
) -> Result<CommandEnvelope, CommandParseError> {
    if payload.command != "/quote" {
        return Err(CommandParseError::UnsupportedCommand(payload.command));
    }

    let text = payload.text.trim().to_owned();
    let mut parts = text.split_whitespace();
    let verb = normalize_quote_command_verb(parts.next().unwrap_or("help"));
    let freeform_args = parts.collect::<Vec<_>>().join(" ");
    let quote_id = freeform_args.split_whitespace().find_map(parse_quote_id_token);
    let account_hint = extract_account_hint(&verb, &freeform_args);

    Ok(CommandEnvelope {
        command: "quote".to_owned(),
        verb: if verb.is_empty() { "help".to_owned() } else { verb },
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
    let verb = normalize_quote_command_verb(parts.next().unwrap_or_default());
    if verb.is_empty() {
        return QuoteCommand::Help;
    }
    let freeform_args = parts.collect::<Vec<_>>().join(" ");
    classify_quote_command(&verb, freeform_args)
}

pub fn infer_thread_quote_command(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let cleaned = strip_common_prefixes(trimmed);
    let normalized = cleaned.to_ascii_lowercase();
    let quote_id = cleaned.split_whitespace().find_map(parse_quote_id_token).unwrap_or_default();

    if let Some(suffix) = remove_prefix(cleaned.as_str(), "/quote") {
        if suffix.is_empty() {
            return Some("help".to_owned());
        }
        return Some(suffix.to_owned());
    }

    if is_help_request(&normalized) {
        return Some("help".to_owned());
    }

    if is_status_request(&normalized) {
        if quote_id.is_empty() {
            return Some("status".to_owned());
        }
        return Some(format!("status {quote_id}"));
    }

    if is_simulate_request(&normalized) {
        let args = normalize_simulation_candidate(&cleaned);
        if args.is_empty() {
            return Some("simulate".to_owned());
        }
        return Some(format!("simulate {args}"));
    }

    if is_list_request(&normalized) {
        if normalized.contains("mine") || normalized.contains("my ") {
            return Some("list mine".to_owned());
        }
        return Some("list".to_owned());
    }

    if is_new_quote_request(&normalized) {
        let account = extract_account_from_thread_message(&cleaned);
        if account.is_empty() {
            return Some("new".to_owned());
        }
        return Some(format!("new for {account}"));
    }

    if is_audit_request(&normalized) {
        if quote_id.is_empty() {
            return Some("audit".to_owned());
        }
        return Some(format!("audit {quote_id}"));
    }

    if is_edit_request(&normalized) {
        if quote_id.is_empty() {
            return Some("edit".to_owned());
        }
        return Some(format!("edit {quote_id}"));
    }

    if is_add_line_request(&normalized) {
        if quote_id.is_empty() {
            return Some("add-line".to_owned());
        }
        return Some(format!("add-line {quote_id}"));
    }

    if is_discount_request(&normalized) {
        if quote_id.is_empty() {
            return Some("discount".to_owned());
        }
        return Some(format!("discount {quote_id}"));
    }

    if is_send_request(&normalized, &quote_id) {
        if quote_id.is_empty() {
            return Some("send".to_owned());
        }
        return Some(format!("send {quote_id}"));
    }

    if is_clone_request(&normalized) {
        if quote_id.is_empty() {
            return Some("clone".to_owned());
        }
        return Some(format!("clone {quote_id}"));
    }

    None
}

fn strip_common_prefixes(input: &str) -> String {
    let mut cleaned = input.trim().to_owned();

    loop {
        let lowered = cleaned.to_ascii_lowercase();
        let prefix_len = if lowered.starts_with("please ") {
            Some(7)
        } else if lowered.starts_with("can you ") {
            Some(8)
        } else if lowered.starts_with("could you ") {
            Some(10)
        } else if lowered.starts_with("i need ") || lowered.starts_with("i want ") {
            Some(7)
        } else if lowered.starts_with("can i ") {
            Some(6)
        } else {
            None
        };

        if let Some(prefix_len) = prefix_len {
            cleaned = cleaned[prefix_len..].trim_start().to_owned();
            continue;
        }

        break;
    }

    cleaned
}

fn remove_prefix<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
    let trimmed = input.trim();
    if !trimmed.starts_with(prefix) {
        return None;
    }
    if trimmed.len() == prefix.len() {
        return Some("");
    }
    if !trimmed.as_bytes().get(prefix.len()).is_some_and(u8::is_ascii_whitespace) {
        return None;
    }
    Some(trimmed[prefix.len()..].trim())
}

fn is_help_request(normalized: &str) -> bool {
    matches!(
        normalized,
        "help" | "help me" | "how to" | "what can i do" | "what can i do in this thread"
    ) || normalized.contains("show me help")
        || normalized.contains("what should i")
        || normalized.contains("usage")
}

fn token_matches(input: &str, expected: &str) -> bool {
    input
        .split_whitespace()
        .map(|token| token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-'))
        .any(|token| token == expected) // ubs:ignore
}

fn is_status_request(normalized: &str) -> bool {
    let has_quote = token_matches(normalized, "quote") || token_matches(normalized, "quotes");
    normalized.starts_with("status")
        || normalized.starts_with("check status")
        || (token_matches(normalized, "check") && token_matches(normalized, "status"))
        || (token_matches(normalized, "progress") && has_quote)
        || (token_matches(normalized, "state") && token_matches(normalized, "quote"))
        || (token_matches(normalized, "track") && token_matches(normalized, "quote"))
        || (normalized.starts_with("update on") && has_quote)
        || (normalized.starts_with("what is the status"))
        || (normalized.starts_with("what's status"))
}

fn is_simulate_request(normalized: &str) -> bool {
    normalized.starts_with("simulate")
        || normalized.contains("what if")
        || normalized.contains("scenario")
        || normalized.contains("preview")
}

fn is_list_request(normalized: &str) -> bool {
    let has_quote = token_matches(normalized, "quote") || token_matches(normalized, "quotes");
    normalized.starts_with("list")
        || (normalized.starts_with("show") && has_quote)
        || normalized.contains("open quote")
        || normalized.contains("all quotes")
        || normalized.contains("my quotes")
        || normalized.contains("latest quote")
        || normalized.contains("open my quotes")
        || normalized.contains("check my quotes")
}

fn is_audit_request(normalized: &str) -> bool {
    let has_quote = token_matches(normalized, "quote") || token_matches(normalized, "quotes");
    normalized.starts_with("audit")
        || normalized.contains("audit trail")
        || (token_matches(normalized, "audit") && has_quote)
        || normalized.contains("audit log")
        || normalized.contains("audit report")
        || normalized.contains("show audit")
}

fn is_edit_request(normalized: &str) -> bool {
    let has_quote = token_matches(normalized, "quote") || token_matches(normalized, "quotes");
    normalized.starts_with("edit")
        || (token_matches(normalized, "edit") && has_quote)
        || (token_matches(normalized, "change") && has_quote)
        || (token_matches(normalized, "modify") && has_quote)
        || (token_matches(normalized, "update") && has_quote)
}

fn is_add_line_request(normalized: &str) -> bool {
    normalized.starts_with("add-line")
        || normalized.starts_with("add line")
        || normalized.contains("add line")
        || normalized.starts_with("addline")
}

fn is_discount_request(normalized: &str) -> bool {
    let has_quote = token_matches(normalized, "quote") || token_matches(normalized, "quotes");
    normalized.starts_with("discount")
        || (normalized.contains("change discount") && has_quote)
        || (normalized.contains("discount to") && has_quote)
        || (normalized.contains("request discount") && has_quote)
        || (normalized.contains("exception") && has_quote)
}

fn is_send_request(normalized: &str, quote_id: &str) -> bool {
    let has_quote = token_matches(normalized, "quote") || token_matches(normalized, "quotes");
    (token_matches(normalized, "send") && (has_quote || !quote_id.is_empty()))
        || ((normalized.contains("send to")
            || normalized.contains("deliver")
            || normalized.contains("mail"))
            && (has_quote || !quote_id.is_empty()))
}

fn is_clone_request(normalized: &str) -> bool {
    normalized.starts_with("clone")
        || (normalized.contains("duplicate")
            && (token_matches(normalized, "quote") || token_matches(normalized, "quotes")))
        || normalized.contains("copy this quote")
        || normalized.contains("copy quote")
}

fn is_new_quote_request(normalized: &str) -> bool {
    normalized.starts_with("new")
        || normalized.starts_with("start")
        || normalized.starts_with("create")
        || normalized.starts_with("open")
        || normalized.contains("new quote")
        || normalized.contains("quote for")
        || normalized.contains("need quote")
}

fn extract_account_from_thread_message(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lowered = trimmed.to_ascii_lowercase();
    if let Some(index) = lowered.find("for ") {
        let raw_account = trimmed[index + 4..].trim();
        return sanitize_inferred_account(raw_account);
    }

    String::new()
}

fn normalize_simulation_candidate(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lowered = trimmed.to_ascii_lowercase();
    if let Some(candidate) = lowered.strip_prefix("what if").map(str::trim) {
        let candidate = candidate.trim();
        if !candidate.is_empty() {
            return candidate.to_owned();
        }
    }

    let kept_tokens: Vec<&str> =
        trimmed.split_whitespace().filter(|token| !is_simulation_noise_token(token)).collect();

    if kept_tokens.is_empty() {
        let quote_id =
            trimmed.split_whitespace().find_map(parse_quote_id_token).unwrap_or_default();
        if !quote_id.is_empty() {
            return quote_id;
        }

        return String::new();
    }

    kept_tokens.join(" ")
}

fn is_simulation_noise_token(token: &str) -> bool {
    matches!(
        token.to_ascii_lowercase().as_str(),
        "can" | "you" | "simulate" | "scenario" | "preview" | "what" | "if"
    )
}

fn sanitize_inferred_account(raw_account: &str) -> String {
    let first_segment = raw_account.split(',').next().unwrap_or(raw_account);
    first_segment.trim().trim_end_matches(['.', ',', ';', '!', '?']).trim().to_owned()
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

    pub fn route(&self, envelope: CommandEnvelope) -> Result<MessageTemplate, CommandRouteError> {
        match classify_quote_command(&envelope.verb, envelope.freeform_args.clone()) {
            QuoteCommand::New { customer_hint, freeform_args } => {
                self.service.new_quote(customer_hint, freeform_args, &envelope)
            }
            QuoteCommand::Status { quote_id, freeform_args } => {
                self.service.status_quote(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::List { filter } => self.service.list_quotes(filter, &envelope),
            QuoteCommand::Audit { quote_id, freeform_args } => {
                self.service.audit_quote(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::Edit { quote_id, freeform_args } => {
                self.service.edit_quote(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::AddLine { quote_id, freeform_args } => {
                self.service.add_line(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::Discount { quote_id, freeform_args } => {
                self.service.request_discount(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::Send { quote_id, freeform_args } => {
                self.service.send_quote(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::Clone { quote_id, freeform_args } => {
                self.service.clone_quote(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::Simulate { request } => self.service.simulate_quote(request, &envelope),
            QuoteCommand::Help => Ok(blocks::help_message()),
            QuoteCommand::Unknown { verb, .. } => {
                let suggestion = suggest_supported_verb(&verb)
                    .map(|candidate| format!(" Did you mean `/quote {candidate}`?"))
                    .unwrap_or_default();
                let supported = SUPPORTED_QUOTE_VERBS.join(", ");
                Ok(blocks::error_message(
                    &format!(
            "I couldn't parse `/quote {verb}`.{suggestion} Use `/quote help` for supported commands: {supported}. \
Tip: use explicit command mode for deterministic parsing (for example `/quote status Q-2026-1234`)."
                    ),
                    &envelope.request_id,
                ))
            }
        }
    }
}

pub trait QuoteCommandService: Send + Sync {
    fn new_quote(
        &self,
        customer_hint: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn status_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn list_quotes(
        &self,
        filter: Option<String>,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn audit_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn edit_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn add_line(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn request_discount(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn send_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn clone_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn simulate_quote(
        &self,
        request: SimulationRequest,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;
}

#[derive(Default)]
pub struct NoopQuoteCommandService;

impl QuoteCommandService for NoopQuoteCommandService {
    fn new_quote(
        &self,
        customer_hint: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let summary = customer_hint.unwrap_or_else(|| "unassigned account".to_owned());
        Ok(blocks::preview_mode_message(
            "/quote new",
            None,
            &format!("new quote intent captured for {summary}; args: {freeform_args}"),
            &_envelope.request_id,
        ))
    }

    fn status_quote(
        &self,
        quote_id: Option<String>,
        _freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        Ok(blocks::preview_mode_message(
            "/quote status",
            Some(&quote_id),
            "status lookup request captured",
            &_envelope.request_id,
        ))
    }

    fn list_quotes(
        &self,
        filter: Option<String>,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let filter = filter.unwrap_or_else(|| "all".to_owned());
        Ok(blocks::preview_mode_message(
            "/quote list",
            Some("list view"),
            &format!("list filter captured ({filter})"),
            &_envelope.request_id,
        ))
    }

    fn audit_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        let detail =
            if freeform_args.is_empty() { "audit requested".to_owned() } else { freeform_args };
        Ok(blocks::preview_mode_message(
            "/quote audit",
            Some(&quote_id),
            &format!("audit request captured · {detail}"),
            &_envelope.request_id,
        ))
    }

    fn edit_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        let detail =
            if freeform_args.is_empty() { "edit requested".to_owned() } else { freeform_args };
        Ok(blocks::preview_mode_message(
            "/quote edit",
            Some(&quote_id),
            &format!("edit request captured · {detail}"),
            &_envelope.request_id,
        ))
    }

    fn add_line(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        let detail =
            if freeform_args.is_empty() { "add-line requested".to_owned() } else { freeform_args };
        Ok(blocks::preview_mode_message(
            "/quote add-line",
            Some(&quote_id),
            &format!("add-line request captured · {detail}"),
            &_envelope.request_id,
        ))
    }

    fn request_discount(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        let detail =
            if freeform_args.is_empty() { "discount requested".to_owned() } else { freeform_args };
        Ok(blocks::preview_mode_message(
            "/quote discount",
            Some(&quote_id),
            &format!("discount request captured · {detail}"),
            &_envelope.request_id,
        ))
    }

    fn send_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        let detail =
            if freeform_args.is_empty() { "send requested".to_owned() } else { freeform_args };
        Ok(blocks::preview_mode_message(
            "/quote send",
            Some(&quote_id),
            &format!("send request captured · {detail}"),
            &_envelope.request_id,
        ))
    }

    fn clone_quote(
        &self,
        quote_id: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = quote_id.unwrap_or_else(|| "unknown".to_owned());
        let detail =
            if freeform_args.is_empty() { "clone requested".to_owned() } else { freeform_args };
        Ok(blocks::preview_mode_message(
            "/quote clone",
            Some(&quote_id),
            &format!("clone request captured · {detail}"),
            &_envelope.request_id,
        ))
    }

    fn simulate_quote(
        &self,
        request: SimulationRequest,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = request.quote_id.unwrap_or_else(|| "unknown".to_owned());
        Ok(blocks::preview_mode_message(
            "/quote simulate",
            Some(&quote_id),
            &format!(
                "simulate request captured: variant={} · adjustments={} · discount={:?}",
                request.variant_key,
                request.line_adjustments.len(),
                request.requested_discount_pct
            ),
            &_envelope.request_id,
        ))
    }
}

fn classify_quote_command(verb: &str, freeform_args: String) -> QuoteCommand {
    match verb {
        "new" | "start" | "create" | "open" => QuoteCommand::New {
            customer_hint: extract_account_hint(verb, &freeform_args),
            freeform_args,
        },
        "status" | "check" | "where" | "track" | "progress" => QuoteCommand::Status {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "list" | "show" => QuoteCommand::List {
            filter: if freeform_args.is_empty() { None } else { Some(freeform_args) },
        },
        "audit" => QuoteCommand::Audit {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "edit" => QuoteCommand::Edit {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "add-line" | "addline" => QuoteCommand::AddLine {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "discount" => QuoteCommand::Discount {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "send" => QuoteCommand::Send {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "clone" => QuoteCommand::Clone {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "simulate" => QuoteCommand::Simulate { request: parse_simulation_request(freeform_args) },
        "what" if freeform_args.to_ascii_lowercase().contains("if") => {
            QuoteCommand::Simulate { request: parse_simulation_request(freeform_args) }
        }
        "help" => QuoteCommand::Help,
        _ => QuoteCommand::Unknown { verb: verb.to_owned(), freeform_args },
    }
}

fn extract_account_hint(verb: &str, args: &str) -> Option<String> {
    if !matches!(verb, "new" | "start" | "create" | "open") {
        return None;
    }

    let trimmed = args.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lowered = trimmed.to_ascii_lowercase();

    if lowered.starts_with("for ") {
        let candidate = sanitize_inferred_account(&trimmed[4..]);
        if !candidate.is_empty() {
            return Some(candidate);
        }
    }

    if let Some(index) = lowered.find(" for ") {
        let candidate = sanitize_inferred_account(&trimmed[index + 4..]);
        if !candidate.is_empty() {
            return Some(candidate.to_owned());
        }
    }

    None
}

pub fn build_simulation_promotion_value(quote_id: &str, variant_key: &str) -> String {
    blocks::simulation_promotion_action_value(quote_id, variant_key)
}

pub fn parse_simulation_promotion_value(value: &str) -> Option<SimulationPromotionAction> {
    let mut action = None::<String>;
    let mut quote_id = None::<String>;
    let mut variant_key = None::<String>;

    for segment in value.split(';') {
        if segment.trim().is_empty() {
            continue;
        }
        let (key, raw_value) = segment.split_once('=')?;
        let normalized_key = key.trim().to_ascii_lowercase();
        let normalized_value = decode_action_value_component(raw_value.trim())?;
        if normalized_value.is_empty() {
            return None;
        }

        match normalized_key.as_str() {
            "action" => {
                if action.replace(normalized_value).is_some() {
                    return None;
                }
            }
            "quote" => {
                let parsed_quote = parse_quote_id_token(&normalized_value)?;
                if quote_id.replace(parsed_quote).is_some() {
                    return None;
                }
            }
            "variant" => {
                if variant_key.replace(normalized_value).is_some() {
                    return None;
                }
            }
            _ => return None,
        }
    }

    Some(SimulationPromotionAction {
        quote_id: quote_id?,
        variant_key: variant_key?,
        action: action?,
    })
}

pub fn help_command_shortcut_message(
    action_id: &str,
    quote_id: Option<&str>,
) -> Option<MessageTemplate> {
    let action = action_id.trim();
    let quote_ref = match quote_id.filter(|value| !value.is_empty()) {
        Some(value) if value != "unknown" => value.to_owned(),
        _ => "<quote_id>".to_owned(),
    };

    match action {
        "quote.help.command.new.v1" => Some(blocks::command_shortcut_message(
            "/quote new for <customer>",
            "Create a new draft directly from your context",
            "new for Acme",
        )),
        "quote.help.command.status.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote status {quote_ref}"),
            "Check deterministic quote state and checkpoint progress",
            &format!("status {quote_ref}"),
        )),
        "quote.help.command.list.v1" => Some(blocks::command_shortcut_message(
            "/quote list mine",
            "Retrieve your active/open quotes for quick auditability",
            "list mine",
        )),
        "quote.help.command.audit.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote audit {quote_ref}"),
            "Inspect pricing policy rationale and rule traces",
            &format!("audit {quote_ref}"),
        )),
        "quote.help.command.simulate.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote simulate {quote_ref} variant=lean discount=10"),
            "Run controlled pricing what-if evaluation",
            &format!("simulate {quote_ref} variant=lean discount=10"),
        )),
        "quote.help.command.discount.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote discount {quote_ref} 12.5"),
            "Request an exception with explicit percent input",
            &format!("discount {quote_ref} 12.5"),
        )),
        "quote.help.command.send.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote send {quote_ref}"),
            "Trigger approval + delivery orchestration",
            &format!("send {quote_ref}"),
        )),
        "quote.help.command.clone.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote clone {quote_ref}"),
            "Create a draft derivative for iterative negotiation",
            &format!("clone {quote_ref}"),
        )),
        "quote.help.command.edit.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote edit {quote_ref}"),
            "Adjust core configuration fields in one explicit command",
            &format!("edit {quote_ref} term=36"),
        )),
        "quote.help.command.add-line.v1" => Some(blocks::command_shortcut_message(
            &format!("/quote add-line {quote_ref} addon:+1"),
            "Apply deterministic line-level adjustment",
            &format!("add-line {quote_ref} addon:+1"),
        )),
        _ => None,
    }
}

pub fn handle_block_action(
    action_id: &str,
    raw_value: Option<&str>,
    fallback_quote_id: Option<&str>,
    request_id: &str,
) -> Result<MessageTemplate, CommandRouteError> {
    let action = action_id.trim();
    let quote_id = action_quote_id(raw_value, fallback_quote_id);
    if let Some(message) = help_command_shortcut_message(action, Some(&quote_id)) {
        return Ok(message);
    }
    let value_pairs = action_value_pairs(raw_value);
    let task_id = value_pairs
        .as_ref()
        .and_then(|pairs| pairs.get("task"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let candidate_id = value_pairs
        .as_ref()
        .and_then(|pairs| pairs.get("candidate"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let packet_id = value_pairs
        .as_ref()
        .and_then(|pairs| pairs.get("packet"))
        .cloned()
        .unwrap_or_else(|| "unknown".to_owned());
    let execution_status = match action {
        "exec.refresh.v1" => format!(
            "execution control `{action}` received for task `{task_id}` (request={request_id}). Queue snapshot refresh is in progress.",
        ),
        "exec.view_status.v1" => format!(
            "execution control `{action}` received for task `{task_id}` (request={request_id}). Fresh status context will emit as workflow state rehydrates.",
        ),
        "exec.view_result.v1" => format!(
            "execution control `{action}` received for task `{task_id}` (request={request_id}). Retrieving deterministic result summary now.",
        ),
        "exec.retry_now.v1" => format!(
            "execution control `{action}` received for task `{task_id}` (request={request_id}). Retry request recorded with idempotent queue semantics.",
        ),
        "exec.cancel.v1" => format!(
            "execution control `{action}` received for task `{task_id}` (request={request_id}). Cancel command will reconcile downstream state transitions.",
        ),
        "exec.view_error.v1" => format!(
            "execution control `{action}` received for task `{task_id}` (request={request_id}). Fetching structured failure context.",
        ),
        "exec.contact_support.v1" => format!(
            "execution control `{action}` received for task `{task_id}` (request={request_id}). Support context has been prepared for handoff.",
        ),
        _ => String::new(),
    };
    let policy_status = |action: &str| {
        let decision = if action.ends_with("approve.v1") {
            "approve"
        } else if action.ends_with("reject.v1") {
            "reject"
        } else {
            "request changes"
        };
        format!("policy packet action `{decision}` captured for candidate `{candidate_id}` (packet `{packet_id}`)")
    };

    match action {
        "quote.refresh.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            &format!("refresh requested (request={request_id})"),
        )),
        "quote.help.v1" => Ok(blocks::help_message()),
        "quote.simulate.promote.v1" => {
            let payload = raw_value.ok_or(CommandRouteError::InvalidSimulationActionPayload)?;
            handle_simulation_promotion_action(payload, request_id)
        }
        "approval.approve.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "approval action captured (approve). This signal is recorded and routed through deterministic flow execution.",
        )),
        "approval.reject.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "approval action captured (reject). This signal is recorded and routed through deterministic flow execution.",
        )),
        "approval.request_changes.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "approval action captured (requested changes). This signal is recorded and routed through deterministic flow execution.",
        )),
        "approval.approve.emoji.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "emoji approval captured (approve). This signal is recorded and routed through deterministic flow execution.",
        )),
        "approval.reject.emoji.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "emoji approval captured (reject). This signal is recorded and routed through deterministic flow execution.",
        )),
        "approval.discuss.emoji.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "emoji approval captured (discuss). This signal is recorded and routed through deterministic flow execution.",
        )),
        "approval.view_quote.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "quote detail view requested. Status card is shown while detailed views are finalized in the deterministic workflow.",
        )),
        "approval.view_policy.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "policy packet view requested. Deterministic policy details will appear in-thread as workflow stages complete.",
        )),
        "policy.packet.approve.v1" | "policy.packet.reject.v1" | "policy.packet.request_changes.v1" => {
            Ok(blocks::quote_status_message(&quote_id, &policy_status(action)))
        }
        "exec.refresh.v1"
            | "exec.view_status.v1"
            | "exec.view_result.v1"
            | "exec.retry_now.v1"
            | "exec.cancel.v1"
            | "exec.view_error.v1"
            | "exec.contact_support.v1" => {
            Ok(blocks::quote_status_message(&quote_id, &execution_status))
        }
        "quote.deal_dna.expand.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "expanding Deal DNA list. Premium Deal DNA panel is loading from deterministic context.",
        )),
        value if value.starts_with("quote.deal_dna.view_details.") => {
            Ok(blocks::quote_status_message(
                &quote_id,
                "opening Deal DNA detail card. Full detail view is queued through structured decision context.",
            ))
        }
        _ => Ok(blocks::error_message(
            &format!(
                "I couldn't process `{action}` in this context (quote `{quote_id}`). \
Use `/quote help` for supported button and slash-command actions."
            ),
            request_id,
        )),
    }
}

pub(crate) fn action_value_pairs(raw_value: Option<&str>) -> Option<HashMap<String, String>> {
    let value = raw_value?;
    if value.trim().is_empty() {
        return Some(HashMap::new());
    }

    let mut pairs = HashMap::with_capacity(4);
    for raw_segment in value.split(';') {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            continue;
        }

        let (raw_key, encoded_value) = segment.split_once('=')?;
        let decoded = decode_action_value_component(encoded_value.trim())?;
        pairs.insert(raw_key.trim().to_ascii_lowercase(), decoded);
    }
    Some(pairs)
}

pub(crate) fn action_quote_id(raw_value: Option<&str>, fallback: Option<&str>) -> String {
    let fallback_quote_id = fallback.unwrap_or("unknown");
    let Some(raw_value) = raw_value else {
        return fallback_quote_id.to_owned();
    };

    let pairs = match action_value_pairs(Some(raw_value)) {
        Some(pairs) => pairs,
        None => return fallback_quote_id.to_owned(),
    };

    if let Some(quoted) = pairs.get("quote").and_then(|value| parse_quote_id_token(value)) {
        return quoted;
    }

    for value in pairs.values() {
        if let Some(quoted) = parse_quote_id_token(value) {
            return quoted;
        }
    }

    fallback_quote_id.to_owned()
}

pub fn handle_simulation_promotion_action(
    payload: &str,
    request_id: &str,
) -> Result<MessageTemplate, CommandRouteError> {
    let action = parse_simulation_promotion_value(payload)
        .ok_or(CommandRouteError::InvalidSimulationActionPayload)?;

    if !action.action.eq_ignore_ascii_case("promote") {
        return Err(CommandRouteError::InvalidSimulationActionPayload);
    }

    Ok(blocks::quote_status_message(
        &action.quote_id,
        &format!(
            "simulation variant `{}` promotion requested (request={request_id})",
            action.variant_key
        ),
    ))
}

fn parse_simulation_request(raw_args: String) -> SimulationRequest {
    let mut request = SimulationRequest {
        quote_id: None,
        variant_key: "variant-1".to_owned(),
        requested_discount_pct: None,
        minimum_margin_pct: None,
        deal_value: None,
        line_adjustments: Vec::new(),
        raw_args: raw_args.clone(),
    };
    let mut adjustment_totals: HashMap<String, i32> = HashMap::new();

    for token in raw_args.split_whitespace() {
        if request.quote_id.is_none() {
            request.quote_id = parse_quote_id_token(token);
            if request.quote_id.is_some() {
                continue;
            }
        }

        if let Some((key, value)) = token.split_once('=') {
            match key.trim().to_ascii_lowercase().as_str() {
                "variant" => {
                    let candidate = value.trim();
                    if !candidate.is_empty() {
                        request.variant_key = candidate.to_owned();
                    }
                }
                "discount" => {
                    request.requested_discount_pct = parse_decimal_token(value);
                }
                "margin" | "min_margin" => {
                    request.minimum_margin_pct = parse_decimal_token(value);
                }
                "deal" | "deal_value" => {
                    request.deal_value = parse_decimal_token(value);
                }
                _ => {}
            }
            continue;
        }

        if let Some((product_id, delta)) = parse_line_adjustment_token(token) {
            if delta == 0 {
                continue;
            }
            adjustment_totals
                .entry(product_id)
                .and_modify(|existing| *existing += delta)
                .or_insert(delta);
        }
    }

    let mut line_adjustments = adjustment_totals
        .into_iter()
        .map(|(product_id, quantity_delta)| SimulationLineAdjustment { product_id, quantity_delta })
        .collect::<Vec<_>>();
    line_adjustments.retain(|item| item.quantity_delta != 0);
    line_adjustments.sort_by(|left, right| left.product_id.cmp(&right.product_id));
    request.line_adjustments = line_adjustments;
    request
}

fn parse_line_adjustment_token(token: &str) -> Option<(String, i32)> {
    let (product_id, raw_delta) = token.split_once(':')?;
    let normalized_product = product_id.trim();
    if normalized_product.is_empty() {
        return None;
    }

    let delta = raw_delta.trim().parse::<i32>().ok()?;
    Some((normalized_product.to_owned(), delta))
}

fn parse_decimal_token(token: &str) -> Option<Decimal> {
    let trimmed = token.trim().trim_end_matches('%');
    Decimal::from_str(trimmed).ok()
}

fn decode_action_value_component(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;

    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return None;
                }

                let high = hex_nibble(bytes[index + 1])?;
                let low = hex_nibble(bytes[index + 2])?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn parse_quote_id_token(token: &str) -> Option<String> {
    let trimmed = token
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
        .to_ascii_uppercase();
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

fn normalize_quote_command_verb(raw: &str) -> String {
    raw.trim()
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{
        action_quote_id, action_value_pairs, build_simulation_promotion_value, handle_block_action,
        handle_simulation_promotion_action, infer_thread_quote_command, normalize_quote_command,
        parse_quote_command, parse_quote_id_token, parse_simulation_promotion_value,
        suggest_supported_verb, CommandEnvelope, CommandRouteError, CommandRouter,
        NoopQuoteCommandService, QuoteCommand, QuoteCommandService, SimulationRequest,
        SlashCommandPayload,
    };
    use crate::blocks::MessageTemplate;

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[test]
    fn routes_new_status_list_help_commands() {
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
            .expect("help route");
        assert!(!help_response.blocks.is_empty());
    }

    #[test]
    fn parse_quote_command_preserves_known_verbs() {
        assert!(matches!(parse_quote_command("new for Acme"), QuoteCommand::New { .. }));
        assert!(matches!(parse_quote_command("start quote for Acme"), QuoteCommand::New { .. }));
        assert!(matches!(parse_quote_command("status Q-2026-0001"), QuoteCommand::Status { .. }));
        assert!(matches!(parse_quote_command("list mine"), QuoteCommand::List { .. }));
        assert!(matches!(parse_quote_command("show mine"), QuoteCommand::List { .. }));
        assert!(matches!(parse_quote_command("audit Q-2026-0101"), QuoteCommand::Audit { .. }));
        assert!(matches!(
            parse_quote_command("edit Q-2026-0101 line discount"),
            QuoteCommand::Edit { .. }
        ));
        assert!(matches!(
            parse_quote_command("add-line Q-2026-0101 addon:+2"),
            QuoteCommand::AddLine { .. }
        ));
        assert!(matches!(
            parse_quote_command("discount Q-2026-0101 28"),
            QuoteCommand::Discount { .. }
        ));
        assert!(matches!(parse_quote_command("send Q-2026-0101 now"), QuoteCommand::Send { .. }));
        assert!(matches!(parse_quote_command("clone Q-2026-0101"), QuoteCommand::Clone { .. }));
        assert!(matches!(parse_quote_command("help?"), QuoteCommand::Help));
        assert!(matches!(
            parse_quote_command("simulate Q-2026-0001 variant=v1 discount=10% plan-pro:+5"),
            QuoteCommand::Simulate { .. }
        ));
        assert!(matches!(parse_quote_command("help"), QuoteCommand::Help));
        assert!(matches!(parse_quote_command("something-else"), QuoteCommand::Unknown { .. }));
    }

    #[test]
    fn suggest_supported_verb_offers_typo_hints_for_unknown_commands() {
        assert_eq!(suggest_supported_verb("statuz"), Some("status"));
        assert_eq!(suggest_supported_verb("edti"), Some("edit"));
        assert_eq!(suggest_supported_verb("sendd"), Some("send"));
        assert_eq!(suggest_supported_verb("hlep"), Some("help"));
        assert_eq!(suggest_supported_verb("xyz"), None);
    }

    #[test]
    fn router_unknown_command_returns_supported_command_help() {
        let router = CommandRouter::new(NoopQuoteCommandService);
        let response = router
            .route(CommandEnvelope {
                command: "quote".to_owned(),
                verb: "statuz".to_owned(),
                quote_id: Some("Q-2026-1111".to_owned()),
                account_hint: None,
                freeform_args: String::new(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-unknown".to_owned(),
            })
            .expect("route");

        assert!(response.fallback_text.contains("/quote help"));
        assert!(response.fallback_text.contains("Did you mean `/quote status`?"));
    }

    #[test]
    fn parse_quote_command_extracts_simulation_arguments() {
        let command = parse_quote_command(
            "simulate Q-2026-4242 variant=renewal-lean discount=12.5% margin=38 deal=85000 plan-pro:+20 support-premium:-1",
        );

        assert!(matches!(command, QuoteCommand::Simulate { .. }), "expected simulate command");
        let request = match command {
            QuoteCommand::Simulate { request } => request,
            _ => return,
        };

        assert_eq!(request.quote_id.as_deref(), Some("Q-2026-4242"));
        assert_eq!(request.variant_key, "renewal-lean");
        assert_eq!(request.requested_discount_pct.map(|d| d.to_string()).as_deref(), Some("12.5"));
        assert_eq!(request.minimum_margin_pct.map(|d| d.to_string()).as_deref(), Some("38"));
        assert_eq!(request.deal_value.map(|d| d.to_string()).as_deref(), Some("85000"));
        assert_eq!(request.line_adjustments.len(), 2);
        assert_eq!(request.line_adjustments[0].product_id, "plan-pro");
        assert_eq!(request.line_adjustments[0].quantity_delta, 20);
        assert_eq!(request.line_adjustments[1].product_id, "support-premium");
        assert_eq!(request.line_adjustments[1].quantity_delta, -1);
    }

    #[test]
    fn simulation_promotion_value_round_trips() {
        let payload = build_simulation_promotion_value("Q-2026-9999", "discounted_10");
        let parsed = parse_simulation_promotion_value(&payload).expect("parse action payload");
        assert_eq!(parsed.quote_id, "Q-2026-9999");
        assert_eq!(parsed.variant_key, "discounted_10");
        assert_eq!(parsed.action, "promote");
    }

    #[test]
    fn simulation_promotion_value_round_trips_with_escaped_characters() {
        let payload =
            build_simulation_promotion_value("Q-2026-9999", "renewal+uplift;quote=Q-2026-0001");
        let parsed = parse_simulation_promotion_value(&payload).expect("parse escaped payload");
        assert_eq!(parsed.quote_id, "Q-2026-9999");
        assert_eq!(parsed.variant_key, "renewal+uplift;quote=Q-2026-0001");
        assert_eq!(parsed.action, "promote");
    }

    #[test]
    fn simulation_promotion_handler_accepts_idempotent_payload() {
        let payload = build_simulation_promotion_value("Q-2026-9999", "discounted_10");
        let message =
            handle_simulation_promotion_action(&payload, "req-sim-promote").expect("handle action");
        assert!(message.fallback_text.contains("Q-2026-9999"));
        assert!(message.fallback_text.contains("promotion requested"));
    }

    #[test]
    fn simulation_promotion_handler_rejects_invalid_payload() {
        let result = handle_simulation_promotion_action("quote=Q-1;variant=v1", "req-invalid");
        assert!(matches!(
            result.expect_err("must fail"),
            CommandRouteError::InvalidSimulationActionPayload
        ));
    }

    #[test]
    fn simulation_promotion_parser_rejects_duplicate_or_tampered_keys() {
        let tampered = "action=promote;quote=Q-2026-1111;variant=v1;quote=Q-2026-9999";
        assert!(parse_simulation_promotion_value(tampered).is_none());
    }

    #[test]
    fn infer_thread_quote_command_maps_thread_language_to_commands() {
        assert_eq!(
            infer_thread_quote_command("Can you check status for Q-2026-1234?").as_deref(),
            Some("status Q-2026-1234")
        );
        assert_eq!(
            infer_thread_quote_command("can you check my quotes").as_deref(),
            Some("list mine")
        );
        assert_eq!(
            infer_thread_quote_command("I need a new quote for Acme").as_deref(),
            Some("new for Acme")
        );
        assert_eq!(
            infer_thread_quote_command("simulate Q-2026-1001 discount=10% addon:+5").as_deref(),
            Some("simulate Q-2026-1001 discount=10% addon:+5")
        );
        assert_eq!(
            infer_thread_quote_command("can you audit Q-2026-1234").as_deref(),
            Some("audit Q-2026-1234")
        );
        assert_eq!(
            infer_thread_quote_command("send the draft for Q-2026-4321").as_deref(),
            Some("send Q-2026-4321")
        );
        assert_eq!(
            infer_thread_quote_command("clone Q-2026-1111").as_deref(),
            Some("clone Q-2026-1111")
        );
        assert_eq!(infer_thread_quote_command("Q-2026-9999").as_deref(), None);
        assert_eq!(
            infer_thread_quote_command("can you show me my quotes").as_deref(),
            Some("list mine")
        );
        assert_eq!(
            infer_thread_quote_command("/quote status Q-2026-1111").as_deref(),
            Some("status Q-2026-1111")
        );
        assert_eq!(infer_thread_quote_command("what should I do").as_deref(), Some("help"));
        assert_eq!(infer_thread_quote_command("random thread banter"), None);
        assert_eq!(infer_thread_quote_command("can we grab coffee"), None);
        assert_eq!(infer_thread_quote_command("how are you"), None);
        assert_eq!(infer_thread_quote_command("update status of the board"), None);
        assert_eq!(infer_thread_quote_command("send my update to the channel"), None);
        assert_eq!(infer_thread_quote_command("can you modify these files"), None);
    }

    #[test]
    fn normalize_quote_command_handles_capitalized_account_prefix() {
        let envelope = normalize_quote_command(SlashCommandPayload {
            command: "/quote".to_owned(),
            text: "new For Acme Corp, Platinum".to_owned(),
            channel_id: "C123".to_owned(),
            user_id: "U123".to_owned(),
            trigger_ts: "1700000000.1".to_owned(),
            request_id: "req-124".to_owned(),
        })
        .expect("normalized");

        assert_eq!(envelope.account_hint.as_deref(), Some("Acme Corp"));
    }

    #[test]
    fn block_action_handlers_route_known_actions_with_fallback_quote() {
        let message = handle_block_action("quote.refresh.v1", None, Some("Q-2026-7001"), "req-1")
            .expect("action handled");
        assert!(message.fallback_text.contains("Q-2026-7001"));
        assert!(message.fallback_text.contains("refresh requested"));

        let message = handle_block_action(
            "approval.approve.emoji.v1",
            Some("task=review"),
            Some("Q-2026-7002"),
            "req-2",
        )
        .expect("action handled");
        assert!(message.fallback_text.contains("Q-2026-7002"));
        assert!(message.fallback_text.contains("emoji approval captured"));
    }

    #[test]
    fn block_action_handles_simulation_promotion_payload() {
        let payload = build_simulation_promotion_value("Q-2026-7777", "discounted_10");
        let message = handle_block_action(
            "quote.simulate.promote.v1",
            Some(&payload),
            Some("Q-2026-7003"),
            "req-3",
        )
        .expect("promotion action handled");
        assert!(message.fallback_text.contains("Q-2026-7777"));
        assert!(message.fallback_text.contains("promotion requested"));
    }

    #[test]
    fn block_action_handles_command_shortcuts() {
        let message = handle_block_action(
            "quote.help.command.status.v1",
            Some("quote=Q-2026-4444"),
            Some("Q-2026-4444"),
            "req-shortcut",
        )
        .expect("shortcut action handled");
        assert!(message.fallback_text.contains("Recommended next step"));
        assert!(message.fallback_text.contains("/quote status Q-2026-4444"));

        let message = handle_block_action(
            "quote.help.command.new.v1",
            None,
            Some("Q-2026-4444"),
            "req-shortcut-2",
        )
        .expect("shortcut action handled");
        assert!(message.fallback_text.contains("Run command: /quote new for <customer>"));
    }

    #[test]
    fn block_action_unknown_action_returns_guidance_message() {
        let response = handle_block_action("mystery.action", None, Some("Q-2026-9001"), "req-4")
            .expect("handled");
        assert!(response.fallback_text.contains("I couldn't process"));
        assert!(response.fallback_text.contains("Q-2026-9001"));
        assert!(response.fallback_text.contains("supported button"));
    }

    #[test]
    fn block_action_rejects_invalid_simulation_promotion_payload() {
        let error = handle_block_action(
            "quote.simulate.promote.v1",
            Some("quote=Q-2026-1111"),
            Some("Q-2026-9001"),
            "req-5",
        )
        .expect_err("missing variant should fail");
        assert!(matches!(error, CommandRouteError::InvalidSimulationActionPayload));
    }

    #[test]
    fn action_value_parsing_and_fallback_quote_detection() {
        let encoded = "quote=Q-2026-8888;task=task%3Aretry%3D1;state=retryable_failed_2_of_3";
        let pairs = action_value_pairs(Some(encoded)).expect("pairs parsed");
        assert_eq!(pairs.get("quote").expect("quote"), "Q-2026-8888");
        assert_eq!(pairs.get("task").expect("task"), "task:retry=1");

        let quoted = action_quote_id(Some(encoded), Some("Q-2026-0001"));
        assert_eq!(quoted, "Q-2026-8888");

        let fallback = action_quote_id(Some("task=abc%3Bx%3Dy"), Some("Q-2026-5555"));
        assert_eq!(fallback, "Q-2026-5555");
    }

    #[test]
    fn parse_quote_id_token_accepts_lowercase_quote_id() {
        assert_eq!(parse_quote_id_token("q-2026-0420"), Some("Q-2026-0420".to_string()));
    }

    #[test]
    fn parse_simulation_request_dedupes_and_filters_zero_deltas() {
        let request = parse_quote_command(
            "simulate Q-2026-1111 addon:+2 addon:-1 addon:+3 support:-1 addon:0",
        );

        if let QuoteCommand::Simulate { request } = request {
            assert_eq!(request.line_adjustments.len(), 2);
            assert_eq!(request.line_adjustments[0].product_id, "addon");
            assert_eq!(request.line_adjustments[0].quantity_delta, 4);
            assert_eq!(request.line_adjustments[1].product_id, "support");
            assert_eq!(request.line_adjustments[1].quantity_delta, -1);
        } else {
            unreachable!("expected simulate command");
        }
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

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[test]
    fn router_calls_service_entrypoints() {
        #[derive(Default)]
        struct RecordingService {
            calls: Mutex<Vec<&'static str>>,
        }

        impl QuoteCommandService for RecordingService {
            fn new_quote(
                &self,
                _customer_hint: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("new");
                Ok(crate::blocks::help_message())
            }

            fn status_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("status");
                Ok(crate::blocks::help_message())
            }

            fn list_quotes(
                &self,
                _filter: Option<String>,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("list");
                Ok(crate::blocks::help_message())
            }

            fn simulate_quote(
                &self,
                _request: SimulationRequest,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("simulate");
                Ok(crate::blocks::help_message())
            }

            fn audit_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("audit");
                Ok(crate::blocks::help_message())
            }

            fn edit_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("edit");
                Ok(crate::blocks::help_message())
            }

            fn add_line(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("add-line");
                Ok(crate::blocks::help_message())
            }

            fn request_discount(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("discount");
                Ok(crate::blocks::help_message())
            }

            fn send_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("send");
                Ok(crate::blocks::help_message())
            }

            fn clone_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("clone");
                Ok(crate::blocks::help_message())
            }
        }

        let router = CommandRouter::new(RecordingService::default());
        for (verb, args) in [
            ("new", "for Acme"),
            ("status", "Q-2026-1111"),
            ("list", "mine"),
            ("simulate", "Q-2026-1111 variant=v1 plan-pro:+1"),
            ("audit", "Q-2026-1111"),
            ("edit", "Q-2026-1111 term change"),
            ("add-line", "Q-2026-1111 addon:+1"),
            ("discount", "Q-2026-1111 25"),
            ("send", "Q-2026-1111"),
            ("clone", "Q-2026-1111"),
        ] {
            router
                .route(CommandEnvelope {
                    command: "quote".to_owned(),
                    verb: verb.to_owned(),
                    quote_id: Some("Q-2026-1111".to_owned()),
                    account_hint: None,
                    freeform_args: args.to_owned(),
                    channel_id: "C1".to_owned(),
                    user_id: "U1".to_owned(),
                    trigger_ts: "1".to_owned(),
                    request_id: format!("req-{verb}"),
                })
                .expect("route");
        }

        let calls = router.service.calls.lock().expect("lock");
        assert_eq!(
            &*calls,
            &[
                "new", "status", "list", "simulate", "audit", "edit", "add-line", "discount",
                "send", "clone"
            ]
        );
    }
}
