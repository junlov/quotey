CREATE TABLE IF NOT EXISTS emoji_approvals (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    approver_user_id TEXT NOT NULL,
    emoji TEXT NOT NULL CHECK (emoji IN ('👍', '👎', '💬')),
    slack_message_id TEXT NOT NULL,
    slack_timestamp TEXT NOT NULL,
    approval_type TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    undo_until TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_emoji_approvals_quote_id
    ON emoji_approvals(quote_id);
CREATE INDEX IF NOT EXISTS idx_emoji_approvals_approver_user_id
    ON emoji_approvals(approver_user_id);

CREATE TABLE IF NOT EXISTS approval_audit_log (
    id TEXT PRIMARY KEY,
    emoji_approval_id TEXT NOT NULL,
    quote_id TEXT NOT NULL,
    actor_user_id TEXT NOT NULL,
    action_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (emoji_approval_id) REFERENCES emoji_approvals(id) ON DELETE CASCADE,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_approval_audit_log_quote_id
    ON approval_audit_log(quote_id);
