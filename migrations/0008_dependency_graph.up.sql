CREATE TABLE IF NOT EXISTS constraint_nodes (
    id TEXT PRIMARY KEY,
    config_id TEXT NOT NULL,
    node_type TEXT NOT NULL,
    node_key TEXT NOT NULL,
    status TEXT NOT NULL,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_constraint_nodes_config_id
    ON constraint_nodes(config_id);
CREATE INDEX IF NOT EXISTS idx_constraint_nodes_node_key
    ON constraint_nodes(node_key);

CREATE TABLE IF NOT EXISTS constraint_edges (
    id TEXT PRIMARY KEY,
    config_id TEXT NOT NULL,
    from_node TEXT NOT NULL,
    to_node TEXT NOT NULL,
    edge_type TEXT NOT NULL,
    condition_expression TEXT,
    FOREIGN KEY (from_node) REFERENCES constraint_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY (to_node) REFERENCES constraint_nodes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_constraint_edges_config_id
    ON constraint_edges(config_id);

CREATE TABLE IF NOT EXISTS archaeology_queries (
    id TEXT PRIMARY KEY,
    config_id TEXT NOT NULL,
    query_type TEXT NOT NULL,
    result_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_archaeology_queries_config_id
    ON archaeology_queries(config_id);
