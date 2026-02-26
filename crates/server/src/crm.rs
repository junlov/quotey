//! CRM integration endpoints and data access helpers.
//!
//! Current implementation provides:
//! - OAuth start + callback wiring for Salesforce and HubSpot
//! - provider connection upsert and status query
//! - quote → CRM sync event capture and replay
//! - CRM → quote inbound webhook ingest
//! - field mapping configuration APIs
//! - sync history + retry support
//!
//! This implementation intentionally focuses on deterministic auditability and
//! a safe, debuggable API surface before full bidirectional remote writes.

use std::collections::{HashMap, HashSet};

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Duration, Utc};
use quotey_core::config::CrmConfig;
use quotey_core::{
    DeterministicExecutionEngine, ExecutionEngineConfig, ExecutionError, ExecutionTask,
    ExecutionTaskId, ExecutionTaskState, ExecutionTransitionEvent, IdempotencyRecord,
    IdempotencyRecordState, OperationKey, QuoteId, RetryPolicy,
};
use quotey_db::repositories::{
    ExecutionQueueRepository, IdempotencyRepository, RepositoryError, SqlExecutionQueueRepository,
};
use quotey_db::DbPool;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use tracing::{error, info, warn};
use uuid::Uuid;

const MAX_SYNC_ATTEMPTS: i32 = 5;
const CRM_SYNC_BASE_RETRY_DELAY_SECONDS: i64 = 30;
const CRM_SYNC_MAX_RETRY_DELAY_SECONDS: i64 = 3600;
const CRM_SYNC_OPERATION_KIND: &str = "crm.quote_sync";
const CRM_SYNC_WORKER_ID: &str = "crm-worker";

#[derive(Clone, Debug)]
struct CrmRuntimeConfig {
    enabled: bool,
    webhook_secret: Option<String>,
    callback_base_url: Option<String>,
    salesforce_client_id: Option<String>,
    salesforce_client_secret: Option<String>,
    hubspot_client_id: Option<String>,
    hubspot_client_secret: Option<String>,
}

impl From<&CrmConfig> for CrmRuntimeConfig {
    fn from(config: &CrmConfig) -> Self {
        Self {
            enabled: config.enabled,
            webhook_secret: config.webhook_secret.clone(),
            callback_base_url: config.callback_base_url.clone(),
            salesforce_client_id: config.salesforce_client_id.clone(),
            salesforce_client_secret: config.salesforce_client_secret.clone(),
            hubspot_client_id: config.hubspot_client_id.clone(),
            hubspot_client_secret: config.hubspot_client_secret.clone(),
        }
    }
}

#[derive(Clone)]
pub struct CrmState {
    db_pool: DbPool,
    config: CrmRuntimeConfig,
    client: Client,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CrmProvider {
    Salesforce,
    Hubspot,
}

impl std::fmt::Display for CrmProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl CrmProvider {
    fn parse(raw: &str) -> Option<Self> {
        match raw.to_lowercase().as_str() {
            "salesforce" => Some(Self::Salesforce),
            "hubspot" => Some(Self::Hubspot),
            _ => None,
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Self::Salesforce => "salesforce",
            Self::Hubspot => "hubspot",
        }
    }

    fn credentials(self, cfg: &CrmRuntimeConfig) -> Option<(&str, &str)> {
        match self {
            Self::Salesforce => match (&cfg.salesforce_client_id, &cfg.salesforce_client_secret) {
                (Some(id), Some(secret)) => Some((id.as_str(), secret.as_str())),
                _ => None,
            },
            Self::Hubspot => match (&cfg.hubspot_client_id, &cfg.hubspot_client_secret) {
                (Some(id), Some(secret)) => Some((id.as_str(), secret.as_str())),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CrmDirection {
    QuoteyToCrm,
    CrmToQuotey,
}

impl CrmDirection {
    fn as_str(&self) -> &'static str {
        match self {
            Self::QuoteyToCrm => "quotey_to_crm",
            Self::CrmToQuotey => "crm_to_quotey",
        }
    }
}

fn to_direction(raw: &str) -> Option<CrmDirection> {
    let normalized = raw.trim().to_ascii_lowercase().replace('-', "_").replace(' ', "_");
    match normalized.as_str() {
        "quotey_to_crm" => Some(CrmDirection::QuoteyToCrm),
        "crm_to_quotey" => Some(CrmDirection::CrmToQuotey),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
struct CrmError {
    error: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProviderConfig {
    provider: &'static str,
    authorize_url: &'static str,
    token_url: &'static str,
    default_scope: &'static str,
}

impl ProviderConfig {
    fn from_provider(provider: CrmProvider) -> Self {
        match provider {
            CrmProvider::Salesforce => Self {
                provider: "salesforce",
                authorize_url: "https://login.salesforce.com/services/oauth2/authorize",
                token_url: "https://login.salesforce.com/services/oauth2/token",
                default_scope: "api refresh_token",
            },
            CrmProvider::Hubspot => Self {
                provider: "hubspot",
                authorize_url: "https://app.hubspot.com/oauth/authorize",
                token_url: "https://api.hubapi.com/oauth/v1/token",
                default_scope: "crm.objects.line_items.read crm.objects.line_items.write crm.objects.contacts.read crm.objects.contacts.write",
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct OAuthConnectResponse {
    provider: String,
    authorization_url: String,
    state_token: String,
    state_expires_at: String,
}

#[derive(Debug, Deserialize)]
struct OAuthConnectRequest {
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OAuthCallbackQuery {
    state: String,
    code: Option<String>,
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct ConnectionResponse {
    provider: String,
    status: String,
    crm_account_id: Option<String>,
    crm_object_id: Option<String>,
    connected: bool,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct SyncEventPayloadRequest {
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    provider: Option<String>,
}

#[derive(Debug, Serialize)]
struct SyncResultResponse {
    quote_id: String,
    provider_results: Vec<SyncAttempt>,
}

#[derive(Debug, Serialize)]
struct SyncAttempt {
    provider: String,
    event_id: String,
    status: String,
    message: String,
    attempts: i32,
}

#[derive(Debug, Serialize)]
struct MappingResponse {
    id: String,
    provider: String,
    direction: String,
    quotey_field: String,
    crm_field: String,
    description: Option<String>,
    expression: Option<String>,
    is_active: bool,
}

#[derive(Debug, Serialize)]
struct MappingCatalogItem {
    quotey_field: String,
    aliases: Vec<String>,
    description: String,
    supported_directions: Vec<String>,
    examples: Vec<MappingCatalogExample>,
}

#[derive(Debug, Serialize)]
struct MappingCatalogExample {
    provider: String,
    direction: String,
    crm_object: String,
    crm_field: String,
}

#[derive(Debug, Serialize)]
struct MappingCatalogItemList {
    provider: Option<String>,
    direction: Option<String>,
    fields: Vec<MappingCatalogItem>,
}

#[derive(Debug, Clone, Copy)]
struct MappingCatalogTemplateExample {
    provider: CrmProvider,
    direction: CrmDirection,
    crm_object: &'static str,
    crm_field: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct MappingCatalogTemplate {
    quotey_field: &'static str,
    aliases: &'static [&'static str],
    description: &'static str,
    supports_quotey_to_crm: bool,
    supports_crm_to_quotey: bool,
    examples: &'static [MappingCatalogTemplateExample],
}

impl MappingCatalogTemplate {
    fn supports(self, direction: CrmDirection) -> bool {
        match direction {
            CrmDirection::QuoteyToCrm => self.supports_quotey_to_crm,
            CrmDirection::CrmToQuotey => self.supports_crm_to_quotey,
        }
    }
}

const MAPPING_FIELD_CATALOG: &[MappingCatalogTemplate] = &[
    MappingCatalogTemplate {
        quotey_field: "quote_id",
        aliases: &["quote_id", "quote.id", "id", "quotey.quote_id"],
        description: "Primary quote identifier used to reconcile CRM sync records.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: true,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "Id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Opportunity",
                crm_field: "Id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Deal",
                crm_field: "id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Deal",
                crm_field: "id",
            },
        ],
    },
    MappingCatalogTemplate {
        quotey_field: "account_id",
        aliases: &["account_id", "account.id", "accountid", "company_id", "cust_id"],
        description: "Customer/account identifier used for CRM account reconciliation.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: true,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "AccountId",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Account",
                crm_field: "Id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Company",
                crm_field: "id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Company",
                crm_field: "id",
            },
        ],
    },
    MappingCatalogTemplate {
        quotey_field: "deal_id",
        aliases: &["deal_id", "deal.id", "dealid", "opportunity_id", "oppty_id"],
        description: "CRM deal/opportunity identifier linked to the quote.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: true,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "Id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Opportunity",
                crm_field: "Id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Deal",
                crm_field: "id",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Deal",
                crm_field: "id",
            },
        ],
    },
    MappingCatalogTemplate {
        quotey_field: "status",
        aliases: &["status", "state", "stage", "quote.status", "quote.status_name"],
        description: "Canonical quote lifecycle status.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: true,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "StageName",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Opportunity",
                crm_field: "StageName",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Deal",
                crm_field: "dealstage",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Deal",
                crm_field: "dealstage",
            },
        ],
    },
    MappingCatalogTemplate {
        quotey_field: "notes",
        aliases: &["notes", "note", "comments", "comment", "memo"],
        description: "Free-form notes attached to the quote.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: true,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "Description",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Opportunity",
                crm_field: "Description",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Deal",
                crm_field: "description",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::CrmToQuotey,
                crm_object: "Deal",
                crm_field: "description",
            },
        ],
    },
    MappingCatalogTemplate {
        quotey_field: "currency",
        aliases: &["currency", "currency_code", "currencycode", "cur"],
        description: "Quote currency ISO code for monetary fields.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: false,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "CurrencyIsoCode",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Deal",
                crm_field: "hscurrency",
            },
        ],
    },
    MappingCatalogTemplate {
        quotey_field: "total_amount",
        aliases: &[
            "total_amount",
            "total",
            "quote.total",
            "quote.total_amount",
            "opportunity_amount",
            "amount_total",
        ],
        description: "Computed quote total amount.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: false,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "Amount",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Deal",
                crm_field: "amount",
            },
        ],
    },
    MappingCatalogTemplate {
        quotey_field: "term_months",
        aliases: &["term_months", "term", "term.months", "termmonths"],
        description: "Quote term in months.",
        supports_quotey_to_crm: true,
        supports_crm_to_quotey: false,
        examples: &[
            MappingCatalogTemplateExample {
                provider: CrmProvider::Salesforce,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Opportunity",
                crm_field: "Term_Months__c",
            },
            MappingCatalogTemplateExample {
                provider: CrmProvider::Hubspot,
                direction: CrmDirection::QuoteyToCrm,
                crm_object: "Deal",
                crm_field: "term_months",
            },
        ],
    },
];

