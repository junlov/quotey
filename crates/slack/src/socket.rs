use std::{sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use thiserror::Error;
use tracing::{debug, info, warn};

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
}

pub struct SocketModeRunner {
    transport: Arc<dyn SocketTransport>,
    dispatcher: EventDispatcher,
    reconnect_policy: ReconnectPolicy,
}

impl Default for SocketModeRunner {
    fn default() -> Self {
        Self {
            transport: Arc::new(NoopSocketTransport),
            dispatcher: default_dispatcher(),
            reconnect_policy: ReconnectPolicy::default(),
        }
    }
}

impl SocketModeRunner {
    pub fn new(
        transport: Arc<dyn SocketTransport>,
        dispatcher: EventDispatcher,
        reconnect_policy: ReconnectPolicy,
    ) -> Self {
        Self { transport, dispatcher, reconnect_policy }
    }

    pub async fn start(&self) -> Result<()> {
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
                            "socket mode retries exhausted; continuing process without crash"
                        );
                        return Ok(());
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
            (quote_id_from_text(&event.text).map(str::to_owned), Some(event.thread_ts.clone()))
        }
        SlackEvent::SlashCommand(payload) => {
            (quote_id_from_text(&payload.text).map(str::to_owned), None)
        }
        SlackEvent::Unsupported { .. } => (None, None),
    }
}

fn quote_id_from_text(text: &str) -> Option<&str> {
    text.split_whitespace().find_map(|token| {
        let candidate = token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-');
        let bytes = candidate.as_bytes();
        if bytes.len() == 11
            && candidate.starts_with("Q-")
            && bytes[2..6].iter().all(u8::is_ascii_digit)
            && bytes[6] == b'-'
            && bytes[7..11].iter().all(u8::is_ascii_digit)
        {
            Some(candidate)
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use super::{ReconnectPolicy, SocketModeRunner, SocketTransport, TransportError};
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

        runner.start().await.expect("runner should degrade gracefully");
        assert_eq!(transport.connect_attempts().await, 3);
    }

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
}
