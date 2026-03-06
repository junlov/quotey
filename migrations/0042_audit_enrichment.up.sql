-- Migration: 0042_audit_enrichment
-- Description: Enrich audit_event with structured actor/entity/action dimensions
-- and before/after snapshots for mutation tracking (PC-4.1)
-- All columns are nullable for backward compatibility with existing events.

ALTER TABLE audit_event ADD COLUMN entity_type TEXT;
ALTER TABLE audit_event ADD COLUMN entity_id TEXT;
ALTER TABLE audit_event ADD COLUMN action TEXT;
ALTER TABLE audit_event ADD COLUMN before_json TEXT;
ALTER TABLE audit_event ADD COLUMN after_json TEXT;

CREATE INDEX idx_audit_event_entity ON audit_event(entity_type, entity_id);
CREATE INDEX idx_audit_event_action ON audit_event(action);
CREATE INDEX idx_audit_event_actor_type ON audit_event(actor_type);
