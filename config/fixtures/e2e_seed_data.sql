-- E2E Seed Dataset for 3 Core Quote Flows
-- This provides stable account/deal/product/policy fixtures for:
-- 1. Net-new quote flow
-- 2. Renewal flow
-- 3. Discount exception flow

-- ============================================
-- PRODUCTS (Shared across all flows)
-- ============================================
INSERT INTO product (id, sku, name, active, created_at, updated_at) VALUES
('prod-plan-pro', 'PLAN-PRO', 'Pro Plan', 1, datetime('now'), datetime('now')),
('prod-plan-ent', 'PLAN-ENT', 'Enterprise Plan', 1, datetime('now'), datetime('now')),
('prod-sso', 'ADDON-SSO', 'SSO Add-on', 1, datetime('now'), datetime('now')),
('prod-support-premium', 'SUPP-PREM', 'Premium Support', 1, datetime('now'), datetime('now')),
('prod-onboarding', 'SERV-ONB', 'Professional Onboarding', 1, datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- CUSTOMERS (Accounts)
-- ============================================

-- Net-new flow customer
INSERT INTO account (id, name, domain, segment, region, industry, created_at, updated_at) VALUES
('acct-netnew-001', 'Acme Corp', 'acme.example.com', 'enterprise', 'US', 'Technology', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Renewal flow customer (existing contract)
INSERT INTO account (id, name, domain, segment, region, industry, created_at, updated_at) VALUES
('acct-renewal-001', 'Globex Industries', 'globex.example.com', 'enterprise', 'US', 'Manufacturing', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Discount exception flow customer
INSERT INTO account (id, name, domain, segment, region, industry, created_at, updated_at) VALUES
('acct-discount-001', 'Initech LLC', 'initech.example.com', 'mid_market', 'US', 'Software', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- DEALS (Opportunities)
-- ============================================

-- Net-new deal
INSERT INTO deal (id, account_id, name, stage, deal_type, amount, currency, close_date, owner, created_at, updated_at) VALUES
('deal-netnew-001', 'acct-netnew-001', 'Acme Corp - New License', 'proposal', 'net_new', 50000.00, 'USD', '2026-03-31', 'rep.sarah', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Renewal deal
INSERT INTO deal (id, account_id, name, stage, deal_type, amount, currency, close_date, owner, created_at, updated_at) VALUES
('deal-renewal-001', 'acct-renewal-001', 'Globex - Annual Renewal', 'negotiation', 'renewal', 75000.00, 'USD', '2026-03-15', 'rep.mike', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Discount exception deal
INSERT INTO deal (id, account_id, name, stage, deal_type, amount, currency, close_date, owner, created_at, updated_at) VALUES
('deal-discount-001', 'acct-discount-001', 'Initech - Expansion Deal', 'negotiation', 'expansion', 35000.00, 'USD', '2026-03-30', 'rep.jenny', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- NET-NEW QUOTE FLOW
-- ============================================

-- Quote: Draft state, needs configuration
INSERT INTO quote (id, deal_id, account_id, status, currency, start_date, end_date, term_months, valid_until, created_by, created_at, updated_at) VALUES
('quote-netnew-001', 'deal-netnew-001', 'acct-netnew-001', 'draft', 'USD', '2026-03-01', '2027-02-28', 12, '2026-02-28', 'rep.sarah', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Quote lines for net-new
INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, created_at, updated_at) VALUES
('ql-netnew-001-1', 'quote-netnew-001', 'prod-plan-ent', 100, 18.00, 1800.00, datetime('now'), datetime('now')),
('ql-netnew-001-2', 'quote-netnew-001', 'prod-sso', 100, 2.00, 200.00, datetime('now'), datetime('now')),
('ql-netnew-001-3', 'quote-netnew-001', 'prod-support-premium', 1, 500.00, 500.00, datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Flow state for net-new
INSERT INTO flow_state (id, quote_id, flow_type, current_step, step_number, required_fields_json, missing_fields_json, created_at, updated_at) VALUES
('fs-netnew-001', 'quote-netnew-001', 'net_new', 'gather_requirements', 1, '["billing_country", "payment_terms"]', '["billing_country", "payment_terms"]', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- RENEWAL QUOTE FLOW
-- ============================================

-- Prior quote (the one being renewed)
INSERT INTO quote (id, deal_id, account_id, status, currency, start_date, end_date, term_months, valid_until, created_by, created_at, updated_at) VALUES
('quote-renewal-prior-001', 'deal-renewal-001', 'acct-renewal-001', 'sent', 'USD', '2025-03-01', '2026-02-28', 12, '2025-02-28', 'rep.mike', datetime('2025-01-15'), datetime('2025-01-15'))
ON CONFLICT (id) DO NOTHING;

-- Prior quote lines
INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, created_at, updated_at) VALUES
('ql-renewal-prior-001-1', 'quote-renewal-prior-001', 'prod-plan-ent', 75, 18.00, 1350.00, datetime('2025-01-15'), datetime('2025-01-15')),
('ql-renewal-prior-001-2', 'quote-renewal-prior-001', 'prod-support-premium', 1, 500.00, 500.00, datetime('2025-01-15'), datetime('2025-01-15'))
ON CONFLICT (id) DO NOTHING;

-- Current renewal quote (with expansion)
INSERT INTO quote (id, deal_id, account_id, status, currency, start_date, end_date, term_months, valid_until, created_by, created_at, updated_at) VALUES
('quote-renewal-001', 'deal-renewal-001', 'acct-renewal-001', 'priced', 'USD', '2026-03-01', '2027-02-28', 12, '2026-02-28', 'rep.mike', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Renewal quote lines (expanded from 75 to 100 seats)
INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, created_at, updated_at) VALUES
('ql-renewal-001-1', 'quote-renewal-001', 'prod-plan-ent', 100, 18.00, 1800.00, datetime('now'), datetime('now')),
('ql-renewal-001-2', 'quote-renewal-001', 'prod-support-premium', 1, 500.00, 500.00, datetime('now'), datetime('now')),
('ql-renewal-001-3', 'quote-renewal-001', 'prod-onboarding', 1, 5000.00, 5000.00, datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Flow state for renewal
INSERT INTO flow_state (id, quote_id, flow_type, current_step, step_number, required_fields_json, missing_fields_json, created_at, updated_at) VALUES
('fs-renewal-001', 'quote-renewal-001', 'renewal', 'validate_expansion', 3, '["prior_quote_id"]', '[]', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- DISCOUNT EXCEPTION QUOTE FLOW
-- ============================================

-- Quote requiring discount approval (>20% for mid_market)
INSERT INTO quote (id, deal_id, account_id, status, currency, start_date, end_date, term_months, valid_until, created_by, created_at, updated_at) VALUES
('quote-discount-001', 'deal-discount-001', 'acct-discount-001', 'approval', 'USD', '2026-03-15', '2027-03-14', 12, '2026-03-01', 'rep.jenny', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Quote lines with high discount
INSERT INTO quote_line (id, quote_id, product_id, quantity, unit_price, subtotal, discount_pct, discount_amount, created_at, updated_at) VALUES
('ql-discount-001-1', 'quote-discount-001', 'prod-plan-pro', 50, 10.00, 500.00, 25.0, 125.00, datetime('now'), datetime('now')),
('ql-discount-001-2', 'quote-discount-001', 'prod-sso', 50, 2.00, 100.00, 25.0, 25.00, datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- Approval request for discount exception
INSERT INTO approval_request (id, quote_id, status, requested_by, approver_role, reason, context_json, created_at, expires_at) VALUES
('apr-discount-001', 'quote-discount-001', 'pending', 'rep.jenny', 'sales_manager', '25% discount required to match competitor pricing', '{"competitor": "VendorX", "justification": "Strategic account, multi-year potential"}', datetime('now'), datetime('now', '+7 days'))
ON CONFLICT (id) DO NOTHING;

-- Flow state for discount exception
INSERT INTO flow_state (id, quote_id, flow_type, current_step, step_number, required_fields_json, missing_fields_json, created_at, updated_at) VALUES
('fs-discount-001', 'quote-discount-001', 'discount_exception', 'awaiting_approval', 4, '["approval_decision"]', '["approval_decision"]', datetime('now'), datetime('now'))
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- AUDIT EVENTS (for traceability)
-- ============================================
INSERT INTO audit_event (id, timestamp, actor, actor_type, quote_id, event_type, event_category, payload_json) VALUES
('ae-netnew-001', datetime('now'), 'rep.sarah', 'human', 'quote-netnew-001', 'quote.created', 'quote', '{"deal_id": "deal-netnew-001"}'),
('ae-renewal-001', datetime('now'), 'rep.mike', 'human', 'quote-renewal-001', 'quote.created', 'quote', '{"deal_id": "deal-renewal-001", "prior_quote_id": "quote-renewal-prior-001"}'),
('ae-discount-001', datetime('now'), 'rep.jenny', 'human', 'quote-discount-001', 'quote.created', 'quote', '{"deal_id": "deal-discount-001"}'),
('ae-discount-002', datetime('now'), 'system', 'system', 'quote-discount-001', 'approval.requested', 'approval', '{"approver_role": "sales_manager", "discount_pct": 25}')
ON CONFLICT (id) DO NOTHING;
