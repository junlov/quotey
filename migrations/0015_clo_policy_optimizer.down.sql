-- Rollback Closed-loop policy optimizer persistence primitives.

DROP INDEX IF EXISTS idx_policy_audit_correlation;
DROP INDEX IF EXISTS idx_policy_audit_event_occurred;
DROP INDEX IF EXISTS idx_policy_audit_candidate_occurred;
DROP TABLE IF EXISTS policy_lifecycle_audit;

DROP INDEX IF EXISTS idx_policy_rollback_apply;
DROP INDEX IF EXISTS idx_policy_rollback_candidate_rolled;
DROP TABLE IF EXISTS policy_rollback_record;

DROP INDEX IF EXISTS idx_policy_apply_idempotency;
DROP INDEX IF EXISTS idx_policy_apply_candidate_applied;
DROP TABLE IF EXISTS policy_apply_record;

DROP INDEX IF EXISTS idx_policy_approval_stale_expires;
DROP INDEX IF EXISTS idx_policy_approval_candidate_decided;
DROP TABLE IF EXISTS policy_approval_decision;

DROP INDEX IF EXISTS idx_policy_replay_checksum;
DROP INDEX IF EXISTS idx_policy_replay_candidate_replayed;
DROP TABLE IF EXISTS policy_replay_evaluation;

DROP INDEX IF EXISTS idx_policy_candidate_base_version;
DROP INDEX IF EXISTS idx_policy_candidate_status_created;
DROP TABLE IF EXISTS policy_candidate;
