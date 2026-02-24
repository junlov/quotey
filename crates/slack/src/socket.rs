use std::{sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::commands::action_quote_id;
use crate::events::{
    default_dispatcher, DispatchError, EventContext, EventDispatcher, SlackEnvelope, SlackEvent,
};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransportError {
    #[error("transport failed to connect: {0}")]
    Connect(String),
    #[error("transport read failed: {0}")]
    Receive(String),
    #[error("transport ack failed: {0}")]
    Acknowledge(String),
    #[error("transport disconnect failed: {0}")]
    Disconnect(String),
}

#[derive(Debug, Error)]
pub enum SocketError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Dispatch(#[from] DispatchError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self { max_retries: 5, base_delay_ms: 250, max_delay_ms: 5_000 }
    }
}

impl ReconnectPolicy {
    fn backoff(&self, attempt: u32) -> Duration {
        let exponent = attempt.min(16);
        let multiplier = 1_u64 << exponent;
        let delay_ms = self.base_delay_ms.saturating_mul(multiplier).min(self.max_delay_ms);
        Duration::from_millis(delay_ms)
    }
}

#[async_trait]
pub trait SocketTransport: Send + Sync {
    async fn connect(&self) -> Result<(), TransportError>;
    async fn next_envelope(&self) -> Result<Option<SlackEnvelope>, TransportError>;
    async fn acknowledge(&self, envelope_id: &str) -> Result<(), TransportError>;
    async fn disconnect(&self) -> Result<(), TransportError>;

