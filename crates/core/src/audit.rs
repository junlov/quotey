use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::quote::QuoteId;

// ---------------------------------------------------------------------------
// Actor type — who performed the action
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorType {
    Agent,
    User,
    System,
    Service,
    Webhook,
}

impl ActorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::User => "user",
            Self::System => "system",
            Self::Service => "service",
            Self::Webhook => "webhook",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "agent" => Some(Self::Agent),
            "user" => Some(Self::User),
            "system" => Some(Self::System),
            "service" => Some(Self::Service),
            "webhook" => Some(Self::Webhook),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Entity type — what entity was affected
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    Quote,
    QuoteLine,
    Approval,
    Negotiation,
    Product,
    SalesRep,
    Integration,
    OrgSetting,
    PortalLink,
    Comment,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quote => "quote",
            Self::QuoteLine => "quote_line",
            Self::Approval => "approval",
            Self::Negotiation => "negotiation",
            Self::Product => "product",
            Self::SalesRep => "sales_rep",
            Self::Integration => "integration",
            Self::OrgSetting => "org_setting",
            Self::PortalLink => "portal_link",
            Self::Comment => "comment",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "quote" => Some(Self::Quote),
            "quote_line" => Some(Self::QuoteLine),
            "approval" => Some(Self::Approval),
            "negotiation" => Some(Self::Negotiation),
            "product" => Some(Self::Product),
            "sales_rep" => Some(Self::SalesRep),
            "integration" => Some(Self::Integration),
            "org_setting" => Some(Self::OrgSetting),
            "portal_link" => Some(Self::PortalLink),
            "comment" => Some(Self::Comment),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Audit action — what was done
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditAction {
    Created,
    Updated,
    Deleted,
    Transitioned,
    Approved,
    Rejected,
    Escalated,
    Priced,
    Locked,
    Unlocked,
    Exported,
    Queried,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Updated => "updated",
            Self::Deleted => "deleted",
            Self::Transitioned => "transitioned",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Escalated => "escalated",
            Self::Priced => "priced",
            Self::Locked => "locked",
            Self::Unlocked => "unlocked",
            Self::Exported => "exported",
            Self::Queried => "queried",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "created" => Some(Self::Created),
            "updated" => Some(Self::Updated),
            "deleted" => Some(Self::Deleted),
            "transitioned" => Some(Self::Transitioned),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            "escalated" => Some(Self::Escalated),
            "priced" => Some(Self::Priced),
            "locked" => Some(Self::Locked),
            "unlocked" => Some(Self::Unlocked),
            "exported" => Some(Self::Exported),
            "queried" => Some(Self::Queried),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Audit category
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditCategory {
    Ingress,
    Flow,
    Pricing,
    Policy,
    Persistence,
    System,
    Funnel,
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
    pub actor_type: ActorType,
    pub outcome: AuditOutcome,
    pub entity_type: Option<EntityType>,
    pub entity_id: Option<String>,
    pub action: Option<AuditAction>,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
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
            actor_type: ActorType::Agent,
            outcome,
            entity_type: None,
            entity_id: None,
            action: None,
            before_json: None,
            after_json: None,
            metadata: BTreeMap::new(),
            occurred_at: Utc::now(),
        }
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    pub fn with_actor_type(mut self, actor_type: ActorType) -> Self {
        self.actor_type = actor_type;
        self
    }

    pub fn with_entity(mut self, entity_type: EntityType, entity_id: impl Into<String>) -> Self {
        self.entity_type = Some(entity_type);
        self.entity_id = Some(entity_id.into());
        self
    }

    pub fn with_action(mut self, action: AuditAction) -> Self {
        self.action = Some(action);
        self
    }

    pub fn with_before(mut self, before_json: impl Into<String>) -> Self {
        self.before_json = Some(before_json.into());
        self
    }

    pub fn with_after(mut self, after_json: impl Into<String>) -> Self {
        self.after_json = Some(after_json.into());
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

// ---------------------------------------------------------------------------
// Funnel telemetry – versioned UX event schema
// ---------------------------------------------------------------------------

/// UX funnel telemetry schema. Bump when funnel event structure changes.
pub const FUNNEL_SCHEMA_VERSION: &str = "funnel.v1";

/// Funnel event type constants for each major UX step.
///
/// These map 1:1 to the acceptance criteria:
/// - start, assumption, pricing render, resume, approval, comment
///
/// Each event carries `schema_version`, `funnel_step`, `funnel_ordinal`,
/// and `channel` metadata so drop-off can be measured per step.
pub mod funnel {
    /// User initiates a new quote flow (Slack command or portal action).
    pub const SESSION_START: &str = "funnel.session_start";
    /// User reviews / edits assumptions (tax, payment terms, billing country).
    pub const ASSUMPTION_REVIEW: &str = "funnel.assumption_review";
    /// Pricing is rendered to the user (quote viewer page or Slack block).
    pub const PRICING_RENDERED: &str = "funnel.pricing_rendered";
    /// User resumes a previously interrupted session.
    pub const SESSION_RESUMED: &str = "funnel.session_resumed";
    /// Approval or rejection action taken on a quote.
    pub const APPROVAL_ACTION: &str = "funnel.approval_action";
    /// Comment added to a quote (overall or per-line).
    pub const COMMENT_ADDED: &str = "funnel.comment_added";
    /// PDF download triggered from the portal.
    pub const PDF_DOWNLOAD: &str = "funnel.pdf_download";
    /// Session completed (quote sent or finalized).
    pub const SESSION_COMPLETED: &str = "funnel.session_completed";
    /// Session dropped (user abandoned or session expired without completion).
    pub const SESSION_DROPPED: &str = "funnel.session_dropped";

    /// Ordinal positions in the happy-path funnel.
    /// Used for measuring drop-off between consecutive steps.
    pub fn step_ordinal(event_type: &str) -> u8 {
        match event_type {
            SESSION_START => 1,
            ASSUMPTION_REVIEW => 2,
            PRICING_RENDERED => 3,
            APPROVAL_ACTION => 4,
            SESSION_COMPLETED => 5,
            // Non-linear steps get ordinal 0 (out-of-band)
            _ => 0,
        }
    }
}

impl AuditEvent {
    /// Build a funnel telemetry event with standard metadata.
    ///
    /// Every funnel event automatically includes:
    /// - `schema_version` – current funnel schema version
    /// - `funnel_step` – the event type constant
    /// - `funnel_ordinal` – numeric position in the happy-path funnel
    /// - `channel` – originating channel (`"portal"`, `"slack"`, `"mcp"`, `"cli"`)
    pub fn funnel(
        event_type: &str,
        channel: &str,
        quote_id: Option<QuoteId>,
        thread_id: Option<String>,
        correlation_id: impl Into<String>,
        actor: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self::new(
            quote_id,
            thread_id,
            correlation_id,
            event_type,
            AuditCategory::Funnel,
            actor,
            outcome,
        )
        .with_metadata("schema_version", FUNNEL_SCHEMA_VERSION)
        .with_metadata("funnel_step", event_type)
        .with_metadata("funnel_ordinal", funnel::step_ordinal(event_type).to_string())
        .with_metadata("channel", channel)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        audit::{
            funnel, AuditCategory, AuditEvent, AuditOutcome, AuditSink, InMemoryAuditSink,
            FUNNEL_SCHEMA_VERSION,
        },
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

    #[test]
    fn funnel_event_carries_versioned_metadata() {
        let event = AuditEvent::funnel(
            funnel::SESSION_START,
            "portal",
            Some(QuoteId("Q-2026-0100".to_owned())),
            None,
            "corr-001",
            "customer@example.com",
            AuditOutcome::Success,
        );

        assert_eq!(event.category, AuditCategory::Funnel);
        assert_eq!(event.event_type, funnel::SESSION_START);
        assert_eq!(event.metadata.get("schema_version").unwrap(), FUNNEL_SCHEMA_VERSION);
        assert_eq!(event.metadata.get("funnel_step").unwrap(), funnel::SESSION_START);
        assert_eq!(event.metadata.get("funnel_ordinal").unwrap(), "1");
        assert_eq!(event.metadata.get("channel").unwrap(), "portal");
    }

    #[test]
    fn funnel_ordinals_match_happy_path_sequence() {
        assert_eq!(funnel::step_ordinal(funnel::SESSION_START), 1);
        assert_eq!(funnel::step_ordinal(funnel::ASSUMPTION_REVIEW), 2);
        assert_eq!(funnel::step_ordinal(funnel::PRICING_RENDERED), 3);
        assert_eq!(funnel::step_ordinal(funnel::APPROVAL_ACTION), 4);
        assert_eq!(funnel::step_ordinal(funnel::SESSION_COMPLETED), 5);
        // Non-linear events get ordinal 0
        assert_eq!(funnel::step_ordinal(funnel::COMMENT_ADDED), 0);
        assert_eq!(funnel::step_ordinal(funnel::SESSION_RESUMED), 0);
        assert_eq!(funnel::step_ordinal(funnel::PDF_DOWNLOAD), 0);
    }

    #[test]
    fn funnel_event_additional_metadata_preserved() {
        let event = AuditEvent::funnel(
            funnel::APPROVAL_ACTION,
            "slack",
            None,
            Some("thread-42".to_owned()),
            "corr-002",
            "manager@example.com",
            AuditOutcome::Success,
        )
        .with_metadata("action", "approved")
        .with_metadata("session_id", "sess-abc");

        assert_eq!(event.metadata.get("action").unwrap(), "approved");
        assert_eq!(event.metadata.get("session_id").unwrap(), "sess-abc");
        // Standard funnel metadata still present
        assert!(event.metadata.contains_key("schema_version"));
        assert!(event.metadata.contains_key("channel"));
    }

    #[test]
    fn funnel_events_roundtrip_through_sink() {
        let sink = InMemoryAuditSink::default();

        // Emit a full funnel sequence
        for event_type in [
            funnel::SESSION_START,
            funnel::ASSUMPTION_REVIEW,
            funnel::PRICING_RENDERED,
            funnel::APPROVAL_ACTION,
            funnel::SESSION_COMPLETED,
        ] {
            sink.emit(AuditEvent::funnel(
                event_type,
                "portal",
                Some(QuoteId("Q-2026-0200".to_owned())),
                None,
                "corr-funnel",
                "rep@example.com",
                AuditOutcome::Success,
            ));
        }

        let events = sink.events();
        assert_eq!(events.len(), 5);
        // Verify monotonically increasing ordinals for happy-path events
        let ordinals: Vec<u8> = events
            .iter()
            .map(|e| e.metadata.get("funnel_ordinal").unwrap().parse::<u8>().unwrap())
            .collect();
        assert_eq!(ordinals, vec![1, 2, 3, 4, 5]);
    }

    // -----------------------------------------------------------------------
    // PC-4.1 enrichment tests
    // -----------------------------------------------------------------------

    #[test]
    fn actor_type_round_trips() {
        use super::ActorType;
        for at in [
            ActorType::Agent,
            ActorType::User,
            ActorType::System,
            ActorType::Service,
            ActorType::Webhook,
        ] {
            assert_eq!(ActorType::parse_label(at.as_str()), Some(at));
        }
        assert_eq!(ActorType::parse_label("unknown"), None);
    }

    #[test]
    fn entity_type_round_trips() {
        use super::EntityType;
        for et in [
            EntityType::Quote,
            EntityType::QuoteLine,
            EntityType::Approval,
            EntityType::Negotiation,
            EntityType::Product,
            EntityType::SalesRep,
            EntityType::Integration,
            EntityType::OrgSetting,
            EntityType::PortalLink,
            EntityType::Comment,
        ] {
            assert_eq!(EntityType::parse_label(et.as_str()), Some(et));
        }
        assert_eq!(EntityType::parse_label("unknown"), None);
    }

    #[test]
    fn audit_action_round_trips() {
        use super::AuditAction;
        for a in [
            AuditAction::Created,
            AuditAction::Updated,
            AuditAction::Deleted,
            AuditAction::Transitioned,
            AuditAction::Approved,
            AuditAction::Rejected,
            AuditAction::Escalated,
            AuditAction::Priced,
            AuditAction::Locked,
            AuditAction::Unlocked,
            AuditAction::Exported,
            AuditAction::Queried,
        ] {
            assert_eq!(AuditAction::parse_label(a.as_str()), Some(a));
        }
        assert_eq!(AuditAction::parse_label("unknown"), None);
    }

    #[test]
    fn enriched_event_carries_entity_and_action() {
        use super::{ActorType, AuditAction, EntityType};

        let event = AuditEvent::new(
            Some(QuoteId("Q-2026-0300".to_owned())),
            None,
            "corr-enrich",
            "quote.status_changed",
            AuditCategory::Flow,
            "user@example.com",
            AuditOutcome::Success,
        )
        .with_actor_type(ActorType::User)
        .with_entity(EntityType::Quote, "Q-2026-0300")
        .with_action(AuditAction::Transitioned)
        .with_before(r#"{"status":"draft"}"#)
        .with_after(r#"{"status":"validated"}"#);

        assert_eq!(event.actor_type, ActorType::User);
        assert_eq!(event.entity_type, Some(EntityType::Quote));
        assert_eq!(event.entity_id.as_deref(), Some("Q-2026-0300"));
        assert_eq!(event.action, Some(AuditAction::Transitioned));
        assert_eq!(event.before_json.as_deref(), Some(r#"{"status":"draft"}"#));
        assert_eq!(event.after_json.as_deref(), Some(r#"{"status":"validated"}"#));
    }

    #[test]
    fn default_actor_type_is_agent() {
        let event = AuditEvent::new(
            None,
            None,
            "c",
            "test",
            AuditCategory::System,
            "sys",
            AuditOutcome::Success,
        );
        assert_eq!(event.actor_type, super::ActorType::Agent);
        assert!(event.entity_type.is_none());
        assert!(event.action.is_none());
        assert!(event.before_json.is_none());
        assert!(event.after_json.is_none());
    }
}
