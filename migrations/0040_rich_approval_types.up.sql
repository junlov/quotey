-- Migration: 0040_rich_approval_types
-- Description: Add typed approvals with payload and revision cycle
-- Adds approval_type, payload_json, decision_note columns to approval_request
-- Adds revision_requested status via table recreation (SQLite can't alter CHECK)

-- Step 1: Create new table with updated schema
CREATE TABLE approval_request_new (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    approver_role TEXT NOT NULL,
    approval_type TEXT NOT NULL DEFAULT 'discount_override'
        CHECK (approval_type IN (
            'discount_override',
            'price_exception',
            'non_standard_terms',
            'custom_bundle',
            'competitor_match',
            'executive_escalation'
        )),
    reason TEXT NOT NULL DEFAULT '',
    justification TEXT NOT NULL DEFAULT '',
    payload_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'escalated', 'revision_requested')),
    decision_note TEXT,
    requested_by TEXT NOT NULL DEFAULT 'agent:mcp',
    requested_by_sales_rep_id TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (requested_by_sales_rep_id) REFERENCES sales_rep(id) ON DELETE SET NULL
);

-- Step 2: Copy data from old table
INSERT INTO approval_request_new
    (id, quote_id, approver_role, reason, justification, status, requested_by,
     requested_by_sales_rep_id, expires_at, created_at, updated_at)
SELECT id, quote_id, approver_role, reason, justification, status, requested_by,
       requested_by_sales_rep_id, expires_at, created_at, updated_at
FROM approval_request;

-- Step 3: Drop old table and indexes
DROP INDEX IF EXISTS idx_approval_request_quote_id;
DROP INDEX IF EXISTS idx_approval_request_status;
DROP INDEX IF EXISTS idx_approval_request_approver_role;
DROP INDEX IF EXISTS idx_approval_request_requested_by_sales_rep_id;
DROP TABLE approval_request;

-- Step 4: Rename new table
ALTER TABLE approval_request_new RENAME TO approval_request;

-- Step 5: Recreate indexes
CREATE INDEX idx_approval_request_quote_id ON approval_request(quote_id);
CREATE INDEX idx_approval_request_status ON approval_request(status);
CREATE INDEX idx_approval_request_approver_role ON approval_request(approver_role);
CREATE INDEX idx_approval_request_type ON approval_request(approval_type);
CREATE INDEX idx_approval_request_requested_by_sales_rep_id ON approval_request(requested_by_sales_rep_id);
