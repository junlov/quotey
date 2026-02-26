-- Migration: 0005_policy_explanations
-- Description: Add policy engine with explainable rules
-- Supports constraint-based rules with human-readable explanations
-- Author: Quotey Team
-- Date: 2024

CREATE TABLE IF NOT EXISTS policy_rules (
    id TEXT PRIMARY KEY,
    rule_key TEXT NOT NULL UNIQUE,
    condition_expression TEXT NOT NULL,
    action_expression TEXT NOT NULL,
    explanation_template TEXT NOT NULL,
    resolution_hints_json TEXT NOT NULL DEFAULT '[]',
    documentation_url TEXT,
    rule_category TEXT NOT NULL CHECK (rule_category IN ('pricing', 'approval', 'config')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_policy_rules_rule_category
    ON policy_rules(rule_category);

CREATE TABLE IF NOT EXISTS explanation_cache (
    id TEXT PRIMARY KEY,
    rule_id TEXT NOT NULL,
    quote_id TEXT,
    explanation_text TEXT NOT NULL,
    resolution_hints_json TEXT NOT NULL,
    generated_at TEXT NOT NULL,
    expires_at TEXT,
    FOREIGN KEY (rule_id) REFERENCES policy_rules(id) ON DELETE CASCADE,
    FOREIGN KEY (quote_id) REFERENCES quote(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_explanation_cache_rule_id
    ON explanation_cache(rule_id);
CREATE INDEX IF NOT EXISTS idx_explanation_cache_quote_id
    ON explanation_cache(quote_id);
