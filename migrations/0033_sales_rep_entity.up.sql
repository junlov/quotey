-- 0033: Sales rep profile entity and additive rep-id bridge columns.
-- This migration introduces first-class rep identity for human-owned quote flows
-- while preserving generic actor_id/audit semantics for system actors.

CREATE TABLE IF NOT EXISTS sales_rep (
    id TEXT PRIMARY KEY,
    external_user_ref TEXT UNIQUE,
    name TEXT NOT NULL,
    email TEXT,
    role TEXT NOT NULL CHECK (role IN ('ae', 'se', 'manager', 'vp', 'cro', 'ops')),
    title TEXT,
    team_id TEXT,
    reports_to TEXT,
    status TEXT NOT NULL CHECK (status IN ('active', 'inactive', 'disabled')),
    max_discount_pct REAL,
    auto_approve_threshold_cents INTEGER,
    capabilities_json TEXT NOT NULL DEFAULT '[]',
    config_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (reports_to) REFERENCES sales_rep(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_sales_rep_external_ref
    ON sales_rep(external_user_ref);
CREATE INDEX IF NOT EXISTS idx_sales_rep_role
    ON sales_rep(role);
CREATE INDEX IF NOT EXISTS idx_sales_rep_reports_to
    ON sales_rep(reports_to);
CREATE INDEX IF NOT EXISTS idx_sales_rep_status
    ON sales_rep(status);

ALTER TABLE quote ADD COLUMN created_by_sales_rep_id TEXT
    REFERENCES sales_rep(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_quote_created_by_sales_rep_id
    ON quote(created_by_sales_rep_id);

ALTER TABLE approval_request ADD COLUMN requested_by_sales_rep_id TEXT
    REFERENCES sales_rep(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_approval_request_requested_by_sales_rep_id
    ON approval_request(requested_by_sales_rep_id);

-- Deterministic bridge backfill:
-- map legacy actor strings when they already match a known sales_rep id or external ref.
UPDATE quote
SET created_by_sales_rep_id = (
    SELECT sr.id
    FROM sales_rep sr
    WHERE sr.id = quote.created_by OR sr.external_user_ref = quote.created_by
    LIMIT 1
)
WHERE created_by_sales_rep_id IS NULL;

UPDATE approval_request
SET requested_by_sales_rep_id = (
    SELECT sr.id
    FROM sales_rep sr
    WHERE sr.id = approval_request.requested_by OR sr.external_user_ref = approval_request.requested_by
    LIMIT 1
)
WHERE requested_by_sales_rep_id IS NULL;
