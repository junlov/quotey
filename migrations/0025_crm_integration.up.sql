-- CRM integration infrastructure
-- Stores OAuth credentials, field mappings, and sync events between Quotey and CRM systems.

CREATE TABLE IF NOT EXISTS crm_integration (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL CHECK(provider IN ('salesforce', 'hubspot')),
    status TEXT NOT NULL DEFAULT 'disconnected' CHECK(status IN ('disconnected', 'connected', 'pending', 'revoked', 'error')),
    crm_account_id TEXT,
    instance_url TEXT,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    token_type TEXT NOT NULL DEFAULT 'Bearer',
    scope TEXT,
    token_expires_at TEXT,
    last_error TEXT,
    last_synced_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (provider)
);

CREATE INDEX idx_crm_integration_provider ON crm_integration(provider);
CREATE INDEX idx_crm_integration_status ON crm_integration(status);

CREATE TABLE IF NOT EXISTS crm_oauth_state (
    state_token TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL CHECK(provider IN ('salesforce', 'hubspot')),
    redirect_uri TEXT NOT NULL,
    scope TEXT,
    code_verifier TEXT,
    requested_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT NOT NULL,
    used INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_crm_oauth_state_expires_at ON crm_oauth_state(expires_at);
CREATE INDEX idx_crm_oauth_state_provider ON crm_oauth_state(provider);

CREATE TABLE IF NOT EXISTS crm_field_mapping (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL CHECK(provider IN ('salesforce', 'hubspot')),
    direction TEXT NOT NULL CHECK(direction IN ('quotey_to_crm', 'crm_to_quotey')),
    quotey_field TEXT NOT NULL,
    crm_field TEXT NOT NULL,
    description TEXT,
    expression TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE (provider, direction, quotey_field, crm_field)
);

CREATE INDEX idx_crm_field_mapping_provider_direction
    ON crm_field_mapping(provider, direction, is_active);
CREATE INDEX idx_crm_field_mapping_quotey_field ON crm_field_mapping(quotey_field);
CREATE INDEX idx_crm_field_mapping_crm_field ON crm_field_mapping(crm_field);

CREATE TABLE IF NOT EXISTS crm_sync_event (
    id TEXT PRIMARY KEY NOT NULL,
    provider TEXT NOT NULL CHECK(provider IN ('salesforce', 'hubspot')),
    direction TEXT NOT NULL CHECK(direction IN ('quotey_to_crm', 'crm_to_quotey')),
    event_type TEXT NOT NULL,
    quote_id TEXT,
    crm_object_type TEXT,
    crm_object_id TEXT,
    payload_json TEXT NOT NULL DEFAULT ('{}'),
    status TEXT NOT NULL DEFAULT 'queued' CHECK(status IN ('queued', 'running', 'success', 'failed', 'retrying', 'skipped')),
    attempts INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE SET NULL
);

CREATE INDEX idx_crm_sync_event_provider ON crm_sync_event(provider);
CREATE INDEX idx_crm_sync_event_direction ON crm_sync_event(direction);
CREATE INDEX idx_crm_sync_event_status ON crm_sync_event(status);
CREATE INDEX idx_crm_sync_event_quote_id ON crm_sync_event(quote_id);
CREATE INDEX idx_crm_sync_event_created_at ON crm_sync_event(created_at);
