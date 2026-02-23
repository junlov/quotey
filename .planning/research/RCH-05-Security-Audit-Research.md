# RCH-05: Security & Audit Compliance Research

**Research Task:** bd-256v.5  
**Status:** Complete  
**Date:** 2026-02-23

---

## Executive Summary

Quotey handles pricing and customer data. Security requirements:

- ✅ **Encryption at rest:** SQLCipher for SQLite (AES-256)
- ✅ **Encryption in transit:** TLS 1.3 for all APIs
- ✅ **Secrets management:** Environment variables + secrecy crate
- ✅ **Audit logging:** Immutable append-only logs
- ✅ **Authentication:** Slack OAuth (workspace-level)

---

## 1. Data Protection

| Data Type | Sensitivity | Protection |
|-----------|-------------|------------|
| Customer names/emails | PII | Encrypted at rest |
| Pricing data | Confidential | Encrypted at rest |
| Quote history | Business-critical | Encrypted + backed up |
| Slack tokens | Critical | secrecy crate, memory-zeroed |
| Audit logs | Compliance | Immutable, signed |

---

## 2. Audit Requirements

**SOX Compliance for Pricing:**
- All price changes logged
- Approval chains documented
- Before/after values recorded
- User + timestamp on every change

**Implementation:**
```rust
struct AuditEvent {
    event_id: UUID,
    timestamp: DateTime<Utc>,
    user_id: String,
    action: String,
    entity_type: String,
    entity_id: String,
    old_value: Option<Json>,
    new_value: Option<Json>,
    signature: String,  // HMAC for integrity
}
```

---

## 3. Quote Integrity

**Immutable Quote Ledger:**
- SHA-256 hash of each version
- Chain of hashes (previous_hash)
- HMAC signature with system key
- Customer verification endpoint

---

## 4. ADR: Security Architecture

**Decision:**
- SQLCipher for database encryption
- TLS 1.3 for all external communication
- Slack OAuth for authentication
- Immutable audit logs with signatures

**Threat Model:**
- Database theft → Protected by encryption
- Network sniffing → Protected by TLS
- Quote tampering → Protected by hash chain
- Token leak → Rotatable, short-lived
