use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use thiserror::Error;

use crate::{
    blocks::MessageTemplate,
    commands::{
        action_quote_id, infer_thread_quote_command, normalize_quote_command, CommandParseError,
        CommandRouteError, CommandRouter, NoopQuoteCommandService, QuoteCommandService,
        SlashCommandPayload,
    },
};
use quotey_core::domain::dialogue::SlackQuoteState;

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
    BlockAction(BlockActionEvent),
    Unsupported { event_type: String },
}

impl SlackEvent {
    pub fn event_type(&self) -> SlackEventType {
        match self {
            Self::SlashCommand(_) => SlackEventType::SlashCommand,
            Self::ThreadMessage(_) => SlackEventType::ThreadMessage,
            Self::ReactionAdded(_) => SlackEventType::ReactionAdded,
            Self::BlockAction(_) => SlackEventType::BlockAction,
            Self::Unsupported { .. } => SlackEventType::Unsupported,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SlackEventType {
    SlashCommand,
    ThreadMessage,
    ReactionAdded,
    BlockAction,
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
pub struct BlockActionEvent {
    pub channel_id: String,
    pub message_ts: String,
    pub thread_ts: Option<String>,
    pub user_id: String,
    pub action_id: String,
    pub value: Option<String>,
    pub quote_id: Option<String>,
    pub request_id: Option<String>,
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
    dispatcher.register(ThreadMessageHandler::new(NoopThreadMessageService::new()));
    dispatcher.register(ReactionAddedHandler::new(NoopReactionApprovalService));
    dispatcher.register(BlockActionHandler::new(NoopBlockActionService));
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
    ) -> Result<Option<MessageTemplate>, EventHandlerError>;
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
        Ok(match message {
            Some(message) => HandlerResult::Responded(message),
            None => HandlerResult::Processed,
        })
    }
}

pub struct NoopThreadMessageService {
    router: CommandRouter<NoopQuoteCommandService>,
}

impl NoopThreadMessageService {
    pub fn new() -> Self {
        Self { router: CommandRouter::new(NoopQuoteCommandService) }
    }
}

impl Default for NoopThreadMessageService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ThreadMessageService for NoopThreadMessageService {
    async fn handle_thread_message(
        &self,
        event: &ThreadMessageEvent,
        _ctx: &EventContext,
    ) -> Result<Option<MessageTemplate>, EventHandlerError> {
        let Some(text) = infer_thread_quote_command(&event.text) else {
            return Ok(None);
        };
        let payload = SlashCommandPayload {
            command: "/quote".to_owned(),
            text,
            channel_id: event.channel_id.clone(),
            user_id: event.user_id.clone(),
            trigger_ts: event.thread_ts.clone(),
            request_id: format!("thread-{}", event.thread_ts),
        };

        let normalized = normalize_quote_command(payload)?;
        self.router.route(normalized).map(Some).map_err(EventHandlerError::from)
    }
}

/// Minimal session info needed to render a resume prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumableSessionInfo {
    pub session_id: String,
    pub state: SlackQuoteState,
    pub started: String,
    pub last_active: String,
    pub is_expired: bool,
}

/// Trait for looking up sessions by thread ID.
/// Implementations can delegate to a database repository or provide mock data.
#[async_trait]
pub trait SessionLookup: Send + Sync {
    async fn find_resumable_session(
        &self,
        thread_id: &str,
    ) -> Result<Option<ResumableSessionInfo>, EventHandlerError>;
}

/// Noop session lookup that never finds any sessions.
pub struct NoopSessionLookup;

#[async_trait]
impl SessionLookup for NoopSessionLookup {
    async fn find_resumable_session(
        &self,
        _thread_id: &str,
    ) -> Result<Option<ResumableSessionInfo>, EventHandlerError> {
        Ok(None)
    }
}

/// A ThreadMessageService that checks for resumable sessions before delegating.
/// If a resumable session exists, returns a resume prompt.
/// If an expired session exists, returns an expired recovery message.
/// Otherwise, delegates to the inner service.
pub struct ResumableThreadMessageService<L, S>
where
    L: SessionLookup,
    S: ThreadMessageService,
{
    lookup: L,
    inner: S,
}

impl<L, S> ResumableThreadMessageService<L, S>
where
    L: SessionLookup,
    S: ThreadMessageService,
{
    pub fn new(lookup: L, inner: S) -> Self {
        Self { lookup, inner }
    }
}

#[async_trait]
impl<L, S> ThreadMessageService for ResumableThreadMessageService<L, S>
where
    L: SessionLookup + 'static,
    S: ThreadMessageService + 'static,
{
    async fn handle_thread_message(
        &self,
        event: &ThreadMessageEvent,
        ctx: &EventContext,
    ) -> Result<Option<MessageTemplate>, EventHandlerError> {
        // First, check for an existing session on this thread
        if let Some(info) = self.lookup.find_resumable_session(&event.thread_ts).await? {
            if info.is_expired {
                return Ok(Some(crate::blocks::session_expired_recovery_message(&event.thread_ts)));
            } else {
                return Ok(Some(crate::blocks::session_resume_prompt(
                    &info.session_id,
                    &info.state,
                    &info.started,
                    &info.last_active,
                )));
            }
        }

        // No existing session, delegate to inner service
        self.inner.handle_thread_message(event, ctx).await
    }
}

#[async_trait]
pub trait BlockActionService: Send + Sync {
    async fn handle_block_action(
        &self,
        event: &BlockActionEvent,
        ctx: &EventContext,
    ) -> Result<Option<MessageTemplate>, EventHandlerError>;
}

pub struct BlockActionHandler<S> {
    service: S,
}

impl<S> BlockActionHandler<S>
where
    S: BlockActionService,
{
    pub fn new(service: S) -> Self {
        Self { service }
    }
}

#[async_trait]
impl<S> EventHandler for BlockActionHandler<S>
where
    S: BlockActionService + 'static,
{
    fn event_type(&self) -> SlackEventType {
        SlackEventType::BlockAction
    }

    async fn handle(
        &self,
        envelope: &SlackEnvelope,
        ctx: &EventContext,
    ) -> Result<HandlerResult, EventHandlerError> {
        let SlackEvent::BlockAction(event) = &envelope.event else {
            return Ok(HandlerResult::Ignored);
        };

        let message = self.service.handle_block_action(event, ctx).await?;
        Ok(match message {
            Some(message) => HandlerResult::Responded(message),
            None => HandlerResult::Processed,
        })
    }
}

pub struct NoopBlockActionService;

#[async_trait]
impl BlockActionService for NoopBlockActionService {
    async fn handle_block_action(
        &self,
        event: &BlockActionEvent,
        ctx: &EventContext,
    ) -> Result<Option<MessageTemplate>, EventHandlerError> {
        let request_id = event.request_id.as_deref().unwrap_or(&ctx.correlation_id);
        if event.action_id == "quote.help.v1" {
            return Ok(Some(crate::blocks::help_message()));
        }
        if let Some(message) = crate::commands::help_command_shortcut_message(
            &event.action_id,
            event.quote_id.as_deref(),
        ) {
            return Ok(Some(message));
        }

        let mut inferred_quote_id = event.quote_id.clone();
        if inferred_quote_id.is_none() {
            let from_value = action_quote_id(event.value.as_deref(), None);
            if from_value != "unknown" {
                inferred_quote_id = Some(from_value);
            }
        }
        if let Some(message) = crate::commands::help_command_shortcut_message(
            &event.action_id,
            inferred_quote_id.as_deref(),
        ) {
            return Ok(Some(message));
        }

        // Handle session resume/restart/new actions
        if let Some(message) = handle_session_action(&event.action_id, event.value.as_deref()) {
            return Ok(Some(message));
        }

        let quote_id = inferred_quote_id.as_deref();
        let detail = match &event.value {
            Some(value) => {
                format!("interactive action `{}` with payload `{value}`", event.action_id)
            }
            None => format!("interactive action `{}` with no payload", event.action_id),
        };

        Ok(Some(crate::blocks::preview_mode_message(
            &format!("button:{action}", action = event.action_id),
            quote_id,
            &detail,
            request_id,
        )))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReactionApprovalOutcome {
    pub quote_id: String,
    pub action: ReactionApprovalAction,
    pub database_recorded: bool,
    pub state_transition_triggered: bool,
    pub confirmation_dm_sent: bool,
    pub undo_window_secs: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReactionApprovalAction {
    Approve,
    Reject,
    Discuss,
}

impl ReactionApprovalAction {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Discuss => "discuss",
        }
    }
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
        let summary = reaction_approval_summary(
            &outcome,
            &event.reaction,
            &event.approval_type,
            &event.reactor_user_id,
        );

        Ok(HandlerResult::Responded(crate::blocks::quote_status_message(
            &outcome.quote_id,
            &summary,
        )))
    }
}

fn reaction_approval_summary(
    outcome: &ReactionApprovalOutcome,
    reaction: &str,
    approval_type: &str,
    reactor_user_id: &str,
) -> String {
    let actor = format!("<@{reactor_user_id}>");
    let action = match outcome.action {
        ReactionApprovalAction::Approve => "approved",
        ReactionApprovalAction::Reject => "rejected",
        ReactionApprovalAction::Discuss => "tagged for discussion",
    };
    let icon = match outcome.action {
        ReactionApprovalAction::Approve => "✅",
        ReactionApprovalAction::Reject => "🚫",
        ReactionApprovalAction::Discuss => "💬",
    };
    let approval_scope =
        if approval_type.trim().is_empty() { "general quote approval" } else { approval_type };

    if outcome.undo_window_secs == 0 {
        format!(
            "{icon} {actor} captured `{reaction}` on `{}` for `{}`. Action has been {action}.",
            outcome.quote_id, approval_scope
        )
    } else {
        format!(
            "{icon} {actor} captured `{reaction}` on `{}` for `{}` and marked as {action}. Undo window: {}s (remove reaction before it expires to revert this signal).",
            outcome.quote_id, approval_scope, outcome.undo_window_secs
        )
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
            action: reaction_approval_action(&event.reaction).ok_or_else(|| {
                EventHandlerError::ReactionApproval("unsupported approval reaction".to_owned())
            })?,
            quote_id,
            database_recorded: true,
            state_transition_triggered: true,
            confirmation_dm_sent: true,
            undo_window_secs: 300,
        })
    }
}

