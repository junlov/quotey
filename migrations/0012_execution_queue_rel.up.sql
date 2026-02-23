CREATE TABLE IF NOT EXISTS execution_queue_task (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    operation_kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    state TEXT NOT NULL CHECK (
        state IN ('queued', 'running', 'retryable_failed', 'failed_terminal', 'completed')
    ),
    retry_count INTEGER NOT NULL DEFAULT 0 CHECK (retry_count >= 0),
    max_retries INTEGER NOT NULL DEFAULT 0 CHECK (max_retries >= 0),
    available_at TEXT NOT NULL,
    claimed_by TEXT,
    claimed_at TEXT,
    last_error TEXT,
    result_fingerprint TEXT,
    state_version INTEGER NOT NULL DEFAULT 1 CHECK (state_version >= 1),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_execution_queue_task_quote_state_available
    ON execution_queue_task(quote_id, state, available_at);
CREATE INDEX IF NOT EXISTS idx_execution_queue_task_idempotency_key
    ON execution_queue_task(idempotency_key);

CREATE TABLE IF NOT EXISTS execution_idempotency_ledger (
    operation_key TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    operation_kind TEXT NOT NULL,
    payload_hash TEXT NOT NULL,
    state TEXT NOT NULL CHECK (
        state IN ('reserved', 'running', 'completed', 'failed_retryable', 'failed_terminal')
    ),
    attempt_count INTEGER NOT NULL DEFAULT 1 CHECK (attempt_count >= 1),
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    result_snapshot_json TEXT,
    error_snapshot_json TEXT,
    expires_at TEXT,
    correlation_id TEXT NOT NULL,
    created_by_component TEXT NOT NULL,
    updated_by_component TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_execution_idempotency_quote_state
    ON execution_idempotency_ledger(quote_id, state);
CREATE INDEX IF NOT EXISTS idx_execution_idempotency_expires_at
    ON execution_idempotency_ledger(expires_at);

CREATE TABLE IF NOT EXISTS execution_queue_transition_audit (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    quote_id TEXT NOT NULL,
    from_state TEXT,
    to_state TEXT NOT NULL CHECK (
        to_state IN ('queued', 'running', 'retryable_failed', 'failed_terminal', 'completed')
    ),
    transition_reason TEXT NOT NULL,
    error_class TEXT,
    decision_context_json TEXT NOT NULL DEFAULT '{}',
    actor_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    idempotency_key TEXT,
    correlation_id TEXT NOT NULL,
    state_version INTEGER NOT NULL CHECK (state_version >= 1),
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (task_id) REFERENCES execution_queue_task(id) ON DELETE CASCADE,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_execution_queue_transition_task_occurred
    ON execution_queue_transition_audit(task_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_execution_queue_transition_quote_occurred
    ON execution_queue_transition_audit(quote_id, occurred_at);
