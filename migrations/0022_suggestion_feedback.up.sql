-- Suggestion feedback: tracks how users interact with product suggestions
-- for learning and score adjustment.
CREATE TABLE suggestion_feedback (
    id              TEXT PRIMARY KEY NOT NULL,
    request_id      TEXT NOT NULL,
    customer_id     TEXT NOT NULL,
    product_id      TEXT NOT NULL,
    product_sku     TEXT NOT NULL,
    score           REAL NOT NULL,
    confidence      TEXT NOT NULL,
    category        TEXT NOT NULL,
    quote_id        TEXT,
    suggested_at    TEXT NOT NULL,
    was_shown       INTEGER NOT NULL DEFAULT 1,
    was_clicked     INTEGER NOT NULL DEFAULT 0,
    was_added_to_quote INTEGER NOT NULL DEFAULT 0,
    context         TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_suggestion_feedback_request_id
    ON suggestion_feedback(request_id);
CREATE INDEX idx_suggestion_feedback_customer_product
    ON suggestion_feedback(customer_id, product_id);
CREATE INDEX idx_suggestion_feedback_product_id
    ON suggestion_feedback(product_id);
