use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::quote::QuoteId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditCategory {
    Ingress,
    Flow,
    Pricing,
    Policy,
    Persistence,
    System,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditOutcome {
    Success,
    Rejected,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditContext {
    pub quote_id: Option<QuoteId>,
    pub thread_id: Option<String>,
    pub correlation_id: String,
    pub actor: String,
}

impl AuditContext {
    pub fn new(
        quote_id: Option<QuoteId>,
        thread_id: Option<String>,
        correlation_id: impl Into<String>,
        actor: impl Into<String>,
    ) -> Self {
        Self { quote_id, thread_id, correlation_id: correlation_id.into(), actor: actor.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: String,
    pub quote_id: Option<QuoteId>,
    pub thread_id: Option<String>,
    pub correlation_id: String,
    pub event_type: String,
    pub category: AuditCategory,
    pub actor: String,
    pub outcome: AuditOutcome,
    pub metadata: BTreeMap<String, String>,
    pub occurred_at: DateTime<Utc>,
}

impl AuditEvent {
    pub fn new(
        quote_id: Option<QuoteId>,
        thread_id: Option<String>,
        correlation_id: impl Into<String>,
        event_type: impl Into<String>,
        category: AuditCategory,
        actor: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            quote_id,
            thread_id,
            correlation_id: correlation_id.into(),
            event_type: event_type.into(),
            category,
            actor: actor.into(),
            outcome,
            metadata: BTreeMap::new(),
            occurred_at: Utc::now(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

pub trait AuditSink: Send + Sync {
    fn emit(&self, event: AuditEvent);
}

#[derive(Clone, Default)]
pub struct InMemoryAuditSink {
    events: Arc<Mutex<Vec<AuditEvent>>>,
}

impl InMemoryAuditSink {
    pub fn events(&self) -> Vec<AuditEvent> {
        match self.events.lock() {
            Ok(events) => events.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl AuditSink for InMemoryAuditSink {
    fn emit(&self, event: AuditEvent) {
        match self.events.lock() {
            Ok(mut events) => events.push(event),
            Err(poisoned) => poisoned.into_inner().push(event),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        audit::{AuditCategory, AuditEvent, AuditOutcome, AuditSink, InMemoryAuditSink},
        domain::quote::QuoteId,
    };

    /// qa-tag: fake-in-memory-critical-path (bd-3vp2.1)
    #[test]
    fn in_memory_sink_records_events_with_correlation_fields() {
        let sink = InMemoryAuditSink::default();
        sink.emit(
            AuditEvent::new(
                Some(QuoteId("Q-2026-0042".to_owned())),
                Some("1730000000.0001".to_owned()),
                "req-123",
                "flow.transition_applied",
                AuditCategory::Flow,
                "flow-engine",
                AuditOutcome::Success,
            )
            .with_metadata("from", "Draft")
            .with_metadata("to", "Validated"),
        );

        let events = sink.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].correlation_id, "req-123");
        assert_eq!(events[0].thread_id.as_deref(), Some("1730000000.0001"));
        assert_eq!(events[0].quote_id.as_ref().map(|id| id.0.as_str()), Some("Q-2026-0042"));
        assert!(events[0].metadata.contains_key("from"));
    }
}
