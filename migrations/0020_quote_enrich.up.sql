-- Add fields needed for MCP quote tools:
-- account_id, deal_id, notes, version
ALTER TABLE quote ADD COLUMN account_id TEXT;
ALTER TABLE quote ADD COLUMN deal_id TEXT;
ALTER TABLE quote ADD COLUMN notes TEXT;
ALTER TABLE quote ADD COLUMN version INTEGER NOT NULL DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_quote_account_id ON quote(account_id);
CREATE INDEX IF NOT EXISTS idx_quote_deal_id ON quote(deal_id);

-- Add discount_pct and notes to quote_line for line-level discounts
ALTER TABLE quote_line ADD COLUMN discount_pct REAL NOT NULL DEFAULT 0.0;
ALTER TABLE quote_line ADD COLUMN notes TEXT;
