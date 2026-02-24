-- Explanation requests and responses for "Explain Any Number" feature
-- Supports deterministic pricing trace and policy evidence assembly

CREATE TABLE IF NOT EXISTS explanation_requests (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    line_id TEXT,  -- NULL for total explanations
    request_type TEXT NOT NULL CHECK (request_type IN ('total', 'line', 'policy')),
    thread_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    quote_version INTEGER NOT NULL,
    pricing_snapshot_id TEXT,  -- References quote_pricing_snapshot
    status TEXT NOT NULL CHECK (status IN ('pending', 'success', 'error', 'missing_evidence')),
    error_code TEXT,  -- NULL unless status = 'error'
    error_message TEXT,  -- NULL unless status = 'error'
    latency_ms INTEGER CHECK (latency_ms >= 0),
    created_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (pricing_snapshot_id) REFERENCES quote_pricing_snapshot(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_explanation_requests_quote
    ON explanation_requests(quote_id, created_at);
CREATE INDEX IF NOT EXISTS idx_explanation_requests_thread
    ON explanation_requests(thread_id);
CREATE INDEX IF NOT EXISTS idx_explanation_requests_correlation
    ON explanation_requests(correlation_id);
CREATE INDEX IF NOT EXISTS idx_explanation_requests_status
    ON explanation_requests(status, created_at);

-- Explanation evidence - deterministic artifacts referenced in explanations
CREATE TABLE IF NOT EXISTS explanation_evidence (
    id TEXT PRIMARY KEY,
    explanation_request_id TEXT NOT NULL,
    evidence_type TEXT NOT NULL CHECK (evidence_type IN ('pricing_trace', 'policy_evaluation', 'rule_citation', 'line_item')),
    evidence_key TEXT NOT NULL,  -- e.g., trace_step_id, policy_id, line_id
    evidence_payload_json TEXT NOT NULL,  -- Serialized evidence data
    source_reference TEXT NOT NULL,  -- e.g., "quote_pricing_snapshot:abc123:step_5"
    display_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY (explanation_request_id) REFERENCES explanation_requests(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_explanation_evidence_request
    ON explanation_evidence(explanation_request_id, evidence_type);
CREATE INDEX IF NOT EXISTS idx_explanation_evidence_source
    ON explanation_evidence(source_reference);

-- Explanation audit trail - all explanation-related events
CREATE TABLE IF NOT EXISTS explanation_audit (
    id TEXT PRIMARY KEY,
    explanation_request_id TEXT NOT NULL,
    event_type TEXT NOT NULL CHECK (event_type IN ('request_received', 'evidence_gathered', 'explanation_generated', 'explanation_delivered', 'error_occurred', 'evidence_missing')),
    event_payload_json TEXT NOT NULL DEFAULT '{}',
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'system', 'agent')),
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (explanation_request_id) REFERENCES explanation_requests(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_explanation_audit_request
    ON explanation_audit(explanation_request_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_explanation_audit_event_type
    ON explanation_audit(event_type, occurred_at);
CREATE INDEX IF NOT EXISTS idx_explanation_audit_correlation
    ON explanation_audit(correlation_id);

-- Explanation cache - pre-computed explanations for common patterns
CREATE TABLE IF NOT EXISTS explanation_cache (
    id TEXT PRIMARY KEY,
    cache_key TEXT NOT NULL UNIQUE,  -- Hash of quote_id + line_id + version + snapshot_id
    quote_id TEXT NOT NULL,
    line_id TEXT,
    quote_version INTEGER NOT NULL,
    pricing_snapshot_id TEXT NOT NULL,
    explanation_summary TEXT NOT NULL,
    evidence_refs_json TEXT NOT NULL DEFAULT '[]',
    hit_count INTEGER NOT NULL DEFAULT 0 CHECK (hit_count >= 0),
    last_hit_at TEXT,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (pricing_snapshot_id) REFERENCES quote_pricing_snapshot(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_explanation_cache_quote
    ON explanation_cache(quote_id, line_id);
CREATE INDEX IF NOT EXISTS idx_explanation_cache_expires
    ON explanation_cache(expires_at);

-- Materialized view for explanation statistics (updated via triggers)
CREATE TABLE IF NOT EXISTS explanation_stats (
    id INTEGER PRIMARY KEY CHECK (id = 1),  -- Singleton table
    total_requests INTEGER NOT NULL DEFAULT 0,
    success_count INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    missing_evidence_count INTEGER NOT NULL DEFAULT 0,
    avg_latency_ms INTEGER,
    p95_latency_ms INTEGER,
    last_updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT OR IGNORE INTO explanation_stats (id, total_requests) VALUES (1, 0);

-- Trigger to update stats on new explanation request
CREATE TRIGGER IF NOT EXISTS explanation_request_stats_insert
AFTER INSERT ON explanation_requests
BEGIN
    UPDATE explanation_stats SET
        total_requests = total_requests + 1,
        success_count = CASE WHEN NEW.status = 'success' THEN success_count + 1 ELSE success_count END,
        error_count = CASE WHEN NEW.status = 'error' THEN error_count + 1 ELSE error_count END,
        missing_evidence_count = CASE WHEN NEW.status = 'missing_evidence' THEN missing_evidence_count + 1 ELSE missing_evidence_count END,
        last_updated_at = datetime('now')
    WHERE id = 1;
END;

-- Trigger to update stats on status change
CREATE TRIGGER IF NOT EXISTS explanation_request_stats_update
AFTER UPDATE OF status ON explanation_requests
WHEN OLD.status != NEW.status
BEGIN
    UPDATE explanation_stats SET
        success_count = CASE 
            WHEN NEW.status = 'success' AND OLD.status != 'success' THEN success_count + 1
            WHEN OLD.status = 'success' AND NEW.status != 'success' THEN success_count - 1
            ELSE success_count 
        END,
        error_count = CASE 
            WHEN NEW.status = 'error' AND OLD.status != 'error' THEN error_count + 1
            WHEN OLD.status = 'error' AND NEW.status != 'error' THEN error_count - 1
            ELSE error_count 
        END,
        missing_evidence_count = CASE 
            WHEN NEW.status = 'missing_evidence' AND OLD.status != 'missing_evidence' THEN missing_evidence_count + 1
            WHEN OLD.status = 'missing_evidence' AND NEW.status != 'missing_evidence' THEN missing_evidence_count - 1
            ELSE missing_evidence_count 
        END,
        last_updated_at = datetime('now')
    WHERE id = 1;
END;
