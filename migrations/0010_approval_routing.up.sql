CREATE TABLE IF NOT EXISTS approval_authorities (
    user_id TEXT PRIMARY KEY,
    role TEXT NOT NULL,
    max_discount_pct REAL NOT NULL,
    max_deal_value REAL NOT NULL,
    account_tiers_json TEXT NOT NULL DEFAULT '[]',
    product_categories_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_approval_authorities_role
    ON approval_authorities(role);

CREATE TABLE IF NOT EXISTS org_hierarchy (
    user_id TEXT PRIMARY KEY,
    manager_id TEXT,
    department TEXT NOT NULL,
    level INTEGER NOT NULL CHECK (level >= 0),
    FOREIGN KEY (manager_id) REFERENCES org_hierarchy(user_id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_org_hierarchy_manager_id
    ON org_hierarchy(manager_id);

CREATE TABLE IF NOT EXISTS routing_rules (
    id TEXT PRIMARY KEY,
    criteria_type TEXT NOT NULL,
    criteria_value TEXT NOT NULL,
    approver_role TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_routing_rules_criteria
    ON routing_rules(criteria_type, criteria_value);
