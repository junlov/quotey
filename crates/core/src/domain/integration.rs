//! Pluggable integration adapter configuration.
//!
//! Each row in `integration_config` represents a registered adapter instance
//! (e.g. "salesforce CRM" or "builtin PDF generator") that the outbox worker
//! can dispatch to.  Adapter logic lives behind the `IntegrationAdapter` trait
//! in `crate::services::integration_adapter`.

use chrono::{DateTime, Utc};

// ---------------------------------------------------------------------------
// Integration type — broad category
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationType {
    Crm,
    Notification,
    Pdf,
    Erp,
}

impl IntegrationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Crm => "crm",
            Self::Notification => "notification",
            Self::Pdf => "pdf",
            Self::Erp => "erp",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "crm" => Some(Self::Crm),
            "notification" => Some(Self::Notification),
            "pdf" => Some(Self::Pdf),
            "erp" => Some(Self::Erp),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Adapter type — concrete provider within a category
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterType {
    Salesforce,
    Hubspot,
    Slack,
    Teams,
    Email,
    Webhook,
    Builtin,
    Docusign,
    Netsuite,
    None,
}

impl AdapterType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Salesforce => "salesforce",
            Self::Hubspot => "hubspot",
            Self::Slack => "slack",
            Self::Teams => "teams",
            Self::Email => "email",
            Self::Webhook => "webhook",
            Self::Builtin => "builtin",
            Self::Docusign => "docusign",
            Self::Netsuite => "netsuite",
            Self::None => "none",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "salesforce" => Some(Self::Salesforce),
            "hubspot" => Some(Self::Hubspot),
            "slack" => Some(Self::Slack),
            "teams" => Some(Self::Teams),
            "email" => Some(Self::Email),
            "webhook" => Some(Self::Webhook),
            "builtin" => Some(Self::Builtin),
            "docusign" => Some(Self::Docusign),
            "netsuite" => Some(Self::Netsuite),
            "none" => Some(Self::None),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Adapter status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterStatus {
    Active,
    Inactive,
    Error,
}

impl AdapterStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
            Self::Error => "error",
        }
    }

    pub fn parse_label(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "inactive" => Some(Self::Inactive),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Integration config entity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct IntegrationConfig {
    pub id: String,
    pub integration_type: IntegrationType,
    pub adapter_type: AdapterType,
    pub name: String,
    pub adapter_config: String,
    pub status: AdapterStatus,
    pub status_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Adapter test result
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AdapterTestResult {
    pub ok: bool,
    pub latency_ms: u64,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integration_type_round_trips() {
        for ty in [
            IntegrationType::Crm,
            IntegrationType::Notification,
            IntegrationType::Pdf,
            IntegrationType::Erp,
        ] {
            let s = ty.as_str();
            assert_eq!(IntegrationType::parse_label(s), Some(ty));
        }
        assert_eq!(IntegrationType::parse_label("unknown"), None);
    }

    #[test]
    fn adapter_type_round_trips() {
        for at in [
            AdapterType::Salesforce,
            AdapterType::Hubspot,
            AdapterType::Slack,
            AdapterType::Teams,
            AdapterType::Email,
            AdapterType::Webhook,
            AdapterType::Builtin,
            AdapterType::Docusign,
            AdapterType::Netsuite,
            AdapterType::None,
        ] {
            let s = at.as_str();
            assert_eq!(AdapterType::parse_label(s), Some(at));
        }
        assert_eq!(AdapterType::parse_label("unknown"), None);
    }

    #[test]
    fn adapter_status_round_trips() {
        for st in [AdapterStatus::Active, AdapterStatus::Inactive, AdapterStatus::Error] {
            let s = st.as_str();
            assert_eq!(AdapterStatus::parse_label(s), Some(st));
        }
        assert_eq!(AdapterStatus::parse_label("unknown"), None);
    }
}
