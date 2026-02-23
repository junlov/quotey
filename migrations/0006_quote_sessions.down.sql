DROP INDEX IF EXISTS idx_session_operations_timestamp;
DROP INDEX IF EXISTS idx_session_operations_session_id;
DROP TABLE IF EXISTS session_operations;

DROP INDEX IF EXISTS idx_session_participants_user_id;
DROP TABLE IF EXISTS session_participants;

DROP INDEX IF EXISTS idx_quote_sessions_status;
DROP INDEX IF EXISTS idx_quote_sessions_quote_id;
DROP TABLE IF EXISTS quote_sessions;
