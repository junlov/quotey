# Immutable Quote Ledger (FEAT-08) - Deep Technical Research

**Feature:** Cryptographic Quote Integrity  
**Researcher:** ResearchAgent (kimi-k2)  
**Date:** 2026-02-23  
**Priority:** P1

---

## 1. Technical Overview

Every quote version is hashed and chained using cryptographic techniques (SHA-256, HMAC) to create a tamper-evident audit trail. Provides zero-trust quote integrity without external blockchain dependencies.

---

## 2. Cryptographic Architecture

### 2.1 Hash Chain Design

```rust
pub struct LedgerEntry {
    /// Unique identifier for this entry
    pub entry_id: Uuid,
    
    /// Quote being tracked
    pub quote_id: QuoteId,
    
    /// Version sequence number
    pub version: u32,
    
    /// SHA-256 hash of canonical quote content
    pub content_hash: String,  // 64 hex chars
    
    /// Hash of previous entry (None for genesis)
    pub prev_hash: Option<String>,
    
    /// Cryptographic timestamp
    pub timestamp: DateTime<Utc>,
    
    /// Actor who made the change
    pub actor_id: String,
    
    /// Type of action performed
    pub action: LedgerAction,
    
    /// HMAC-SHA256 signature of this entry
    pub signature: String,
    
    /// Optional: quote snapshot for quick retrieval
    pub quote_snapshot_json: Option<String>,
}

pub enum LedgerAction {
    Created,
    Updated { field_changes: Vec<FieldChange> },
    Approved { approver_id: String },
    Rejected { reason: String },
    Finalized,
    Sent { customer_email: String },
    Amended { parent_version: u32 },
    Custom(String),
}

pub struct FieldChange {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}
```

### 2.2 Hash Generation

```rust
pub struct HashGenerator {
    hasher: Sha256,
}

impl HashGenerator {
    /// Generate content hash from quote
    pub fn hash_quote(&self, quote: &Quote) -> String {
        // Create canonical representation (deterministic ordering)
        let canonical = CanonicalQuote::from(quote);
        let json = serde_json::to_string(&canonical).unwrap();
        
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        hex::encode(hasher.finalize())
    }
    
    /// Generate entry hash (includes chain linkage)
    pub fn hash_entry(&self, entry: &LedgerEntry) -> String {
        let data = format!(
            "{}|{}|{}|{}|{}|{}",
            entry.quote_id.0,
            entry.version,
            entry.content_hash,
            entry.prev_hash.as_deref().unwrap_or("genesis"),
            entry.timestamp.to_rfc3339(),
            entry.actor_id
        );
        
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Canonical representation ensures deterministic hashing
#[derive(Serialize)]
struct CanonicalQuote {
    id: String,
    version: u32,
    status: String,
    lines: Vec<CanonicalLine>,
    customer_id: String,
    total: String,  // Decimal as string for precision
}

impl From<&Quote> for CanonicalQuote {
    fn from(quote: &Quote) -> Self {
        // Sort lines by product_id for determinism
        let mut lines: Vec<_> = quote.lines.iter()
            .map(CanonicalLine::from)
            .collect();
        lines.sort_by(|a, b| a.product_id.cmp(&b.product_id));
        
        Self {
            id: quote.id.0.clone(),
            version: quote.version,
            status: format!("{:?}", quote.status),
            lines,
            customer_id: quote.customer_id.0.clone(),
            total: quote.total.to_string(),
        }
    }
}
```

### 2.3 HMAC Signature

```rust
pub struct LedgerSigner {
    signing_key: HmacSha256,
}

impl LedgerSigner {
    pub fn new(secret_key: &[u8]) -> Self {
        Self {
            signing_key: HmacSha256::new_from_slice(secret_key)
                .expect("Invalid key length"),
        }
    }
    
    /// Sign a ledger entry
    pub fn sign(&self, entry: &LedgerEntry, entry_hash: &str) -> String {
        let data = format!(
            "{}.{}.{}.{}.{}",
            entry.entry_id,
            entry.quote_id.0,
            entry.version,
            entry_hash,
            entry.timestamp.timestamp()
        );
        
        let mut mac = self.signing_key.clone();
        mac.update(data.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
    
    /// Verify entry signature
    pub fn verify(&self, entry: &LedgerEntry) -> bool {
        let expected = self.sign(entry, &entry.entry_hash());
        entry.signature == expected
    }
}
```

