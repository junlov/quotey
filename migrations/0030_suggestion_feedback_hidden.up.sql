-- Add hidden/rejected signal tracking for suggestion feedback.
ALTER TABLE suggestion_feedback
    ADD COLUMN was_hidden INTEGER NOT NULL DEFAULT 0;
