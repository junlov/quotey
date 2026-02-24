use async_trait::async_trait;
use rust_decimal::Decimal;
use std::str::FromStr;
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
            QuoteCommand::Simulate { request } => {
                self.service.simulate_quote(request, &envelope).await
            }
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

    async fn simulate_quote(
        &self,
        request: SimulationRequest,
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

    async fn simulate_quote(
        &self,
        request: SimulationRequest,
        _envelope: &CommandEnvelope,
    ) -> Result<MessageTemplate, CommandRouteError> {
        let quote_id = request.quote_id.unwrap_or_else(|| "unknown".to_owned());
        Ok(blocks::quote_status_message(
            &quote_id,
            &format!(
                "simulate variant={} adjustments={} discount={:?}",
                request.variant_key,
                request.line_adjustments.len(),
                request.requested_discount_pct
            ),
        ))
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
        "simulate" => QuoteCommand::Simulate { request: parse_simulation_request(freeform_args) },
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
            request
                .line_adjustments
                .push(SimulationLineAdjustment { product_id, quantity_delta: delta });
        }
    }

    request.line_adjustments.sort_by(|left, right| left.product_id.cmp(&right.product_id));
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
        build_simulation_promotion_value, handle_simulation_promotion_action,
        normalize_quote_command, parse_quote_command, parse_simulation_promotion_value,
        CommandEnvelope, CommandRouteError, CommandRouter, NoopQuoteCommandService, QuoteCommand,
        QuoteCommandService, SimulationRequest, SlashCommandPayload,
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
        assert!(matches!(
            parse_quote_command("simulate Q-2026-0001 variant=v1 discount=10% plan-pro:+5"),
            QuoteCommand::Simulate { .. }
        ));
        assert!(matches!(parse_quote_command("help"), QuoteCommand::Help));
        assert!(matches!(parse_quote_command("something-else"), QuoteCommand::Unknown { .. }));
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

            async fn simulate_quote(
                &self,
                _request: SimulationRequest,
                _envelope: &CommandEnvelope,
            ) -> Result<MessageTemplate, CommandRouteError> {
                self.calls.lock().expect("lock").push("simulate");
                Ok(crate::blocks::help_message())
            }
        }

        let router = CommandRouter::new(RecordingService::default());
        for (verb, args) in [
            ("new", "for Acme"),
            ("status", "Q-2026-1111"),
            ("list", "mine"),
            ("simulate", "Q-2026-1111 variant=v1 plan-pro:+1"),
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
                .await
                .expect("route");
        }

        let calls = router.service.calls.lock().expect("lock");
        assert_eq!(&*calls, &["new", "status", "list", "simulate"]);
    }
}