---

## 3. Database Schema

### 3.1 Core Tables

```sql
-- Ledger entries (append-only)
CREATE TABLE quote_ledger (
    id TEXT PRIMARY KEY,
    entry_id TEXT NOT NULL UNIQUE,
    quote_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    prev_hash TEXT,  -- NULL for genesis entry
    timestamp TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    action_details_json TEXT,
    signature TEXT NOT NULL,
    quote_snapshot_json TEXT,  -- Optional denormalization
    
    UNIQUE(quote_id, version)
);

CREATE INDEX idx_ledger_quote ON quote_ledger(quote_id, version DESC);
CREATE INDEX idx_ledger_hash ON quote_ledger(content_hash);
CREATE INDEX idx_ledger_timestamp ON quote_ledger(timestamp DESC);

-- Ledger verification metadata
CREATE TABLE ledger_verification (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    verification_type TEXT NOT NULL,  -- 'full_chain', 'single_entry'
    verified_at TEXT NOT NULL,
    verification_result BOOLEAN NOT NULL,
    broken_at_version INTEGER,  -- NULL if chain intact
    expected_hash TEXT,
    actual_hash TEXT,
    verifier_actor_id TEXT
);

CREATE INDEX idx_verification_quote ON ledger_verification(quote_id, verified_at DESC);

-- Chain integrity checkpoints (periodic snapshots)
CREATE TABLE ledger_checkpoints (
    id TEXT PRIMARY KEY,
    checkpoint_timestamp TEXT NOT NULL,
    quote_count INTEGER NOT NULL,
    entry_count INTEGER NOT NULL,
    cumulative_hash TEXT NOT NULL,  -- Merkle root of all entries
    signature TEXT NOT NULL
);

-- Tamper detection alerts
CREATE TABLE ledger_tamper_alerts (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    detected_at TEXT NOT NULL,
    alert_type TEXT NOT NULL,  -- 'hash_mismatch', 'signature_invalid', 'chain_broken'
    details_json TEXT NOT NULL,
    acknowledged BOOLEAN DEFAULT FALSE,
    acknowledged_by TEXT,
    acknowledged_at TEXT
);
```

### 3.2 Genesis Entry

```sql
-- Every quote starts with a genesis entry
INSERT INTO quote_ledger (
    id, entry_id, quote_id, version, content_hash,
    prev_hash, timestamp, actor_id, action_type, signature
) VALUES (
    'genesis-' || quote_id,
    uuid(),
    quote_id,
    1,
    content_hash,
    NULL,  -- No previous hash for genesis
    timestamp,
    actor_id,
    'Created',
    signature
);
```

---

## 4. Chain Verification

### 4.1 Full Chain Verification

```rust
pub struct ChainVerifier;

impl ChainVerifier {
    /// Verify entire chain for a quote
    pub async fn verify_chain(
        &self,
        quote_id: &QuoteId,
        ledger: &dyn LedgerRepository,
    ) -> Result<VerificationResult, VerificationError> {
        let entries = ledger.get_entries(quote_id).await?;
        
        if entries.is_empty() {
            return Ok(VerificationResult::EmptyChain);
        }
        
        // Verify genesis
        if entries[0].prev_hash.is_some() {
            return Ok(VerificationResult::Invalid {
                reason: "Genesis entry has prev_hash".to_string(),
                at_version: 1,
            });
        }
        
        // Verify chain linkage
        for window in entries.windows(2) {
            let prev = &window[0];
            let curr = &window[1];
            
            // Check version continuity
            if curr.version != prev.version + 1 {
                return Ok(VerificationResult::Broken {
                    reason: format!(
                        "Version gap: {} → {}",
                        prev.version, curr.version
                    ),
                    at_version: curr.version,
                });
            }
            
            // Check hash linkage
            let expected_prev_hash = self.hash_entry(prev);
            if curr.prev_hash.as_ref() != Some(&expected_prev_hash) {
                return Ok(VerificationResult::Broken {
                    reason: "Hash chain mismatch".to_string(),
                    at_version: curr.version,
                    expected: expected_prev_hash,
                    actual: curr.prev_hash.clone(),
                });
            }
            
            // Verify signature
            if !self.verify_signature(curr) {
                return Ok(VerificationResult::Invalid {
                    reason: "Invalid signature".to_string(),
                    at_version: curr.version,
                });
            }
        }
        
        Ok(VerificationResult::Valid {
            entry_count: entries.len(),
            chain_hash: self.compute_chain_hash(&entries),
        })
    }
    
    /// Quick verification of single entry
    pub fn verify_entry(&self, entry: &LedgerEntry, prev: Option<&LedgerEntry>) -> bool {
        // Verify signature
        if !self.verify_signature(entry) {
            return false;
        }
        
        // Verify chain link if not genesis
        if let Some(prev_entry) = prev {
            let expected_hash = self.hash_entry(prev_entry);
            if entry.prev_hash != Some(expected_hash) {
                return false;
            }
        } else if entry.prev_hash.is_some() {
            return false;  // Genesis shouldn't have prev_hash
        }
        
        true
    }
    
    fn compute_chain_hash(&self, entries: &[LedgerEntry]) -> String {
        let mut hasher = Sha256::new();
        for entry in entries {
            hasher.update(entry.content_hash.as_bytes());
        }
        hex::encode(hasher.finalize())
    }
}

pub enum VerificationResult {
    Valid { entry_count: usize, chain_hash: String },
    Broken { reason: String, at_version: u32, expected: Option<String>, actual: Option<String> },
    Invalid { reason: String, at_version: u32 },
    EmptyChain,
}
```

