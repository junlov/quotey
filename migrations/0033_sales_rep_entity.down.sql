DROP INDEX IF EXISTS idx_approval_request_requested_by_sales_rep_id;
DROP INDEX IF EXISTS idx_quote_created_by_sales_rep_id;

-- Intentionally keep additive bridge columns on rollback.
-- SQLite's transactional DDL around FK-backed DROP COLUMN can fail under migrator undo.
-- Dropping bridge indexes and the sales_rep table cleanly disables rep bridge behavior.

DROP INDEX IF EXISTS idx_sales_rep_status;
DROP INDEX IF EXISTS idx_sales_rep_reports_to;
DROP INDEX IF EXISTS idx_sales_rep_role;
DROP INDEX IF EXISTS idx_sales_rep_external_ref;
DROP TABLE IF EXISTS sales_rep;