fn mapping_catalog(
    provider_filter: Option<CrmProvider>,
    direction_filter: Option<CrmDirection>,
) -> Vec<MappingCatalogItem> {
    MAPPING_FIELD_CATALOG
        .iter()
        .filter(|entry| {
            if let Some(direction) = direction_filter {
                entry.supports(direction)
            } else {
                entry.supports_quotey_to_crm || entry.supports_crm_to_quotey
            }
        })
        .filter_map(|entry| {
            let mut examples = Vec::new();
            for example in entry.examples {
                if let Some(provider) = provider_filter {
                    if example.provider != provider {
                        continue;
                    }
                }
                if let Some(direction) = direction_filter {
                    if example.direction != direction {
                        continue;
                    }
                }
                examples.push(MappingCatalogExample {
                    provider: example.provider.as_str().to_string(),
                    direction: example.direction.as_str().to_string(),
                    crm_object: example.crm_object.to_string(),
                    crm_field: example.crm_field.to_string(),
                });
            }
            if examples.is_empty() {
                return None;
            }

            let mut supported_directions = Vec::new();
            if entry.supports_quotey_to_crm {
                supported_directions.push(CrmDirection::QuoteyToCrm.as_str().to_string());
            }
            if entry.supports_crm_to_quotey {
                supported_directions.push(CrmDirection::CrmToQuotey.as_str().to_string());
            }

            Some(MappingCatalogItem {
                quotey_field: entry.quotey_field.to_string(),
                aliases: entry.aliases.iter().map(|alias| (*alias).to_string()).collect(),
                description: entry.description.to_string(),
                supported_directions,
                examples,
            })
        })
        .collect()
}

fn normalize_quotey_field_key(raw: &str) -> String {
    let normalized = raw.trim().to_ascii_lowercase().replace(' ', "_").replace('-', "_");
    normalized.trim_start_matches("quotey.").trim_start_matches("quote.").to_string()
}

fn normalize_quotey_field_for_direction(raw: &str, direction: CrmDirection) -> String {
    let candidate = normalize_quotey_field_key(raw);
    if candidate.is_empty() {
        return String::new();
    }

    for template in MAPPING_FIELD_CATALOG {
        if !template.supports(direction) {
            continue;
        }
        if template.quotey_field == candidate {
            return candidate;
        }
        if template.aliases.iter().any(|alias| normalize_quotey_field_key(alias) == candidate) {
            return template.quotey_field.to_string();
        }
    }

    candidate
}

fn is_supported_quotey_field(quotey_field: &str, direction: CrmDirection) -> bool {
    let normalized = normalize_quotey_field_for_direction(quotey_field, direction);
    MAPPING_FIELD_CATALOG
        .iter()
        .any(|entry| entry.supports(direction) && entry.quotey_field == normalized)
}

fn normalize_and_validate_quotey_field(
    raw_quotey_field: &str,
    direction: CrmDirection,
) -> Option<String> {
    let normalized = normalize_quotey_field_for_direction(raw_quotey_field, direction);
    if normalized.is_empty() || !is_supported_quotey_field(&normalized, direction) {
        None
    } else {
        Some(normalized)
    }
}

fn supported_quotey_field_names(direction: CrmDirection) -> Vec<String> {
    MAPPING_FIELD_CATALOG
        .iter()
        .filter(|entry| entry.supports(direction))
        .map(|entry| entry.quotey_field.to_string())
        .collect()
}

#[derive(Debug, Deserialize)]
struct UpsertMappingItem {
    id: Option<String>,
    quotey_field: String,
    crm_field: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    expression: Option<String>,
    #[serde(default)]
    is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpsertMappingsRequest {
    provider: String,
    direction: String,
    mappings: Vec<UpsertMappingItem>,
}

#[derive(Debug, Serialize)]
struct SyncEventResponse {
    id: String,
    provider: String,
    direction: String,
    event_type: String,
    quote_id: Option<String>,
    crm_object_type: Option<String>,
    crm_object_id: Option<String>,
    status: String,
    attempts: i32,
    error_message: Option<String>,
    next_retry_in_seconds: Option<i64>,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct SyncEventStatsResponse {
    total_events: i64,
    by_status: HashMap<String, i64>,
    by_provider: HashMap<String, i64>,
    by_direction: HashMap<String, i64>,
    failed_last_24h: i64,
    success_last_24h: i64,
}

#[derive(Debug, Deserialize)]
struct SyncEventsQuery {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    quote_id: Option<String>,
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebhookPayload {
    #[serde(default)]
    quote_id: Option<String>,
    #[serde(default)]
    account_id: Option<String>,
    #[serde(default)]
    deal_id: Option<String>,
    #[serde(default)]
    stage: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    contact_id: Option<String>,
    #[serde(default)]
    contact_email: Option<String>,
    #[serde(default)]
    contact_name: Option<String>,
    #[serde(default)]
    event_type: Option<String>,
    #[serde(flatten)]
    payload: HashMap<String, Value>,
}

#[derive(Debug, Default)]
struct InboundQuoteUpdate {
    quote_id: Option<String>,
    account_id: Option<String>,
    deal_id: Option<String>,
    status: Option<String>,
    notes: Option<String>,
}

impl InboundQuoteUpdate {
    fn from_payload(payload: &WebhookPayload) -> Self {
        let mut quote_id = payload.quote_id.clone();
        let mut notes = payload.notes.clone();
        if notes.is_none() {
            notes = build_contact_snapshot_note(payload);
        }
        Self {
            quote_id: quote_id.take().filter(|value| !value.trim().is_empty()),
            account_id: payload.account_id.clone(),
            deal_id: payload.deal_id.clone(),
            status: payload.status.clone().or_else(|| payload.stage.clone()),
            notes,
        }
    }

    fn is_empty(&self) -> bool {
        self.quote_id.is_none()
            && self.account_id.is_none()
            && self.deal_id.is_none()
            && self.status.is_none()
            && self.notes.is_none()
    }
}

fn infer_crm_object_identity(
    quote_id: Option<&str>,
    account_id: Option<&str>,
    deal_id: Option<&str>,
) -> (Option<String>, Option<String>) {
    if let Some(value) = quote_id.map(str::trim).filter(|value| !value.is_empty()) {
        return (Some("quote".to_string()), Some(value.to_string()));
    }
    if let Some(value) = deal_id.map(str::trim).filter(|value| !value.is_empty()) {
        return (Some("deal".to_string()), Some(value.to_string()));
    }
    if let Some(value) = account_id.map(str::trim).filter(|value| !value.is_empty()) {
        return (Some("account".to_string()), Some(value.to_string()));
    }
    (None, None)
}

fn webhook_string_field(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn normalize_webhook_payload(raw: &Value) -> WebhookPayload {
    let payload: HashMap<String, Value> = raw
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let quote_id = payload.get("quote_id").and_then(webhook_string_field);
    let account_id = payload.get("account_id").and_then(webhook_string_field);
    let deal_id = payload.get("deal_id").and_then(webhook_string_field);
    let stage = payload.get("stage").and_then(webhook_string_field);
    let status = payload.get("status").and_then(webhook_string_field);
    let notes = payload.get("notes").and_then(webhook_string_field);
    let contact_id = payload.get("contact_id").and_then(webhook_string_field);
    let contact_email = payload.get("contact_email").and_then(webhook_string_field);
    let contact_name = payload.get("contact_name").and_then(webhook_string_field);
    let event_type = payload.get("event_type").and_then(webhook_string_field);

    WebhookPayload {
        quote_id,
        account_id,
        deal_id,
        stage,
        status,
        notes,
        contact_id,
        contact_email,
        contact_name,
        event_type,
        payload,
    }
}

fn build_contact_snapshot_note(payload: &WebhookPayload) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(contact_name) =
        payload.contact_name.as_deref().filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("contact_added: {}", contact_name.trim()));
    }
    if let Some(contact_email) =
        payload.contact_email.as_deref().filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("contact_email: {}", contact_email.trim()));
    }
    if let Some(contact_id) = payload.contact_id.as_deref().filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("contact_id: {}", contact_id.trim()));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn webhook_to_mapping_source(payload: &WebhookPayload) -> Value {
    let mut source = serde_json::Map::new();
    for (key, value) in &payload.payload {
        source.insert(key.clone(), value.clone());
    }
    if let Some(quote_id) = payload.quote_id.as_deref().filter(|value| !value.trim().is_empty()) {
        source.insert("quote_id".to_string(), json!(quote_id));
    }
    if let Some(account_id) = payload.account_id.as_deref().filter(|value| !value.trim().is_empty())
    {
        source.insert("account_id".to_string(), json!(account_id));
    }
    if let Some(deal_id) = payload.deal_id.as_deref().filter(|value| !value.trim().is_empty()) {
        source.insert("deal_id".to_string(), json!(deal_id));
    }
    if let Some(stage) = payload.stage.as_deref().filter(|value| !value.trim().is_empty()) {
        source.insert("stage".to_string(), json!(stage));
    }
    if let Some(status) = payload.status.as_deref().filter(|value| !value.trim().is_empty()) {
        source.insert("status".to_string(), json!(status));
    }
    if let Some(notes) = payload.notes.as_deref().filter(|value| !value.trim().is_empty()) {
        source.insert("notes".to_string(), json!(notes));
    }
    if let Some(contact_name) =
        payload.contact_name.as_deref().filter(|value| !value.trim().is_empty())
    {
        source.insert("contact_name".to_string(), json!(contact_name));
    }
    if let Some(contact_email) =
        payload.contact_email.as_deref().filter(|value| !value.trim().is_empty())
    {
        source.insert("contact_email".to_string(), json!(contact_email));
    }
    if let Some(contact_id) = payload.contact_id.as_deref().filter(|value| !value.trim().is_empty())
    {
        source.insert("contact_id".to_string(), json!(contact_id));
    }

    Value::Object(source)
}

fn extract_inbound_quote_update(
    payload: &WebhookPayload,
    provider: CrmProvider,
    mappings: &[CrmFieldMapping],
) -> InboundQuoteUpdate {
    let mut updates = InboundQuoteUpdate::from_payload(payload);
    let source = webhook_to_mapping_source(payload);

    for mapping in mappings {
        if mapping.provider != provider || !mapping.is_active {
            continue;
        }

        let value = mapping
            .expression
            .as_deref()
            .and_then(|expression| extract_template_value(expression, &source))
            .or_else(|| {
                resolve_payload_value(&source, &mapping.crm_field).and_then(|v| value_to_text(&v))
            });

        let Some(value) = value else {
            continue;
        };

        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
            continue;
        }
        let value = trimmed.to_string();
        let quotey_field =
            normalize_quotey_field_for_direction(&mapping.quotey_field, CrmDirection::CrmToQuotey);
        if !is_supported_quotey_field(&quotey_field, CrmDirection::CrmToQuotey) {
            continue;
        }

        match quotey_field.as_str() {
            "quote_id" => {
                if updates.quote_id.is_none() {
                    updates.quote_id = Some(value);
                }
            }
            "account_id" => {
                if updates.account_id.is_none() {
                    updates.account_id = Some(value);
                }
            }
            "deal_id" => {
                if updates.deal_id.is_none() {
                    updates.deal_id = Some(value);
                }
            }
            "status" => {
                if updates.status.is_none() {
                    updates.status = Some(value);
                }
            }
            "notes" => {
                if updates.notes.is_none() {
                    updates.notes = Some(value);
                }
            }
            _ => {}
        }
    }

    updates
}

#[derive(Debug, Deserialize)]
struct SyncRetryPath {
    event_id: String,
}

#[derive(Debug, Serialize)]
struct CrmStatus {
    provider: String,
    status: String,
    crm_account_id: Option<String>,
    last_synced_at: Option<String>,
    last_error: Option<String>,
    has_tokens: bool,
}

#[derive(Debug, Serialize)]
struct CrmStatusPayload {
    enabled: bool,
    providers: Vec<CrmStatus>,
}

#[derive(Debug, Clone)]
struct CrmIntegration {
    provider: CrmProvider,
    status: String,
    access_token: String,
    refresh_token: Option<String>,
    token_type: String,
    instance_url: Option<String>,
    scope: Option<String>,
    token_expires_at: Option<String>,
    crm_account_id: Option<String>,
}

