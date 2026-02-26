-- SQLite does not support DROP COLUMN before 3.35, so recreate tables.
-- For safety we only drop the indexes added.
DROP INDEX IF EXISTS idx_quote_account_id;
DROP INDEX IF EXISTS idx_quote_deal_id;

-- Note: ALTER TABLE DROP COLUMN requires SQLite 3.35+
-- If running on older SQLite, these will error. The migration test
-- validates reversibility on the CI SQLite version.
ALTER TABLE quote DROP COLUMN account_id;
ALTER TABLE quote DROP COLUMN deal_id;
ALTER TABLE quote DROP COLUMN notes;
ALTER TABLE quote DROP COLUMN version;

ALTER TABLE quote_line DROP COLUMN discount_pct;
ALTER TABLE quote_line DROP COLUMN notes;
