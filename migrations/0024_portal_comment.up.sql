-- Portal comments: customer comments on quotes and individual line items.
-- Supports threaded replies via parent_id self-reference.
CREATE TABLE portal_comment (
    id              TEXT PRIMARY KEY NOT NULL,
    quote_id        TEXT NOT NULL,
    quote_line_id   TEXT,                          -- NULL = overall quote comment
    parent_id       TEXT,                          -- NULL = top-level comment
    author_name     TEXT NOT NULL,
    author_email    TEXT NOT NULL,
    body            TEXT NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (quote_id)      REFERENCES quote(id)      ON DELETE CASCADE,
    FOREIGN KEY (quote_line_id) REFERENCES quote_line(id)  ON DELETE CASCADE,
    FOREIGN KEY (parent_id)     REFERENCES portal_comment(id) ON DELETE CASCADE
);

CREATE INDEX idx_portal_comment_quote_id ON portal_comment(quote_id);
CREATE INDEX idx_portal_comment_line_id  ON portal_comment(quote_line_id);
CREATE INDEX idx_portal_comment_parent   ON portal_comment(parent_id);