#[derive(Debug, Clone)]
struct CrmFieldMapping {
    provider: CrmProvider,
    id: String,
    quotey_field: String,
    crm_field: String,
    description: Option<String>,
    expression: Option<String>,
    is_active: bool,
}

pub fn router(db_pool: DbPool, config: CrmConfig) -> Router {
    let state =
        CrmState { db_pool, config: CrmRuntimeConfig::from(&config), client: Client::new() };

    Router::new()
        .route("/api/v1/crm/connect/{provider}", get(start_oauth))
        .route("/api/v1/crm/oauth/{provider}/callback", get(oauth_callback))
        .route("/api/v1/crm/sync/{quote_id}", post(sync_quote_to_crm))
        .route("/api/v1/crm/mappings/catalog", get(list_mapping_catalog))
        .route("/api/v1/crm/mappings", get(list_mappings).post(upsert_mappings))
        .route("/api/v1/crm/events", get(list_sync_events))
        .route("/api/v1/crm/events/stats", get(sync_event_stats))
        .route("/api/v1/crm/events/{event_id}/retry", post(retry_sync_event))
        .route("/api/v1/crm/status", get(crm_status))
        .route("/api/v1/crm/webhook/{provider}", post(webhook_ingest))
        .with_state(state)
}

async fn crm_state_guard(
    headers: &HeaderMap,
    state: &CrmState,
    provider: Option<CrmProvider>,
    require_webhook_secret: bool,
) -> Result<(), (StatusCode, Json<CrmError>)> {
    if !state.config.enabled {
        return Err((
            StatusCode::FORBIDDEN,
            Json(CrmError { error: "crm integration is disabled".to_string() }),
        ));
    }

    if let Some(provider) = provider {
        if provider.credentials(&state.config).is_none() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(CrmError {
                    error: format!(
                        "crm credentials not configured for provider `{}`",
                        provider.as_str()
                    ),
                }),
            ));
        }
    }

    if require_webhook_secret {
        if let Some(secret) = &state.config.webhook_secret {
            let provided =
                headers.get("x-quotey-webhook-secret").and_then(|value| value.to_str().ok());
            if let Some(value) = provided {
                if value != secret {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(CrmError { error: "invalid webhook secret".to_string() }),
                    ));
                }
            } else {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(CrmError { error: "missing webhook secret".to_string() }),
                ));
            }
        }
    }
    Ok(())
}

fn next_retry_delay_seconds(status: &str, attempts: i32) -> Option<i64> {
    if !matches!(status, "failed" | "retrying" | "skipped") {
        return None;
    }
    if attempts >= MAX_SYNC_ATTEMPTS {
        return None;
    }
    if attempts < 1 {
        return Some(CRM_SYNC_BASE_RETRY_DELAY_SECONDS);
    }
    let mut delay = CRM_SYNC_BASE_RETRY_DELAY_SECONDS;
    for _ in 1..attempts {
        delay = delay.saturating_mul(2);
    }
    Some(delay.min(CRM_SYNC_MAX_RETRY_DELAY_SECONDS))
}

#[derive(Debug, Clone, Copy)]
enum SyncAttemptResultKind {
    Success,
    RetryableFailure,
    TerminalFailure,
}

#[derive(Debug)]
struct SyncAttemptResult {
    kind: SyncAttemptResultKind,
    message: String,
    error_class: Option<String>,
}

fn crm_sync_max_retries() -> u32 {
    MAX_SYNC_ATTEMPTS.saturating_sub(1) as u32
}

fn crm_execution_engine_config() -> ExecutionEngineConfig {
    ExecutionEngineConfig {
        claim_timeout_seconds: 300,
        default_max_retries: crm_sync_max_retries(),
        retry_backoff_multiplier: 2,
        retry_base_delay_seconds: CRM_SYNC_BASE_RETRY_DELAY_SECONDS,
    }
}

fn crm_sync_queue_status(task_state: &ExecutionTaskState) -> &'static str {
    match task_state {
        ExecutionTaskState::Queued => "queued",
        ExecutionTaskState::Running => "running",
        ExecutionTaskState::RetryableFailed => "retrying",
        ExecutionTaskState::FailedTerminal => "failed",
        ExecutionTaskState::Completed => "success",
    }
}

fn sync_status_is_retryable(status: &str) -> bool {
    matches!(status, "failed" | "retrying" | "skipped")
}

fn classify_retry_policy(error: &str) -> RetryPolicy {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("token expired") && normalized.contains("refresh token") {
        RetryPolicy::Retry
    } else if normalized.contains("timeout") || normalized.contains("temporar") {
        RetryPolicy::Retry
    } else {
        RetryPolicy::FailTerminal
    }
}

fn repository_error(error: RepositoryError) -> (StatusCode, Json<CrmError>) {
    error!(error = %error, "crm execution repository error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(CrmError { error: "an internal repository error occurred".to_string() }),
    )
}

fn map_execution_error(error: ExecutionError) -> (StatusCode, Json<CrmError>) {
    match error {
        ExecutionError::TaskNotFound(task_id) => (
            StatusCode::NOT_FOUND,
            Json(CrmError { error: format!("execution task `{task_id}` not found") }),
        ),
        ExecutionError::ClaimConflict(task_id, actor) => (
            StatusCode::CONFLICT,
            Json(CrmError {
                error: format!("execution task `{task_id}` is already claimed by `{}`", actor),
            }),
        ),
        ExecutionError::TaskNotYetAvailable(task_id) => (
            StatusCode::CONFLICT,
            Json(CrmError {
                error: format!("execution task `{task_id}` is not yet available for retry"),
            }),
        ),
        ExecutionError::InvalidTransition { from, to, reason } => (
            StatusCode::CONFLICT,
            Json(CrmError {
                error: format!("invalid execution transition {:?} -> {:?}: {reason}", from, to),
            }),
        ),
        ExecutionError::IdempotencyConflict(op, state) => (
            StatusCode::CONFLICT,
            Json(CrmError { error: format!("idempotency conflict for `{op}` in state `{state}`") }),
        ),
    }
}

async fn start_oauth(
    Path(provider_raw): Path<String>,
    State(state): State<CrmState>,
    Query(payload): Query<OAuthConnectRequest>,
) -> Result<Json<OAuthConnectResponse>, (StatusCode, Json<CrmError>)> {
    let provider = parse_provider(&provider_raw)?;
    crm_state_guard(&HeaderMap::new(), &state, Some(provider), false).await?;

    let provider_config = ProviderConfig::from_provider(provider);
    let state_token = Uuid::new_v4().simple().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::minutes(10);
    let callback = callback_url(&state, &provider)?;
    let scope = payload
        .scope
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| provider_config.default_scope.to_string());
    let scope = normalize_scope(&scope);

    sqlx::query(
        "INSERT INTO crm_oauth_state (state_token, provider, redirect_uri, scope, requested_at, expires_at)\n             VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&state_token)
    .bind(provider.as_str())
    .bind(&callback)
    .bind(&scope)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    let authorization_url = format!(
        "{authorize}?response_type=code&client_id={client}&redirect_uri={redirect_uri}&scope={scope}&state={state}",
        authorize = provider_config.authorize_url,
        client = provider_credentials(&state, provider)?,
        redirect_uri = callback,
        scope = encode_query(&scope),
        state = encode_query(&state_token),
    );

    Ok(Json(OAuthConnectResponse {
        provider: provider.as_str().to_string(),
        authorization_url,
        state_token,
        state_expires_at: expires_at.to_rfc3339(),
    }))
}

async fn oauth_callback(
    Path(provider_raw): Path<String>,
    State(state): State<CrmState>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Result<Json<ConnectionResponse>, (StatusCode, Json<CrmError>)> {
    let provider = parse_provider(&provider_raw)?;
    if let Some(error) = query.error {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CrmError {
                error: format!(
                    "oauth provider returned error: {}",
                    error_description(query.error_description),
                ),
            }),
        ));
    }
    let code = query.code.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(CrmError { error: "authorization code missing".to_string() }),
        )
    })?;

    let state_row = fetch_and_reserve_oauth_state(&state, &query.state).await?;
    let provider_config = ProviderConfig::from_provider(provider);
    let (client_id, client_secret) = provider.credentials(&state.config).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(CrmError {
                error: format!(
                    "crm credentials not configured for provider `{}`",
                    provider.as_str()
                ),
            }),
        )
    })?;

    let token_request = TokenExchangeRequest {
        grant_type: "authorization_code",
        code: &code,
        client_id,
        client_secret,
        redirect_uri: &state_row.redirect_uri,
        scope: Some(&state_row.scope),
    };

    let token = exchange_token(&state, provider_config.token_url, token_request).await?;
    let discovered_account_id = discover_provider_account_id(
        &state,
        provider,
        token.token_type.as_deref().unwrap_or("Bearer"),
        &token.access_token,
    )
    .await;

    let token_expires_at = token.expires_in.and_then(|seconds| {
        Utc::now()
            .checked_add_signed(Duration::seconds(seconds.into()))
            .map(|value| value.to_rfc3339())
    });

    let row_id = format!("CRMINT-{}", Uuid::new_v4().simple());
    let status = "connected";
    let crm_account_id = discovered_account_id.or(token.crm_account_id.clone());

    sqlx::query(
        "INSERT INTO crm_integration (\n            id, provider, status, crm_account_id, instance_url,\n            access_token, refresh_token, token_type, scope,\n            token_expires_at, last_error, updated_at\n         )\n         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)\n         ON CONFLICT(provider) DO UPDATE SET\n            status = excluded.status,\n            crm_account_id = excluded.crm_account_id,\n            instance_url = excluded.instance_url,\n            access_token = excluded.access_token,\n            refresh_token = excluded.refresh_token,\n            token_type = excluded.token_type,\n            scope = excluded.scope,\n            token_expires_at = excluded.token_expires_at,\n            last_error = NULL,\n            updated_at = excluded.updated_at",
    )
    .bind(&row_id)
    .bind(provider.as_str())
    .bind(status)
    .bind(&crm_account_id)
    .bind(token.instance_url.as_deref())
    .bind(&token.access_token)
    .bind(token.refresh_token.as_deref())
    .bind(token.token_type.as_deref().unwrap_or("Bearer"))
    .bind(token.scope.as_deref())
    .bind(token_expires_at.as_deref())
    .bind(None::<String>)
    .bind(Utc::now().to_rfc3339())
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    Ok(Json(ConnectionResponse {
        provider: provider.as_str().to_string(),
        status: status.to_string(),
        crm_account_id,
        crm_object_id: token.crm_account_id,
        connected: true,
        updated_at: Utc::now().to_rfc3339(),
    }))
}

