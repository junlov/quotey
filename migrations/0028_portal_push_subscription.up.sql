-- Stores browser push subscriptions for mobile/web portal approval notifications.
CREATE TABLE portal_push_subscription (
    id            TEXT PRIMARY KEY NOT NULL,
    endpoint      TEXT NOT NULL UNIQUE,
    p256dh        TEXT NOT NULL,
    auth          TEXT NOT NULL,
    user_agent    TEXT,
    device_label  TEXT,
    revoked       INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_portal_push_subscription_revoked
    ON portal_push_subscription(revoked);