    fn is_noop(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct NoopSocketTransport;

#[async_trait]
impl SocketTransport for NoopSocketTransport {
    async fn connect(&self) -> Result<(), TransportError> {
        Ok(())
    }

    async fn next_envelope(&self) -> Result<Option<SlackEnvelope>, TransportError> {
        Ok(None)
    }

    async fn acknowledge(&self, _envelope_id: &str) -> Result<(), TransportError> {
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), TransportError> {
        Ok(())
    }

    fn is_noop(&self) -> bool {
        true
    }
}

pub struct SocketModeRunner {
    transport: Arc<dyn SocketTransport>,
    dispatcher: EventDispatcher,
    reconnect_policy: ReconnectPolicy,
    is_noop_transport: bool,
}

impl Default for SocketModeRunner {
    fn default() -> Self {
        Self {
            transport: Arc::new(NoopSocketTransport),
            dispatcher: default_dispatcher(),
            reconnect_policy: ReconnectPolicy::default(),
            is_noop_transport: true,
        }
    }
}

impl SocketModeRunner {
    pub fn new(
        transport: Arc<dyn SocketTransport>,
        dispatcher: EventDispatcher,
        reconnect_policy: ReconnectPolicy,
    ) -> Self {
        Self { is_noop_transport: transport.is_noop(), transport, dispatcher, reconnect_policy }
    }

    pub fn is_noop_transport(&self) -> bool {
        self.is_noop_transport
    }

    pub async fn start(&self) -> Result<()> {
        if self.is_noop_transport {
            info!(
                event_name = "system.slack.transport_mode",
                transport = "noop",
                "slack transport is running in no-op mode"
            );
        } else {
            info!(
                event_name = "system.slack.transport_mode",
                transport = "socket",
                "slack transport is running in socket mode"
            );
        }

        for attempt in 0..=self.reconnect_policy.max_retries {
            match self.connect_and_pump(attempt).await {
                Ok(()) => return Ok(()),
                Err(transport_error) => {
                    warn!(
                        attempt,
                        max_retries = self.reconnect_policy.max_retries,
                        error = %transport_error,
                        "socket mode transport failed"
                    );

                    if attempt >= self.reconnect_policy.max_retries {
                        warn!(
                            max_retries = self.reconnect_policy.max_retries,
                            "socket mode retries exhausted after {} attempts; returning startup error",
                            attempt + 1
                        );
                        return Err(anyhow!(
                            "socket mode retries exhausted after {} attempts: {}",
                            attempt + 1,
                            transport_error
                        ));
                    }

                    let delay = self.reconnect_policy.backoff(attempt);
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn connect_and_pump(&self, attempt: u32) -> Result<(), TransportError> {
        info!(attempt, "opening socket mode transport connection");
        self.transport.connect().await?;
        info!(attempt, "socket mode transport connected");

        loop {
            let Some(envelope) = self.transport.next_envelope().await? else {
                info!(attempt, "socket mode transport stream closed");
                self.transport.disconnect().await?;
                return Ok(());
            };
            let (quote_id, thread_id) = correlation_fields(&envelope);

            info!(
                event_name = "ingress.slack.envelope_received",
                envelope_id = %envelope.envelope_id,
                event_type = ?envelope.event.event_type(),
                correlation_id = %envelope.envelope_id,
                quote_id = quote_id.as_deref().unwrap_or("unknown"),
                thread_id = thread_id.as_deref().unwrap_or("unknown"),
                "received slack envelope"
            );

            if let Err(error) = self.transport.acknowledge(&envelope.envelope_id).await {
                warn!(
                    event_name = "ingress.slack.ack_sent",
                    envelope_id = %envelope.envelope_id,
                    correlation_id = %envelope.envelope_id,
                    quote_id = quote_id.as_deref().unwrap_or("unknown"),
                    thread_id = thread_id.as_deref().unwrap_or("unknown"),
                    error = %error,
                    "failed to acknowledge slack envelope"
                );
            } else {
                debug!(
                    event_name = "ingress.slack.ack_sent",
                    envelope_id = %envelope.envelope_id,
                    correlation_id = %envelope.envelope_id,
                    quote_id = quote_id.as_deref().unwrap_or("unknown"),
                    thread_id = thread_id.as_deref().unwrap_or("unknown"),
                    "acknowledged slack envelope"
                );
            }

            let context = EventContext { correlation_id: envelope.envelope_id.clone() };
            if let Err(error) = self.dispatcher.dispatch(&envelope, &context).await {
                warn!(
                    envelope_id = %envelope.envelope_id,
                    correlation_id = %envelope.envelope_id,
                    quote_id = quote_id.as_deref().unwrap_or("unknown"),
                    thread_id = thread_id.as_deref().unwrap_or("unknown"),
                    error = %error,
                    "event dispatch failed; continuing socket loop"
                );
            }
        }
    }
}

fn correlation_fields(envelope: &SlackEnvelope) -> (Option<String>, Option<String>) {
    match &envelope.event {
        SlackEvent::ThreadMessage(event) => {
            (quote_id_from_text(&event.text), Some(event.thread_ts.clone()))
        }
        SlackEvent::ReactionAdded(event) => (
            event.quote_id.clone(),
            event.thread_ts.clone().or_else(|| Some(event.message_ts.clone())),
        ),
        SlackEvent::SlashCommand(payload) => (quote_id_from_text(&payload.text), None),
        SlackEvent::BlockAction(event) => (
            event.quote_id.clone().or_else(|| {
                let quote_id = action_quote_id(event.value.as_deref(), None);
                (quote_id != "unknown").then_some(quote_id)
            }),
            event.thread_ts.clone().or_else(|| Some(event.message_ts.clone())),
        ),
        SlackEvent::Unsupported { .. } => (None, None),
    }
}

fn quote_id_from_text(text: &str) -> Option<String> {
    text.split_whitespace().find_map(|token| {
        let candidate = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
        let normalized = candidate.to_ascii_uppercase();
        let bytes = candidate.as_bytes();
        let normalized_bytes = normalized.as_bytes();
        if bytes.len() == 11
            && normalized.starts_with("Q-")
            && normalized_bytes[2..6].iter().all(u8::is_ascii_digit)
            && normalized_bytes[6] == b'-'
            && normalized_bytes[7..11].iter().all(u8::is_ascii_digit)
        {
            Some(normalized)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use super::{
        NoopSocketTransport, ReconnectPolicy, SocketModeRunner, SocketTransport, TransportError,
    };
    use crate::events::{EventDispatcher, SlackEnvelope, SlackEvent};
    use async_trait::async_trait;
    use tokio::sync::Mutex;

    #[derive(Default)]
    struct ScriptedTransport {
        state: Mutex<ScriptedState>,
    }

    #[derive(Default)]
    struct ScriptedState {
        connect_results: VecDeque<Result<(), TransportError>>,
        envelopes: VecDeque<Result<Option<SlackEnvelope>, TransportError>>,
        disconnect_results: VecDeque<Result<(), TransportError>>,
        connect_attempts: usize,
        acknowledgements: Vec<String>,
        disconnect_calls: usize,
    }

    impl ScriptedTransport {
        fn with_script(
            connect_results: Vec<Result<(), TransportError>>,
            envelopes: Vec<Result<Option<SlackEnvelope>, TransportError>>,
            disconnect_results: Vec<Result<(), TransportError>>,
        ) -> Self {
            Self {
                state: Mutex::new(ScriptedState {
                    connect_results: connect_results.into(),
                    envelopes: envelopes.into(),
                    disconnect_results: disconnect_results.into(),
                    connect_attempts: 0,
                    acknowledgements: Vec::new(),
                    disconnect_calls: 0,
                }),
            }
        }

        async fn connect_attempts(&self) -> usize {
            self.state.lock().await.connect_attempts
        }

        async fn acknowledgements(&self) -> Vec<String> {
            self.state.lock().await.acknowledgements.clone()
        }
    }

    #[async_trait]
    impl SocketTransport for ScriptedTransport {
        async fn connect(&self) -> Result<(), TransportError> {
            let mut state = self.state.lock().await;
            state.connect_attempts += 1;
            state.connect_results.pop_front().unwrap_or(Ok(()))
        }

        async fn next_envelope(&self) -> Result<Option<SlackEnvelope>, TransportError> {
            let mut state = self.state.lock().await;
            state.envelopes.pop_front().unwrap_or(Ok(None))
        }

        async fn acknowledge(&self, envelope_id: &str) -> Result<(), TransportError> {
            let mut state = self.state.lock().await;
            state.acknowledgements.push(envelope_id.to_owned());
            Ok(())
        }

        async fn disconnect(&self) -> Result<(), TransportError> {
            let mut state = self.state.lock().await;
            state.disconnect_calls += 1;
            state.disconnect_results.pop_front().unwrap_or(Ok(()))
        }
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[tokio::test]
    async fn reconnects_after_initial_connect_failure() {
        let transport = Arc::new(ScriptedTransport::with_script(
            vec![Err(TransportError::Connect("network down".to_owned())), Ok(())],
            vec![
                Ok(Some(SlackEnvelope {
                    envelope_id: "env-1".to_owned(),
                    event: SlackEvent::Unsupported { event_type: "test".to_owned() },
                })),
                Ok(None),
            ],
            vec![Ok(())],
        ));

        let runner = SocketModeRunner::new(
            transport.clone(),
            EventDispatcher::default(),
            ReconnectPolicy { max_retries: 2, base_delay_ms: 0, max_delay_ms: 0 },
        );

        runner.start().await.expect("runner should not fail");

        assert_eq!(transport.connect_attempts().await, 2);
        assert_eq!(transport.acknowledgements().await, vec!["env-1"]);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[tokio::test]
    async fn exhausts_retries_without_crashing() {
        let transport = Arc::new(ScriptedTransport::with_script(
            vec![
                Err(TransportError::Connect("fail-1".to_owned())),
                Err(TransportError::Connect("fail-2".to_owned())),
                Err(TransportError::Connect("fail-3".to_owned())),
            ],
            vec![],
            vec![],
        ));

        let runner = SocketModeRunner::new(
            transport.clone(),
            EventDispatcher::default(),
            ReconnectPolicy { max_retries: 2, base_delay_ms: 0, max_delay_ms: 0 },
        );

        assert!(runner.start().await.is_err());
        assert_eq!(transport.connect_attempts().await, 3);
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn default_transport_mode_is_noop() {
        let runner = SocketModeRunner::default();
        assert!(runner.is_noop_transport());
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn new_runner_transport_mode_is_configured() {
        let runner = SocketModeRunner::new(
            Arc::new(NoopSocketTransport),
            EventDispatcher::default(),
            ReconnectPolicy::default(),
        );
        assert!(runner.is_noop_transport());
    }

    #[test]
    fn new_runner_transport_mode_reflects_non_noop_transport() {
        let transport =
            Arc::new(ScriptedTransport::with_script(vec![Ok(())], vec![Ok(None)], vec![Ok(())]));
        let runner = SocketModeRunner::new(
            transport,
            EventDispatcher::default(),
            ReconnectPolicy::default(),
        );

        assert!(!runner.is_noop_transport());
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn extracts_quote_and_thread_correlation_fields() {
        let envelope = SlackEnvelope {
            envelope_id: "env-2".to_owned(),
            event: SlackEvent::ThreadMessage(crate::events::ThreadMessageEvent {
                channel_id: "C1".to_owned(),
                thread_ts: "1730000000.1000".to_owned(),
                user_id: "U1".to_owned(),
                text: "status Q-2026-0032".to_owned(),
            }),
        };

        let (quote_id, thread_id) = super::correlation_fields(&envelope);
        assert_eq!(quote_id.as_deref(), Some("Q-2026-0032"));
        assert_eq!(thread_id.as_deref(), Some("1730000000.1000"));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn extracts_quote_and_thread_correlation_fields_from_reactions() {
        let envelope = SlackEnvelope {
            envelope_id: "env-3".to_owned(),
            event: SlackEvent::ReactionAdded(crate::events::ReactionAddedEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.2000".to_owned(),
                thread_ts: Some("1730000000.1000".to_owned()),
                reactor_user_id: "U9".to_owned(),
                reaction: "👍".to_owned(),
                quote_id: Some("Q-2026-0042".to_owned()),
                approval_type: "discount".to_owned(),
            }),
        };

        let (quote_id, thread_id) = super::correlation_fields(&envelope);
        assert_eq!(quote_id.as_deref(), Some("Q-2026-0042"));
        assert_eq!(thread_id.as_deref(), Some("1730000000.1000"));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn reaction_correlation_falls_back_to_message_ts_when_thread_ts_missing() {
        let envelope = SlackEnvelope {
            envelope_id: "env-4".to_owned(),
            event: SlackEvent::ReactionAdded(crate::events::ReactionAddedEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.2500".to_owned(),
                thread_ts: None,
                reactor_user_id: "U9".to_owned(),
                reaction: "👍".to_owned(),
                quote_id: Some("Q-2026-0043".to_owned()),
                approval_type: "discount".to_owned(),
            }),
        };

        let (quote_id, thread_id) = super::correlation_fields(&envelope);
        assert_eq!(quote_id.as_deref(), Some("Q-2026-0043"));
        assert_eq!(thread_id.as_deref(), Some("1730000000.2500"));
    }

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.3.1)
    #[test]
    fn extracts_quote_and_thread_correlation_fields_from_block_actions() {
        let envelope = SlackEnvelope {
            envelope_id: "env-5".to_owned(),
            event: SlackEvent::BlockAction(crate::events::BlockActionEvent {
                channel_id: "C1".to_owned(),
                message_ts: "1730000000.3000".to_owned(),
                thread_ts: Some("1730000000.1000".to_owned()),
                user_id: "U10".to_owned(),
                action_id: "quote.refresh.v1".to_owned(),
                value: Some("quote=Q-2026-0050".to_owned()),
                quote_id: None,
                request_id: Some("req-block".to_owned()),
            }),
        };

        let (quote_id, thread_id) = super::correlation_fields(&envelope);
        assert_eq!(quote_id.as_deref(), Some("Q-2026-0050"));
        assert_eq!(thread_id.as_deref(), Some("1730000000.1000"));
    }

    #[test]
    fn extracts_lowercase_quote_id_from_slash_command_text() {
        let envelope = SlackEnvelope {
            envelope_id: "env-6".to_owned(),
            event: SlackEvent::SlashCommand(crate::commands::SlashCommandPayload {
                command: "/quote".to_owned(),
                text: "status q-2026-0042".to_owned(),
                channel_id: "C1".to_owned(),
                user_id: "U10".to_owned(),
                trigger_ts: "1730000000.3000".to_owned(),
                request_id: "req-6".to_owned(),
            }),
        };

        let (quote_id, _) = super::correlation_fields(&envelope);
        assert_eq!(quote_id.as_deref(), Some("Q-2026-0042"));
    }
}
