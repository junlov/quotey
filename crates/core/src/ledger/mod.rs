use std::collections::HashMap;

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::quote::{Quote, QuoteId};

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerAction {
    Create,
    Update,
    Approve,
    Reject,
    Custom(String),
}

impl LedgerAction {
    fn as_key(&self) -> String {
        match self {
            Self::Create => "create".to_string(),
            Self::Update => "update".to_string(),
            Self::Approve => "approve".to_string(),
            Self::Reject => "reject".to_string(),
            Self::Custom(value) => value.to_ascii_lowercase(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub entry_id: String,
    pub quote_id: QuoteId,
    pub version: u32,
    pub content_hash: String,
    pub prev_hash: Option<String>,
    pub entry_hash: String,
    pub timestamp: DateTime<Utc>,
    pub actor_id: String,
    pub action: LedgerAction,
    pub signature: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationResult {
    pub quote_id: QuoteId,
    pub valid: bool,
    pub verified_entries: usize,
    pub latest_hash: Option<String>,
    pub failure_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LedgerService {
    signing_key: Vec<u8>,
    entries_by_quote: HashMap<String, Vec<LedgerEntry>>,
}

impl LedgerService {
    pub fn new(signing_key: impl AsRef<[u8]>) -> Self {
        Self { signing_key: signing_key.as_ref().to_vec(), entries_by_quote: HashMap::new() }
    }

    pub fn append_entry(
        &mut self,
        quote: &Quote,
        action: LedgerAction,
        actor_id: impl Into<String>,
    ) -> LedgerEntry {
        let actor_id = actor_id.into();
        let chain = self.entries_by_quote.entry(quote.id.0.clone()).or_default();
        let version = u32::try_from(chain.len()).unwrap_or(u32::MAX).saturating_add(1);
        let prev_hash = chain.last().map(|entry| entry.entry_hash.clone());
        let timestamp = Utc::now();
        let content_hash = content_hash(quote);
        let entry_hash = hash_entry_material(
            &quote.id,
            version,
            &content_hash,
            prev_hash.as_deref(),
            timestamp,
            &actor_id,
            &action,
        );
        let signature = hmac_hex(&self.signing_key, entry_hash.as_bytes());

        let entry = LedgerEntry {
            entry_id: Uuid::new_v4().to_string(),
            quote_id: quote.id.clone(),
            version,
            content_hash,
            prev_hash,
            entry_hash,
            timestamp,
            actor_id,
            action,
            signature,
        };

        chain.push(entry.clone());
        entry
    }

    pub fn verify_chain(&self, quote_id: &QuoteId) -> VerificationResult {
        let Some(entries) = self.entries_by_quote.get(&quote_id.0) else {
            return VerificationResult {
                quote_id: quote_id.clone(),
                valid: false,
                verified_entries: 0,
                latest_hash: None,
                failure_reason: Some("no ledger entries found for quote".to_string()),
            };
        };

        let mut previous_hash: Option<String> = None;
        for (index, entry) in entries.iter().enumerate() {
            let expected_version = u32::try_from(index).unwrap_or(u32::MAX).saturating_add(1);
            if entry.version != expected_version {
                return VerificationResult {
                    quote_id: quote_id.clone(),
                    valid: false,
                    verified_entries: index,
                    latest_hash: previous_hash,
                    failure_reason: Some(format!(
                        "version mismatch at entry {}: expected {}, found {}",
                        entry.entry_id, expected_version, entry.version
                    )),
                };
            }

            if entry.prev_hash != previous_hash {
                return VerificationResult {
                    quote_id: quote_id.clone(),
                    valid: false,
                    verified_entries: index,
                    latest_hash: previous_hash,
                    failure_reason: Some(format!(
                        "previous hash mismatch at entry {}",
                        entry.entry_id
                    )),
                };
            }

            let computed_entry_hash = hash_entry_material(
                &entry.quote_id,
                entry.version,
                &entry.content_hash,
                entry.prev_hash.as_deref(),
                entry.timestamp,
                &entry.actor_id,
                &entry.action,
            );
            if computed_entry_hash != entry.entry_hash {
                return VerificationResult {
                    quote_id: quote_id.clone(),
                    valid: false,
                    verified_entries: index,
                    latest_hash: previous_hash,
                    failure_reason: Some(format!(
                        "entry hash mismatch at entry {}",
                        entry.entry_id
                    )),
                };
            }

            let expected_signature = hmac_hex(&self.signing_key, entry.entry_hash.as_bytes());
            if expected_signature != entry.signature {
                return VerificationResult {
                    quote_id: quote_id.clone(),
                    valid: false,
                    verified_entries: index,
                    latest_hash: previous_hash,
                    failure_reason: Some(format!("signature mismatch at entry {}", entry.entry_id)),
                };
            }

            previous_hash = Some(entry.entry_hash.clone());
        }

        VerificationResult {
            quote_id: quote_id.clone(),
            valid: true,
            verified_entries: entries.len(),
            latest_hash: previous_hash,
            failure_reason: None,
        }
    }

    pub fn entries_for_quote(&self, quote_id: &QuoteId) -> Vec<LedgerEntry> {
        self.entries_by_quote.get(&quote_id.0).cloned().unwrap_or_default()
    }
}

fn content_hash(quote: &Quote) -> String {
    let canonical_payload = match serde_json::to_vec(quote) {
        Ok(payload) => payload,
        Err(_) => quote.id.0.as_bytes().to_vec(),
    };
    sha256_hex(&canonical_payload)
}

fn hash_entry_material(
    quote_id: &QuoteId,
    version: u32,
    content_hash: &str,
    prev_hash: Option<&str>,
    timestamp: DateTime<Utc>,
    actor_id: &str,
    action: &LedgerAction,
) -> String {
    let material = format!(
        "{}|{}|{}|{}|{}|{}|{}",
        quote_id.0,
        version,
        content_hash,
        prev_hash.unwrap_or(""),
        timestamp.to_rfc3339(),
        actor_id,
        action.as_key(),
    );
    sha256_hex(material.as_bytes())
}

fn hmac_hex(secret: &[u8], payload: &[u8]) -> String {
    let mut mac = match HmacSha256::new_from_slice(secret) {
        Ok(mac) => mac,
        Err(_) => return sha256_hex(payload),
    };
    mac.update(payload);
    encode_hex(mac.finalize().into_bytes().as_slice())
}

fn sha256_hex(payload: &[u8]) -> String {
    let digest = Sha256::digest(payload);
    encode_hex(digest.as_slice())
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rust_decimal::Decimal;

    use super::{LedgerAction, LedgerService};
    use crate::domain::product::ProductId;
    use crate::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};

    #[test]
    fn append_entry_produces_consistent_content_hash_for_same_quote() {
        let quote = sample_quote("Q-ledger-1", 10);
        let mut service_a = LedgerService::new("secret-key");
        let mut service_b = LedgerService::new("secret-key");

        let entry_a = service_a.append_entry(&quote, LedgerAction::Create, "u-ae");
        let entry_b = service_b.append_entry(&quote, LedgerAction::Create, "u-ae");

        assert_eq!(entry_a.content_hash, entry_b.content_hash);
        assert_eq!(entry_a.prev_hash, None);
    }

    #[test]
    fn append_entry_links_previous_hash_chain() {
        let mut service = LedgerService::new("secret-key");
        let quote_v1 = sample_quote("Q-ledger-2", 5);
        let quote_v2 = sample_quote("Q-ledger-2", 8);

        let entry_1 = service.append_entry(&quote_v1, LedgerAction::Create, "u-ae");
        let entry_2 = service.append_entry(&quote_v2, LedgerAction::Update, "u-manager");

        assert_eq!(entry_1.version, 1);
        assert_eq!(entry_2.version, 2);
        assert_eq!(entry_2.prev_hash, Some(entry_1.entry_hash));
    }

    #[test]
    fn verify_chain_succeeds_for_untampered_entries() {
        let mut service = LedgerService::new("secret-key");
        let quote_v1 = sample_quote("Q-ledger-3", 3);
        let quote_v2 = sample_quote("Q-ledger-3", 6);
        let quote_v3 = sample_quote("Q-ledger-3", 9);

        service.append_entry(&quote_v1, LedgerAction::Create, "u-ae");
        service.append_entry(&quote_v2, LedgerAction::Update, "u-manager");
        service.append_entry(&quote_v3, LedgerAction::Approve, "u-vp");

        let result = service.verify_chain(&QuoteId("Q-ledger-3".to_string()));
        assert!(result.valid);
        assert_eq!(result.verified_entries, 3);
        assert!(result.failure_reason.is_none());
    }

    #[test]
    fn verify_chain_detects_tampering() {
        let mut service = LedgerService::new("secret-key");
        let quote = sample_quote("Q-ledger-4", 12);

        service.append_entry(&quote, LedgerAction::Create, "u-ae");
        service.append_entry(&quote, LedgerAction::Update, "u-manager");

        let entries = service.entries_by_quote.get_mut("Q-ledger-4").expect("entries");
        entries[1].signature = "tampered-signature".to_string();

        let result = service.verify_chain(&QuoteId("Q-ledger-4".to_string()));
        assert!(!result.valid);
        assert!(result.failure_reason.unwrap_or_default().contains("signature mismatch"));
    }

    fn sample_quote(quote_id: &str, quantity: u32) -> Quote {
        Quote {
            id: QuoteId(quote_id.to_string()),
            status: QuoteStatus::Draft,
            lines: vec![QuoteLine {
                product_id: ProductId("starter".to_string()),
                quantity,
                unit_price: Decimal::new(9_999, 2),
            }],
            created_at: Utc::now(),
        }
    }
}
