-- Immutable quote pricing snapshots used by Explain Any Number deterministic evidence.
-- Includes optional ledger linkage to validate quote-version provenance.

CREATE TABLE IF NOT EXISTS quote_pricing_snapshot (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version >= 1),
    ledger_entry_id TEXT,
    ledger_content_hash TEXT,
    subtotal REAL NOT NULL,
    discount_total REAL NOT NULL DEFAULT 0,
    tax_total REAL NOT NULL DEFAULT 0,
    total REAL NOT NULL,
    currency TEXT NOT NULL,
    price_book_id TEXT,
    pricing_trace_json TEXT NOT NULL,
    policy_evaluation_json TEXT,
    priced_at TEXT NOT NULL,
    priced_by TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (ledger_entry_id) REFERENCES quote_ledger(entry_id) ON DELETE SET NULL,
    UNIQUE (quote_id, version)
);

CREATE INDEX IF NOT EXISTS idx_quote_pricing_snapshot_quote_version
    ON quote_pricing_snapshot(quote_id, version);
CREATE INDEX IF NOT EXISTS idx_quote_pricing_snapshot_ledger_entry
    ON quote_pricing_snapshot(ledger_entry_id);
