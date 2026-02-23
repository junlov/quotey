CREATE TABLE IF NOT EXISTS quote (
    id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'draft',
    currency TEXT NOT NULL DEFAULT 'USD',
    start_date TEXT,
    end_date TEXT,
    term_months INTEGER,
    valid_until TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_quote_status ON quote(status);
CREATE INDEX IF NOT EXISTS idx_quote_created_at ON quote(created_at);

CREATE TABLE IF NOT EXISTS quote_line (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price REAL,
    subtotal REAL,
    attributes_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_quote_line_quote_id ON quote_line(quote_id);

CREATE TABLE IF NOT EXISTS flow_state (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    flow_type TEXT NOT NULL,
    current_step TEXT NOT NULL,
    step_number INTEGER NOT NULL,
    required_fields_json TEXT,
    missing_fields_json TEXT,
    last_prompt TEXT,
    last_user_input TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_flow_state_quote_id ON flow_state(quote_id);

CREATE TABLE IF NOT EXISTS audit_event (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    actor TEXT NOT NULL,
    actor_type TEXT NOT NULL,
    quote_id TEXT,
    event_type TEXT NOT NULL,
    event_category TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    metadata_json TEXT,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_event_quote_id ON audit_event(quote_id);
CREATE INDEX IF NOT EXISTS idx_audit_event_timestamp ON audit_event(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_event_type ON audit_event(event_type);
