-- Reverse migration: 0042_audit_enrichment
-- SQLite doesn't support DROP COLUMN before 3.35, so recreate the table.

DROP INDEX IF EXISTS idx_audit_event_entity;
DROP INDEX IF EXISTS idx_audit_event_action;
DROP INDEX IF EXISTS idx_audit_event_actor_type;

CREATE TABLE audit_event_old (
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

INSERT INTO audit_event_old (id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json)
SELECT id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json, metadata_json
FROM audit_event;

DROP INDEX IF EXISTS idx_audit_event_quote_id;
DROP INDEX IF EXISTS idx_audit_event_timestamp;
DROP INDEX IF EXISTS idx_audit_event_type;
DROP TABLE audit_event;

ALTER TABLE audit_event_old RENAME TO audit_event;

CREATE INDEX idx_audit_event_quote_id ON audit_event(quote_id);
CREATE INDEX idx_audit_event_timestamp ON audit_event(timestamp);
CREATE INDEX idx_audit_event_type ON audit_event(event_type);