async fn sync_quote_to_crm(
    Path(quote_id): Path<String>,
    State(state): State<CrmState>,
    Query(query): Query<SyncEventPayloadRequest>,
) -> Result<Json<SyncResultResponse>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;
    ensure_quote_exists(&state, &quote_id).await?;

    let direction =
        query.direction.as_deref().and_then(to_direction).unwrap_or(CrmDirection::QuoteyToCrm);
    if direction != CrmDirection::QuoteyToCrm {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CrmError { error: "direction must be quotey_to_crm".to_string() }),
        ));
    }

    let mut payload = fetch_quote_payload(&state, &quote_id).await?;
    let quote_line_payload =
        fetch_quote_lines_payload(&state, &quote_id).await.unwrap_or_else(|_| json!([]));
    payload["lines"] = quote_line_payload;

    let selected_provider = query.provider.as_deref().and_then(CrmProvider::parse);
    let providers = list_connected_integrations(&state, selected_provider).await?;

    if providers.is_empty() {
        return Err((
            StatusCode::PRECONDITION_FAILED,
            Json(CrmError {
                error: match query.provider.as_deref() {
                    Some(provider) => format!("CRM provider `{provider}` is not connected"),
                    None => "no CRM provider is connected".to_string(),
                },
            }),
        ));
    }

    let mut results = Vec::with_capacity(providers.len());
    for integration in providers {
        let mappings = fetch_mappings(&state, direction, Some(integration.provider)).await?;
        let mut event_payload =
            map_quote_payload_for_provider(&payload, &integration.provider, &mappings);
        event_payload["provider"] = json!(integration.provider.as_str());
        event_payload["crm_account_id"] = json!(integration.crm_account_id);
        event_payload["quote_id"] = json!(&quote_id);

        let event_id = format!("CRMES-{}", Uuid::new_v4().simple());
        create_sync_event(
            &state,
            &event_id,
            integration.provider,
            direction,
            "quote_sync",
            Some(&quote_id),
            Some("quote"),
            Some(&quote_id),
            &event_payload,
            "queued",
            1,
            None,
        )
        .await?;

        let execution_result = run_crm_sync_task(
            &state,
            &integration,
            &event_id,
            quote_id.as_str(),
            direction,
            "quote_sync",
            &event_payload,
            1,
            "queued",
            None,
        )
        .await?;

        results.push(SyncAttempt {
            provider: integration.provider.as_str().to_string(),
            event_id,
            status: execution_result.status.to_string(),
            message: execution_result.message,
            attempts: execution_result.attempts,
        });
    }

    Ok(Json(SyncResultResponse { quote_id, provider_results: results }))
}

#[derive(Debug)]
struct QueueSyncResult {
    status: String,
    message: String,
    attempts: i32,
}

fn crm_execution_key(
    quote_id: &str,
    provider: CrmProvider,
    event_type: &str,
    event_id: &str,
) -> OperationKey {
    OperationKey(format!("crm:{quote_id}:{provider}:{event_type}:{event_id}"))
}

async fn run_crm_sync_task(
    state: &CrmState,
    integration: &CrmIntegration,
    event_id: &str,
    quote_id: &str,
    direction: CrmDirection,
    event_type: &str,
    payload: &Value,
    attempt_number: i32,
    initial_status: &str,
    initial_error: Option<String>,
) -> Result<QueueSyncResult, (StatusCode, Json<CrmError>)> {
    let attempts = attempt_number.max(1);

    let repository = SqlExecutionQueueRepository::new(state.db_pool.clone());
    let execution_engine = DeterministicExecutionEngine::with_config(crm_execution_engine_config());
    let operation_key = crm_execution_key(quote_id, integration.provider, event_type, event_id);
    let payload_json = payload.to_string();
    let payload_hash = DeterministicExecutionEngine::hash_payload(&payload_json);
    let event_quote_id = QuoteId(quote_id.to_string());

    let (mut task, mut idempotency_record) = execution_engine.create_task(
        event_quote_id,
        CRM_SYNC_OPERATION_KIND,
        payload_json.clone(),
        operation_key,
        event_id.to_string(),
    );

    let initial_retry_count = attempts.saturating_sub(1) as u32;
    task.retry_count = initial_retry_count;
    task.max_retries = crm_sync_max_retries();
    task.payload_json = payload_json.clone();
    idempotency_record.attempt_count = attempts as u32;
    idempotency_record.payload_hash = payload_hash.clone();

    repository.save_task(task.clone()).await.map_err(repository_error)?;
    repository.save_operation(idempotency_record.clone()).await.map_err(repository_error)?;

    update_sync_event_status(&state, event_id, initial_status, attempts, initial_error).await?;

    let claimed = execution_engine
        .claim_task(task, CRM_SYNC_WORKER_ID, &mut idempotency_record)
        .map_err(map_execution_error)?;
    task = claimed.task;
    repository.save_task(task.clone()).await.map_err(repository_error)?;
    repository.append_transition(claimed.transition.clone()).await.map_err(repository_error)?;
    update_sync_event_status(
        &state,
        event_id,
        crm_sync_queue_status(&ExecutionTaskState::Running),
        attempts,
        None,
    )
    .await?;

    let attempt = simulate_crm_sync_attempt(integration, payload, direction).await;
    let result = match attempt {
        Ok(message) => {
            let completed = execution_engine
                .complete_task(task, payload_hash.clone(), &mut idempotency_record)
                .map_err(map_execution_error)?;
            task = completed.task;
            repository.append_transition(completed.transition).await.map_err(repository_error)?;
            repository.save_task(task.clone()).await.map_err(repository_error)?;
            repository
                .save_operation({
                    idempotency_record.result_snapshot_json = Some(payload_hash.clone());
                    idempotency_record.clone()
                })
                .await
                .map_err(repository_error)?;

            let _ =
                set_integration_sync_state(&state, integration.provider, "connected", None).await;
            QueueSyncResult {
                status: crm_sync_queue_status(&ExecutionTaskState::Completed).to_string(),
                message,
                attempts,
            }
        }
        Err(attempt_failure) => {
            let retry_policy =
                if matches!(attempt_failure.kind, SyncAttemptResultKind::RetryableFailure) {
                    RetryPolicy::Retry
                } else {
                    classify_retry_policy(&attempt_failure.message)
                };
            let failed = execution_engine
                .fail_task(
                    task,
                    attempt_failure.message.clone(),
                    attempt_failure.error_class.unwrap_or_else(|| "crm_sync_failure".to_string()),
                    retry_policy,
                    &mut idempotency_record,
                )
                .map_err(map_execution_error)?;
            task = failed.task;
            repository.append_transition(failed.transition).await.map_err(repository_error)?;
            repository.save_task(task.clone()).await.map_err(repository_error)?;
            repository
                .save_operation({
                    idempotency_record.error_snapshot_json = Some(attempt_failure.message.clone());
                    idempotency_record.updated_by_component = "crm-sync-worker".to_string();
                    idempotency_record.clone()
                })
                .await
                .map_err(repository_error)?;

            let _ = set_integration_sync_state(
                &state,
                integration.provider,
                if matches!(task.state, ExecutionTaskState::FailedTerminal) {
                    "error"
                } else {
                    "connected"
                },
                Some(&attempt_failure.message),
            )
            .await;

            QueueSyncResult {
                status: crm_sync_queue_status(&task.state).to_string(),
                message: attempt_failure.message,
                attempts,
            }
        }
    };

    update_sync_event_status(
        &state,
        event_id,
        &result.status,
        result.attempts,
        Some(result.message.clone()),
    )
    .await?;
    Ok(result)
}

async fn list_mappings(
    State(state): State<CrmState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<MappingResponse>>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;

    let provider = params
        .get("provider")
        .map(|raw| {
            CrmProvider::parse(raw).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(CrmError { error: "provider must be salesforce or hubspot".to_string() }),
                )
            })
        })
        .transpose()?;
    let direction = params
        .get("direction")
        .map(|value| {
            to_direction(value).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(CrmError {
                        error: "direction must be quotey_to_crm or crm_to_quotey".to_string(),
                    }),
                )
            })
        })
        .transpose()?;

    let mut where_clauses = Vec::new();
    let mut where_values = Vec::new();
    if let Some(p) = provider {
        where_clauses.push("provider = ?");
        where_values.push(p.as_str().to_string());
    }
    if let Some(dir) = direction {
        where_clauses.push("direction = ?");
        where_values.push(dir.as_str().to_string());
    }

    let mut query = String::from(
        "SELECT id, provider, direction, quotey_field, crm_field, description, expression, is_active, updated_at\n     FROM crm_field_mapping",
    );
    if !where_clauses.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&where_clauses.join(" AND "));
    }

    query.push_str(" ORDER BY provider, direction, updated_at DESC, quotey_field");

    let mut rows = sqlx::query(&query);
    for value in where_values {
        rows = rows.bind(value);
    }
    let rows = rows.fetch_all(&state.db_pool).await.map_err(db_error)?;

    let mut out = Vec::with_capacity(rows.len());
    let mut seen = HashSet::new();
    for row in rows {
        let row_id = row.try_get::<String, _>("id").unwrap_or_default();
        let provider =
            match CrmProvider::parse(&row.try_get::<String, _>("provider").unwrap_or_default()) {
                Some(value) => value,
                None => {
                    warn!(
                        row_id,
                        "skipping crm field mapping with unsupported provider in storage"
                    );
                    continue;
                }
            };
        let direction =
            match row.try_get::<String, _>("direction").ok().as_deref().and_then(to_direction) {
                Some(value) => value,
                None => {
                    warn!(
                        row_id,
                        "skipping crm field mapping with unsupported direction in storage"
                    );
                    continue;
                }
            };
        let Some(quotey_field) = row
            .try_get::<String, _>("quotey_field")
            .ok()
            .and_then(|quotey_field| normalize_and_validate_quotey_field(&quotey_field, direction))
        else {
            warn!(row_id, "skipping crm field mapping with unsupported quotey_field");
            continue;
        };
        let crm_field =
            row.try_get::<String, _>("crm_field").unwrap_or_default().trim().to_string();
        let dedupe_key = (
            provider.as_str().to_string(),
            direction.as_str().to_string(),
            quotey_field.clone(),
            crm_field.clone(),
        );
        if !seen.insert(dedupe_key) {
            continue;
        }

        out.push(MappingResponse {
            id: row_id,
            provider: provider.as_str().to_string(),
            direction: direction.as_str().to_string(),
            quotey_field,
            crm_field,
            description: row.try_get("description").ok(),
            expression: row.try_get("expression").ok(),
            is_active: row.try_get::<i64, _>("is_active").unwrap_or(0) != 0,
        });
    }

    Ok(Json(out))
}

async fn list_mapping_catalog(
    State(state): State<CrmState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<MappingCatalogItemList>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;

    let provider_filter = params
        .get("provider")
        .map(|raw| {
            CrmProvider::parse(raw).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(CrmError { error: "provider must be salesforce or hubspot".to_string() }),
                )
            })
        })
        .transpose()?;
    let direction_filter = params
        .get("direction")
        .map(|raw| {
            to_direction(raw).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(CrmError {
                        error: "direction must be quotey_to_crm or crm_to_quotey".to_string(),
                    }),
                )
            })
        })
        .transpose()?;

    let provider = provider_filter.as_ref().map(CrmProvider::as_str).map(str::to_string);
    let direction = direction_filter.as_ref().map(CrmDirection::as_str).map(str::to_string);
    let fields = mapping_catalog(provider_filter, direction_filter);

    Ok(Json(MappingCatalogItemList { provider, direction, fields }))
}

