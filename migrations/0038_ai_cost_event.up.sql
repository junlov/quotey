CREATE TABLE IF NOT EXISTS ai_cost_event (
    id TEXT PRIMARY KEY,
    quote_id TEXT,
    tool_name TEXT NOT NULL,
    model_name TEXT NOT NULL DEFAULT 'unknown',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER GENERATED ALWAYS AS (input_tokens + output_tokens) STORED,
    estimated_cost_cents REAL NOT NULL DEFAULT 0.0,
    actor_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE SET NULL
);

CREATE INDEX idx_ai_cost_event_quote_id ON ai_cost_event(quote_id);
CREATE INDEX idx_ai_cost_event_tool_name ON ai_cost_event(tool_name);
CREATE INDEX idx_ai_cost_event_created_at ON ai_cost_event(created_at);
