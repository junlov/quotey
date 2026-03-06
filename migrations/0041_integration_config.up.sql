-- Migration: 0041_integration_config
-- Description: General-purpose adapter registry for pluggable integrations
-- Supports CRM, notification, PDF, and ERP adapter types through a single table

CREATE TABLE integration_config (
    id TEXT PRIMARY KEY,
    integration_type TEXT NOT NULL
        CHECK (integration_type IN ('crm', 'notification', 'pdf', 'erp')),
    adapter_type TEXT NOT NULL
        CHECK (adapter_type IN (
            'salesforce', 'hubspot', 'slack', 'teams', 'email',
            'webhook', 'builtin', 'docusign', 'netsuite', 'none'
        )),
    name TEXT NOT NULL DEFAULT '',
    adapter_config TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'active'
        CHECK (status IN ('active', 'inactive', 'error')),
    status_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_integration_config_type ON integration_config(integration_type);
CREATE INDEX idx_integration_config_adapter ON integration_config(adapter_type);
CREATE INDEX idx_integration_config_status ON integration_config(status);
