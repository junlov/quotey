-- Migration: 0026_explicit_assumptions
-- Description: Add fields to track explicit vs assumed values for tax, currency, payment terms
-- Author: Quotey Team
-- Date: 2026-02-26

-- Add columns to track whether values were explicitly set or defaulted
ALTER TABLE quote ADD COLUMN currency_explicit INTEGER DEFAULT 0;
ALTER TABLE quote ADD COLUMN tax_rate_explicit INTEGER DEFAULT 0;
ALTER TABLE quote ADD COLUMN tax_rate_value REAL DEFAULT 0.0;
ALTER TABLE quote ADD COLUMN payment_terms TEXT DEFAULT 'net_30';
ALTER TABLE quote ADD COLUMN payment_terms_explicit INTEGER DEFAULT 0;
ALTER TABLE quote ADD COLUMN billing_country TEXT;
ALTER TABLE quote ADD COLUMN billing_country_explicit INTEGER DEFAULT 0;

-- Create index for assumption queries
CREATE INDEX IF NOT EXISTS idx_quote_assumptions ON quote(currency_explicit, tax_rate_explicit, payment_terms_explicit);

-- Update existing quotes to mark their current values as assumed (not explicit)
UPDATE quote SET currency_explicit = 0 WHERE currency_explicit IS NULL;
UPDATE quote SET tax_rate_explicit = 0 WHERE tax_rate_explicit IS NULL;
UPDATE quote SET payment_terms_explicit = 0 WHERE payment_terms_explicit IS NULL;