### 4.2 Merkle Tree for Batch Verification

```rust
pub struct MerkleTree {
    leaves: Vec<String>,
    tree: Vec<Vec<String>>,
}

impl MerkleTree {
    pub fn from_entries(entries: &[LedgerEntry]) -> Self {
        let leaves: Vec<String> = entries
            .iter()
            .map(|e| e.content_hash.clone())
            .collect();
        
        let tree = Self::build_tree(&leaves);
        
        Self { leaves, tree }
    }
    
    fn build_tree(leaves: &[String]) -> Vec<Vec<String>> {
        let mut tree = vec![leaves.to_vec()];
        
        while tree.last().unwrap().len() > 1 {
            let current = tree.last().unwrap();
            let mut next = Vec::new();
            
            for chunk in current.chunks(2) {
                let combined = if chunk.len() == 2 {
                    format!("{}{}", chunk[0], chunk[1])
                } else {
                    format!("{}{}", chunk[0], chunk[0])  // Duplicate if odd
                };
                
                let hash = Self::hash(&combined);
                next.push(hash);
            }
            
            tree.push(next);
        }
        
        tree
    }
    
    pub fn root(&self) -> Option<&String> {
        self.tree.last()?.first()
    }
    
    pub fn proof(&self, index: usize) -> MerkleProof {
        let mut proof = Vec::new();
        let mut idx = index;
        
        for level in &self.tree[..self.tree.len()-1] {
            let sibling = if idx % 2 == 0 {
                level.get(idx + 1).or_else(|| level.get(idx))
            } else {
                level.get(idx - 1)
            };
            
            if let Some(sib) = sibling {
                proof.push(sib.clone());
            }
            
            idx /= 2;
        }
        
        MerkleProof { leaf_index: index, path: proof }
    }
    
    fn hash(data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        hex::encode(hasher.finalize())
    }
}
```

---

## 5. Integration with Quote Lifecycle

### 5.1 Ledger Service

