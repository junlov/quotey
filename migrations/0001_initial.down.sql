DROP INDEX IF EXISTS idx_audit_event_type;
DROP INDEX IF EXISTS idx_audit_event_timestamp;
DROP INDEX IF EXISTS idx_audit_event_quote_id;
DROP TABLE IF EXISTS audit_event;

DROP INDEX IF EXISTS idx_flow_state_quote_id;
DROP TABLE IF EXISTS flow_state;

DROP INDEX IF EXISTS idx_quote_line_quote_id;
DROP TABLE IF EXISTS quote_line;

DROP INDEX IF EXISTS idx_quote_created_at;
DROP INDEX IF EXISTS idx_quote_status;
DROP TABLE IF EXISTS quote;