fn reaction_approval_action(reaction: &str) -> Option<ReactionApprovalAction> {
    let normalized = normalize_reaction_token(reaction);
    match normalized.as_str() {
        "✅" | "white_check_mark" | "check" => Some(ReactionApprovalAction::Approve),
        "👍" | "thumbsup" | "+1" => Some(ReactionApprovalAction::Approve),
        "👎" | "thumbsdown" | "-1" => Some(ReactionApprovalAction::Reject),
        "💬" | "speech_balloon" | "🚀" | "rocket" => Some(ReactionApprovalAction::Discuss),
        _ => None,
    }
}

fn is_supported_approval_reaction(reaction: &str) -> bool {
    reaction_approval_action(reaction).is_some()
}

fn normalize_reaction_token(reaction: &str) -> String {
    reaction.trim().trim_matches(':').to_ascii_lowercase()
}

/// Handle session-related block actions (resume, restart, new).
/// Returns a message template if the action is session-related, None otherwise.
fn handle_session_action(action_id: &str, value: Option<&str>) -> Option<MessageTemplate> {
    match action_id {
        "session.resume.v1" => {
            let session_id = parse_session_id_from_value(value.unwrap_or(""));
            Some(crate::blocks::session_resumed_message(&session_id))
        }
        "session.restart.v1" => {
            let session_id = parse_session_id_from_value(value.unwrap_or(""));
            Some(crate::blocks::session_restarted_message(&session_id))
        }
        "session.new.v1" => {
            let thread_ts = parse_thread_ts_from_value(value.unwrap_or(""));
            Some(crate::blocks::new_quote_started_message(&thread_ts))
        }
        _ => None,
    }
}

