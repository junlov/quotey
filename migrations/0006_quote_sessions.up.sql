CREATE TABLE IF NOT EXISTS quote_sessions (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'paused', 'closed')),
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_quote_sessions_quote_id
    ON quote_sessions(quote_id);
CREATE INDEX IF NOT EXISTS idx_quote_sessions_status
    ON quote_sessions(status);

CREATE TABLE IF NOT EXISTS session_participants (
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    role TEXT NOT NULL,
    joined_at TEXT NOT NULL,
    last_activity TEXT NOT NULL,
    PRIMARY KEY (session_id, user_id),
    FOREIGN KEY (session_id) REFERENCES quote_sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_session_participants_user_id
    ON session_participants(user_id);

CREATE TABLE IF NOT EXISTS session_operations (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    operation_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES quote_sessions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_session_operations_session_id
    ON session_operations(session_id);
CREATE INDEX IF NOT EXISTS idx_session_operations_timestamp
    ON session_operations(timestamp);