async fn upsert_mappings(
    State(state): State<CrmState>,
    Json(payload): Json<UpsertMappingsRequest>,
) -> Result<Json<Vec<MappingResponse>>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;
    let provider = CrmProvider::parse(&payload.provider).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(CrmError { error: "provider must be salesforce or hubspot".to_string() }),
        )
    })?;
    let direction = to_direction(&payload.direction).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(CrmError {
                error: "direction must be quotey_to_crm or crm_to_quotey".to_string(),
            }),
        )
    })?;

    let mut response = Vec::with_capacity(payload.mappings.len());
    for mapping in payload.mappings {
        let quotey_field = mapping.quotey_field.trim().to_string();
        let crm_field = mapping.crm_field.trim().to_string();
        if quotey_field.is_empty() || crm_field.is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(CrmError { error: "quotey_field and crm_field are required".to_string() }),
            ));
        }
        let Some(normalized_quotey_field) =
            normalize_and_validate_quotey_field(&quotey_field, direction)
        else {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(CrmError {
                    error: format!(
                        "unsupported quotey_field '{}'. supported: {}",
                        quotey_field,
                        supported_quotey_field_names(direction).join(", ")
                    ),
                }),
            ));
        };
        let id = mapping.id.unwrap_or_else(|| Uuid::new_v4().simple().to_string());
        let is_active = mapping.is_active.unwrap_or(true);
        let description = mapping.description.clone().and_then(|value| {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        });
        let expression = mapping.expression.clone().and_then(|value| {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        });

        sqlx::query(
            "INSERT INTO crm_field_mapping (\n                id,\n                provider,\n                direction,\n                quotey_field,\n                crm_field,\n                description,\n                expression,\n                is_active,\n                updated_at\n             )\n             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)\n             ON CONFLICT(provider, direction, quotey_field, crm_field)\n             DO UPDATE SET\n                description = excluded.description,\n                expression = excluded.expression,\n                is_active = excluded.is_active,\n                updated_at = excluded.updated_at",
        )
        .bind(&id)
        .bind(provider.as_str())
        .bind(direction.as_str())
        .bind(&normalized_quotey_field)
        .bind(&crm_field)
        .bind(description.clone())
        .bind(expression.clone())
        .bind(if is_active { 1 } else { 0 })
        .bind(Utc::now().to_rfc3339())
        .execute(&state.db_pool)
        .await
        .map_err(db_error)?;

        response.push(MappingResponse {
            id,
            provider: provider.as_str().to_string(),
            direction: direction.as_str().to_string(),
            quotey_field: normalized_quotey_field,
            crm_field,
            description,
            expression,
            is_active,
        });
    }

    Ok(Json(response))
}

async fn list_sync_events(
    State(state): State<CrmState>,
    Query(query): Query<SyncEventsQuery>,
) -> Result<Json<Vec<SyncEventResponse>>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;

    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let mut statement = String::from(
        "SELECT id, provider, direction, event_type, quote_id, crm_object_type, crm_object_id, status, attempts, error_message,\n            created_at, updated_at, completed_at\n         FROM crm_sync_event",
    );
    let mut where_clauses = Vec::new();
    if query.provider.is_some() {
        where_clauses.push("provider = ?");
    }
    if query.direction.is_some() {
        where_clauses.push("direction = ?");
    }
    if query.status.is_some() {
        where_clauses.push("status = ?");
    }
    if query.quote_id.is_some() {
        where_clauses.push("quote_id = ?");
    }

    if !where_clauses.is_empty() {
        statement.push_str(" WHERE ");
        statement.push_str(&where_clauses.join(" AND "));
    }
    statement.push_str(" ORDER BY created_at DESC LIMIT ");
    statement.push_str(&limit.to_string());

    let mut db_query = sqlx::query(&statement);
    if let Some(provider) = query.provider {
        db_query = db_query.bind(provider);
    }
    if let Some(direction) = query.direction {
        db_query = db_query.bind(direction);
    }
    if let Some(status) = query.status {
        db_query = db_query.bind(status);
    }
    if let Some(quote_id) = query.quote_id {
        db_query = db_query.bind(quote_id);
    }

    let rows = db_query.fetch_all(&state.db_pool).await.map_err(db_error)?;
    let mut events = Vec::with_capacity(rows.len());
    for row in rows {
        let status: String = row.try_get("status").unwrap_or_default();
        let attempts = row.try_get::<i32, _>("attempts").unwrap_or(0);
        events.push(SyncEventResponse {
            id: row.try_get("id").unwrap_or_default(),
            provider: row.try_get("provider").unwrap_or_default(),
            direction: row.try_get("direction").unwrap_or_default(),
            event_type: row.try_get("event_type").unwrap_or_default(),
            quote_id: row.try_get("quote_id").ok(),
            crm_object_type: row.try_get("crm_object_type").ok(),
            crm_object_id: row.try_get("crm_object_id").ok(),
            status: status.clone(),
            attempts,
            error_message: row.try_get("error_message").ok(),
            next_retry_in_seconds: next_retry_delay_seconds(&status, attempts),
            created_at: row.try_get("created_at").unwrap_or_default(),
            updated_at: row.try_get("updated_at").unwrap_or_default(),
            completed_at: row.try_get("completed_at").ok(),
        });
    }

    Ok(Json(events))
}

async fn retry_sync_event(
    Path(SyncRetryPath { event_id }): Path<SyncRetryPath>,
    State(state): State<CrmState>,
) -> Result<Json<SyncEventResponse>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;
    let maybe_event = fetch_sync_event(&state, &event_id).await?;

    let event = maybe_event.ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(CrmError { error: "sync event not found".to_string() }))
    })?;

    let direction = to_direction(&event.direction).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(CrmError { error: "invalid event direction".to_string() }))
    })?;

    if event.attempts >= MAX_SYNC_ATTEMPTS {
        return Err((
            StatusCode::CONFLICT,
            Json(CrmError {
                error: format!("event has reached retry limit of {} attempts", MAX_SYNC_ATTEMPTS),
            }),
        ));
    }

    if event.provider.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CrmError { error: "invalid event data".to_string() }),
        ));
    }
    if !sync_status_is_retryable(event.status.as_str()) {
        return Err((
            StatusCode::CONFLICT,
            Json(CrmError {
                error: "only failed, retrying, or skipped events can be retried".to_string(),
            }),
        ));
    }

    let next_attempt = event.attempts.saturating_add(1);
    let provider = CrmProvider::parse(&event.provider).ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(CrmError { error: "invalid event provider".to_string() }))
    })?;
    let payload: Value = serde_json::from_str(&event.payload_json).map_err(|error| {
        error!(error = %error, event_id = %event_id, "stored retry payload is invalid json");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CrmError { error: "stored sync event payload is corrupted".to_string() }),
        )
    })?;

    update_sync_event_status(&state, &event_id, "queued", next_attempt, None).await?;

    match direction {
        CrmDirection::QuoteyToCrm => {
            let quote_id = event.quote_id.as_deref().ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(CrmError {
                        error: "quotey_to_crm retry requires quote_id in event record".to_string(),
                    }),
                )
            })?;

            let integrations = list_connected_integrations(&state, Some(provider)).await?;
            if integrations.is_empty() {
                let reason = format!(
                    "provider `{}` is not connected; reconnect and retry",
                    provider.as_str()
                );
                update_sync_event_status(&state, &event_id, "failed", next_attempt, Some(reason))
                    .await?;
            } else {
                let integration = integrations.first().cloned().ok_or_else(|| {
                    (
                        StatusCode::CONFLICT,
                        Json(CrmError {
                            error: "provider integration missing after lookup".to_string(),
                        }),
                    )
                })?;
                let _ = run_crm_sync_task(
                    &state,
                    &integration,
                    &event_id,
                    quote_id,
                    direction,
                    &event.event_type,
                    &payload,
                    next_attempt,
                    "queued",
                    None,
                )
                .await?;
            }
        }
        CrmDirection::CrmToQuotey => {
            let webhook_payload = serde_json::from_value(payload.clone())
                .unwrap_or_else(|_| normalize_webhook_payload(&payload));
            let mappings =
                fetch_mappings(&state, CrmDirection::CrmToQuotey, Some(provider)).await?;
            let inbound_update =
                extract_inbound_quote_update(&webhook_payload, provider, &mappings);
            let quote_id = resolve_webhook_quote_id(
                &state,
                inbound_update.quote_id.as_deref().or_else(|| event.quote_id.as_deref()),
                inbound_update.account_id.as_deref(),
                inbound_update.deal_id.as_deref(),
            )
            .await?;

            if inbound_update.is_empty() {
                update_sync_event_status(
                    &state,
                    &event_id,
                    "skipped",
                    next_attempt,
                    Some("retry payload has no actionable fields".to_string()),
                )
                .await?;
            } else {
                if let Some(ref quote_id) = quote_id {
                    if let Err((_code, err)) = apply_crm_update_to_quote(
                        &state,
                        quote_id,
                        inbound_update.account_id.as_deref(),
                        inbound_update.deal_id.as_deref(),
                        inbound_update.status.as_deref(),
                        inbound_update.notes.as_deref(),
                    )
                    .await
                    {
                        update_sync_event_status(
                            &state,
                            &event_id,
                            "failed",
                            next_attempt,
                            Some(err.0.error.clone()),
                        )
                        .await?;
                    } else {
                        update_sync_event_status(&state, &event_id, "success", next_attempt, None)
                            .await?;
                    }
                } else {
                    update_sync_event_status(
                        &state,
                        &event_id,
                        "skipped",
                        next_attempt,
                        Some("retry could not resolve quote_id from payload".to_string()),
                    )
                    .await?;
                }
            }
        }
    }

    let event = fetch_sync_event(&state, &event_id).await?.ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(CrmError { error: "sync event disappeared while retrying".to_string() }),
        )
    })?;

    Ok(Json(SyncEventResponse {
        id: event_id,
        provider: event.provider,
        direction: event.direction,
        event_type: event.event_type,
        quote_id: event.quote_id,
        crm_object_type: event.crm_object_type,
        crm_object_id: event.crm_object_id,
        status: event.status.clone(),
        attempts: event.attempts,
        error_message: event.error_message,
        next_retry_in_seconds: next_retry_delay_seconds(&event.status, event.attempts),
        created_at: event.created_at,
        updated_at: event.updated_at,
        completed_at: event.completed_at,
    }))
}

async fn crm_status(
    State(state): State<CrmState>,
) -> Result<Json<CrmStatusPayload>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;
    let rows = sqlx::query(
        "SELECT provider, status, crm_account_id, last_synced_at, last_error,\n         access_token, refresh_token, token_expires_at\n         FROM crm_integration",
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(db_error)?;

    let mut configured_integrations: HashMap<
        String,
        (String, Option<String>, Option<String>, Option<String>, bool),
    > = rows
        .into_iter()
        .filter_map(|row| {
            let provider = row.try_get::<String, _>("provider").ok()?;
            let status =
                row.try_get::<String, _>("status").unwrap_or_else(|_| "disconnected".to_string());
            let crm_account_id = row.try_get("crm_account_id").ok();
            let last_synced_at = row.try_get("last_synced_at").ok();
            let last_error = row.try_get("last_error").ok();
            let access_token = row.try_get::<String, _>("access_token").unwrap_or_default();
            let refresh_token = row.try_get::<Option<String>, _>("refresh_token").ok();
            let token_expires_at =
                row.try_get::<Option<String>, _>("token_expires_at").ok().flatten();
            let has_tokens = (!access_token.is_empty())
                && (!has_expired_token(&token_expires_at) || refresh_token.is_some());
            Some((provider, (status, crm_account_id, last_synced_at, last_error, has_tokens)))
        })
        .collect();

    let providers: Vec<CrmProvider> = vec![CrmProvider::Salesforce, CrmProvider::Hubspot];
    let mut status_rows = Vec::with_capacity(providers.len());
    for provider in providers {
        let key = provider.as_str().to_string();
        let entry = configured_integrations.remove(&key);
        let (status, crm_account_id, last_synced_at, last_error, has_tokens) = entry
            .unwrap_or_else(|| {
                if provider.credentials(&state.config).is_some() {
                    ("disconnected".to_string(), None, None, None, false)
                } else {
                    ("not_configured".to_string(), None, None, None, false)
                }
            });

        status_rows.push(CrmStatus {
            provider: key,
            status,
            crm_account_id,
            last_synced_at,
            last_error,
            has_tokens,
        });
    }

    Ok(Json(CrmStatusPayload { enabled: state.config.enabled, providers: status_rows }))
}

