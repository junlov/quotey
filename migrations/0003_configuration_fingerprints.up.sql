CREATE TABLE IF NOT EXISTS configuration_fingerprints (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    fingerprint_hash TEXT NOT NULL,
    configuration_vector BLOB NOT NULL,
    outcome_status TEXT NOT NULL CHECK (outcome_status IN ('won', 'lost', 'pending')),
    final_price REAL NOT NULL,
    close_date TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_configuration_fingerprints_quote_id
    ON configuration_fingerprints(quote_id);
CREATE INDEX IF NOT EXISTS idx_configuration_fingerprints_fingerprint_hash
    ON configuration_fingerprints(fingerprint_hash);

CREATE TABLE IF NOT EXISTS similarity_cache (
    id TEXT PRIMARY KEY,
    source_fingerprint_id TEXT NOT NULL,
    candidate_fingerprint_id TEXT NOT NULL,
    similarity_score REAL NOT NULL,
    algorithm_version TEXT NOT NULL,
    computed_at TEXT NOT NULL,
    FOREIGN KEY (source_fingerprint_id) REFERENCES configuration_fingerprints(id) ON DELETE CASCADE,
    FOREIGN KEY (candidate_fingerprint_id) REFERENCES configuration_fingerprints(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_similarity_cache_source_candidate_version
    ON similarity_cache(source_fingerprint_id, candidate_fingerprint_id, algorithm_version);
CREATE INDEX IF NOT EXISTS idx_similarity_cache_source_fingerprint_id
    ON similarity_cache(source_fingerprint_id);
CREATE INDEX IF NOT EXISTS idx_similarity_cache_candidate_fingerprint_id
    ON similarity_cache(candidate_fingerprint_id);
