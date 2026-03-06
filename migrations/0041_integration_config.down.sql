-- Reverse migration: 0041_integration_config
DROP INDEX IF EXISTS idx_integration_config_type;
DROP INDEX IF EXISTS idx_integration_config_adapter;
DROP INDEX IF EXISTS idx_integration_config_status;
DROP TABLE IF EXISTS integration_config;