```rust
pub struct LedgerService {
    ledger_repo: Arc<dyn LedgerRepository>,
    hash_generator: HashGenerator,
    signer: LedgerSigner,
}

impl LedgerService {
    /// Record a new quote creation
    pub async fn record_creation(
        &self,
        quote: &Quote,
        actor_id: &str,
    ) -> Result<LedgerEntry, LedgerError> {
        let content_hash = self.hash_generator.hash_quote(quote);
        
        let entry = LedgerEntry {
            entry_id: Uuid::new_v4(),
            quote_id: quote.id.clone(),
            version: 1,
            content_hash,
            prev_hash: None,
            timestamp: Utc::now(),
            actor_id: actor_id.to_string(),
            action: LedgerAction::Created,
            signature: String::new(), // Will be set below
            quote_snapshot_json: Some(serde_json::to_string(quote)?),
        };
        
        // Sign the entry
        let entry_hash = self.hash_generator.hash_entry(&entry);
        let signature = self.signer.sign(&entry, &entry_hash);
        
        let signed_entry = LedgerEntry { signature, ..entry };
        
        // Persist
        self.ledger_repo.append_entry(&signed_entry).await?;
        
        Ok(signed_entry)
    }
    
    /// Record a quote update
    pub async fn record_update(
        &self,
        quote: &Quote,
        changes: Vec<FieldChange>,
        actor_id: &str,
    ) -> Result<LedgerEntry, LedgerError> {
        // Get previous entry
        let prev_entry = self.ledger_repo
            .get_latest_entry(&quote.id)
            .await?
            .ok_or(LedgerError::NoPreviousEntry)?;
        
        let content_hash = self.hash_generator.hash_quote(quote);
        let prev_hash = self.hash_generator.hash_entry(&prev_entry);
        
        let entry = LedgerEntry {
            entry_id: Uuid::new_v4(),
            quote_id: quote.id.clone(),
            version: prev_entry.version + 1,
            content_hash,
            prev_hash: Some(prev_hash),
            timestamp: Utc::now(),
            actor_id: actor_id.to_string(),
            action: LedgerAction::Updated { field_changes: changes },
            signature: String::new(),
            quote_snapshot_json: Some(serde_json::to_string(quote)?),
        };
        
        let entry_hash = self.hash_generator.hash_entry(&entry);
        let signature = self.signer.sign(&entry, &entry_hash);
        let signed_entry = LedgerEntry { signature, ..entry };
        
        self.ledger_repo.append_entry(&signed_entry).await?;
        
        Ok(signed_entry)
    }
    
    /// Verify quote integrity
    pub async fn verify_quote(
        &self,
        quote_id: &QuoteId,
    ) -> Result<VerificationReport, LedgerError> {
        let entries = self.ledger_repo.get_entries(quote_id).await?;
        
        let mut report = VerificationReport {
            quote_id: quote_id.clone(),
            entry_count: entries.len(),
            is_valid: true,
            issues: Vec::new(),
            chain_hash: String::new(),
        };
        
        if entries.is_empty() {
            report.is_valid = false;
            report.issues.push("No ledger entries found".to_string());
            return Ok(report);
        }
        
        // Verify chain
        let verifier = ChainVerifier;
        match verifier.verify_chain(quote_id, &self.ledger_repo).await? {
            VerificationResult::Valid { chain_hash, .. } => {
                report.chain_hash = chain_hash;
            }
            VerificationResult::Broken { reason, at_version, .. } => {
                report.is_valid = false;
                report.issues.push(format!(
                    "Chain broken at version {}: {}",
                    at_version, reason
                ));
            }
            VerificationResult::Invalid { reason, at_version } => {
                report.is_valid = false;
                report.issues.push(format!(
                    "Invalid entry at version {}: {}",
                    at_version, reason
                ));
            }
            _ => {}
        }
        
        Ok(report)
    }
}
```

### 5.2 Integration with Flow Engine

```rust
impl FlowEngine {
    pub async fn transition_with_ledger(
        &self,
        quote: &mut Quote,
        target: QuoteStatus,
        actor_id: &str,
        ledger: &LedgerService,
    ) -> Result<TransitionResult, FlowError> {
        // Perform the transition
        let result = self.transition(quote, target)?;
        
        // Record in ledger
        let action = match target {
            QuoteStatus::Draft => LedgerAction::Created,
            QuoteStatus::Approved => LedgerAction::Approved { 
                approver_id: actor_id.to_string() 
            },
            QuoteStatus::Finalized => LedgerAction::Finalized,
            QuoteStatus::Sent => LedgerAction::Sent { 
                customer_email: quote.customer_email.clone().unwrap_or_default() 
            },
            _ => LedgerAction::Custom(format!("Status: {:?}", target)),
        };
        
        ledger.record_action(quote, action, actor_id).await?;
        
        Ok(result)
    }
}
```

---

## 6. Verification API

### 6.1 Public Verification Endpoint

