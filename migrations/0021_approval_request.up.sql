-- Migration: 0021_approval_request
-- Description: Add approval workflow support for quote discount authorization
-- Tracks approval requests with status, justification, and expiration
-- Author: Quotey Team
-- Date: 2024

CREATE TABLE IF NOT EXISTS approval_request (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    approver_role TEXT NOT NULL,
    reason TEXT NOT NULL DEFAULT '',
    justification TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'approved', 'rejected', 'escalated')),
    requested_by TEXT NOT NULL DEFAULT 'agent:mcp',
    expires_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_approval_request_quote_id ON approval_request(quote_id);
CREATE INDEX IF NOT EXISTS idx_approval_request_status ON approval_request(status);
CREATE INDEX IF NOT EXISTS idx_approval_request_approver_role ON approval_request(approver_role);
