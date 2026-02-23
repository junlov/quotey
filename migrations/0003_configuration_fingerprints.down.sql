DROP INDEX IF EXISTS idx_similarity_cache_candidate_fingerprint_id;
DROP INDEX IF EXISTS idx_similarity_cache_source_fingerprint_id;
DROP INDEX IF EXISTS idx_similarity_cache_source_candidate_version;
DROP TABLE IF EXISTS similarity_cache;

DROP INDEX IF EXISTS idx_configuration_fingerprints_fingerprint_hash;
DROP INDEX IF EXISTS idx_configuration_fingerprints_quote_id;
DROP TABLE IF EXISTS configuration_fingerprints;