```rust
pub struct VerificationController {
    ledger_service: Arc<LedgerService>,
}

impl VerificationController {
    /// Public verification endpoint
    pub async fn verify(
        &self,
        quote_id: &str,
    ) -> Result<VerificationResponse, VerificationError> {
        let quote_id = QuoteId(quote_id.to_string());
        let report = self.ledger_service.verify_quote(&quote_id).await?;
        
        Ok(VerificationResponse {
            quote_id: quote_id.0,
            is_valid: report.is_valid,
            version_count: report.entry_count,
            chain_hash: report.chain_hash,
            issues: report.issues,
            verified_at: Utc::now(),
        })
    }
    
    /// Get full audit trail
    pub async fn audit_trail(
        &self,
        quote_id: &str,
    ) -> Result<AuditTrailResponse, VerificationError> {
        let quote_id = QuoteId(quote_id.to_string());
        let entries = self.ledger_service.get_entries(&quote_id).await?;
        
        let trail: Vec<AuditEntry> = entries.iter()
            .map(|e| AuditEntry {
                version: e.version,
                action: format!("{:?}", e.action),
                actor: e.actor_id.clone(),
                timestamp: e.timestamp,
                hash: e.content_hash.clone(),
                prev_hash: e.prev_hash.clone(),
            })
            .collect();
        
        Ok(AuditTrailResponse {
            quote_id: quote_id.0,
            entries: trail,
            chain_valid: self.verify_chain_integrity(&entries),
        })
    }
}

#[derive(Serialize)]
pub struct VerificationResponse {
    pub quote_id: String,
    pub is_valid: bool,
    pub version_count: usize,
    pub chain_hash: String,
    pub issues: Vec<String>,
    pub verified_at: DateTime<Utc>,
}
```

### 6.2 Slack Verification Command

```rust
pub fn render_verification_result(
    report: &VerificationReport,
) -> MessageTemplate {
    let status_emoji = if report.is_valid { "✅" } else { "❌" };
    
    let mut builder = MessageBuilder::new("Quote Verification")
        .section("header", |s| {
            s.mrkdwn(format!(
                "{} **Quote Integrity Verification**\n\n*Quote:* {}",
                status_emoji,
                report.quote_id.0
            ))
        });
    
    if report.is_valid {
        builder = builder.section("details", |s| {
            s.mrkdwn(format!(
                "*Status:* Untampered\n*Versions:* {}\n*Chain Hash:* `{}...`",
                report.entry_count,
                &report.chain_hash[..16]
            ))
        });
    } else {
        builder = builder.section("issues", |s| {
            let issues = report.issues.join("\n• ");
            s.mrkdwn(format!("*Issues Detected:*\n• {}", issues))
        });
    }
    
    builder.context("footer", |c| {
        c.mrkdwn(format!("Verified at {}", Utc::now().format("%H:%M UTC")))
    })
    .build()
}
```

---

## 7. Security Considerations

| Threat | Mitigation |
|--------|------------|
| Hash collision attack | Use SHA-256 (collision-resistant) |
| Signature forgery | HMAC with secure key rotation |
| Replay attacks | Timestamp validation + nonce |
| Key compromise | Hardware security module (HSM) support |
| Database tampering | Chain verification detects any change |
| Audit log deletion | Append-only table constraints |

### 7.1 Key Management

```rust
pub struct KeyManager {
    current_key: HmacKey,
    key_history: Vec<(DateTime<Utc>, HmacKey)>,
}

impl KeyManager {
    /// Rotate signing key periodically
    pub fn rotate_key(&mut self) -> Result<(), KeyError> {
        let new_key = HmacKey::generate()?;
        
        // Store old key for verification
        self.key_history.push((Utc::now(), self.current_key.clone()));
        
        // Update current key
        self.current_key = new_key;
        
        Ok(())
    }
    
    /// Get appropriate key for verification
    pub fn get_verification_key(
        &self,
        timestamp: DateTime<Utc>,
    ) -> Option<&HmacKey> {
        // Find key that was active at timestamp
        for (rotated_at, key) in &self.key_history {
            if timestamp < *rotated_at {
                return Some(key);
            }
        }
        Some(&self.current_key)
    }
}
```

---

## 8. Performance

| Operation | Target |
|-----------|--------|
| Hash generation | <5ms |
| Entry append | <20ms |
| Chain verification | O(n), <100ms for 100 entries |
| Merkle proof | O(log n) |
| Full audit export | <5s |

---

*Research compiled by ResearchAgent for the quotey project.*