/// Parse session ID from a value string like "session=abc123;action=resume"
fn parse_session_id_from_value(value: &str) -> String {
    value.split(';').find_map(|part| part.strip_prefix("session=")).unwrap_or("unknown").to_string()
}

/// Parse thread_ts from a value string like "thread=123.456;action=new_quote"
fn parse_thread_ts_from_value(value: &str) -> String {
    value.split(';').find_map(|part| part.strip_prefix("thread=")).unwrap_or("unknown").to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        async_trait, default_dispatcher, BlockActionEvent, EventContext, EventDispatcher,
        EventHandlerError, HandlerResult, NoopSessionLookup, ReactionAddedEvent,
        ReactionApprovalAction, ResumableSessionInfo, ResumableThreadMessageService, SessionLookup,
        SlackEnvelope, SlackEvent, ThreadMessageEvent, ThreadMessageService,
    };
    use crate::commands::SlashCommandPayload;
    use quotey_core::domain::dialogue::SlackQuoteState;

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
    async fn dispatcher_routes_quotey_branding_slash_command() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-quotey-branding-1".to_owned(),
            event: SlackEvent::SlashCommand(SlashCommandPayload {
                command: "/quotey".to_owned(),
                text: "branding".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U1".to_owned(),
                trigger_ts: "1".to_owned(),
                request_id: "req-quotey-branding-1".to_owned(),
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
        assert_eq!(dispatcher.handler_count(), 4);
    }

    #[tokio::test]
    async fn dispatcher_routes_block_actions() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-block-1".to_owned(),
            event: SlackEvent::BlockAction(BlockActionEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.6000".to_owned(),
                thread_ts: Some("1730000000.5000".to_owned()),
                user_id: "U6".to_owned(),
                action_id: "quote.refresh.v1".to_owned(),
                value: None,
                quote_id: Some("Q-2026-1009".to_owned()),
                request_id: Some("req-block-1".to_owned()),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");

        assert!(matches!(result, HandlerResult::Responded(_)));
    }

    #[tokio::test]
    async fn dispatcher_routes_unknown_block_action_to_guidance_message() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-block-2".to_owned(),
            event: SlackEvent::BlockAction(BlockActionEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.7000".to_owned(),
                thread_ts: Some("1730000000.5000".to_owned()),
                user_id: "U7".to_owned(),
                action_id: "unknown.action".to_owned(),
                value: None,
                quote_id: Some("Q-2026-1010".to_owned()),
                request_id: Some("req-block-2".to_owned()),
            }),
        };

        let result = dispatcher.dispatch(&envelope, &EventContext::default()).await;
        let message = result.expect("unknown action should resolve to a guidance card");
        assert!(matches!(message, super::HandlerResult::Responded(_)));
        let message = match message {
            super::HandlerResult::Responded(message) => message,
            _ => unreachable!(),
        };
        assert!(message.fallback_text.contains("Preview mode active"));
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

    #[tokio::test]
    async fn dispatcher_silently_ignores_thread_noise_when_command_is_not_inferred() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-thread-noise-1".to_owned(),
            event: SlackEvent::ThreadMessage(ThreadMessageEvent {
                channel_id: "C1".to_owned(),
                thread_ts: "1730000001.0000".to_owned(),
                user_id: "U8".to_owned(),
                text: "random thread banter".to_owned(),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");

        assert_eq!(result, HandlerResult::Processed);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[tokio::test]
    async fn dispatcher_routes_rocket_alias_as_discussion_reaction() {
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

        assert!(matches!(result, HandlerResult::Responded(_)));
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

    #[test]
    fn reaction_action_supports_checkmark_and_rocket_aliases() {
        assert_eq!(super::reaction_approval_action("✅"), Some(ReactionApprovalAction::Approve));
        assert_eq!(super::reaction_approval_action("🚀"), Some(ReactionApprovalAction::Discuss));
        assert_eq!(super::reaction_approval_action("👍"), Some(ReactionApprovalAction::Approve));
        assert_eq!(super::reaction_approval_action("👎"), Some(ReactionApprovalAction::Reject));
    }

    // Session resume tests

    /// A mock session lookup that returns a fixed session for specific thread IDs.
    struct MockSessionLookup {
        sessions: std::collections::HashMap<String, ResumableSessionInfo>,
    }

    impl MockSessionLookup {
        fn new() -> Self {
            Self { sessions: std::collections::HashMap::new() }
        }

        fn with_session(mut self, thread_id: &str, info: ResumableSessionInfo) -> Self {
            self.sessions.insert(thread_id.to_string(), info);
            self
        }
    }

    #[async_trait]
    impl SessionLookup for MockSessionLookup {
        async fn find_resumable_session(
            &self,
            thread_id: &str,
        ) -> Result<Option<ResumableSessionInfo>, EventHandlerError> {
            Ok(self.sessions.get(thread_id).cloned())
        }
    }

    #[tokio::test]
    async fn resumable_service_shows_resume_prompt_when_session_is_resumable() {
        let lookup = MockSessionLookup::new().with_session(
            "thread-resume",
            ResumableSessionInfo {
                session_id: "session-123".to_string(),
                state: SlackQuoteState::ContextCollection,
                started: "2024-01-01".to_string(),
                last_active: "2024-01-01".to_string(),
                is_expired: false,
            },
        );
        let inner = super::NoopThreadMessageService::new();
        let service = ResumableThreadMessageService::new(lookup, inner);

        let event = ThreadMessageEvent {
            channel_id: "C1".to_owned(),
            thread_ts: "thread-resume".to_owned(),
            user_id: "U1".to_owned(),
            text: "any message".to_owned(),
        };

        let result = service
            .handle_thread_message(&event, &EventContext::default())
            .await
            .expect("should not error");

        let message = result.expect("should return a message");
        assert!(message.fallback_text.contains("Resume session"));
    }

    #[tokio::test]
    async fn resumable_service_shows_expired_recovery_when_session_is_expired() {
        let lookup = MockSessionLookup::new().with_session(
            "thread-expired",
            ResumableSessionInfo {
                session_id: "session-456".to_string(),
                state: SlackQuoteState::IntentCapture,
                started: "2024-01-01".to_string(),
                last_active: "2024-01-01".to_string(),
                is_expired: true,
            },
        );
        let inner = super::NoopThreadMessageService::new();
        let service = ResumableThreadMessageService::new(lookup, inner);

        let event = ThreadMessageEvent {
            channel_id: "C1".to_owned(),
            thread_ts: "thread-expired".to_owned(),
            user_id: "U1".to_owned(),
            text: "any message".to_owned(),
        };

        let result = service
            .handle_thread_message(&event, &EventContext::default())
            .await
            .expect("should not error");

        let message = result.expect("should return a message");
        assert!(message.fallback_text.contains("session has expired"));
    }

    #[tokio::test]
    async fn resumable_service_delegates_to_inner_when_no_session_exists() {
        let lookup = MockSessionLookup::new();
        let inner = super::NoopThreadMessageService::new();
        let service = ResumableThreadMessageService::new(lookup, inner);

        let event = ThreadMessageEvent {
            channel_id: "C1".to_owned(),
            thread_ts: "thread-new".to_owned(),
            user_id: "U1".to_owned(),
            text: "quote for Acme Corp".to_owned(),
        };

        let result = service
            .handle_thread_message(&event, &EventContext::default())
            .await
            .expect("should not error");

        // NoopThreadMessageService returns a message for valid commands
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn noop_session_lookup_always_returns_none() {
        let lookup = NoopSessionLookup;
        let result = lookup.find_resumable_session("any-thread").await.expect("should not error");
        assert!(result.is_none());
    }

    #[test]
    fn session_action_handler_resume_returns_resumed_message() {
        let message =
            super::handle_session_action("session.resume.v1", Some("session=abc123;action=resume"));
        assert!(message.is_some());
        let msg = message.unwrap();
        assert!(msg.fallback_text.contains("Session Resumed"));
    }

    #[test]
    fn session_action_handler_restart_returns_restarted_message() {
        let message = super::handle_session_action(
            "session.restart.v1",
            Some("session=old456;action=restart"),
        );
        assert!(message.is_some());
        let msg = message.unwrap();
        assert!(msg.fallback_text.contains("Starting Fresh"));
    }

    #[test]
    fn session_action_handler_new_returns_new_quote_message() {
        let message =
            super::handle_session_action("session.new.v1", Some("thread=123.456;action=new_quote"));
        assert!(message.is_some());
        let msg = message.unwrap();
        assert!(msg.fallback_text.contains("New Quote"));
    }

    #[test]
    fn session_action_handler_returns_none_for_unknown_action() {
        let message = super::handle_session_action("unknown.action", None);
        assert!(message.is_none());
    }

    #[test]
    fn parse_session_id_extracts_id_from_value() {
        assert_eq!(super::parse_session_id_from_value("session=abc123;action=resume"), "abc123");
        assert_eq!(super::parse_session_id_from_value("action=restart;session=xyz789"), "xyz789");
        assert_eq!(super::parse_session_id_from_value("no-session-here"), "unknown");
    }

    #[test]
    fn parse_thread_ts_extracts_ts_from_value() {
        assert_eq!(super::parse_thread_ts_from_value("thread=123.456;action=new_quote"), "123.456");
        assert_eq!(super::parse_thread_ts_from_value("action=new;thread=999.888"), "999.888");
        assert_eq!(super::parse_thread_ts_from_value("no-thread-here"), "unknown");
    }

    #[tokio::test]
    async fn dispatcher_routes_session_resume_block_action() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-session-resume".to_owned(),
            event: SlackEvent::BlockAction(BlockActionEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.8000".to_owned(),
                thread_ts: Some("1730000000.7000".to_owned()),
                user_id: "U9".to_owned(),
                action_id: "session.resume.v1".to_owned(),
                value: Some("session=session-xyz;action=resume".to_owned()),
                quote_id: None,
                request_id: Some("req-session-resume".to_owned()),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");
        assert!(matches!(result, HandlerResult::Responded(_)));
        if let HandlerResult::Responded(msg) = result {
            assert!(msg.fallback_text.contains("Session Resumed"));
        }
    }

    #[tokio::test]
    async fn dispatcher_routes_session_restart_block_action() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-session-restart".to_owned(),
            event: SlackEvent::BlockAction(BlockActionEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.9000".to_owned(),
                thread_ts: Some("1730000000.7000".to_owned()),
                user_id: "U10".to_owned(),
                action_id: "session.restart.v1".to_owned(),
                value: Some("session=session-old;action=restart".to_owned()),
                quote_id: None,
                request_id: Some("req-session-restart".to_owned()),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");
        assert!(matches!(result, HandlerResult::Responded(_)));
        if let HandlerResult::Responded(msg) = result {
            assert!(msg.fallback_text.contains("Starting Fresh"));
        }
    }

    #[tokio::test]
    async fn dispatcher_routes_session_new_block_action() {
        let dispatcher = default_dispatcher();
        let envelope = SlackEnvelope {
            envelope_id: "env-session-new".to_owned(),
            event: SlackEvent::BlockAction(BlockActionEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000001.0000".to_owned(),
                thread_ts: Some("1730000000.7000".to_owned()),
                user_id: "U11".to_owned(),
                action_id: "session.new.v1".to_owned(),
                value: Some("thread=1730000000.7000;action=new_quote".to_owned()),
                quote_id: None,
                request_id: Some("req-session-new".to_owned()),
            }),
        };

        let result =
            dispatcher.dispatch(&envelope, &EventContext::default()).await.expect("dispatch");
        assert!(matches!(result, HandlerResult::Responded(_)));
        if let HandlerResult::Responded(msg) = result {
            assert!(msg.fallback_text.contains("New Quote"));
        }
    }
}
