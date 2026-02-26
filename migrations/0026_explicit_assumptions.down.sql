-- Migration: 0026_explicit_assumptions (rollback)
-- Description: Remove explicit assumption tracking columns

DROP INDEX IF EXISTS idx_quote_assumptions;

ALTER TABLE quote DROP COLUMN currency_explicit;
ALTER TABLE quote DROP COLUMN tax_rate_explicit;
ALTER TABLE quote DROP COLUMN tax_rate_value;
ALTER TABLE quote DROP COLUMN payment_terms;
ALTER TABLE quote DROP COLUMN payment_terms_explicit;
ALTER TABLE quote DROP COLUMN billing_country;
ALTER TABLE quote DROP COLUMN billing_country_explicit;
