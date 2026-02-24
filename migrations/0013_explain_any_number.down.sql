-- Rollback Explain Any Number feature

DROP TRIGGER IF EXISTS explanation_request_stats_insert;
DROP TRIGGER IF EXISTS explanation_request_stats_update;
DROP TABLE IF EXISTS explanation_request_stats;
DROP TABLE IF EXISTS explanation_response_cache;
DROP TABLE IF EXISTS explanation_audit;
DROP TABLE IF EXISTS explanation_evidence;
DROP TABLE IF EXISTS explanation_requests;
