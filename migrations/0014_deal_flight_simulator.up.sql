-- Deal Flight Simulator persistence primitives
-- Stores deterministic scenario runs, variants, deltas, and audit trail artifacts.

CREATE TABLE IF NOT EXISTS deal_flight_scenario_run (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    base_quote_version INTEGER NOT NULL CHECK (base_quote_version > 0),
    request_params_json TEXT NOT NULL DEFAULT '{}',
    variant_count INTEGER NOT NULL CHECK (variant_count >= 1 AND variant_count <= 5),
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'success', 'failed', 'promoted', 'cancelled')
    ),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_sim_run_quote_created
    ON deal_flight_scenario_run(quote_id, created_at);
CREATE INDEX IF NOT EXISTS idx_sim_run_thread_created
    ON deal_flight_scenario_run(thread_id, created_at);
CREATE INDEX IF NOT EXISTS idx_sim_run_correlation
    ON deal_flight_scenario_run(correlation_id);
CREATE INDEX IF NOT EXISTS idx_sim_run_status_created
    ON deal_flight_scenario_run(status, created_at);

CREATE TABLE IF NOT EXISTS deal_flight_scenario_variant (
    id TEXT PRIMARY KEY,
    scenario_run_id TEXT NOT NULL,
    variant_key TEXT NOT NULL,
    variant_order INTEGER NOT NULL CHECK (variant_order >= 0),
    params_json TEXT NOT NULL DEFAULT '{}',
    pricing_result_json TEXT NOT NULL,
    policy_result_json TEXT NOT NULL,
    approval_route_json TEXT NOT NULL,
    configuration_result_json TEXT NOT NULL,
    rank_score REAL NOT NULL DEFAULT 0.0,
    rank_order INTEGER NOT NULL CHECK (rank_order >= 0),
    selected_for_promotion INTEGER NOT NULL DEFAULT 0 CHECK (
        selected_for_promotion IN (0, 1)
    ),
    created_at TEXT NOT NULL,
    FOREIGN KEY (scenario_run_id) REFERENCES deal_flight_scenario_run(id) ON DELETE CASCADE,
    UNIQUE (scenario_run_id, variant_key),
    UNIQUE (scenario_run_id, variant_order)
);

CREATE INDEX IF NOT EXISTS idx_sim_variant_run_rank
    ON deal_flight_scenario_variant(scenario_run_id, rank_order);
CREATE INDEX IF NOT EXISTS idx_sim_variant_run_selected
    ON deal_flight_scenario_variant(scenario_run_id, selected_for_promotion);

CREATE TABLE IF NOT EXISTS deal_flight_scenario_delta (
    id TEXT PRIMARY KEY,
    scenario_variant_id TEXT NOT NULL,
    delta_type TEXT NOT NULL CHECK (
        delta_type IN ('price', 'policy', 'approval', 'configuration')
    ),
    delta_payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (scenario_variant_id) REFERENCES deal_flight_scenario_variant(id) ON DELETE CASCADE,
    UNIQUE (scenario_variant_id, delta_type)
);

CREATE INDEX IF NOT EXISTS idx_sim_delta_variant_type
    ON deal_flight_scenario_delta(scenario_variant_id, delta_type);

CREATE TABLE IF NOT EXISTS deal_flight_scenario_audit (
    id TEXT PRIMARY KEY,
    scenario_run_id TEXT NOT NULL,
    scenario_variant_id TEXT,
    event_type TEXT NOT NULL CHECK (
        event_type IN (
            'request_received',
            'variant_generated',
            'comparison_rendered',
            'promotion_requested',
            'promotion_applied',
            'error_occurred'
        )
    ),
    event_payload_json TEXT NOT NULL DEFAULT '{}',
    actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'system', 'agent')),
    actor_id TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    FOREIGN KEY (scenario_run_id) REFERENCES deal_flight_scenario_run(id) ON DELETE CASCADE,
    FOREIGN KEY (scenario_variant_id) REFERENCES deal_flight_scenario_variant(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_sim_audit_run_occurred
    ON deal_flight_scenario_audit(scenario_run_id, occurred_at);
CREATE INDEX IF NOT EXISTS idx_sim_audit_event_occurred
    ON deal_flight_scenario_audit(event_type, occurred_at);
CREATE INDEX IF NOT EXISTS idx_sim_audit_correlation
    ON deal_flight_scenario_audit(correlation_id);
