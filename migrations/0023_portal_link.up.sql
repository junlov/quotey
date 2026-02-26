-- Portal sharing links with time-limited tokens for customer-facing quote access.
CREATE TABLE portal_link (
    id         TEXT PRIMARY KEY NOT NULL,
    quote_id   TEXT NOT NULL,
    token      TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    revoked    INTEGER NOT NULL DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX idx_portal_link_token ON portal_link(token);
CREATE INDEX idx_portal_link_quote_id ON portal_link(quote_id);
