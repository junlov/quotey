-- Rollback Deal Autopsy & Revenue Genome persistence primitives.

DROP INDEX IF EXISTS idx_counterfactual_autopsy;
DROP INDEX IF EXISTS idx_counterfactual_quote;
DROP TABLE IF EXISTS counterfactual_simulation;

DROP INDEX IF EXISTS idx_genome_query_audit_type_queried;
DROP TABLE IF EXISTS genome_query_audit;

DROP INDEX IF EXISTS idx_attribution_edge_weight;
DROP INDEX IF EXISTS idx_attribution_edge_target;
DROP INDEX IF EXISTS idx_attribution_edge_source;
DROP TABLE IF EXISTS attribution_edge;

DROP INDEX IF EXISTS idx_attribution_node_hash;
DROP INDEX IF EXISTS idx_attribution_node_type_stage_seg;
DROP TABLE IF EXISTS attribution_node;

DROP INDEX IF EXISTS idx_attribution_score_contribution;
DROP INDEX IF EXISTS idx_attribution_score_fork;
DROP INDEX IF EXISTS idx_attribution_score_autopsy;
DROP TABLE IF EXISTS attribution_score;

DROP INDEX IF EXISTS idx_decision_fork_type_stage;
DROP INDEX IF EXISTS idx_decision_fork_autopsy_seq;
DROP TABLE IF EXISTS decision_fork;

DROP INDEX IF EXISTS idx_deal_autopsy_outcome_created;
DROP INDEX IF EXISTS idx_deal_autopsy_quote;
DROP TABLE IF EXISTS deal_autopsy;
