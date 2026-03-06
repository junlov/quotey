-- Reverse migration: 0040_rich_approval_types
-- Recreate original table without the new columns and with original CHECK

CREATE TABLE approval_request_old (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    approver_role TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    justification TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'approved', 'rejected', 'escalated')),
    requested_by TEXT NOT NULL DEFAULT 'agent:mcp',
    requested_by_sales_rep_id TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (requested_by_sales_rep_id) REFERENCES sales_rep(id) ON DELETE SET NULL
);

INSERT INTO approval_request_old
    (id, quote_id, approver_role, reason, justification, status, requested_by,
     requested_by_sales_rep_id, expires_at, created_at, updated_at)
SELECT id, quote_id, approver_role, reason, justification,
       CASE
           WHEN status = 'revision_requested' THEN 'pending'
           ELSE status
       END AS status,
       requested_by,
       requested_by_sales_rep_id, expires_at, created_at, updated_at
FROM approval_request;

DROP INDEX IF EXISTS idx_approval_request_type;
DROP INDEX IF EXISTS idx_approval_request_quote_id;
DROP INDEX IF EXISTS idx_approval_request_status;
DROP INDEX IF EXISTS idx_approval_request_approver_role;
DROP TABLE approval_request;

ALTER TABLE approval_request_old RENAME TO approval_request;

CREATE INDEX idx_approval_request_quote_id ON approval_request(quote_id);
CREATE INDEX idx_approval_request_status ON approval_request(status);
CREATE INDEX idx_approval_request_approver_role ON approval_request(approver_role);
CREATE INDEX idx_approval_request_requested_by_sales_rep_id ON approval_request(requested_by_sales_rep_id);
