-- Down migration: remove outbox dead letter table
DROP TABLE IF EXISTS outbox_dead_letter;
