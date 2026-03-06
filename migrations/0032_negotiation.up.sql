-- 0032: Negotiation session and turn tables for NXT Deterministic Negotiation Autopilot (W2)

CREATE TABLE IF NOT EXISTS negotiation_session (
    id                TEXT    PRIMARY KEY NOT NULL,
    quote_id          TEXT    NOT NULL REFERENCES quote(id),
    actor_id          TEXT    NOT NULL,
    state             TEXT    NOT NULL DEFAULT 'draft'
                              CHECK (state IN ('draft','active','counter_pending',
                                               'approval_pending','approved',
                                               'accepted','rejected','expired','cancelled')),
    -- Deterministic version refs for replay invariance
    policy_version    TEXT    NOT NULL DEFAULT '',
    pricing_version   TEXT    NOT NULL DEFAULT '',
    -- Idempotency: unique constraint prevents duplicate sessions per quote+actor
    idempotency_key   TEXT    NOT NULL DEFAULT '',
    -- Walk-away / expiry controls
    max_turns         INTEGER NOT NULL DEFAULT 20,
    expires_at        TEXT,
    -- Audit
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_negotiation_session_quote_id
    ON negotiation_session(quote_id);

CREATE INDEX IF NOT EXISTS idx_negotiation_session_actor_id
    ON negotiation_session(actor_id);

CREATE INDEX IF NOT EXISTS idx_negotiation_session_state
    ON negotiation_session(state);

CREATE UNIQUE INDEX IF NOT EXISTS idx_negotiation_session_idempotency
    ON negotiation_session(quote_id, actor_id, idempotency_key);


CREATE TABLE IF NOT EXISTS negotiation_turn (
    id                TEXT    PRIMARY KEY NOT NULL,
    session_id        TEXT    NOT NULL REFERENCES negotiation_session(id),
    turn_number       INTEGER NOT NULL,
    -- What the user/system requested
    request_type      TEXT    NOT NULL CHECK (request_type IN ('open','counter','accept','reject','escalate','cancel')),
    request_payload   TEXT    NOT NULL DEFAULT '{}',
    -- Deterministic concession envelope (serialized JSON)
    envelope_json     TEXT,
    -- Deterministic counteroffer plan (serialized JSON)
    plan_json         TEXT,
    -- What was chosen
    chosen_offer_id   TEXT,
    -- Outcome of this turn
    outcome           TEXT    NOT NULL DEFAULT 'pending'
                              CHECK (outcome IN ('pending','offered','accepted','rejected',
                                                 'escalated','expired','cancelled')),
    -- Boundary evaluation result
    boundary_json     TEXT,
    -- Transition idempotency key (prevents duplicate committed actions)
    transition_key    TEXT    NOT NULL DEFAULT '',
    -- Audit
    created_at        TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_negotiation_turn_session_id
    ON negotiation_turn(session_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_negotiation_turn_session_number
    ON negotiation_turn(session_id, turn_number);

CREATE UNIQUE INDEX IF NOT EXISTS idx_negotiation_turn_transition_key
    ON negotiation_turn(transition_key)
    WHERE transition_key != '';
