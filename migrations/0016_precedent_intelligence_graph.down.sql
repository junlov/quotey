-- Rollback Precedent Intelligence Graph persistence primitives.

DROP INDEX IF EXISTS idx_precedent_similarity_correlation;
DROP INDEX IF EXISTS idx_precedent_similarity_candidate_quote;
DROP INDEX IF EXISTS idx_precedent_similarity_source_fp_version;
DROP INDEX IF EXISTS idx_precedent_similarity_source_quote_score;
DROP TABLE IF EXISTS precedent_similarity_evidence;

DROP INDEX IF EXISTS idx_precedent_approval_status_decided;
DROP INDEX IF EXISTS idx_precedent_approval_quote_routed;
DROP TABLE IF EXISTS precedent_approval_path_evidence;
