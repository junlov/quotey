CREATE TABLE IF NOT EXISTS quote_comment (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id) ON DELETE CASCADE,
    author_type TEXT NOT NULL CHECK (author_type IN ('rep', 'manager', 'system', 'ai', 'integration')),
    author_id TEXT NOT NULL,
    body TEXT NOT NULL,
    metadata_json TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_quote_comment_quote_id ON quote_comment(quote_id, created_at);
CREATE INDEX IF NOT EXISTS idx_quote_comment_author ON quote_comment(author_type, author_id);
