use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;

use crate::{
    blocks::MessageTemplate,
    commands::{
        normalize_quote_command, CommandParseError, CommandRouteError, CommandRouter,
        NoopQuoteCommandService, QuoteCommandService, SlashCommandPayload,
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackEnvelope {
    pub envelope_id: String,
    pub event: SlackEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackEvent {
    SlashCommand(SlashCommandPayload),
    ThreadMessage(ThreadMessageEvent),
    ReactionAdded(ReactionAddedEvent),
    Unsupported { event_type: String },
}

impl SlackEvent {
    pub fn event_type(&self) -> SlackEventType {
        match self {
            Self::SlashCommand(_) => SlackEventType::SlashCommand,
            Self::ThreadMessage(_) => SlackEventType::ThreadMessage,
            Self::ReactionAdded(_) => SlackEventType::ReactionAdded,
            Self::Unsupported { .. } => SlackEventType::Unsupported,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SlackEventType {
    SlashCommand,
    ThreadMessage,
    ReactionAdded,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadMessageEvent {
    pub channel_id: String,
    pub thread_ts: String,
    pub user_id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReactionAddedEvent {
    pub channel_id: String,
    pub message_ts: String,
    pub thread_ts: Option<String>,
    pub reactor_user_id: String,
    pub reaction: String,
    pub quote_id: Option<String>,
    pub approval_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventContext {
    pub correlation_id: String,
}

impl Default for EventContext {
    fn default() -> Self {
        Self { correlation_id: "unknown-correlation-id".to_owned() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandlerResult {
    Responded(MessageTemplate),
    Processed,
    Ignored,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EventHandlerError {
    #[error(transparent)]
    Parse(#[from] CommandParseError),
    #[error(transparent)]
    Route(#[from] CommandRouteError),
    #[error("thread message handler failure: {0}")]
    ThreadMessage(String),
    #[error("reaction approval handler failure: {0}")]
    ReactionApproval(String),
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DispatchError {
    #[error(transparent)]
    Handler(#[from] EventHandlerError),
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    fn event_type(&self) -> SlackEventType;
    async fn handle(
        &self,
        envelope: &SlackEnvelope,
        ctx: &EventContext,
    ) -> Result<HandlerResult, EventHandlerError>;
}

#[derive(Default)]
pub struct EventDispatcher {
    handlers: HashMap<SlackEventType, Arc<dyn EventHandler>>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<H>(&mut self, handler: H)
    where
        H: EventHandler + 'static,
    {
        self.handlers.insert(handler.event_type(), Arc::new(handler));
    }

    pub async fn dispatch(
        &self,
        envelope: &SlackEnvelope,
        ctx: &EventContext,
    ) -> Result<HandlerResult, DispatchError> {
        let Some(handler) = self.handlers.get(&envelope.event.event_type()) else {
            return Ok(HandlerResult::Ignored);
        };

        handler.handle(envelope, ctx).await.map_err(DispatchError::from)
    }

    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

pub fn default_dispatcher() -> EventDispatcher {
    let mut dispatcher = EventDispatcher::new();
    dispatcher.register(SlashCommandHandler::new(NoopQuoteCommandService));
    dispatcher.register(ThreadMessageHandler::new(NoopThreadMessageService));
    dispatcher.register(ReactionAddedHandler::new(NoopReactionApprovalService));
    dispatcher
}

pub struct SlashCommandHandler<S> {
    router: CommandRouter<S>,
}

impl<S> SlashCommandHandler<S>
where
    S: QuoteCommandService,
{
    pub fn new(service: S) -> Self {
        Self { router: CommandRouter::new(service) }
    }
}

#[async_trait]
impl<S> EventHandler for SlashCommandHandler<S>
where
    S: QuoteCommandService + 'static,
{
    fn event_type(&self) -> SlackEventType {
        SlackEventType::SlashCommand
    }

    async fn handle(
        &self,
        envelope: &SlackEnvelope,
        _ctx: &EventContext,
    ) -> Result<HandlerResult, EventHandlerError> {
        let SlackEvent::SlashCommand(payload) = &envelope.event else {
            return Ok(HandlerResult::Ignored);
        };

        let normalized = normalize_quote_command(payload.clone())?;
        let message = self.router.route(normalized)?;
        Ok(HandlerResult::Responded(message))
    }
}

#[async_trait]
pub trait ThreadMessageService: Send + Sync {
    async fn handle_thread_message(
        &self,
        event: &ThreadMessageEvent,
        ctx: &EventContext,
    ) -> Result<MessageTemplate, EventHandlerError>;
}

pub struct ThreadMessageHandler<S> {
    service: S,
}

impl<S> ThreadMessageHandler<S>
where
    S: ThreadMessageService,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }
}

#[async_trait]
impl<S> EventHandler for ThreadMessageHandler<S>
where
    S: ThreadMessageService + 'static,
{
    fn event_type(&self) -> SlackEventType {
        SlackEventType::ThreadMessage
    }

    async fn handle(
        &self,
        envelope: &SlackEnvelope,
        ctx: &EventContext,
    ) -> Result<HandlerResult, EventHandlerError> {
        let SlackEvent::ThreadMessage(event) = &envelope.event else {
            return Ok(HandlerResult::Ignored);
        };

        let message = self.service.handle_thread_message(event, ctx).await?;
        Ok(HandlerResult::Responded(message))
    }
}

#[derive(Default)]
pub struct NoopThreadMessageService;

#[async_trait]
impl ThreadMessageService for NoopThreadMessageService {
    async fn handle_thread_message(
        &self,
        event: &ThreadMessageEvent,
        _ctx: &EventContext,
    ) -> Result<MessageTemplate, EventHandlerError> {
        Ok(crate::blocks::quote_status_message(
            "thread",
            &format!("received from {}: {}", event.user_id, event.text),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReactionApprovalOutcome {
    pub quote_id: String,
    pub database_recorded: bool,
    pub state_transition_triggered: bool,
    pub confirmation_dm_sent: bool,
    pub undo_window_secs: u32,
}

#[async_trait]
pub trait ReactionApprovalService: Send + Sync {
    async fn process_reaction_approval(
        &self,
        event: &ReactionAddedEvent,
        ctx: &EventContext,
    ) -> Result<ReactionApprovalOutcome, EventHandlerError>;
}

pub struct ReactionAddedHandler<S> {
    service: S,
}

impl<S> ReactionAddedHandler<S>
where
    S: ReactionApprovalService,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }
}

#[async_trait]
impl<S> EventHandler for ReactionAddedHandler<S>
where
    S: ReactionApprovalService + 'static,
{
    fn event_type(&self) -> SlackEventType {
        SlackEventType::ReactionAdded
    }

    async fn handle(
        &self,
        envelope: &SlackEnvelope,
        ctx: &EventContext,
    ) -> Result<HandlerResult, EventHandlerError> {
        let SlackEvent::ReactionAdded(event) = &envelope.event else {
            return Ok(HandlerResult::Ignored);
        };

        if !is_supported_approval_reaction(&event.reaction) || event.quote_id.is_none() {
            return Ok(HandlerResult::Processed);
        }

        let outcome = self.service.process_reaction_approval(event, ctx).await?;
        let summary = format!(
            "emoji approval captured ({}) | db={} state_transition={} dm={} undo={}s",
            event.reaction,
            outcome.database_recorded,
            outcome.state_transition_triggered,
            outcome.confirmation_dm_sent,
            outcome.undo_window_secs,
        );

        Ok(HandlerResult::Responded(crate::blocks::quote_status_message(
            &outcome.quote_id,
            &summary,
        )))
    }
}

#[derive(Default)]
pub struct NoopReactionApprovalService;

#[async_trait]
impl ReactionApprovalService for NoopReactionApprovalService {
    async fn process_reaction_approval(
        &self,
        event: &ReactionAddedEvent,
        _ctx: &EventContext,
    ) -> Result<ReactionApprovalOutcome, EventHandlerError> {
        let quote_id = event.quote_id.clone().ok_or_else(|| {
            EventHandlerError::ReactionApproval("missing quote id for reaction approval".to_owned())
        })?;

        Ok(ReactionApprovalOutcome {
            quote_id,
            database_recorded: true,
            state_transition_triggered: true,
            confirmation_dm_sent: true,
            undo_window_secs: 300,
        })
    }
}

fn is_supported_approval_reaction(reaction: &str) -> bool {
    let normalized = normalize_reaction_token(reaction);

    matches!(
        normalized.as_str(),
        "👍" | "👎" | "💬" | "+1" | "-1" | "thumbsup" | "thumbsdown" | "speech_balloon"
    )
}

fn normalize_reaction_token(reaction: &str) -> String {
    reaction.trim().trim_matches(':').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{
        default_dispatcher, EventContext, EventDispatcher, HandlerResult, ReactionAddedEvent,
        SlackEnvelope, SlackEvent, ThreadMessageEvent,
    };
    use crate::commands::SlashCommandPayload;

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[tokio::test]
    async fn dispatcher_routes_slash_commands() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-1".to_owned(),
            event: SlackEvent::SlashCommand(SlashCommandPayload {
                command: "/quote".to_owned(),
                text: "help".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-1".to_owned(),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");

        assert!(matches!(result, HandlerResult::Responded(_)));
    }

    #[tokio::test]
    async fn dispatcher_returns_ignored_when_no_handler_registered() {
        let dispatcher = EventDispatcher::new();
        let envelope = SlackEnvelope {
            envelope_id: "env-2".to_owned(),
            event: SlackEvent::ThreadMessage(ThreadMessageEvent {
                channel_id: "C1".to_owned(),
                thread_ts: "T1".to_owned(),
                user_id: "U2".to_owned(),
                text: "hello".to_owned(),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");

        assert_eq!(result, HandlerResult::Ignored);
    }

    #[test]
    fn default_dispatcher_registers_handlers() {
        let dispatcher = default_dispatcher();
        assert_eq!(dispatcher.handler_count(), 3);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[tokio::test]
    async fn dispatcher_routes_reaction_added_for_supported_emoji() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-3".to_owned(),
            event: SlackEvent::ReactionAdded(ReactionAddedEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.2000".to_owned(),
                thread_ts: Some("1730000000.1000".to_owned()),
                reactor_user_id: "U3".to_owned(),
                reaction: "👍".to_owned(),
                quote_id: Some("Q-2026-1001".to_owned()),
                approval_type: "discount".to_owned(),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");

        assert!(matches!(result, HandlerResult::Responded(_)));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[tokio::test]
    async fn dispatcher_processes_but_does_not_respond_for_non_approval_emoji() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-4".to_owned(),
            event: SlackEvent::ReactionAdded(ReactionAddedEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.3000".to_owned(),
                thread_ts: Some("1730000000.1000".to_owned()),
                reactor_user_id: "U4".to_owned(),
                reaction: "rocket".to_owned(),
                quote_id: Some("Q-2026-1002".to_owned()),
                approval_type: "discount".to_owned(),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");

        assert_eq!(result, HandlerResult::Processed);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[tokio::test]
    async fn dispatcher_accepts_colon_wrapped_case_variant_reaction_alias() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-5".to_owned(),
            event: SlackEvent::ReactionAdded(ReactionAddedEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.4000".to_owned(),
                thread_ts: Some("1730000000.1000".to_owned()),
                reactor_user_id: "U5".to_owned(),
                reaction: ":THUMBSUP:".to_owned(),
                quote_id: Some("Q-2026-1003".to_owned()),
                approval_type: "discount".to_owned(),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");

        assert!(matches!(result, HandlerResult::Responded(_)));
    }

    #[test]
    fn reaction_token_normalization_handles_spacing_and_colons() {
        assert_eq!(super::normalize_reaction_token(" :THUMBSDOWN: "), "thumbsdown");
    }
}
