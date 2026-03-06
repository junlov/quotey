-- Add index for funnel telemetry queries.
-- Funnel events use event_category = 'funnel' and are queried by
-- quote_id + event_type for drop-off analysis.
CREATE INDEX IF NOT EXISTS idx_audit_event_funnel
    ON audit_event(event_category, quote_id, event_type)
    WHERE event_category = 'funnel';
