-- Deal Autopsy & Revenue Genome persistence primitives.
-- Stores terminal deal analysis, decision forks, attribution scores,
-- pattern graph (nodes/edges), genome query audit, and counterfactual simulations.

CREATE TABLE IF NOT EXISTS deal_autopsy (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL,
    outcome_status TEXT NOT NULL CHECK (
        outcome_status IN ('won', 'lost', 'expired', 'cancelled')
    ),
    outcome_value_bps INTEGER NOT NULL,
    outcome_revenue_cents INTEGER NOT NULL DEFAULT 0,
    decision_fork_count INTEGER NOT NULL DEFAULT 0 CHECK (decision_fork_count >= 0),
    attribution_checksum TEXT NOT NULL,
    audit_trail_refs_json TEXT NOT NULL DEFAULT '[]',
    autopsy_version TEXT NOT NULL DEFAULT 'rgn_autopsy.v1',
    idempotency_key TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_deal_autopsy_quote
    ON deal_autopsy(quote_id);
CREATE INDEX IF NOT EXISTS idx_deal_autopsy_outcome_created
    ON deal_autopsy(outcome_status, created_at);

CREATE TABLE IF NOT EXISTS decision_fork (
    id TEXT PRIMARY KEY,
    autopsy_id TEXT NOT NULL,
    fork_type TEXT NOT NULL CHECK (
        fork_type IN (
            'pricing_path',
            'discount_level',
            'constraint_resolution',
            'approval_exception',
            'negotiation_concession',
            'product_selection',
            'term_selection',
            'bundle_choice'
        )
    ),
    stage TEXT NOT NULL CHECK (
        stage IN (
            'configuration',
            'pricing',
            'policy',
            'approval',
            'negotiation',
            'finalization'
        )
    ),
    option_chosen_json TEXT NOT NULL,
    options_considered_json TEXT NOT NULL DEFAULT '[]',
    audit_ref TEXT NOT NULL,
    audit_ref_type TEXT NOT NULL CHECK (
        audit_ref_type IN (
            'ledger_entry',
            'audit_event',
            'pricing_trace',
            'negotiation_turn',
            'approval_decision'
        )
    ),
    sequence_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    FOREIGN KEY (autopsy_id) REFERENCES deal_autopsy(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_decision_fork_autopsy_seq
    ON decision_fork(autopsy_id, sequence_order);
CREATE INDEX IF NOT EXISTS idx_decision_fork_type_stage
    ON decision_fork(fork_type, stage);

CREATE TABLE IF NOT EXISTS attribution_score (
    id TEXT PRIMARY KEY,
    autopsy_id TEXT NOT NULL,
    fork_id TEXT NOT NULL,
    outcome_contribution_bps INTEGER NOT NULL,
    confidence_bps INTEGER NOT NULL CHECK (confidence_bps >= 0 AND confidence_bps <= 10000),
    evidence_count INTEGER NOT NULL DEFAULT 0 CHECK (evidence_count >= 0),
    evidence_refs_json TEXT NOT NULL DEFAULT '[]',
    attribution_method TEXT NOT NULL DEFAULT 'deterministic_trace',
    created_at TEXT NOT NULL,
    FOREIGN KEY (autopsy_id) REFERENCES deal_autopsy(id) ON DELETE CASCADE,
    FOREIGN KEY (fork_id) REFERENCES decision_fork(id) ON DELETE CASCADE,
    UNIQUE (autopsy_id, fork_id)
);

CREATE INDEX IF NOT EXISTS idx_attribution_score_autopsy
    ON attribution_score(autopsy_id);
CREATE INDEX IF NOT EXISTS idx_attribution_score_fork
    ON attribution_score(fork_id);
CREATE INDEX IF NOT EXISTS idx_attribution_score_contribution
    ON attribution_score(outcome_contribution_bps);

CREATE TABLE IF NOT EXISTS attribution_node (
    id TEXT PRIMARY KEY,
    fork_type TEXT NOT NULL,
    stage TEXT NOT NULL,
    segment_key TEXT NOT NULL DEFAULT 'all',
    option_value_hash TEXT NOT NULL,
    option_value_summary TEXT NOT NULL,
    sample_count INTEGER NOT NULL DEFAULT 0 CHECK (sample_count >= 0),
    first_seen_at TEXT NOT NULL,
    last_updated_at TEXT NOT NULL,
    UNIQUE (fork_type, stage, segment_key, option_value_hash)
);

CREATE INDEX IF NOT EXISTS idx_attribution_node_type_stage_seg
    ON attribution_node(fork_type, stage, segment_key);
CREATE INDEX IF NOT EXISTS idx_attribution_node_hash
    ON attribution_node(option_value_hash);

CREATE TABLE IF NOT EXISTS attribution_edge (
    id TEXT PRIMARY KEY,
    source_node_id TEXT NOT NULL,
    target_node_id TEXT NOT NULL,
    outcome_weight_bps INTEGER NOT NULL DEFAULT 0,
    sample_count INTEGER NOT NULL DEFAULT 0 CHECK (sample_count >= 0),
    win_rate_bps INTEGER NOT NULL DEFAULT 0 CHECK (win_rate_bps >= 0 AND win_rate_bps <= 10000),
    avg_margin_delta_bps INTEGER NOT NULL DEFAULT 0,
    avg_revenue_cents INTEGER NOT NULL DEFAULT 0,
    first_seen_at TEXT NOT NULL,
    last_updated_at TEXT NOT NULL,
    FOREIGN KEY (source_node_id) REFERENCES attribution_node(id) ON DELETE CASCADE,
    FOREIGN KEY (target_node_id) REFERENCES attribution_node(id) ON DELETE CASCADE,
    UNIQUE (source_node_id, target_node_id)
);

CREATE INDEX IF NOT EXISTS idx_attribution_edge_source
    ON attribution_edge(source_node_id);
CREATE INDEX IF NOT EXISTS idx_attribution_edge_target
    ON attribution_edge(target_node_id);
CREATE INDEX IF NOT EXISTS idx_attribution_edge_weight
    ON attribution_edge(outcome_weight_bps);

CREATE TABLE IF NOT EXISTS genome_query_audit (
    id TEXT PRIMARY KEY,
    query_type TEXT NOT NULL CHECK (
        query_type IN (
            'strategy_recommendation',
            'counterfactual',
            'pattern_detection',
            'segment_analysis',
            'policy_impact',
            'decision_comparison'
        )
    ),
    query_params_json TEXT NOT NULL DEFAULT '{}',
    result_checksum TEXT NOT NULL,
    result_summary_json TEXT NOT NULL DEFAULT '{}',
    segments_analyzed INTEGER NOT NULL DEFAULT 0,
    evidence_count INTEGER NOT NULL DEFAULT 0,
    query_duration_ms INTEGER NOT NULL DEFAULT 0,
    queried_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_genome_query_audit_type_queried
    ON genome_query_audit(query_type, queried_at);

CREATE TABLE IF NOT EXISTS counterfactual_simulation (
    id TEXT PRIMARY KEY,
    original_quote_id TEXT NOT NULL,
    original_autopsy_id TEXT NOT NULL,
    alternative_decisions_json TEXT NOT NULL,
    replay_checksum TEXT NOT NULL,
    projected_outcome_status TEXT NOT NULL CHECK (
        projected_outcome_status IN ('won', 'lost', 'expired', 'cancelled', 'unknown')
    ),
    projected_margin_delta_bps INTEGER NOT NULL DEFAULT 0,
    projected_revenue_delta_cents INTEGER NOT NULL DEFAULT 0,
    delta_vs_actual_json TEXT NOT NULL DEFAULT '{}',
    evidence_chain_json TEXT NOT NULL DEFAULT '[]',
    confidence_bps INTEGER NOT NULL CHECK (confidence_bps >= 0 AND confidence_bps <= 10000),
    simulated_at TEXT NOT NULL,
    FOREIGN KEY (original_quote_id) REFERENCES quote(id) ON DELETE CASCADE,
    FOREIGN KEY (original_autopsy_id) REFERENCES deal_autopsy(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_counterfactual_quote
    ON counterfactual_simulation(original_quote_id);
CREATE INDEX IF NOT EXISTS idx_counterfactual_autopsy
    ON counterfactual_simulation(original_autopsy_id);
