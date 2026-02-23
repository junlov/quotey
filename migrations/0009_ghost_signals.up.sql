CREATE TABLE IF NOT EXISTS buying_signals (
    id TEXT PRIMARY KEY,
    slack_channel_id TEXT NOT NULL,
    slack_message_id TEXT NOT NULL,
    signal_type TEXT NOT NULL,
    confidence_score REAL NOT NULL CHECK (confidence_score >= 0.0 AND confidence_score <= 1.0),
    detected_company TEXT,
    extracted_intent TEXT NOT NULL,
    matched_rep_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('new', 'processed', 'dismissed')),
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_buying_signals_matched_rep_id
    ON buying_signals(matched_rep_id);
CREATE INDEX IF NOT EXISTS idx_buying_signals_status
    ON buying_signals(status);

CREATE TABLE IF NOT EXISTS ghost_quotes (
    id TEXT PRIMARY KEY,
    signal_id TEXT NOT NULL,
    draft_quote_id TEXT,
    confidence_score REAL NOT NULL CHECK (confidence_score >= 0.0 AND confidence_score <= 1.0),
    rep_notified_at TEXT,
    rep_response TEXT CHECK (rep_response IN ('accepted', 'dismissed')),
    created_at TEXT NOT NULL,
    FOREIGN KEY (signal_id) REFERENCES buying_signals(id) ON DELETE CASCADE,
    FOREIGN KEY (draft_quote_id) REFERENCES quote(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_ghost_quotes_signal_id
    ON ghost_quotes(signal_id);
