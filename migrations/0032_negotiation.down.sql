DROP INDEX IF EXISTS idx_negotiation_turn_transition_key;
DROP INDEX IF EXISTS idx_negotiation_turn_session_number;
DROP INDEX IF EXISTS idx_negotiation_turn_session_id;
DROP TABLE IF EXISTS negotiation_turn;

DROP INDEX IF EXISTS idx_negotiation_session_idempotency;
DROP INDEX IF EXISTS idx_negotiation_session_state;
DROP INDEX IF EXISTS idx_negotiation_session_actor_id;
DROP INDEX IF EXISTS idx_negotiation_session_quote_id;
DROP TABLE IF EXISTS negotiation_session;
