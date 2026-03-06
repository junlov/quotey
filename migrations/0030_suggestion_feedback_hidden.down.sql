-- SQLite cannot DROP COLUMN directly; rebuild suggestion_feedback without was_hidden.
PRAGMA foreign_keys = OFF;
BEGIN TRANSACTION;

CREATE TABLE suggestion_feedback_revert (
    id                  TEXT PRIMARY KEY NOT NULL,
    request_id          TEXT NOT NULL,
    customer_id         TEXT NOT NULL,
    product_id          TEXT NOT NULL,
    product_sku         TEXT NOT NULL,
    score               REAL NOT NULL,
    confidence          TEXT NOT NULL,
    category            TEXT NOT NULL,
    quote_id            TEXT,
    suggested_at        TEXT NOT NULL,
    was_shown           INTEGER NOT NULL DEFAULT 1,
    was_clicked         INTEGER NOT NULL DEFAULT 0,
    was_added_to_quote  INTEGER NOT NULL DEFAULT 0,
    context             TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at          TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO suggestion_feedback_revert (
    id,
    request_id,
    customer_id,
    product_id,
    product_sku,
    score,
    confidence,
    category,
    quote_id,
    suggested_at,
    was_shown,
    was_clicked,
    was_added_to_quote,
    context,
    created_at,
    updated_at
)
SELECT
    id,
    request_id,
    customer_id,
    product_id,
    product_sku,
    score,
    confidence,
    category,
    quote_id,
    suggested_at,
    was_shown,
    was_clicked,
    was_added_to_quote,
    context,
    created_at,
    updated_at
FROM suggestion_feedback;

DROP TABLE suggestion_feedback;
ALTER TABLE suggestion_feedback_revert RENAME TO suggestion_feedback;

CREATE INDEX idx_suggestion_feedback_request_id
    ON suggestion_feedback(request_id);
CREATE INDEX idx_suggestion_feedback_customer_product
    ON suggestion_feedback(customer_id, product_id);
CREATE INDEX idx_suggestion_feedback_product_id
    ON suggestion_feedback(product_id);

COMMIT;
PRAGMA foreign_keys = ON;
