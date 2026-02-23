DROP INDEX IF EXISTS idx_dialogue_turns_session_turn_number;
DROP TABLE IF EXISTS dialogue_turns;

DROP INDEX IF EXISTS idx_dialogue_sessions_user_id;
DROP INDEX IF EXISTS idx_dialogue_sessions_slack_thread_id;
DROP TABLE IF EXISTS dialogue_sessions;
