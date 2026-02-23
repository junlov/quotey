DROP INDEX IF EXISTS idx_execution_queue_transition_quote_occurred;
DROP INDEX IF EXISTS idx_execution_queue_transition_task_occurred;
DROP TABLE IF EXISTS execution_queue_transition_audit;

DROP INDEX IF EXISTS idx_execution_idempotency_expires_at;
DROP INDEX IF EXISTS idx_execution_idempotency_quote_state;
DROP TABLE IF EXISTS execution_idempotency_ledger;

DROP INDEX IF EXISTS idx_execution_queue_task_idempotency_key;
DROP INDEX IF EXISTS idx_execution_queue_task_quote_state_available;
DROP TABLE IF EXISTS execution_queue_task;