async fn sync_event_stats(
    State(state): State<CrmState>,
) -> Result<Json<SyncEventStatsResponse>, (StatusCode, Json<CrmError>)> {
    crm_state_guard(&HeaderMap::new(), &state, None, false).await?;

    let aggregated = sqlx::query(
        "SELECT status, provider, direction, COUNT(*) AS count\n         FROM crm_sync_event\n         GROUP BY status, provider, direction",
    )
    .fetch_all(&state.db_pool)
    .await
    .map_err(db_error)?;

    let mut by_status = HashMap::new();
    let mut by_provider = HashMap::new();
    let mut by_direction = HashMap::new();
    let mut total = 0i64;
    for row in aggregated {
        let status = row.try_get::<String, _>("status").unwrap_or_else(|_| "unknown".to_string());
        let provider =
            row.try_get::<String, _>("provider").unwrap_or_else(|_| "unknown".to_string());
        let direction =
            row.try_get::<String, _>("direction").unwrap_or_else(|_| "unknown".to_string());
        let count = row.try_get::<i64, _>("count").unwrap_or(0);

        total += count;
        *by_status.entry(status).or_insert(0) += count;
        *by_provider.entry(provider).or_insert(0) += count;
        *by_direction.entry(direction).or_insert(0) += count;
    }

    let cutoff = (Utc::now() - Duration::hours(24)).to_rfc3339();

    let failed_last_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)\n         FROM crm_sync_event\n         WHERE status = 'failed' AND updated_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(&state.db_pool)
    .await
    .map_err(db_error)?;

    let success_last_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)\n         FROM crm_sync_event\n         WHERE status = 'success' AND updated_at >= ?",
    )
    .bind(&cutoff)
    .fetch_one(&state.db_pool)
    .await
    .map_err(db_error)?;

    Ok(Json(SyncEventStatsResponse {
        total_events: total,
        by_status,
        by_provider,
        by_direction,
        failed_last_24h,
        success_last_24h,
    }))
}

async fn webhook_ingest(
    Path(provider_raw): Path<String>,
    State(state): State<CrmState>,
    headers: HeaderMap,
    Json(payload): Json<WebhookPayload>,
) -> Result<(StatusCode, Json<SyncEventResponse>), (StatusCode, Json<CrmError>)> {
    let provider = parse_provider(&provider_raw)?;
    crm_state_guard(&headers, &state, Some(provider), true).await?;

    let event_type = payload.event_type.as_deref().unwrap_or("crm_update");
    let mappings = fetch_mappings(&state, CrmDirection::CrmToQuotey, Some(provider)).await?;
    let inbound_update = extract_inbound_quote_update(&payload, provider, &mappings);
    let quote_id = resolve_webhook_quote_id(
        &state,
        inbound_update.quote_id.as_deref(),
        inbound_update.account_id.as_deref(),
        inbound_update.deal_id.as_deref(),
    )
    .await?;
    let event_id = format!("CRMEV-{}", Uuid::new_v4().simple());
    let now = Utc::now().to_rfc3339();

    let mut event_payload = serde_json::to_value(&payload).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(CrmError { error: format!("invalid webhook payload: {error}") }),
        )
    })?;

    let mut error_msg = None;
    let mut status = "success";
    if let Some(ref quote_id) = quote_id {
        event_payload["quote_id"] = json!(quote_id);
    }
    if let Some(account_id) = inbound_update.account_id.as_deref() {
        event_payload["account_id"] = json!(account_id);
    }
    if let Some(deal_id) = inbound_update.deal_id.as_deref() {
        event_payload["deal_id"] = json!(deal_id);
    }
    if let Some(status_value) = inbound_update.status.as_deref() {
        event_payload["status"] = json!(status_value);
    }
    if let Some(notes) = inbound_update.notes.as_deref() {
        event_payload["notes"] = json!(notes);
    }
    let (crm_object_type, crm_object_id) = infer_crm_object_identity(
        quote_id.as_deref(),
        inbound_update.account_id.as_deref(),
        inbound_update.deal_id.as_deref(),
    );

    if inbound_update.is_empty() {
        status = "skipped";
        error_msg = Some("webhook has no actionable fields".to_string());
    } else if let Some(ref quote_id) = quote_id {
        if let Err((_, err)) = apply_crm_update_to_quote(
            &state,
            quote_id,
            inbound_update.account_id.as_deref(),
            inbound_update.deal_id.as_deref(),
            inbound_update.status.as_deref(),
            inbound_update.notes.as_deref(),
        )
        .await
        {
            status = "failed";
            error_msg = Some(err.0.error.clone());
        }
    } else {
        status = "skipped";
        error_msg = Some("webhook could not resolve quote_id".to_string());
    }

    create_sync_event(
        &state,
        &event_id,
        provider,
        CrmDirection::CrmToQuotey,
        event_type,
        quote_id.as_deref(),
        crm_object_type.as_deref(),
        crm_object_id.as_deref(),
        &event_payload,
        status,
        1,
        error_msg.clone(),
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(SyncEventResponse {
            id: event_id,
            provider: provider.as_str().to_string(),
            direction: CrmDirection::CrmToQuotey.as_str().to_string(),
            event_type: event_type.to_string(),
            quote_id,
            crm_object_type,
            crm_object_id,
            status: status.to_string(),
            attempts: 1,
            error_message: error_msg,
            next_retry_in_seconds: next_retry_delay_seconds(status, 1),
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: Some(now),
        }),
    ))
}

async fn resolve_webhook_quote_id(
    state: &CrmState,
    explicit_quote_id: Option<&str>,
    account_id: Option<&str>,
    deal_id: Option<&str>,
) -> Result<Option<String>, (StatusCode, Json<CrmError>)> {
    if let Some(raw_quote_id) = explicit_quote_id.map(str::trim).filter(|value| !value.is_empty()) {
        let found = sqlx::query_scalar("SELECT id FROM quote WHERE id = ?")
            .bind(raw_quote_id)
            .fetch_optional(&state.db_pool)
            .await
            .map_err(db_error)?;
        return Ok(found);
    }

    if let Some(raw_account_id) = account_id.map(str::trim).filter(|value| !value.is_empty()) {
        let found = sqlx::query_scalar(
            "SELECT id FROM quote WHERE account_id = ? ORDER BY COALESCE(updated_at, created_at) DESC LIMIT 1",
        )
        .bind(raw_account_id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(db_error)?;
        if found.is_some() {
            return Ok(found);
        }
    }

    if let Some(raw_deal_id) = deal_id.map(str::trim).filter(|value| !value.is_empty()) {
        let found = sqlx::query_scalar(
            "SELECT id FROM quote WHERE deal_id = ? ORDER BY COALESCE(updated_at, created_at) DESC LIMIT 1",
        )
        .bind(raw_deal_id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(db_error)?;
        if found.is_some() {
            return Ok(found);
        }
    }

    Ok(None)
}

async fn ensure_quote_exists(
    state: &CrmState,
    quote_id: &str,
) -> Result<(), (StatusCode, Json<CrmError>)> {
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM quote WHERE id = ?")
        .bind(quote_id)
        .fetch_optional(&state.db_pool)
        .await
        .map_err(db_error)?;

    if exists.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(CrmError { error: format!("quote `{quote_id}` not found") }),
        ));
    }
    Ok(())
}

async fn fetch_quote_payload(
    state: &CrmState,
    quote_id: &str,
) -> Result<Value, (StatusCode, Json<CrmError>)> {
    let row = sqlx::query(
        "SELECT id, status, currency, created_by, account_id, deal_id, notes, version,\n         term_months,\n         (SELECT COALESCE(SUM(subtotal), 0) FROM quote_line WHERE quote_line.quote_id = quote.id) AS total_amount\n         FROM quote\n         WHERE id = ?",
    )
    .bind(quote_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(CrmError {
                error: format!("quote `{quote_id}` not found"),
            }),
        )
    })?;

    Ok(json!({
        "quote_id": row.try_get::<String, _>("id").unwrap_or_default(),
        "status": row.try_get::<String, _>("status").unwrap_or_default(),
        "currency": row.try_get::<String, _>("currency").unwrap_or_default(),
        "created_by": row.try_get::<String, _>("created_by").unwrap_or_default(),
        "account_id": row.try_get::<String, _>("account_id").ok(),
        "deal_id": row.try_get::<String, _>("deal_id").ok(),
        "notes": row.try_get::<String, _>("notes").ok(),
        "version": row.try_get::<i64, _>("version").unwrap_or(1),
        "term_months": row.try_get::<i64, _>("term_months").ok(),
        "total_amount": row.try_get::<f64, _>("total_amount").unwrap_or(0.0),
    }))
}

async fn fetch_quote_lines_payload(
    state: &CrmState,
    quote_id: &str,
) -> Result<Value, (StatusCode, Json<CrmError>)> {
    let lines = sqlx::query(
        "SELECT id, product_id, quantity, unit_price, discount_pct, subtotal, notes\n         FROM quote_line\n         WHERE quote_id = ?\n         ORDER BY created_at ASC",
    )
    .bind(quote_id)
    .fetch_all(&state.db_pool)
    .await
    .map_err(db_error)?;

    let rows: Vec<Value> = lines
        .into_iter()
        .map(|row| {
            json!({
                "id": row.try_get::<String, _>("id").unwrap_or_default(),
                "product_id": row.try_get::<String, _>("product_id").unwrap_or_default(),
                "quantity": row.try_get::<i64, _>("quantity").unwrap_or(0),
                "unit_price": row.try_get::<f64, _>("unit_price").unwrap_or(0.0),
                "discount_pct": row.try_get::<f64, _>("discount_pct").unwrap_or(0.0),
                "subtotal": row.try_get::<f64, _>("subtotal").unwrap_or(0.0),
                "notes": row.try_get::<String, _>("notes").ok(),
            })
        })
        .collect();
    Ok(json!(rows))
}

fn parse_provider(raw: &str) -> Result<CrmProvider, (StatusCode, Json<CrmError>)> {
    CrmProvider::parse(raw).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(CrmError { error: "provider must be salesforce or hubspot".to_string() }),
        )
    })
}

fn callback_url(
    state: &CrmState,
    provider: &CrmProvider,
) -> Result<String, (StatusCode, Json<CrmError>)> {
    let base = state
        .config
        .callback_base_url
        .as_deref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(CrmError { error: "oauth callback base URL is not configured".to_string() }),
            )
        })?;

    if !base.starts_with("http://") && !base.starts_with("https://") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(CrmError {
                error: "crm.callback_base_url must start with http:// or https://".to_string(),
            }),
        ));
    }

    let base = base.trim_end_matches('/');
    Ok(format!("{base}/api/v1/crm/oauth/{}/callback", provider.as_str()))
}

fn error_description(value: Option<String>) -> String {
    value.unwrap_or_else(|| "an unknown oauth error occurred".to_string())
}

fn normalize_scope(raw: &str) -> String {
    let mut seen = HashSet::new();
    let mut parts = Vec::new();
    for part in raw
        .split(|c: char| c.is_ascii_whitespace() || c == ',' || c == ';')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if seen.insert(part.to_string()) {
            parts.push(part.to_string());
        }
    }
    parts.join(" ")
}

