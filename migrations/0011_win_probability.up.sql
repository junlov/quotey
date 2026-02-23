CREATE TABLE IF NOT EXISTS deal_outcomes (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('won', 'lost')),
    final_price REAL NOT NULL,
    close_date TEXT NOT NULL,
    customer_segment TEXT,
    product_mix_json TEXT NOT NULL DEFAULT '[]',
    sales_cycle_days INTEGER CHECK (sales_cycle_days >= 0),
    created_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_deal_outcomes_quote_id
    ON deal_outcomes(quote_id);
CREATE INDEX IF NOT EXISTS idx_deal_outcomes_outcome
    ON deal_outcomes(outcome);

CREATE TABLE IF NOT EXISTS win_probability_models (
    id TEXT PRIMARY KEY,
    version TEXT NOT NULL UNIQUE,
    training_date TEXT NOT NULL,
    accuracy_score REAL NOT NULL CHECK (accuracy_score >= 0.0 AND accuracy_score <= 1.0),
    model_path TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS prediction_cache (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    model_version TEXT NOT NULL,
    win_probability REAL NOT NULL CHECK (win_probability >= 0.0 AND win_probability <= 1.0),
    predicted_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prediction_cache_quote_model
    ON prediction_cache(quote_id, model_version);
