-- Rollback CRM integration tables.

DROP INDEX IF EXISTS idx_crm_sync_event_created_at;
DROP INDEX IF EXISTS idx_crm_sync_event_quote_id;
DROP INDEX IF EXISTS idx_crm_sync_event_status;
DROP INDEX IF EXISTS idx_crm_sync_event_direction;
DROP INDEX IF EXISTS idx_crm_sync_event_provider;
DROP TABLE IF EXISTS crm_sync_event;

DROP INDEX IF EXISTS idx_crm_field_mapping_crm_field;
DROP INDEX IF EXISTS idx_crm_field_mapping_quotey_field;
DROP INDEX IF EXISTS idx_crm_field_mapping_provider_direction;
DROP TABLE IF EXISTS crm_field_mapping;

DROP INDEX IF EXISTS idx_crm_oauth_state_provider;
DROP INDEX IF EXISTS idx_crm_oauth_state_expires_at;
DROP TABLE IF EXISTS crm_oauth_state;

DROP INDEX IF EXISTS idx_crm_integration_status;
DROP INDEX IF EXISTS idx_crm_integration_provider;
DROP TABLE IF EXISTS crm_integration;
