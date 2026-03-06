-- 0036: Add locking columns to quote table for concurrent editing safety (PC-1.4).
-- Locks are time-bounded and auto-expire when lock_expires_at < NOW().

ALTER TABLE quote ADD COLUMN locked_by TEXT;
ALTER TABLE quote ADD COLUMN locked_at TEXT;
ALTER TABLE quote ADD COLUMN lock_expires_at TEXT;