async fn discover_provider_account_id(
    state: &CrmState,
    provider: CrmProvider,
    token_type: &str,
    access_token: &str,
) -> Option<String> {
    let token_type = if token_type.trim().is_empty() { "Bearer" } else { token_type };
    let auth = format!("{token_type} {}", access_token);
    let userinfo_url = match provider {
        CrmProvider::Salesforce => {
            Some("https://login.salesforce.com/services/oauth2/userinfo".to_string())
        }
        CrmProvider::Hubspot => Some("https://api.hubapi.com/oauth/v3/userinfo".to_string()),
    };

    let userinfo_url = match userinfo_url {
        Some(url) => url,
        None => return None,
    };

    let response =
        state.client.get(&userinfo_url).header("Authorization", auth).send().await.ok()?;

    if !response.status().is_success() {
        warn!(
            provider = provider.as_str(),
            status = %response.status(),
            "failed to resolve CRM account identity"
        );
        return None;
    }

    let payload: Value = response.json().await.ok()?;
    let candidates = match provider {
        CrmProvider::Salesforce => ["organization_id", "user_id", "id"].as_ref(),
        CrmProvider::Hubspot => ["user_id", "hub_id", "id"].as_ref(),
    };

    for key in candidates {
        if let Some(value) = payload.get(*key).and_then(Value::as_str) {
            if !value.trim().is_empty() {
                return Some(value.to_string());
            }
        }
    }

    None
}

fn resolve_payload_value(source: &Value, field: &str) -> Option<Value> {
    if field.is_empty() {
        return None;
    }

    let mut current = source;
    for raw_segment in field.split('.') {
        let segment = raw_segment.trim();
        if segment.is_empty() {
            continue;
        }
        if let Some(array_index) = segment.split_once('[') {
            let (array_field, bracket_tail) = array_index;
            current = current.get(array_field)?;
            if !bracket_tail.ends_with(']') {
                return None;
            }
            let index = bracket_tail.trim_end_matches(']').parse::<usize>().ok()?;
            current = current.get(index)?;
            continue;
        }

        current = current.get(segment)?;
    }

    Some(current.clone())
}

fn extract_template_value(expression: &str, source: &Value) -> Option<String> {
    let expression = expression.trim();
    if expression.is_empty() {
        return None;
    }
    if expression.starts_with("${")
        && expression.ends_with("}")
        && !expression[2..expression.len() - 1].contains("${")
    {
        let field = expression.trim_start_matches("${").trim_end_matches('}');
        return resolve_payload_value(source, field).and_then(|value| value_to_text(&value));
    }
    if expression.starts_with("{{")
        && expression.ends_with("}}")
        && !expression[2..expression.len() - 2].contains("{{")
    {
        let field = expression.trim_start_matches("{{").trim_end_matches("}}");
        return resolve_payload_value(source, field).and_then(|value| value_to_text(&value));
    }

    let mut output = String::new();
    let mut cursor = 0;
    while let Some(start) = expression[cursor..].find("${") {
        let absolute_start = cursor + start;
        let prefix = &expression[cursor..absolute_start];
        output.push_str(prefix);
        let remainder = &expression[absolute_start + 2..];
        let end = match remainder.find('}') {
            Some(idx) => idx,
            None => return None,
        };
        let key = &remainder[..end];
        if let Some(value) =
            resolve_payload_value(source, key).as_ref().and_then(|v| value_to_text(v))
        {
            output.push_str(&value);
        }
        cursor = absolute_start + 2 + end + 1;
    }
    output.push_str(&expression[cursor..]);
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn value_to_text(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some("null".to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.clone()),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string(value).ok().filter(|s| !s.is_empty())
        }
    }
}

fn set_nested_value(target: &mut Value, field: &str, value: Value) {
    if !target.is_object() {
        *target = json!({});
    }

    let segments: Vec<&str> =
        field.split('.').map(str::trim).filter(|segment| !segment.is_empty()).collect();
    let Some(first) = segments.first() else {
        return;
    };
    let mut cursor = target.as_object_mut().expect("target guaranteed object");
    if segments.len() == 1 {
        cursor.insert((*first).to_string(), value);
        return;
    }

    for segment in segments.iter().take(segments.len() - 1) {
        let seg = segment.to_string();
        if !cursor.contains_key(&seg) {
            cursor.insert(seg.clone(), json!({}));
        }
        cursor = cursor.get_mut(&seg).and_then(|v| v.as_object_mut()).expect("nested object");
    }

    cursor.insert(segments[segments.len() - 1].to_string(), value);
}

fn map_quote_payload_for_provider(
    payload: &Value,
    provider: &CrmProvider,
    mappings: &[CrmFieldMapping],
) -> Value {
    let mut mapped = payload.clone();
    if !mapped.is_object() {
        mapped = json!({});
    }

    if payload.is_null() {
        return mapped;
    }

    for mapping in mappings {
        if mapping.provider != *provider || !mapping.is_active {
            continue;
        }

        let quotey_field =
            normalize_quotey_field_for_direction(&mapping.quotey_field, CrmDirection::QuoteyToCrm);
        if !is_supported_quotey_field(&quotey_field, CrmDirection::QuoteyToCrm) {
            continue;
        }
        let value = if let Some(expression) = mapping.expression.as_deref() {
            match extract_template_value(expression, payload) {
                Some(rendered) => Value::String(rendered),
                None => resolve_payload_value(payload, &quotey_field).unwrap_or(Value::Null),
            }
        } else {
            resolve_payload_value(payload, &quotey_field).unwrap_or(Value::Null)
        };

        if value.is_null() {
            continue;
        }
        set_nested_value(&mut mapped, &mapping.crm_field, value);
    }

    mapped
}

fn has_expired_token(token_expires_at: &Option<String>) -> bool {
    let Some(expires_at) = token_expires_at else {
        return false;
    };
    match DateTime::parse_from_rfc3339(expires_at) {
        Ok(expiry) => expiry.timestamp() <= Utc::now().timestamp(),
        Err(_) => true,
    }
}

async fn simulate_crm_sync_attempt(
    integration: &CrmIntegration,
    payload: &Value,
    direction: CrmDirection,
) -> Result<String, SyncAttemptResult> {
    if direction != CrmDirection::QuoteyToCrm {
        return Err(SyncAttemptResult {
            kind: SyncAttemptResultKind::TerminalFailure,
            message: "unsupported sync direction".to_string(),
            error_class: Some("unsupported_sync_direction".to_string()),
        });
    }

    if payload.get("quote_id").and_then(Value::as_str).is_none() {
        return Err(SyncAttemptResult {
            kind: SyncAttemptResultKind::TerminalFailure,
            message: "quote payload missing quote_id".to_string(),
            error_class: Some("missing_quote_id".to_string()),
        });
    }
    if integration.access_token.trim().is_empty() {
        return Err(SyncAttemptResult {
            kind: SyncAttemptResultKind::TerminalFailure,
            message: "provider integration has no access token".to_string(),
            error_class: Some("missing_access_token".to_string()),
        });
    }
    if has_expired_token(&integration.token_expires_at) && integration.refresh_token.is_none() {
        return Err(SyncAttemptResult {
            kind: SyncAttemptResultKind::TerminalFailure,
            message: "provider integration access token expired".to_string(),
            error_class: Some("access_token_expired".to_string()),
        });
    }

    let has_line_items =
        payload.get("lines").and_then(|v| v.as_array()).is_some_and(|lines| !lines.is_empty());
    if !has_line_items {
        warn!(
            provider = integration.provider.as_str(),
            quote_id = %payload.get("quote_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "sync payload has no line items"
        );
    }

    if has_expired_token(&integration.token_expires_at) {
        if let Some(refresh_token) = &integration.refresh_token {
            info!(
                provider = integration.provider.as_str(),
                refresh_token_hint = &refresh_token[..std::cmp::min(4, refresh_token.len())],
                "crm token is expired; best effort sync"
            );
            return Err(SyncAttemptResult {
                kind: SyncAttemptResultKind::RetryableFailure,
                message:
                    "access token expired; refresh token available for background worker retry"
                        .to_string(),
                error_class: Some("access_token_expired".to_string()),
            });
        }
        return Err(SyncAttemptResult {
            kind: SyncAttemptResultKind::TerminalFailure,
            message: "access token expired and refresh token is missing".to_string(),
            error_class: Some("access_token_expired".to_string()),
        });
    }

    Ok("sync payload validated successfully".to_string())
}

fn db_error(error: sqlx::Error) -> (StatusCode, Json<CrmError>) {
    error!(error = %error, "crm db error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(CrmError { error: "an internal database error occurred".to_string() }),
    )
}

async fn list_connected_integrations(
    state: &CrmState,
    provider_filter: Option<CrmProvider>,
) -> Result<Vec<CrmIntegration>, (StatusCode, Json<CrmError>)> {
    let mut statement = String::from(
        "SELECT provider, status, access_token, refresh_token, token_type, instance_url, scope,\n         token_expires_at, crm_account_id\n         FROM crm_integration\n         WHERE status = 'connected'",
    );
    if provider_filter.is_some() {
        statement.push_str(" AND provider = ?");
    }

    let mut rows = sqlx::query(&statement);
    if let Some(provider) = provider_filter {
        rows = rows.bind(provider.as_str());
    }
    let rows = rows.fetch_all(&state.db_pool).await.map_err(db_error)?;

    let mut integrations = Vec::new();
    for row in rows {
        let provider =
            CrmProvider::parse(&row.try_get::<String, _>("provider").unwrap_or_default())
                .ok_or_else(|| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(CrmError {
                            error: "invalid provider stored for CRM integration".to_string(),
                        }),
                    )
                })?;

        integrations.push(CrmIntegration {
            provider,
            status: row.try_get("status").unwrap_or_else(|_| "connected".to_string()),
            access_token: row.try_get("access_token").unwrap_or_default(),
            refresh_token: row.try_get("refresh_token").ok(),
            token_type: row.try_get("token_type").unwrap_or_else(|_| "Bearer".to_string()),
            instance_url: row.try_get("instance_url").ok(),
            scope: row.try_get("scope").ok(),
            token_expires_at: row.try_get("token_expires_at").ok(),
            crm_account_id: row.try_get("crm_account_id").ok(),
        });
    }
    Ok(integrations)
}

