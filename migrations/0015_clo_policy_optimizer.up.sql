-- Closed-loop policy optimizer persistence primitives.
-- Stores candidate lifecycle, replay evidence, approval decisions, signed apply records,
-- rollback chains, and immutable lifecycle audit events.

CREATE TABLE IF NOT EXISTS policy_candidate (
    id TEXT PRIMARY KEY,
    base_policy_version INTEGER NOT NULL CHECK (base_policy_version >= 1),
    proposed_policy_version INTEGER NOT NULL CHECK (proposed_policy_version >= base_policy_version),
    status TEXT NOT NULL CHECK (
        status IN (
            'draft',
            'replayed',
            'review_ready',
            'approved',
            'rejected',
            'changes_requested',
            'applied',
            'monitoring',
            'rolled_back'
        )
    ),
    policy_diff_json TEXT NOT NULL,
    provenance_json TEXT NOT NULL DEFAULT '{}',
    confidence_score REAL NOT NULL CHECK (confidence_score >= 0.0 AND confidence_score <= 1.0),
    cohort_scope_json TEXT NOT NULL DEFAULT '{}',
    latest_replay_checksum TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    created_by_actor_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    review_ready_at TEXT,
    approved_at TEXT,
    applied_at TEXT,
    monitoring_started_at TEXT,
    rolled_back_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_policy_candidate_status_created
    ON policy_candidate(status, created_at);
CREATE INDEX IF NOT EXISTS idx_policy_candidate_base_version
    ON policy_candidate(base_policy_version, proposed_policy_version);

CREATE TABLE IF NOT EXISTS policy_replay_evaluation (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    replay_checksum TEXT NOT NULL,
    engine_version TEXT NOT NULL,
    cohort_scope_json TEXT NOT NULL DEFAULT '{}',
    cohort_size INTEGER NOT NULL CHECK (cohort_size >= 0),
    projected_margin_delta_bps INTEGER NOT NULL,
    projected_win_rate_delta_bps INTEGER NOT NULL,
    projected_approval_latency_delta_seconds INTEGER NOT NULL,
    blast_radius_score REAL NOT NULL CHECK (blast_radius_score >= 0.0 AND blast_radius_score <= 1.0),
    hard_violation_count INTEGER NOT NULL DEFAULT 0 CHECK (hard_violation_count >= 0),
    risk_flags_json TEXT NOT NULL DEFAULT '[]',
    deterministic_pass INTEGER NOT NULL CHECK (deterministic_pass IN (0, 1)),
    idempotency_key TEXT NOT NULL UNIQUE,
    replayed_at TEXT NOT NULL,
    FOREIGN KEY (candidate_id) REFERENCES policy_candidate(id) ON DELETE CASCADE,
    UNIQUE (candidate_id, replay_checksum)
);

CREATE INDEX IF NOT EXISTS idx_policy_replay_candidate_replayed
    ON policy_replay_evaluation(candidate_id, replayed_at);
CREATE INDEX IF NOT EXISTS idx_policy_replay_checksum
    ON policy_replay_evaluation(replay_checksum);

CREATE TABLE IF NOT EXISTS policy_approval_decision (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    replay_evaluation_id TEXT,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'rejected', 'changes_requested')),
    reason TEXT,
    decision_payload_json TEXT NOT NULL DEFAULT '{}',
    actor_id TEXT NOT NULL,
    actor_role TEXT NOT NULL,
    channel_ref TEXT,
    signature TEXT,
    signature_key_id TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    decided_at TEXT NOT NULL,
    expires_at TEXT,
    is_stale INTEGER NOT NULL DEFAULT 0 CHECK (is_stale IN (0, 1)),
    FOREIGN KEY (candidate_id) REFERENCES policy_candidate(id) ON DELETE CASCADE,
    FOREIGN KEY (replay_evaluation_id) REFERENCES policy_replay_evaluation(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_policy_approval_candidate_decided
    ON policy_approval_decision(candidate_id, decided_at);
CREATE INDEX IF NOT EXISTS idx_policy_approval_stale_expires
    ON policy_approval_decision(is_stale, expires_at);

CREATE TABLE IF NOT EXISTS policy_apply_record (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    approval_decision_id TEXT NOT NULL,
    prior_policy_version INTEGER NOT NULL CHECK (prior_policy_version >= 1),
    applied_policy_version INTEGER NOT NULL CHECK (applied_policy_version >= prior_policy_version),
    replay_checksum TEXT NOT NULL,
    apply_signature TEXT NOT NULL,
    signature_key_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    verification_checksum TEXT NOT NULL,
    apply_audit_json TEXT NOT NULL DEFAULT '{}',
    applied_at TEXT NOT NULL,
    FOREIGN KEY (candidate_id) REFERENCES policy_candidate(id) ON DELETE CASCADE,
    FOREIGN KEY (approval_decision_id) REFERENCES policy_approval_decision(id) ON DELETE RESTRICT,
    UNIQUE (candidate_id, applied_policy_version)
);

CREATE INDEX IF NOT EXISTS idx_policy_apply_candidate_applied
    ON policy_apply_record(candidate_id, applied_at);
CREATE INDEX IF NOT EXISTS idx_policy_apply_idempotency
    ON policy_apply_record(idempotency_key);

CREATE TABLE IF NOT EXISTS policy_rollback_record (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    apply_record_id TEXT NOT NULL,
    rollback_target_version INTEGER NOT NULL CHECK (rollback_target_version >= 1),
    rollback_reason TEXT NOT NULL,
    verification_checksum TEXT NOT NULL,
    rollback_signature TEXT NOT NULL,
    signature_key_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    parent_rollback_id TEXT,
    rollback_depth INTEGER NOT NULL DEFAULT 0 CHECK (rollback_depth >= 0),
    rollback_metadata_json TEXT NOT NULL DEFAULT '{}',
    rolled_back_at TEXT NOT NULL,
    FOREIGN KEY (candidate_id) REFERENCES policy_candidate(id) ON DELETE CASCADE,
    FOREIGN KEY (apply_record_id) REFERENCES policy_apply_record(id) ON DELETE CASCADE,
    FOREIGN KEY (parent_rollback_id) REFERENCES policy_rollback_record(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_policy_rollback_candidate_rolled
    ON policy_rollback_record(candidate_id, rolled_back_at);
CREATE INDEX IF NOT EXISTS idx_policy_rollback_apply
    ON policy_rollback_record(apply_record_id, rolled_back_at);

CREATE TABLE IF NOT EXISTS policy_lifecycle_audit (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL,
    replay_evaluation_id TEXT,
    approval_decision_id TEXT,
    apply_record_id TEXT,
    rollback_record_id TEXT,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'candidate_created',
            'replay_completed',
            'review_packet_built',
            'approved',
            'rejected',
            'changes_requested',
            'applied',
            'monitoring_started',
            'rolled_back',
            'stale_approval_detected'
        )
    ),
    event_payload_json TEXT NOT NULL DEFAULT '{}',
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'system', 'agent')),
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    idempotency_key TEXT,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (candidate_id) REFERENCES policy_candidate(id) ON DELETE CASCADE,
    FOREIGN KEY (replay_evaluation_id) REFERENCES policy_replay_evaluation(id) ON DELETE SET NULL,
    FOREIGN KEY (approval_decision_id) REFERENCES policy_approval_decision(id) ON DELETE SET NULL,
    FOREIGN KEY (apply_record_id) REFERENCES policy_apply_record(id) ON DELETE SET NULL,
    FOREIGN KEY (rollback_record_id) REFERENCES policy_rollback_record(id) ON DELETE SET NULL,
    UNIQUE (event_type, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_policy_audit_candidate_occurred
    ON policy_lifecycle_audit(candidate_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_policy_audit_event_occurred
    ON policy_lifecycle_audit(event_type, occurred_at);
CREATE INDEX IF NOT EXISTS idx_policy_audit_correlation
    ON policy_lifecycle_audit(correlation_id);
