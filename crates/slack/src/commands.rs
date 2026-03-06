use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

use crate::blocks::{self, MessageTemplate};
use quotey_core::cpq::anomaly::{
    AnomalyDetector, AnomalyRuleEvaluationInput, AnomalyRuleKind, AnomalySeverity,
};

const SUPPORTED_QUOTE_VERBS: [&str; 15] = [
    "help",
    "new",
    "status",
    "list",
    "audit",
    "edit",
    "add-line",
    "discount",
    "finalize",
    "send",
    "clone",
    "simulate",
    "suggest",
    "parse-email",
    "parse-rfp",
];
const SUPPORTED_QUOTEY_VERBS: [&str; 7] =
    ["help", "branding", "crm-status", "crm", "crm-mapping", "mapping", "map"];

/// Maximum input length for slash command text (prevents excessive allocation from crafted input).
const MAX_COMMAND_INPUT_LEN: usize = 2048;
const DEFAULT_BRANDING_COMPANY_NAME: &str = "Quotey";
const DEFAULT_BRANDING_PRIMARY_COLOR: &str = "#2563eb";
const DEFAULT_BRANDING_SECONDARY_COLOR: &str = "#1e40af";
const DEFAULT_BRANDING_ACCENT_COLOR: &str = "#3b82f6";

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
    Finalize { request: FinalizeRequest },
    Send { quote_id: Option<String>, freeform_args: String },
    Clone { quote_id: Option<String>, freeform_args: String },
    Simulate { request: SimulationRequest },
    Suggest { quote_id: Option<String>, customer_hint: Option<String>, freeform_args: String },
    ParseEmail { freeform_args: String },
    ParseRfp { freeform_args: String },
    Branding { freeform_args: String },
    CrmStatus { freeform_args: String },
    CrmMapping { freeform_args: String },
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FinalizeRequest {
    pub quote_id: Option<String>,
    pub requested_discount_pct: Option<Decimal>,
    pub customer_avg_discount_pct: Option<Decimal>,
    pub customer_discount_std_dev: Option<Decimal>,
    pub margin_pct: Option<Decimal>,
    pub category_floor_pct: Option<Decimal>,
    pub requested_quantity: Option<Decimal>,
    pub customer_avg_quantity: Option<Decimal>,
    pub quote_total: Option<Decimal>,
    pub similar_deals_avg_total: Option<Decimal>,
    pub override_justification: Option<String>,
    pub raw_args: String,
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
    let command = match payload.command.as_str() {
        "/quote" => "quote",
        "/quotey" => "quotey",
        _ => return Err(CommandParseError::UnsupportedCommand(payload.command)),
    };

    let raw_text = payload.text.trim();
    if raw_text.len() > MAX_COMMAND_INPUT_LEN {
        return Err(CommandParseError::UnsupportedCommand(
            "command text exceeds maximum length".to_string(),
        ));
    }
    let text = raw_text.to_owned();
    let mut parts = text.split_whitespace();
    let verb = if command == "quote" {
        normalize_quote_command_verb(parts.next().unwrap_or("help"))
    } else {
        normalize_quotey_command_verb(parts.next().unwrap_or("help"))
    };
    let freeform_args = parts.collect::<Vec<_>>().join(" ");
    let quote_id = freeform_args.split_whitespace().find_map(parse_quote_id_token);
    let account_hint = extract_account_hint(&verb, &freeform_args);

    Ok(CommandEnvelope {
        command: command.to_owned(),
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
    if trimmed.is_empty() || trimmed.len() > MAX_COMMAND_INPUT_LEN {
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

    if is_suggest_request(&normalized) {
        if quote_id.is_empty() {
            return Some("suggest".to_owned());
        }
        return Some(format!("suggest {quote_id}"));
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

fn is_suggest_request(normalized: &str) -> bool {
    normalized.starts_with("suggest")
        || normalized.starts_with("recommend")
        || normalized.contains("suggestions")
        || normalized.contains("what should i add")
        || normalized.contains("product recommendation")
        || (token_matches(normalized, "similar") && token_matches(normalized, "products"))
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
        let command = if envelope.command.eq_ignore_ascii_case("quotey") {
            classify_quotey_command(&envelope.verb, envelope.freeform_args.clone())
        } else {
            classify_quote_command(&envelope.verb, envelope.freeform_args.clone())
        };
        match command {
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
            QuoteCommand::Finalize { request } => self.service.finalize_quote(request, &envelope),
            QuoteCommand::Send { quote_id, freeform_args } => {
                self.service.send_quote(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::Clone { quote_id, freeform_args } => {
                self.service.clone_quote(quote_id, freeform_args, &envelope)
            }
            QuoteCommand::Simulate { request } => self.service.simulate_quote(request, &envelope),
            QuoteCommand::Suggest { quote_id, customer_hint, freeform_args } => {
                self.service.suggest_products(quote_id, customer_hint, freeform_args, &envelope)
            }
            QuoteCommand::ParseEmail { freeform_args } => {
                self.service.parse_email(freeform_args, &envelope)
            }
            QuoteCommand::ParseRfp { freeform_args } => {
                self.service.parse_rfp(freeform_args, &envelope)
            }
            QuoteCommand::Branding { freeform_args } => {
                self.service.manage_branding(freeform_args, &envelope)
            }
            QuoteCommand::CrmStatus { freeform_args } => {
                self.service.crm_sync_status(freeform_args, &envelope)
            }
            QuoteCommand::CrmMapping { freeform_args } => {
                self.service.crm_field_mapping(freeform_args, &envelope)
            }
            QuoteCommand::Help => Ok(blocks::help_message()),
            QuoteCommand::Unknown { verb, .. } => {
                let is_quotey = envelope.command.eq_ignore_ascii_case("quotey");
                let suggestion = if is_quotey {
                    suggest_supported_quotey_verb(&verb)
                        .map(|candidate| format!(" Did you mean `/quotey {candidate}`?"))
                        .unwrap_or_default()
                } else {
                    suggest_supported_verb(&verb)
                        .map(|candidate| format!(" Did you mean `/quote {candidate}`?"))
                        .unwrap_or_default()
                };
                let supported = if is_quotey {
                    SUPPORTED_QUOTEY_VERBS.join(", ")
                } else {
                    SUPPORTED_QUOTE_VERBS.join(", ")
                };
                let command_name = if is_quotey { "quotey" } else { "quote" };
                let help_command = if is_quotey { "/quotey help" } else { "/quote help" };
                Ok(blocks::error_message(
                    &format!(
            "I couldn't parse `/{command_name} {verb}`.{suggestion} Use `{help_command}` for supported commands: {supported}. \
Tip: use explicit command mode for deterministic parsing."
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

    fn finalize_quote(
        &self,
        request: FinalizeRequest,
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

    fn suggest_products(
        &self,
        quote_id: Option<String>,
        customer_hint: Option<String>,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn parse_email(
        &self,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn parse_rfp(
        &self,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn manage_branding(
        &self,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn crm_sync_status(
        &self,
        freeform_args: String,
        envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError>;

    fn crm_field_mapping(
        &self,
        freeform_args: String,
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
        if let Some(customer_hint) = customer_hint.filter(|value| !value.trim().is_empty()) {
            return Ok(blocks::suggestion_message(&blocks::SuggestionCardView {
                quote_id: None,
                customer_hint,
                suggestions: noop_suggestion_items(),
                request_id: _envelope.request_id.clone(),
            }));
        }
        let summary = "unassigned account".to_owned();
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

    fn finalize_quote(
        &self,
        request: FinalizeRequest,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = request.quote_id.unwrap_or_else(|| "unknown".to_owned());
        if let Some(justification) = request.override_justification {
            return Ok(blocks::quote_status_message(
                &quote_id,
                &format!(
                    "anomaly override captured with justification; finalization can proceed (justification: {justification})"
                ),
            ));
        }

        let input = AnomalyRuleEvaluationInput {
            requested_discount_pct: request
                .requested_discount_pct
                .and_then(|v| v.to_f64())
                .unwrap_or(18.0),
            customer_avg_discount_pct: request
                .customer_avg_discount_pct
                .and_then(|v| v.to_f64())
                .unwrap_or(7.8),
            customer_discount_std_dev: request
                .customer_discount_std_dev
                .and_then(|v| v.to_f64())
                .unwrap_or(2.0),
            margin_pct: request.margin_pct.and_then(|v| v.to_f64()).unwrap_or(52.0),
            category_floor_pct: request.category_floor_pct.and_then(|v| v.to_f64()).unwrap_or(60.0),
            requested_quantity: request
                .requested_quantity
                .and_then(|v| v.to_f64())
                .unwrap_or(150.0),
            customer_avg_quantity: request
                .customer_avg_quantity
                .and_then(|v| v.to_f64())
                .unwrap_or(65.0),
            quote_total: request.quote_total.and_then(|v| v.to_f64()).unwrap_or(21_420.0),
            similar_deals_avg_total: request
                .similar_deals_avg_total
                .and_then(|v| v.to_f64())
                .unwrap_or(16_000.0),
        };

        let hits = AnomalyDetector::default().evaluate_rules(&input);
        if hits.is_empty() {
            return Ok(blocks::preview_mode_message(
                "/quote finalize",
                Some(&quote_id),
                "no pricing anomalies detected; finalization can proceed",
                &_envelope.request_id,
            ));
        }

        let headline = format!(
            "{} pricing anomal{} require{} explicit review before finalization.",
            hits.len(),
            if hits.len() == 1 { "y" } else { "ies" },
            if hits.len() == 1 { "s" } else { "" }
        );
        let items = hits
            .into_iter()
            .map(|hit| blocks::AnomalyWarningItemView {
                rule_label: anomaly_rule_label(hit.rule).to_owned(),
                severity_label: anomaly_severity_label(hit.severity).to_owned(),
                reason: hit.reason,
            })
            .collect();
        Ok(blocks::anomaly_warning_message(&blocks::AnomalyWarningView {
            quote_id,
            headline,
            items,
            request_id: _envelope.request_id.clone(),
        }))
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

    fn suggest_products(
        &self,
        quote_id: Option<String>,
        customer_hint: Option<String>,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let customer_hint = customer_hint
            .or_else(|| extract_account_hint("new", &freeform_args))
            .unwrap_or_else(|| "Current customer".to_owned());
        let suggestions = noop_suggestion_items();
        Ok(blocks::suggestion_message(&blocks::SuggestionCardView {
            quote_id,
            customer_hint,
            suggestions,
            request_id: _envelope.request_id.clone(),
        }))
    }

    fn parse_email(
        &self,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let detail = if freeform_args.trim().is_empty() {
            "email content required after `/quote parse-email`".to_owned()
        } else {
            format!(
                "email parser request captured; extracted from {} characters of source text",
                freeform_args.len()
            )
        };
        Ok(blocks::preview_mode_message(
            "/quote parse-email",
            Some("email parser"),
            &detail,
            &_envelope.request_id,
        ))
    }

    fn parse_rfp(
        &self,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let detail = if freeform_args.trim().is_empty() {
            "RFP content required after `/quote parse-rfp`".to_owned()
        } else {
            format!(
                "RFP parser request captured; extracted from {} characters of source text",
                freeform_args.len()
            )
        };
        Ok(blocks::preview_mode_message(
            "/quote parse-rfp",
            Some("rfp parser"),
            &detail,
            &_envelope.request_id,
        ))
    }

    fn manage_branding(
        &self,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let preview = parse_branding_preview(&freeform_args);
        Ok(blocks::branding_settings_message(&blocks::BrandingSettingsView {
            company_name: preview.company_name,
            current_logo_url: preview.current_logo_url,
            primary_color: preview.primary_color,
            secondary_color: preview.secondary_color,
            accent_color: preview.accent_color,
            pending_updates: preview.pending_updates,
            validation_warnings: preview.validation_warnings,
            status_message: if freeform_args.trim().is_empty() {
                None
            } else {
                Some(
                    "Applied branding updates from command input. Review preview and click Save Branding to stage persistence."
                        .to_owned(),
                )
            },
            request_id: _envelope.request_id.clone(),
        }))
    }

    fn crm_sync_status(
        &self,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let preview = parse_crm_status_preview(&freeform_args);
        Ok(blocks::crm_sync_alert_summary_message(&blocks::CrmSyncAlertSummaryView {
            failed_last_24h: preview.failed_last_24h,
            stale_retrying: preview.stale_retrying,
            near_retry_limit: preview.near_retry_limit,
            request_id: _envelope.request_id.clone(),
        }))
    }

    fn crm_field_mapping(
        &self,
        freeform_args: String,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let preview = parse_crm_field_mapping_preview(&freeform_args);
        Ok(blocks::crm_field_mapping_message(&blocks::CrmFieldMappingSummaryView {
            quotey_to_crm: preview
                .quotey_to_crm
                .iter()
                .map(|(quotey_field, crm_field)| blocks::CrmFieldMappingEntryView {
                    source_field: quotey_field.clone(),
                    target_field: crm_field.clone(),
                })
                .collect(),
            crm_to_quotey: preview
                .crm_to_quotey
                .iter()
                .map(|(crm_field, quotey_field)| blocks::CrmFieldMappingEntryView {
                    source_field: crm_field.clone(),
                    target_field: quotey_field.clone(),
                })
                .collect(),
            request_id: _envelope.request_id.clone(),
        }))
    }
}

fn noop_suggestion_items() -> Vec<blocks::SuggestionItemView> {
    vec![
        blocks::SuggestionItemView {
            product_id: "prod_support_premium".to_owned(),
            product_name: "Premium Support".to_owned(),
            product_sku: "SUPPORT-PREMIUM".to_owned(),
            score: 0.85,
            confidence: "High".to_owned(),
            category_description: "Similar enterprise customers purchased this".to_owned(),
            reasoning: vec![
                "Enterprise deals often include premium support in year one".to_owned(),
                "High seat count increases onboarding/support demand".to_owned(),
            ],
            unit_price: Some(499.0),
        },
        blocks::SuggestionItemView {
            product_id: "prod_sso".to_owned(),
            product_name: "SSO Add-on".to_owned(),
            product_sku: "ADDON-SSO-001".to_owned(),
            score: 0.72,
            confidence: "Medium".to_owned(),
            category_description: "Cross-sell from security and compliance profile".to_owned(),
            reasoning: vec![
                "Most comparable deals include centralized identity".to_owned(),
                "Security review notes prioritize SSO support".to_owned(),
            ],
            unit_price: Some(2.0),
        },
        blocks::SuggestionItemView {
            product_id: "prod_onboarding".to_owned(),
            product_name: "Onboarding Package".to_owned(),
            product_sku: "SERV-ONBOARD-001".to_owned(),
            score: 0.65,
            confidence: "Medium".to_owned(),
            category_description: "High-impact activation accelerator".to_owned(),
            reasoning: vec![
                "Recommended for larger deployment footprints".to_owned(),
                "Reduces time-to-value and implementation risk".to_owned(),
            ],
            unit_price: Some(1500.0),
        },
    ]
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
        "finalize" | "finalise" => {
            QuoteCommand::Finalize { request: parse_finalize_request(freeform_args) }
        }
        "send" => QuoteCommand::Send {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "clone" => QuoteCommand::Clone {
            quote_id: freeform_args.split_whitespace().find_map(parse_quote_id_token),
            freeform_args,
        },
        "simulate" => QuoteCommand::Simulate { request: parse_simulation_request(freeform_args) },
        "suggest" | "recommend" | "suggestions" => {
            let quote_id = freeform_args.split_whitespace().find_map(parse_quote_id_token);
            let customer_hint = extract_account_hint("new", &freeform_args);
            QuoteCommand::Suggest { quote_id, customer_hint, freeform_args }
        }
        "parse-email" | "parse_email" | "parseemail" => QuoteCommand::ParseEmail { freeform_args },
        "parse-rfp" | "parse_rfp" | "parserfp" => QuoteCommand::ParseRfp { freeform_args },
        "what" if freeform_args.to_ascii_lowercase().contains("if") => {
            QuoteCommand::Simulate { request: parse_simulation_request(freeform_args) }
        }
        "help" => QuoteCommand::Help,
        _ => QuoteCommand::Unknown { verb: verb.to_owned(), freeform_args },
    }
}

fn classify_quotey_command(verb: &str, freeform_args: String) -> QuoteCommand {
    match verb {
        "branding" => QuoteCommand::Branding { freeform_args },
        "crm-mapping" | "crm_mapping" | "crmmapping" | "mapping" | "map" => {
            QuoteCommand::CrmMapping { freeform_args }
        }
        "crm" => {
            let normalized = freeform_args.trim().to_ascii_lowercase();
            if normalized.starts_with("mapping")
                || normalized.starts_with("map")
                || normalized.starts_with("field-mapping")
                || normalized.starts_with("field_mapping")
            {
                QuoteCommand::CrmMapping { freeform_args }
            } else {
                QuoteCommand::CrmStatus { freeform_args }
            }
        }
        "crm-status" | "crm_status" | "crmstatus" | "sync-status" | "sync" => {
            QuoteCommand::CrmStatus { freeform_args }
        }
        "help" => QuoteCommand::Help,
        _ => QuoteCommand::Unknown { verb: verb.to_owned(), freeform_args },
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CrmStatusPreview {
    failed_last_24h: u32,
    stale_retrying: u32,
    near_retry_limit: u32,
}

fn parse_crm_status_preview(args: &str) -> CrmStatusPreview {
    let mut preview =
        CrmStatusPreview { failed_last_24h: 0, stale_retrying: 0, near_retry_limit: 0 };

    for token in args.split_whitespace() {
        let Some((raw_key, raw_value)) = token.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value.trim().trim_end_matches(',').trim_end_matches('.');
        let Some(parsed) = value.parse::<u32>().ok() else {
            continue;
        };

        match key.as_str() {
            "failed" | "failed24h" | "failed_last_24h" => preview.failed_last_24h = parsed,
            "stale" | "stale_retrying" | "retrying" => preview.stale_retrying = parsed,
            "near" | "near_limit" | "near_retry_limit" | "retry_budget" => {
                preview.near_retry_limit = parsed
            }
            _ => {}
        }
    }

    preview
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CrmFieldMappingPreview {
    quotey_to_crm: Vec<(String, String)>,
    crm_to_quotey: Vec<(String, String)>,
}

fn parse_crm_mapping_pair(raw: &str) -> Option<(String, String)> {
    let cleaned = raw.trim().trim_end_matches(',').trim_end_matches('.');
    let split = cleaned.split_once("->").or_else(|| cleaned.split_once(':'))?;
    let source = split.0.trim();
    let target = split.1.trim();
    if source.is_empty() || target.is_empty() {
        return None;
    }
    Some((source.to_owned(), target.to_owned()))
}

fn parse_crm_field_mapping_preview(args: &str) -> CrmFieldMappingPreview {
    let mut preview = CrmFieldMappingPreview {
        quotey_to_crm: vec![
            ("quote.total".to_owned(), "Opportunity.Amount".to_owned()),
            ("quote.discount".to_owned(), "Opportunity.Discount".to_owned()),
        ],
        crm_to_quotey: vec![
            ("Account.Industry".to_owned(), "account.industry".to_owned()),
            ("Opportunity.StageName".to_owned(), "quote.stage".to_owned()),
        ],
    };
    let mut has_q2c_override = false;
    let mut has_c2q_override = false;

    for token in args.split_whitespace() {
        let Some((raw_key, raw_value)) = token.split_once('=') else {
            continue;
        };
        let key = raw_key.trim().to_ascii_lowercase();
        let Some((source, target)) = parse_crm_mapping_pair(raw_value) else {
            continue;
        };

        match key.as_str() {
            "q2c" | "quotey_to_crm" | "outbound" => {
                if !has_q2c_override {
                    preview.quotey_to_crm.clear();
                    has_q2c_override = true;
                }
                preview.quotey_to_crm.push((source, target));
            }
            "c2q" | "crm_to_quotey" | "inbound" => {
                if !has_c2q_override {
                    preview.crm_to_quotey.clear();
                    has_c2q_override = true;
                }
                preview.crm_to_quotey.push((source, target));
            }
            _ => {}
        }
    }

    preview
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrandingPreview {
    company_name: String,
    current_logo_url: Option<String>,
    primary_color: String,
    secondary_color: String,
    accent_color: String,
    pending_updates: Vec<String>,
    validation_warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BrandingPreviewState {
    company_name: String,
    current_logo_url: Option<String>,
    primary_color: String,
    secondary_color: String,
    accent_color: String,
    company_updated: bool,
    logo_updated: bool,
    primary_updated: bool,
    secondary_updated: bool,
    accent_updated: bool,
    validation_warnings: Vec<String>,
}

impl Default for BrandingPreviewState {
    fn default() -> Self {
        Self {
            company_name: DEFAULT_BRANDING_COMPANY_NAME.to_owned(),
            current_logo_url: None,
            primary_color: DEFAULT_BRANDING_PRIMARY_COLOR.to_owned(),
            secondary_color: DEFAULT_BRANDING_SECONDARY_COLOR.to_owned(),
            accent_color: DEFAULT_BRANDING_ACCENT_COLOR.to_owned(),
            company_updated: false,
            logo_updated: false,
            primary_updated: false,
            secondary_updated: false,
            accent_updated: false,
            validation_warnings: Vec::new(),
        }
    }
}

impl BrandingPreviewState {
    fn apply_pair(&mut self, raw_key: &str, raw_value: &str) {
        let key = raw_key.trim().to_ascii_lowercase();
        let value = raw_value
            .trim()
            .trim_matches(|ch| ch == '"' || ch == '\'')
            .trim_end_matches(',')
            .trim_end_matches('.')
            .trim();
        if value.is_empty() {
            return;
        }

        match key.as_str() {
            "company" | "company_name" | "name" => {
                self.company_name = value.to_owned();
                self.company_updated = true;
            }
            "logo" | "logo_url" | "current_logo" | "current_logo_url" => {
                let normalized = value.to_ascii_lowercase();
                self.current_logo_url =
                    if matches!(normalized.as_str(), "none" | "clear" | "remove") {
                        None
                    } else {
                        Some(value.to_owned())
                    };
                self.logo_updated = true;
            }
            "primary" | "primary_color" => {
                if let Some(color) = normalize_branding_color(value) {
                    self.primary_color = color;
                    self.primary_updated = true;
                } else {
                    self.validation_warnings.push(format!(
                        "Ignored invalid primary color `{value}` (use #RRGGBB or #RGB).",
                    ));
                }
            }
            "secondary" | "secondary_color" => {
                if let Some(color) = normalize_branding_color(value) {
                    self.secondary_color = color;
                    self.secondary_updated = true;
                } else {
                    self.validation_warnings.push(format!(
                        "Ignored invalid secondary color `{value}` (use #RRGGBB or #RGB).",
                    ));
                }
            }
            "accent" | "accent_color" => {
                if let Some(color) = normalize_branding_color(value) {
                    self.accent_color = color;
                    self.accent_updated = true;
                } else {
                    self.validation_warnings.push(format!(
                        "Ignored invalid accent color `{value}` (use #RRGGBB or #RGB).",
                    ));
                }
            }
            _ => {}
        }
    }

    fn finish(self) -> BrandingPreview {
        let mut pending_updates = Vec::new();
        if self.company_updated {
            pending_updates.push(format!("Company name → {}", self.company_name));
        }
        if self.logo_updated {
            let logo = self.current_logo_url.as_deref().unwrap_or("(cleared)");
            pending_updates.push(format!("Logo URL → {logo}"));
        }
        if self.primary_updated {
            pending_updates.push(format!("Primary color → {}", self.primary_color));
        }
        if self.secondary_updated {
            pending_updates.push(format!("Secondary color → {}", self.secondary_color));
        }
        if self.accent_updated {
            pending_updates.push(format!("Accent color → {}", self.accent_color));
        }

        BrandingPreview {
            company_name: self.company_name,
            current_logo_url: self.current_logo_url,
            primary_color: self.primary_color,
            secondary_color: self.secondary_color,
            accent_color: self.accent_color,
            pending_updates,
            validation_warnings: self.validation_warnings,
        }
    }
}

fn parse_key_value_args(args: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut tokens = args.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        let Some((raw_key, raw_value)) = token.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        if key.is_empty() {
            continue;
        }

        let mut value = raw_value.trim().to_owned();
        if let Some(quote_char) = value.chars().next().filter(|ch| *ch == '"' || *ch == '\'') {
            while !value.ends_with(quote_char) {
                let Some(next) = tokens.next() else {
                    break;
                };
                value.push(' ');
                value.push_str(next);
            }
        }
        pairs.push((key.to_owned(), value));
    }

    pairs
}

fn normalize_branding_color(value: &str) -> Option<String> {
    let raw = value.trim();
    let hex = raw.strip_prefix('#').unwrap_or(raw);

    if hex.len() == 3 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        let mut expanded = String::with_capacity(6);
        for ch in hex.chars() {
            expanded.push(ch);
            expanded.push(ch);
        }
        return Some(format!("#{}", expanded.to_ascii_lowercase()));
    }

    if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Some(format!("#{}", hex.to_ascii_lowercase()));
    }

    None
}

fn parse_branding_preview(args: &str) -> BrandingPreview {
    let mut state = BrandingPreviewState::default();
    for (key, value) in parse_key_value_args(args) {
        state.apply_pair(&key, &value);
    }
    state.finish()
}

fn parse_branding_preview_from_action_pairs(
    pairs: Option<&HashMap<String, String>>,
) -> BrandingPreview {
    let mut state = BrandingPreviewState::default();
    if let Some(pairs) = pairs {
        for (key, value) in pairs {
            state.apply_pair(key, value);
        }
    }
    state.finish()
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
        "quotey.branding.open_modal.v1" => {
            let preview = parse_branding_preview_from_action_pairs(value_pairs.as_ref());
            Ok(blocks::branding_settings_message(&blocks::BrandingSettingsView {
                company_name: preview.company_name,
                current_logo_url: preview.current_logo_url,
                primary_color: preview.primary_color,
                secondary_color: preview.secondary_color,
                accent_color: preview.accent_color,
                pending_updates: preview.pending_updates,
                validation_warnings: preview.validation_warnings,
                status_message: Some(
                    "Modal preview ready: current logo, upload field, color pickers, live preview, and save control are staged."
                        .to_owned(),
                ),
                request_id: request_id.to_owned(),
            }))
        }
        "quotey.branding.save.v1" => {
            let preview = parse_branding_preview_from_action_pairs(value_pairs.as_ref());
            let status_message = if preview.pending_updates.is_empty() {
                "No staged branding updates found. Provide values in `/quotey branding company=... logo=... primary=#... secondary=#... accent=#...` before saving."
                    .to_owned()
            } else {
                format!(
                    "Save request captured for {} branding update(s). Persist these values in your branding config source of truth.",
                    preview.pending_updates.len()
                )
            };
            Ok(blocks::branding_settings_message(&blocks::BrandingSettingsView {
                company_name: preview.company_name,
                current_logo_url: preview.current_logo_url,
                primary_color: preview.primary_color,
                secondary_color: preview.secondary_color,
                accent_color: preview.accent_color,
                pending_updates: preview.pending_updates,
                validation_warnings: preview.validation_warnings,
                status_message: Some(status_message),
                request_id: request_id.to_owned(),
            }))
        }
        "quotey.crm.events.history.v1" => Ok(blocks::preview_mode_message(
            "/quotey crm-status",
            None,
            "CRM sync history requested. Fetch `/api/v1/crm/events` with filters for provider/status/quote.",
            request_id,
        )),
        "quotey.crm.events.stats.v1" => Ok(blocks::preview_mode_message(
            "/quotey crm-status",
            None,
            "CRM aggregate metrics requested. Fetch `/api/v1/crm/events/stats` for status/provider/direction totals.",
            request_id,
        )),
        "quotey.crm.events.retry.v1" => Ok(blocks::preview_mode_message(
            "/quotey crm-status",
            None,
            "CRM replay requested. Use `POST /api/v1/crm/events/{event_id}/retry` for targeted retry or `POST /api/v1/crm/sync/batch` for bounded replay.",
            request_id,
        )),
        "quotey.crm.mapping.edit.v1" => Ok(blocks::preview_mode_message(
            "/quotey crm mapping",
            None,
            "CRM field mapping editor requested. Persist updates via `POST /api/v1/crm/mappings` (upsert) with provider + direction + field pairs.",
            request_id,
        )),
        "quotey.crm.mapping.refresh.v1" => Ok(blocks::preview_mode_message(
            "/quotey crm mapping",
            None,
            "CRM mapping refresh requested. Fetch current mappings from `/api/v1/crm/mappings` and regroup by direction for Slack display.",
            request_id,
        )),
        "quotey.crm.mapping.export.v1" => Ok(blocks::preview_mode_message(
            "/quotey crm mapping",
            None,
            "CRM mapping export requested. Snapshot active mappings from `/api/v1/crm/mappings` for change review and approvals.",
            request_id,
        )),
        "quote.anomaly.override.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            &format!(
                "anomaly override selected. Provide explicit rationale using `/quote finalize {quote_id} override_reason=...` so finalization remains auditable."
            ),
        )),
        "quote.anomaly.adjust.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            &format!(
                "quote adjustment requested. Update pricing inputs, then rerun `/quote finalize {quote_id}` to re-evaluate anomaly checks."
            ),
        )),
        "quote.anomaly.similar.v1" => Ok(blocks::quote_status_message(
            &quote_id,
            "similar-deal context requested. Comparable finalized deal baselines are being prepared for review.",
        )),
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
        value if value.starts_with("suggest.add.") => {
            let product_id = value_pairs
                .as_ref()
                .and_then(|pairs| pairs.get("product"))
                .cloned()
                .unwrap_or_else(|| "unknown".to_owned());
            let sku = value_pairs
                .as_ref()
                .and_then(|pairs| pairs.get("sku"))
                .cloned()
                .unwrap_or_else(|| "unknown".to_owned());
            Ok(blocks::quote_status_message(
                &quote_id,
                &format!(
                    "suggestion accepted: product `{product_id}` (SKU `{sku}`) queued for addition. \
                     Use `/quote add-line {quote_id} {sku}:+1` to confirm the line item."
                ),
            ))
        }
        value if value.starts_with("suggest.details.") => {
            let product_id = value_pairs
                .as_ref()
                .and_then(|pairs| pairs.get("product"))
                .cloned()
                .unwrap_or_else(|| "unknown".to_owned());
            Ok(blocks::quote_status_message(
                &quote_id,
                &format!(
                    "loading product detail for `{product_id}`. Catalog data is being retrieved."
                ),
            ))
        }
        value if value.starts_with("suggest.hide.") => {
            let product_id = value_pairs
                .as_ref()
                .and_then(|pairs| pairs.get("product"))
                .cloned()
                .unwrap_or_else(|| "unknown".to_owned());
            Ok(blocks::quote_status_message(
                &quote_id,
                &format!(
                    "suggestion hidden: `{product_id}` removed from this suggestion pass. \
                     We recorded your feedback for future ranking updates."
                ),
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

/// Extract a suggestion feedback event from a block action, if applicable.
///
/// Returns `Some(event)` when `action_id` matches a `suggest.add.*`,
/// `suggest.details.*`, or `suggest.hide.*` pattern. The caller is responsible for persisting
/// the event via a `SuggestionFeedbackRepository`.
pub fn extract_suggestion_feedback(
    action_id: &str,
    raw_value: Option<&str>,
    request_id: &str,
) -> Option<quotey_core::suggestions::SuggestionFeedbackEvent> {
    use quotey_core::suggestions::SuggestionFeedbackEvent;

    let value_pairs = action_value_pairs(raw_value);
    let correlated_request_id = value_pairs
        .as_ref()
        .and_then(|pairs| pairs.get("request"))
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| request_id.to_owned());

    if action_id.starts_with("suggest.add.") {
        let product_id = value_pairs
            .as_ref()
            .and_then(|pairs| pairs.get("product"))
            .cloned()
            .unwrap_or_default();
        let product_sku =
            value_pairs.as_ref().and_then(|pairs| pairs.get("sku")).cloned().unwrap_or_default();
        let quote_id = value_pairs.as_ref().and_then(|pairs| pairs.get("quote")).cloned();

        if product_id.is_empty() {
            return None;
        }

        return Some(SuggestionFeedbackEvent::Added {
            request_id: correlated_request_id,
            product_id,
            product_sku,
            quote_id,
        });
    }

    if action_id.starts_with("suggest.details.") {
        let product_id = value_pairs
            .as_ref()
            .and_then(|pairs| pairs.get("product"))
            .cloned()
            .unwrap_or_default();

        if product_id.is_empty() {
            return None;
        }

        return Some(SuggestionFeedbackEvent::Clicked {
            request_id: correlated_request_id,
            product_id,
        });
    }

    if action_id.starts_with("suggest.hide.") {
        let product_id = value_pairs
            .as_ref()
            .and_then(|pairs| pairs.get("product"))
            .cloned()
            .unwrap_or_default();

        if product_id.is_empty() {
            return None;
        }

        return Some(SuggestionFeedbackEvent::Hidden {
            request_id: correlated_request_id,
            product_id,
        });
    }

    None
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

        let (raw_key, encoded_value) = match segment.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };

        let decoded = match decode_action_value_component(encoded_value.trim()) {
            Some(value) => value,
            None => continue,
        };

        let key = raw_key.trim();
        if key.is_empty() {
            continue;
        }

        pairs.insert(key.to_ascii_lowercase(), decoded);
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

fn parse_finalize_request(raw_args: String) -> FinalizeRequest {
    let mut request = FinalizeRequest {
        quote_id: None,
        requested_discount_pct: None,
        customer_avg_discount_pct: None,
        customer_discount_std_dev: None,
        margin_pct: None,
        category_floor_pct: None,
        requested_quantity: None,
        customer_avg_quantity: None,
        quote_total: None,
        similar_deals_avg_total: None,
        override_justification: None,
        raw_args: raw_args.clone(),
    };

    for token in raw_args.split_whitespace() {
        if request.quote_id.is_none() {
            request.quote_id = parse_quote_id_token(token);
            if request.quote_id.is_some() {
                continue;
            }
        }

        if let Some((key, value)) = token.split_once('=') {
            let normalized_key = key.trim().to_ascii_lowercase();
            match normalized_key.as_str() {
                "discount" | "requested_discount" => {
                    request.requested_discount_pct = parse_decimal_token(value);
                }
                "avg_discount" | "customer_avg_discount" => {
                    request.customer_avg_discount_pct = parse_decimal_token(value);
                }
                "discount_stddev" | "discount_std_dev" => {
                    request.customer_discount_std_dev = parse_decimal_token(value);
                }
                "margin" => {
                    request.margin_pct = parse_decimal_token(value);
                }
                "margin_floor" | "category_floor" => {
                    request.category_floor_pct = parse_decimal_token(value);
                }
                "quantity" | "requested_quantity" => {
                    request.requested_quantity = parse_decimal_token(value);
                }
                "avg_quantity" | "customer_avg_quantity" => {
                    request.customer_avg_quantity = parse_decimal_token(value);
                }
                "total" | "quote_total" => {
                    request.quote_total = parse_decimal_token(value);
                }
                "similar_total" | "similar_avg_total" => {
                    request.similar_deals_avg_total = parse_decimal_token(value);
                }
                "override_reason" | "reason" => {
                    let reason = value.trim().trim_matches('"');
                    if !reason.is_empty() {
                        request.override_justification = Some(reason.to_owned());
                    }
                }
                _ => {}
            }
        }
    }

    request
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

fn anomaly_rule_label(rule: AnomalyRuleKind) -> &'static str {
    match rule {
        AnomalyRuleKind::Discount => "Discount",
        AnomalyRuleKind::Margin => "Margin",
        AnomalyRuleKind::Quantity => "Quantity",
        AnomalyRuleKind::Price => "Price",
    }
}

fn anomaly_severity_label(severity: AnomalySeverity) -> &'static str {
    match severity {
        AnomalySeverity::None => "none",
        AnomalySeverity::Info => "info",
        AnomalySeverity::Warning => "warning",
        AnomalySeverity::Critical => "critical",
    }
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

fn normalize_quotey_command_verb(raw: &str) -> String {
    raw.trim()
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '-')
        .to_ascii_lowercase()
}

fn suggest_supported_quotey_verb(input: &str) -> Option<&'static str> {
    let input = input.trim().to_ascii_lowercase();
    let mut best: Option<(usize, &'static str)> = None;
    for &candidate in &SUPPORTED_QUOTEY_VERBS {
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

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;
    use std::sync::Mutex;

    use super::{
        action_quote_id, action_value_pairs, build_simulation_promotion_value, handle_block_action,
        handle_simulation_promotion_action, infer_thread_quote_command, normalize_quote_command,
        parse_quote_command, parse_quote_id_token, parse_simulation_promotion_value,
        suggest_supported_verb, CommandEnvelope, CommandRouteError, CommandRouter, FinalizeRequest,
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
        assert!(matches!(
            parse_quote_command("finalize Q-2026-0101 discount=18 margin=52"),
            QuoteCommand::Finalize { .. }
        ));
        assert!(matches!(parse_quote_command("send Q-2026-0101 now"), QuoteCommand::Send { .. }));
        assert!(matches!(parse_quote_command("clone Q-2026-0101"), QuoteCommand::Clone { .. }));
        assert!(matches!(parse_quote_command("help?"), QuoteCommand::Help));
        assert!(matches!(
            parse_quote_command("simulate Q-2026-0001 variant=v1 discount=10% plan-pro:+5"),
            QuoteCommand::Simulate { .. }
        ));
        assert!(matches!(
            parse_quote_command("parse-email Need 150 seats and premium support"),
            QuoteCommand::ParseEmail { .. }
        ));
        assert!(matches!(
            parse_quote_command("parse-rfp Security requirements table attached"),
            QuoteCommand::ParseRfp { .. }
        ));
        assert!(matches!(parse_quote_command("help"), QuoteCommand::Help));
        assert!(matches!(parse_quote_command("something-else"), QuoteCommand::Unknown { .. }));
    }

    #[test]
    fn suggest_supported_verb_offers_typo_hints_for_unknown_commands() {
        assert_eq!(suggest_supported_verb("statuz"), Some("status"));
        assert_eq!(suggest_supported_verb("edti"), Some("edit"));
        assert_eq!(suggest_supported_verb("sendd"), Some("send"));
        assert_eq!(suggest_supported_verb("finalzie"), Some("finalize"));
        assert_eq!(suggest_supported_verb("hlep"), Some("help"));
        assert_eq!(suggest_supported_verb("parse-emial"), Some("parse-email"));
        assert_eq!(suggest_supported_verb("parse-rpf"), Some("parse-rfp"));
        assert_eq!(suggest_supported_verb("xyz"), None);
    }

    #[test]
    fn normalize_quote_command_accepts_quotey_branding() {
        let envelope = normalize_quote_command(SlashCommandPayload {
            command: "/quotey".to_owned(),
            text: "branding".to_owned(),
            channel_id: "C123".to_owned(),
            user_id: "U123".to_owned(),
            trigger_ts: "1700000000.1".to_owned(),
            request_id: "req-quotey-branding".to_owned(),
        })
        .expect("normalized");

        assert_eq!(envelope.command, "quotey");
        assert_eq!(envelope.verb, "branding");
    }

    #[test]
    fn normalize_quote_command_accepts_quotey_crm_status() {
        let envelope = normalize_quote_command(SlashCommandPayload {
            command: "/quotey".to_owned(),
            text: "crm-status failed=4 stale=2 near=1".to_owned(),
            channel_id: "C123".to_owned(),
            user_id: "U123".to_owned(),
            trigger_ts: "1700000000.1".to_owned(),
            request_id: "req-quotey-crm-status".to_owned(),
        })
        .expect("normalized");

        assert_eq!(envelope.command, "quotey");
        assert_eq!(envelope.verb, "crm-status");
    }

    #[test]
    fn normalize_quote_command_accepts_quotey_crm_mapping() {
        let envelope = normalize_quote_command(SlashCommandPayload {
            command: "/quotey".to_owned(),
            text: "crm mapping q2c=quote.total:Opportunity.Amount".to_owned(),
            channel_id: "C123".to_owned(),
            user_id: "U123".to_owned(),
            trigger_ts: "1700000000.1".to_owned(),
            request_id: "req-quotey-crm-mapping".to_owned(),
        })
        .expect("normalized");

        assert_eq!(envelope.command, "quotey");
        assert_eq!(envelope.verb, "crm");
        assert_eq!(envelope.freeform_args, "mapping q2c=quote.total:Opportunity.Amount");
    }

    #[test]
    fn parse_crm_status_preview_extracts_supported_counter_aliases() {
        let preview = super::parse_crm_status_preview(
            "failed_last_24h=7 retrying=3 retry_budget=2 unknown=99 malformed=abc",
        );
        assert_eq!(preview.failed_last_24h, 7);
        assert_eq!(preview.stale_retrying, 3);
        assert_eq!(preview.near_retry_limit, 2);
    }

    #[test]
    fn parse_crm_field_mapping_preview_extracts_directional_pairs() {
        let preview = super::parse_crm_field_mapping_preview(
            "q2c=quote.margin:Opportunity.Margin__c c2q=Account.Industry:account.industry",
        );
        assert_eq!(preview.quotey_to_crm.len(), 1);
        assert_eq!(
            preview.quotey_to_crm[0],
            ("quote.margin".to_owned(), "Opportunity.Margin__c".to_owned())
        );
        assert_eq!(preview.crm_to_quotey.len(), 1);
        assert_eq!(
            preview.crm_to_quotey[0],
            ("Account.Industry".to_owned(), "account.industry".to_owned())
        );
    }

    #[test]
    fn parse_branding_preview_extracts_logo_and_color_updates() {
        let preview = super::parse_branding_preview(
            "company=\"Acme CPQ\" logo=https://example.com/logo.svg primary=#123abc secondary=456 accent=DEF",
        );

        assert_eq!(preview.company_name, "Acme CPQ");
        assert_eq!(preview.current_logo_url.as_deref(), Some("https://example.com/logo.svg"));
        assert_eq!(preview.primary_color, "#123abc");
        assert_eq!(preview.secondary_color, "#445566");
        assert_eq!(preview.accent_color, "#ddeeff");
        assert_eq!(preview.pending_updates.len(), 5);
        assert!(preview.pending_updates.iter().any(|line| line.contains("Company name")));
        assert!(preview.pending_updates.iter().any(|line| line.contains("Logo URL")));
        assert!(preview.validation_warnings.is_empty());
    }

    #[test]
    fn parse_branding_preview_tracks_invalid_color_inputs() {
        let preview = super::parse_branding_preview("logo=clear primary=12z accent=#1234");

        assert_eq!(preview.current_logo_url, None);
        assert_eq!(preview.primary_color, super::DEFAULT_BRANDING_PRIMARY_COLOR);
        assert_eq!(preview.accent_color, super::DEFAULT_BRANDING_ACCENT_COLOR);
        assert_eq!(preview.pending_updates, vec!["Logo URL → (cleared)".to_owned()]);
        assert_eq!(preview.validation_warnings.len(), 2);
        assert!(preview.validation_warnings.iter().any(|line| line.contains("primary")));
        assert!(preview.validation_warnings.iter().any(|line| line.contains("accent")));
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
    fn block_action_handles_anomaly_controls() {
        let message = handle_block_action(
            "quote.anomaly.override.v1",
            Some("quote=Q-2026-7003"),
            Some("Q-2026-7003"),
            "req-anomaly-1",
        )
        .expect("anomaly override handled");
        assert!(message.fallback_text.contains("override_reason"));

        let message = handle_block_action(
            "quote.anomaly.adjust.v1",
            Some("quote=Q-2026-7003"),
            Some("Q-2026-7003"),
            "req-anomaly-2",
        )
        .expect("anomaly adjust handled");
        assert!(message.fallback_text.contains("rerun `/quote finalize"));
    }

    #[test]
    fn block_action_handles_hide_suggestion() {
        let message = handle_block_action(
            "suggest.hide.0.v1",
            Some("request=req-1;quote=Q-2026-7003;product=prod_sso"),
            Some("Q-2026-7003"),
            "req-hide",
        )
        .expect("hide action handled");
        assert!(message.fallback_text.contains("suggestion hidden"));
        assert!(message.fallback_text.contains("prod_sso"));
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
    fn block_action_handles_branding_controls() {
        let message = handle_block_action(
            "quotey.branding.open_modal.v1",
            Some(
                "company=Acme%20Corp;logo=https%3A%2F%2Fexample.com%2Flogo.png;primary=%23012345;secondary=%23654321;accent=%23ff6600",
            ),
            None,
            "req-quotey-branding-open",
        )
        .expect("branding open handled");
        assert!(message.fallback_text.contains("Branding configuration"));
        assert!(message.blocks.iter().any(|block| {
            matches!(
                block,
                crate::blocks::Block::Section {
                    text: crate::blocks::TextObject::Mrkdwn { text },
                    ..
                } if text.contains("Modal preview ready")
            )
        }));
        assert!(message.blocks.iter().any(|block| {
            matches!(
                block,
                crate::blocks::Block::Section {
                    text: crate::blocks::TextObject::Mrkdwn { text },
                    ..
                } if text.contains("Acme Corp")
            )
        }));

        let message = handle_block_action(
            "quotey.branding.save.v1",
            Some("company=Acme%20Corp;logo=clear;primary=%23012345;secondary=%23654321"),
            None,
            "req-quotey-branding-save",
        )
        .expect("branding save handled");
        assert!(message.fallback_text.contains("Branding configuration"));
        assert!(message.blocks.iter().any(|block| {
            matches!(
                block,
                crate::blocks::Block::Section {
                    text: crate::blocks::TextObject::Mrkdwn { text },
                    ..
                } if text.contains("Save request captured")
            )
        }));
    }

    #[test]
    fn block_action_handles_crm_status_controls() {
        let history = handle_block_action(
            "quotey.crm.events.history.v1",
            None,
            None,
            "req-quotey-crm-history",
        )
        .expect("crm history handled");
        assert!(history.blocks.iter().any(|block| {
            matches!(
                block,
                crate::blocks::Block::Section {
                    text: crate::blocks::TextObject::Mrkdwn { text },
                    ..
                } if text.contains("/api/v1/crm/events")
            )
        }));

        let stats =
            handle_block_action("quotey.crm.events.stats.v1", None, None, "req-quotey-crm-stats")
                .expect("crm stats handled");
        assert!(stats.blocks.iter().any(|block| {
            matches!(
                block,
                crate::blocks::Block::Section {
                    text: crate::blocks::TextObject::Mrkdwn { text },
                    ..
                } if text.contains("/api/v1/crm/events/stats")
            )
        }));

        let retry =
            handle_block_action("quotey.crm.events.retry.v1", None, None, "req-quotey-crm-retry")
                .expect("crm retry handled");
        assert!(retry.blocks.iter().any(|block| {
            matches!(
                block,
                crate::blocks::Block::Section {
                    text: crate::blocks::TextObject::Mrkdwn { text },
                    ..
                } if text.contains("/api/v1/crm/events/{event_id}/retry")
            )
        }));
    }

    #[test]
    fn block_action_handles_crm_mapping_controls() {
        let edit = handle_block_action(
            "quotey.crm.mapping.edit.v1",
            None,
            None,
            "req-quotey-crm-map-edit",
        )
        .expect("crm mapping edit handled");
        assert!(edit.blocks.iter().any(|block| {
            matches!(
                block,
                crate::blocks::Block::Section {
                    text: crate::blocks::TextObject::Mrkdwn { text },
                    ..
                } if text.contains("/api/v1/crm/mappings")
            )
        }));

        let refresh = handle_block_action(
            "quotey.crm.mapping.refresh.v1",
            None,
            None,
            "req-quotey-crm-map-refresh",
        )
        .expect("crm mapping refresh handled");
        assert!(refresh.fallback_text.contains("Preview mode active"));

        let export = handle_block_action(
            "quotey.crm.mapping.export.v1",
            None,
            None,
            "req-quotey-crm-map-export",
        )
        .expect("crm mapping export handled");
        assert!(export.fallback_text.contains("Preview mode active"));
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
    fn action_value_parsing_tolerates_invalid_segments() {
        let encoded = "malformed;quote=Q-2026-7777;task=task%3Aretry%3D1;bad=%zz";
        let pairs = action_value_pairs(Some(encoded)).expect("pairs parsed");
        assert_eq!(pairs.get("quote").expect("quote"), "Q-2026-7777");
        assert_eq!(pairs.get("task").expect("task"), "task:retry=1");
        assert!(!pairs.contains_key("malformed"));
        assert!(!pairs.contains_key("bad"));
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

            fn finalize_quote(
                &self,
                _request: FinalizeRequest,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("finalize");
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

            fn suggest_products(
                &self,
                _quote_id: Option<String>,
                _customer_hint: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("suggest");
                Ok(crate::blocks::help_message())
            }

            fn parse_email(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("parse-email");
                Ok(crate::blocks::help_message())
            }

            fn parse_rfp(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("parse-rfp");
                Ok(crate::blocks::help_message())
            }

            fn manage_branding(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("branding");
                Ok(crate::blocks::help_message())
            }

            fn crm_sync_status(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("crm-status");
                Ok(crate::blocks::help_message())
            }

            fn crm_field_mapping(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("crm-mapping");
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
            ("finalize", "Q-2026-1111 discount=12 margin=58"),
            ("send", "Q-2026-1111"),
            ("clone", "Q-2026-1111"),
            ("suggest", "Q-2026-1111"),
            ("parse-email", "Need 100 seats with annual billing"),
            ("parse-rfp", "Section 3: security controls and SLA requirements"),
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
                "new",
                "status",
                "list",
                "simulate",
                "audit",
                "edit",
                "add-line",
                "discount",
                "finalize",
                "send",
                "clone",
                "suggest",
                "parse-email",
                "parse-rfp"
            ]
        );
    }

    #[test]
    fn router_routes_quotey_branding_command() {
        #[derive(Default)]
        struct BrandingRecordingService {
            calls: Mutex<Vec<&'static str>>,
        }

        impl QuoteCommandService for BrandingRecordingService {
            fn new_quote(
                &self,
                _customer_hint: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn status_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn list_quotes(
                &self,
                _filter: Option<String>,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn audit_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn edit_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn add_line(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn request_discount(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn finalize_quote(
                &self,
                _request: FinalizeRequest,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn send_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn clone_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn simulate_quote(
                &self,
                _request: SimulationRequest,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn suggest_products(
                &self,
                _quote_id: Option<String>,
                _customer_hint: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn parse_email(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn parse_rfp(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn manage_branding(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("branding");
                Ok(crate::blocks::help_message())
            }

            fn crm_sync_status(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("crm-status");
                Ok(crate::blocks::help_message())
            }

            fn crm_field_mapping(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("crm-mapping");
                Ok(crate::blocks::help_message())
            }
        }

        let router = CommandRouter::new(BrandingRecordingService::default());
        router
            .route(CommandEnvelope {
                command: "quotey".to_owned(),
                verb: "branding".to_owned(),
                quote_id: None,
                account_hint: None,
                freeform_args: String::new(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-branding-route".to_owned(),
            })
            .expect("route");
        assert_eq!(*router.service.calls.lock().expect("lock"), vec!["branding"]);
    }

    #[test]
    fn router_routes_quotey_crm_status_command() {
        #[derive(Default)]
        struct CrmRecordingService {
            calls: Mutex<Vec<&'static str>>,
        }

        impl QuoteCommandService for CrmRecordingService {
            fn new_quote(
                &self,
                _customer_hint: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn status_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn list_quotes(
                &self,
                _filter: Option<String>,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn audit_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn edit_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn add_line(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn request_discount(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn finalize_quote(
                &self,
                _request: FinalizeRequest,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn send_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn clone_quote(
                &self,
                _quote_id: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn simulate_quote(
                &self,
                _request: SimulationRequest,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn suggest_products(
                &self,
                _quote_id: Option<String>,
                _customer_hint: Option<String>,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn parse_email(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn parse_rfp(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn manage_branding(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                Ok(crate::blocks::help_message())
            }
            fn crm_sync_status(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("crm-status");
                Ok(crate::blocks::help_message())
            }

            fn crm_field_mapping(
                &self,
                _freeform_args: String,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("crm-mapping");
                Ok(crate::blocks::help_message())
            }
        }

        let router = CommandRouter::new(CrmRecordingService::default());
        router
            .route(CommandEnvelope {
                command: "quotey".to_owned(),
                verb: "crm-status".to_owned(),
                quote_id: None,
                account_hint: None,
                freeform_args: "failed=3 stale=1 near=0".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-crm-status-route".to_owned(),
            })
            .expect("route");

        router
            .route(CommandEnvelope {
                command: "quotey".to_owned(),
                verb: "crm".to_owned(),
                quote_id: None,
                account_hint: None,
                freeform_args: "mapping q2c=quote.total:Opportunity.Amount".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-crm-mapping-route".to_owned(),
            })
            .expect("route mapping");

        assert_eq!(*router.service.calls.lock().expect("lock"), vec!["crm-status", "crm-mapping"]);
    }

    #[test]
    fn noop_service_finalize_renders_anomaly_warning_ui() {
        let service = NoopQuoteCommandService;
        let envelope = CommandEnvelope {
            command: "quote".to_owned(),
            verb: "finalize".to_owned(),
            quote_id: Some("Q-2026-8888".to_owned()),
            account_hint: None,
            freeform_args: "Q-2026-8888 discount=18 margin=52".to_owned(),
            channel_id: "C1".to_owned(),
            user_id: "U1".to_owned(),
            trigger_ts: "1".to_owned(),
            request_id: "req-finalize".to_owned(),
        };

        let message = service
            .finalize_quote(
                FinalizeRequest {
                    quote_id: Some("Q-2026-8888".to_owned()),
                    requested_discount_pct: Some(Decimal::new(18, 0)),
                    customer_avg_discount_pct: Some(Decimal::new(78, 1)),
                    customer_discount_std_dev: Some(Decimal::new(2, 0)),
                    margin_pct: Some(Decimal::new(52, 0)),
                    category_floor_pct: Some(Decimal::new(60, 0)),
                    requested_quantity: Some(Decimal::new(150, 0)),
                    customer_avg_quantity: Some(Decimal::new(65, 0)),
                    quote_total: Some(Decimal::new(21420, 0)),
                    similar_deals_avg_total: Some(Decimal::new(16000, 0)),
                    override_justification: None,
                    raw_args: "Q-2026-8888 discount=18 margin=52".to_owned(),
                },
                &envelope,
            )
            .expect("finalize should return warning ui");

        assert!(message.fallback_text.contains("flagged"));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            crate::blocks::Block::Actions { block_id, .. } if block_id == "quote.anomaly.actions.v1"
        )));
    }

    #[test]
    fn extract_suggestion_feedback_add_action() {
        use quotey_core::suggestions::SuggestionFeedbackEvent;

        let value = "action=add_suggested;quote=Q-2026-001;product=prod_sso;sku=ADDON-SSO-001";
        let event = super::extract_suggestion_feedback("suggest.add.0.v1", Some(value), "req-42")
            .expect("should extract add event");

        assert_eq!(
            event,
            SuggestionFeedbackEvent::Added {
                request_id: "req-42".to_owned(),
                product_id: "prod_sso".to_owned(),
                product_sku: "ADDON-SSO-001".to_owned(),
                quote_id: Some("Q-2026-001".to_owned()),
            }
        );
    }

    #[test]
    fn extract_suggestion_feedback_details_action() {
        use quotey_core::suggestions::SuggestionFeedbackEvent;

        let value = "action=view_product;product=prod_sso";
        let event =
            super::extract_suggestion_feedback("suggest.details.0.v1", Some(value), "req-43")
                .expect("should extract clicked event");

        assert_eq!(
            event,
            SuggestionFeedbackEvent::Clicked {
                request_id: "req-43".to_owned(),
                product_id: "prod_sso".to_owned(),
            }
        );
    }

    #[test]
    fn extract_suggestion_feedback_irrelevant_action_returns_none() {
        let result = super::extract_suggestion_feedback(
            "quote.refresh.v1",
            Some("quote=Q-2026-001"),
            "req-44",
        );
        assert!(result.is_none());
    }

    #[test]
    fn extract_suggestion_feedback_hide_action() {
        use quotey_core::suggestions::SuggestionFeedbackEvent;

        let value = "action=hide_suggestion;request=req-origin;product=prod_sso";
        let event =
            super::extract_suggestion_feedback("suggest.hide.0.v1", Some(value), "req-click")
                .expect("should extract hide event");

        assert_eq!(
            event,
            SuggestionFeedbackEvent::Hidden {
                request_id: "req-origin".to_owned(),
                product_id: "prod_sso".to_owned(),
            }
        );
    }

    #[test]
    fn extract_suggestion_feedback_missing_product_returns_none() {
        let result = super::extract_suggestion_feedback(
            "suggest.add.0.v1",
            Some("action=add_suggested"),
            "req-45",
        );
        assert!(result.is_none());
    }

    #[test]
    fn extract_suggestion_feedback_uses_request_from_action_payload() {
        use quotey_core::suggestions::SuggestionFeedbackEvent;

        let value = "action=add_suggested;request=req-origin;quote=Q-2026-001;product=prod_sso;sku=ADDON-SSO-001";
        let event =
            super::extract_suggestion_feedback("suggest.add.0.v1", Some(value), "req-click")
                .expect("should extract add event");

        assert_eq!(
            event,
            SuggestionFeedbackEvent::Added {
                request_id: "req-origin".to_owned(),
                product_id: "prod_sso".to_owned(),
                product_sku: "ADDON-SSO-001".to_owned(),
                quote_id: Some("Q-2026-001".to_owned()),
            }
        );
    }

    #[test]
    fn noop_service_suggest_products_renders_suggestion_ui() {
        let service = NoopQuoteCommandService;
        let envelope = CommandEnvelope {
            command: "quote".to_owned(),
            verb: "suggest".to_owned(),
            quote_id: Some("Q-2026-1234".to_owned()),
            account_hint: Some("Acme Corp".to_owned()),
            freeform_args: "for Acme Corp".to_owned(),
            channel_id: "C1".to_owned(),
            user_id: "U1".to_owned(),
            trigger_ts: "1".to_owned(),
            request_id: "req-suggest-ui".to_owned(),
        };

        let message = service
            .suggest_products(
                Some("Q-2026-1234".to_owned()),
                Some("Acme Corp".to_owned()),
                "for Acme Corp".to_owned(),
                &envelope,
            )
            .expect("suggestions should render");

        assert!(message.fallback_text.contains("product suggestion"));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            crate::blocks::Block::Section { block_id, .. } if block_id == "suggest.header.v1"
        )));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            crate::blocks::Block::Actions { block_id, .. }
                if block_id == "suggest.item.actions.0.v1"
        )));
    }

    #[test]
    fn noop_service_new_quote_with_customer_renders_suggestions() {
        let service = NoopQuoteCommandService;
        let envelope = CommandEnvelope {
            command: "quote".to_owned(),
            verb: "new".to_owned(),
            quote_id: None,
            account_hint: Some("Acme Corp".to_owned()),
            freeform_args: "for Acme Corp".to_owned(),
            channel_id: "C1".to_owned(),
            user_id: "U1".to_owned(),
            trigger_ts: "1".to_owned(),
            request_id: "req-new-suggest".to_owned(),
        };

        let message = service
            .new_quote(Some("Acme Corp".to_owned()), "for Acme Corp".to_owned(), &envelope)
            .expect("new quote should render suggestion card");

        assert!(message.fallback_text.contains("suggestion"));
        assert!(message.blocks.iter().any(|block| matches!(
            block,
            crate::blocks::Block::Section { block_id, .. } if block_id == "suggest.header.v1"
        )));
    }
}