async fn fetch_mappings(
    state: &CrmState,
    direction: CrmDirection,
    provider_filter: Option<CrmProvider>,
) -> Result<Vec<CrmFieldMapping>, (StatusCode, Json<CrmError>)> {
    let mut statement = String::from(
        "SELECT id, provider, direction, quotey_field, crm_field, description, expression, is_active\n         FROM crm_field_mapping\n         WHERE direction = ?\n         ORDER BY provider, direction, updated_at DESC, quotey_field",
    );
    if provider_filter.is_some() {
        statement.push_str(" AND provider = ?");
    }

    let mut rows = sqlx::query(&statement).bind(direction.as_str());
    if let Some(provider) = provider_filter {
        rows = rows.bind(provider.as_str());
    }
    let rows = rows.fetch_all(&state.db_pool).await.map_err(db_error)?;

    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for row in rows {
        let id = row.try_get::<String, _>("id").unwrap_or_default();
        let provider =
            match CrmProvider::parse(&row.try_get::<String, _>("provider").unwrap_or_default()) {
                Some(value) => value,
                None => {
                    warn!(id, "skipping crm field mapping with unsupported provider");
                    continue;
                }
            };
        let Some(row_direction) =
            row.try_get::<String, _>("direction").ok().as_deref().and_then(to_direction)
        else {
            warn!(id, "skipping crm field mapping with unsupported direction");
            continue;
        };
        if row_direction != direction {
            warn!(id, "skipping crm field mapping with direction mismatch");
            continue;
        }
        let Some(quotey_field) = row
            .try_get::<String, _>("quotey_field")
            .ok()
            .and_then(|quotey_field| normalize_and_validate_quotey_field(&quotey_field, direction))
        else {
            warn!(id, "skipping crm field mapping with unsupported quotey_field");
            continue;
        };
        let crm_field = row
            .try_get::<String, _>("crm_field")
            .ok()
            .map(|crm_field| crm_field.trim().to_string())
            .unwrap_or_default();
        let dedupe_key = (
            provider.as_str().to_string(),
            direction.as_str().to_string(),
            quotey_field.clone(),
            crm_field.clone(),
        );
        if !seen.insert(dedupe_key) {
            continue;
        }

        out.push(CrmFieldMapping {
            provider,
            id,
            quotey_field,
            crm_field,
            description: row.try_get("description").ok(),
            expression: row.try_get("expression").ok(),
            is_active: row.try_get::<i64, _>("is_active").unwrap_or(0) != 0,
        });
    }

    Ok(out)
}

fn provider_credentials(
    state: &CrmState,
    provider: CrmProvider,
) -> Result<&str, (StatusCode, Json<CrmError>)> {
    match provider {
        CrmProvider::Salesforce => {
            state.config.salesforce_client_id.as_deref().filter(|v| !v.is_empty()).ok_or_else(
                || {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(CrmError { error: "missing salesforce client_id".to_string() }),
                    )
                },
            )
        }
        CrmProvider::Hubspot => {
            state.config.hubspot_client_id.as_deref().filter(|v| !v.is_empty()).ok_or_else(|| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(CrmError { error: "missing hubspot client_id".to_string() }),
                )
            })
        }
    }
}

#[derive(Debug)]
struct OAuthStateRow {
    redirect_uri: String,
    scope: String,
}

async fn fetch_and_reserve_oauth_state(
    state: &CrmState,
    token: &str,
) -> Result<OAuthStateRow, (StatusCode, Json<CrmError>)> {
    let row = sqlx::query(
        "SELECT redirect_uri, scope\n         FROM crm_oauth_state\n         WHERE state_token = ?\n           AND used = 0\n           AND expires_at > ?",
    )
    .bind(token)
    .bind(Utc::now().to_rfc3339())
    .fetch_optional(&state.db_pool)
    .await
    .map_err(db_error)?
    .ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(CrmError {
                error: "invalid or expired oauth state token".to_string(),
            }),
        )
    })?;

    let redirect_uri = row.try_get::<String, _>("redirect_uri").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CrmError { error: "failed to read oauth state row".to_string() }),
        )
    })?;
    let scope = row.try_get::<String, _>("scope").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(CrmError { error: "failed to read oauth state row".to_string() }),
        )
    })?;

    sqlx::query("UPDATE crm_oauth_state SET used = 1 WHERE state_token = ?")
        .bind(token)
        .execute(&state.db_pool)
        .await
        .map_err(db_error)?;

    Ok(OAuthStateRow { redirect_uri, scope })
}

#[derive(Serialize, Deserialize)]
struct OAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    instance_url: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    expires_in: Option<i64>,
    crm_account_id: Option<String>,
}

struct TokenExchangeRequest<'a> {
    grant_type: &'a str,
    code: &'a str,
    client_id: &'a str,
    client_secret: &'a str,
    redirect_uri: &'a str,
    scope: Option<&'a String>,
}

async fn exchange_token(
    state: &CrmState,
    token_url: &str,
    request: TokenExchangeRequest<'_>,
) -> Result<OAuthTokenResponse, (StatusCode, Json<CrmError>)> {
    let response = state
        .client
        .post(token_url)
        .form(&[
            ("grant_type", request.grant_type),
            ("code", request.code),
            ("client_id", request.client_id),
            ("client_secret", request.client_secret),
            ("redirect_uri", request.redirect_uri),
        ])
        .send()
        .await
        .map_err(|error| {
            error!(error = %error, "crm token exchange request failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(CrmError { error: "oauth token exchange request failed".to_string() }),
            )
        })?;

    if !response.status().is_success() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(CrmError {
                error: format!("oauth token endpoint returned {}", response.status()),
            }),
        ));
    }

    let token: OAuthTokenResponse = response.json().await.map_err(|error| {
        (
            StatusCode::BAD_GATEWAY,
            Json(CrmError { error: format!("failed to decode oauth token response: {error}") }),
        )
    })?;
    if token.access_token.is_empty() {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(CrmError { error: "token endpoint returned empty access token".to_string() }),
        ));
    }
    Ok(token)
}

#[derive(Debug)]
struct SyncEventRow {
    provider: String,
    direction: String,
    event_type: String,
    quote_id: Option<String>,
    crm_object_type: Option<String>,
    crm_object_id: Option<String>,
    status: String,
    payload_json: String,
    attempts: i32,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

async fn fetch_sync_event(
    state: &CrmState,
    event_id: &str,
) -> Result<Option<SyncEventRow>, (StatusCode, Json<CrmError>)> {
    let row = sqlx::query(
        "SELECT provider, direction, event_type, quote_id, crm_object_type, crm_object_id, payload_json, status, attempts, error_message,\n         created_at, updated_at, completed_at\n         FROM crm_sync_event WHERE id = ?",
    )
    .bind(event_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(db_error)?
    .map(|r| SyncEventRow {
        provider: r.try_get("provider").unwrap_or_default(),
        direction: r.try_get("direction").unwrap_or_default(),
        event_type: r.try_get("event_type").unwrap_or_default(),
        quote_id: r.try_get("quote_id").ok(),
        crm_object_type: r.try_get("crm_object_type").ok(),
        crm_object_id: r.try_get("crm_object_id").ok(),
        status: r.try_get("status").unwrap_or_default(),
        payload_json: r.try_get("payload_json").unwrap_or_default(),
        attempts: r.try_get("attempts").unwrap_or_default(),
        error_message: r.try_get("error_message").ok(),
        created_at: r.try_get("created_at").unwrap_or_default(),
        updated_at: r.try_get("updated_at").unwrap_or_default(),
        completed_at: r.try_get("completed_at").ok(),
    });

    Ok(row)
}

async fn create_sync_event(
    state: &CrmState,
    event_id: &str,
    provider: CrmProvider,
    direction: CrmDirection,
    event_type: &str,
    quote_id: Option<&str>,
    crm_object_type: Option<&str>,
    crm_object_id: Option<&str>,
    payload: &Value,
    status: &str,
    attempts: i32,
    error_message: Option<String>,
) -> Result<(), (StatusCode, Json<CrmError>)> {
    sqlx::query(
        "INSERT INTO crm_sync_event (\n            id, provider, direction, event_type, quote_id, crm_object_type, crm_object_id,\n            payload_json, status, attempts, error_message, created_at, updated_at\n         )\n         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event_id)
    .bind(provider.as_str())
    .bind(direction.as_str())
    .bind(event_type)
    .bind(quote_id)
    .bind(crm_object_type)
    .bind(crm_object_id)
    .bind(payload.to_string())
    .bind(status)
    .bind(attempts)
    .bind(error_message)
    .bind(Utc::now().to_rfc3339())
    .bind(Utc::now().to_rfc3339())
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;

    if direction == CrmDirection::CrmToQuotey {
        let quote = quote_id.unwrap_or_default();
        record_audit(&state.db_pool, quote, "crm.webhook_ingested", direction.as_str()).await;
    }
    Ok(())
}

async fn set_integration_sync_state(
    state: &CrmState,
    provider: CrmProvider,
    status: &str,
    last_error: Option<&str>,
) -> Result<(), (StatusCode, Json<CrmError>)> {
    let now = Utc::now().to_rfc3339();
    let synced_at = if status == "connected" { Some(now.clone()) } else { None };
    sqlx::query(
        "UPDATE crm_integration\n         SET status = ?, last_error = ?, last_synced_at = COALESCE(?, last_synced_at), updated_at = ?\n         WHERE provider = ?",
    )
    .bind(status)
    .bind(last_error)
    .bind(synced_at)
    .bind(now)
    .bind(provider.as_str())
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;
    Ok(())
}

async fn update_sync_event_status(
    state: &CrmState,
    event_id: &str,
    status: &str,
    attempts: i32,
    error_message: Option<String>,
) -> Result<(), (StatusCode, Json<CrmError>)> {
    let now = Utc::now().to_rfc3339();
    let completed = if status == "success" || status == "failed" || status == "skipped" {
        Some(now.clone())
    } else {
        None
    };

    sqlx::query(
        "UPDATE crm_sync_event\n         SET status = ?, attempts = ?, error_message = ?, updated_at = ?, completed_at = ?\n         WHERE id = ?",
    )
    .bind(status)
    .bind(attempts)
    .bind(error_message)
    .bind(now)
    .bind(completed)
    .bind(event_id)
    .execute(&state.db_pool)
    .await
    .map_err(db_error)?;
    Ok(())
}

async fn apply_crm_update_to_quote(
    state: &CrmState,
    quote_id: &str,
    account_id: Option<&str>,
    deal_id: Option<&str>,
    status: Option<&str>,
    notes: Option<&str>,
) -> Result<(), (StatusCode, Json<CrmError>)> {
    if account_id.is_none() && deal_id.is_none() && status.is_none() && notes.is_none() {
        return Ok(());
    }

    let mut sets = Vec::new();
    if account_id.is_some() {
        sets.push("account_id = ?");
    }
    if deal_id.is_some() {
        sets.push("deal_id = ?");
    }
    if status.is_some() {
        sets.push("status = ?");
    }
    if notes.is_some() {
        sets.push("notes = ?");
    }
    sets.push("updated_at = ?");
    let statement = format!("UPDATE quote SET {} WHERE id = ?", sets.join(", "));
    let mut query = sqlx::query(&statement);
    for value in [account_id, deal_id, status, notes] {
        if let Some(item) = value {
            query = query.bind(item);
        }
    }
    query = query.bind(Utc::now().to_rfc3339()).bind(quote_id);
    let result = query.execute(&state.db_pool).await.map_err(db_error)?;
    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(CrmError { error: format!("quote `{quote_id}` not found") }),
        ));
    }

    record_audit(&state.db_pool, quote_id, "crm.webhook", "crm_to_quotey_update").await;
    Ok(())
}

async fn record_audit(pool: &DbPool, quote_id: &str, event_type: &str, category: &str) {
    let audit_id = format!("CRMAUD-{}", &Uuid::new_v4().simple().to_string());
    let now = Utc::now().to_rfc3339();
    if let Err(error) = sqlx::query(
        "INSERT INTO audit_event (id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json)\n         VALUES (?, ?, 'server', 'crm', ?, ?, ?, '{}')",
    )
    .bind(audit_id)
    .bind(now)
    .bind(quote_id)
    .bind(event_type)
    .bind(category)
    .execute(pool)
    .await
    {
        warn!(error = %error, quote_id = %quote_id, "crm audit_event write failed");
    }
}

fn encode_query(value: &str) -> String {
    value.replace('+', "%2B").replace(' ', "%20").replace('/', "%2F").replace(':', "%3A")
}

#[cfg(test)]
mod tests {}
