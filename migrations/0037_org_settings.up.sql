CREATE TABLE IF NOT EXISTS org_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL,
    description TEXT,
    updated_at TEXT NOT NULL,
    updated_by TEXT
);

-- Seed default settings
INSERT OR IGNORE INTO org_settings (key, value_json, description, updated_at) VALUES
    ('require_manager_approval_above_discount_pct', '0.10', 'Discount % threshold requiring manager approval', datetime('now')),
    ('require_finance_approval_above_deal_value_cents', '10000000', 'Deal value in cents requiring finance approval', datetime('now')),
    ('auto_approve_standard_pricing', 'true', 'Auto-approve quotes with no discount or policy violation', datetime('now')),
    ('allow_custom_line_items', 'false', 'Allow reps to add custom (non-catalog) line items', datetime('now')),
    ('max_quote_age_days', '90', 'Maximum age in days before a quote expires', datetime('now')),
    ('stale_draft_threshold_days', '7', 'Days before a draft quote is flagged as stale', datetime('now')),
    ('auto_expire_sent_days', '30', 'Days before a sent quote auto-expires', datetime('now')),
    ('approval_sla_hours', '4', 'Hours before a pending approval auto-escalates', datetime('now'));
