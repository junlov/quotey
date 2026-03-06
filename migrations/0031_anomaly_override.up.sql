-- Anomaly override: reps can acknowledge and override anomaly flags with justification.
-- All overrides are audit-logged for compliance.

CREATE TABLE IF NOT EXISTS anomaly_override (
    id            TEXT    PRIMARY KEY NOT NULL,
    quote_id      TEXT    NOT NULL REFERENCES quote(id),
    rule_kind     TEXT    NOT NULL,            -- 'discount', 'margin', 'quantity', 'price'
    severity      TEXT    NOT NULL,            -- 'info', 'warning', 'critical'
    justification TEXT    NOT NULL,
    overridden_by TEXT    NOT NULL,            -- user/rep who overrode
    created_at    TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_anomaly_override_quote_id ON anomaly_override(quote_id);
CREATE INDEX IF NOT EXISTS idx_anomaly_override_by       ON anomaly_override(overridden_by);
