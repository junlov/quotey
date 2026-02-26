-- Migration: 0004_dialogue_state
-- Description: Add dialogue session management for conversational quote flows
-- Tracks multi-turn conversations with state and intent history
-- Author: Quotey Team
-- Date: 2024

CREATE TABLE IF NOT EXISTS dialogue_sessions (
    id TEXT PRIMARY KEY,
    slack_thread_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    current_intent_json TEXT,
    pending_clarifications_json TEXT,
    quote_draft_id TEXT,
    status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'expired')),
    FOREIGN KEY (quote_draft_id) REFERENCES quote(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_dialogue_sessions_slack_thread_id
    ON dialogue_sessions(slack_thread_id);
CREATE INDEX IF NOT EXISTS idx_dialogue_sessions_user_id
    ON dialogue_sessions(user_id);

CREATE TABLE IF NOT EXISTS dialogue_turns (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL,
    turn_number INTEGER NOT NULL CHECK (turn_number >= 1),
    user_message TEXT NOT NULL,
    bot_response TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES dialogue_sessions(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_dialogue_turns_session_turn_number
    ON dialogue_turns(session_id, turn_number);
