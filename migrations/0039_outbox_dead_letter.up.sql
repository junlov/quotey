-- Up migration: create outbox dead letter table for manual intervention
-- This stores failed operations that exceeded max retries

CREATE TABLE outbox_dead_letter (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    operation_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    
    -- Failure context
    failed_at TEXT NOT NULL,
    failure_reason TEXT NOT NULL,
    error_class TEXT,
    stack_trace TEXT,
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 0,
    
    -- Manual intervention
    resolution_status TEXT NOT NULL DEFAULT 'pending',
    resolved_by TEXT,
    resolved_at TEXT,
    resolution_notes TEXT,
    
    -- Audit
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    original_created_at TEXT NOT NULL,
    
    FOREIGN KEY (quote_id) REFERENCES quote(id)
);

-- Indexes for common queries
CREATE INDEX idx_outbox_dl_resolution ON outbox_dead_letter(resolution_status, failed_at);
CREATE INDEX idx_outbox_dl_quote ON outbox_dead_letter(quote_id);
CREATE INDEX idx_outbox_dl_idempotency ON outbox_dead_letter(idempotency_key);
