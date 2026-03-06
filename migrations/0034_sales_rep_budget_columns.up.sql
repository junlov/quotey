-- 0034: Add discount budget tracking columns to sales_rep.
-- These enable per-rep monthly discount authority limits (PC-1.3 / PC-3.2).

ALTER TABLE sales_rep ADD COLUMN discount_budget_monthly_cents INTEGER NOT NULL DEFAULT 0;
ALTER TABLE sales_rep ADD COLUMN spent_discount_cents INTEGER NOT NULL DEFAULT 0;
