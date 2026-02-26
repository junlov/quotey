-- Migration: 0007_quote_ledger
-- Description: Add immutable quote ledger for audit trail
-- Stores content-addressed quote versions for deterministic replay
-- Author: Quotey Team
-- Date: 2024

CREATE TABLE IF NOT EXISTS quote_ledger (
    entry_id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    content_hash TEXT NOT NULL,
    prev_hash TEXT,
    actor_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    signature TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_quote_ledger_quote_id
    ON quote_ledger(quote_id);
CREATE INDEX IF NOT EXISTS idx_quote_ledger_content_hash
    ON quote_ledger(content_hash);

CREATE TABLE IF NOT EXISTS ledger_verifications (
    entry_id TEXT NOT NULL,
    verified_at TEXT NOT NULL,
    verification_result TEXT NOT NULL,
    details_json TEXT,
    PRIMARY KEY (entry_id, verified_at),
    FOREIGN KEY (entry_id) REFERENCES quote_ledger(entry_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ledger_verifications_entry_id
    ON ledger_verifications(entry_id);
