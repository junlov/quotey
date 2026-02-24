-- Precedent Intelligence Graph persistence primitives.
-- Stores deterministic approval-path evidence and similarity lineage records
-- used to render replayable precedent recommendations.

CREATE TABLE IF NOT EXISTS precedent_approval_path_evidence (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    route_version INTEGER NOT NULL CHECK (route_version >= 1),
    route_payload_json TEXT NOT NULL DEFAULT '{}',
    decision_status TEXT NOT NULL CHECK (
        decision_status IN ('pending', 'approved', 'rejected', 'escalated')
    ),
    decision_actor_id TEXT,
    decision_reason TEXT,
    routed_by_actor_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    correlation_id TEXT NOT NULL,
    routed_at TEXT NOT NULL,
    decided_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    UNIQUE (quote_id, route_version)
);

CREATE INDEX IF NOT EXISTS idx_precedent_approval_quote_routed
    ON precedent_approval_path_evidence(quote_id, routed_at DESC);
CREATE INDEX IF NOT EXISTS idx_precedent_approval_status_decided
    ON precedent_approval_path_evidence(decision_status, decided_at);

CREATE TABLE IF NOT EXISTS precedent_similarity_evidence (
    id TEXT PRIMARY KEY,
    source_quote_id TEXT NOT NULL,
    source_fingerprint_id TEXT NOT NULL,
    candidate_quote_id TEXT NOT NULL,
    candidate_fingerprint_id TEXT NOT NULL,
    similarity_score REAL NOT NULL CHECK (similarity_score >= 0.0 AND similarity_score <= 1.0),
    strategy_version TEXT NOT NULL,
    score_components_json TEXT NOT NULL DEFAULT '{}',
    evidence_payload_json TEXT NOT NULL DEFAULT '{}',
    idempotency_key TEXT NOT NULL UNIQUE,
    correlation_id TEXT NOT NULL,
    computed_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (source_quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (candidate_quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (source_fingerprint_id) REFERENCES configuration_fingerprints(id) ON DELETE CASCADE,
    FOREIGN KEY (candidate_fingerprint_id) REFERENCES configuration_fingerprints(id) ON DELETE CASCADE,
    UNIQUE (source_fingerprint_id, candidate_fingerprint_id, strategy_version)
);

CREATE INDEX IF NOT EXISTS idx_precedent_similarity_source_quote_score
    ON precedent_similarity_evidence(source_quote_id, similarity_score DESC, computed_at DESC);
CREATE INDEX IF NOT EXISTS idx_precedent_similarity_source_fp_version
    ON precedent_similarity_evidence(source_fingerprint_id, strategy_version, computed_at DESC);
CREATE INDEX IF NOT EXISTS idx_precedent_similarity_candidate_quote
    ON precedent_similarity_evidence(candidate_quote_id, computed_at DESC);
CREATE INDEX IF NOT EXISTS idx_precedent_similarity_correlation
    ON precedent_similarity_evidence(correlation_id);
