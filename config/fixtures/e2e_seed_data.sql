-- Canonical deterministic E2E seed dataset for the current Quotey schema.
-- Contains stable fixtures for three quote flows:
-- 1) net-new
-- 2) renewal
-- 3) discount exception
--
-- Supported tables in current migrations for this dataset:
-- quote, quote_line, flow_state, audit_event.
--
-- Canonical fixture metadata is carried in `flow_state.metadata_json`
-- to make account/deal/policy expectations explicit without adding
-- additional mutable domain tables for seed data.

-- ============================================
-- NET-NEW QUOTE FLOW
-- ============================================

-- Quote: draft state, needs configuration
INSERT INTO quote (
  id, status, currency, start_date, end_date, term_months, valid_until,
  created_by, created_at, updated_at
) VALUES
(
  'quote-netnew-001', 'draft', 'USD',
  '2026-03-01', '2027-02-28', 12, '2026-02-28',
  'rep.sarah', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

-- Quote lines for net-new
INSERT INTO quote_line (
  id, quote_id, product_id, quantity, unit_price, subtotal, created_at, updated_at
) VALUES
(
  'ql-netnew-001-1', 'quote-netnew-001', 'prod-plan-ent',
  100, 18.00, 1800.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
),
(
  'ql-netnew-001-2', 'quote-netnew-001', 'prod-sso',
  100, 2.00, 200.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
),
(
  'ql-netnew-001-3', 'quote-netnew-001', 'prod-support-premium',
  1, 500.00, 500.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

-- Flow state for net-new
INSERT INTO flow_state (
  id, quote_id, flow_type, current_step, step_number,
  required_fields_json, missing_fields_json, metadata_json, created_at, updated_at
) VALUES
(
  'fs-netnew-001', 'quote-netnew-001', 'net_new', 'gather_requirements',
  1, '["billing_country", "payment_terms"]', '["billing_country", "payment_terms"]',
  '{"account_id":"acct-netnew-001","account_name":"Acme Corp","deal_id":"deal-netnew-001","deal_name":"Acme Corp - New License","policy_profile":"standard","product_ids":["prod-plan-ent","prod-sso","prod-support-premium"],"product_names":["Enterprise Plan","SSO Add-on","Premium Support"],"channel":"e2e"}',
  '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- RENEWAL QUOTE FLOW
-- ============================================

-- Prior quote (historical context)
INSERT INTO quote (
  id, status, currency, start_date, end_date, term_months, valid_until,
  created_by, created_at, updated_at
) VALUES
(
  'quote-renewal-prior-001', 'sent', 'USD',
  '2025-03-01', '2026-02-28', 12, '2025-02-28',
  'rep.mike', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO quote_line (
  id, quote_id, product_id, quantity, unit_price, subtotal, created_at, updated_at
) VALUES
(
  'ql-renewal-prior-001-1', 'quote-renewal-prior-001', 'prod-plan-ent',
  75, 18.00, 1350.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
),
(
  'ql-renewal-prior-001-2', 'quote-renewal-prior-001', 'prod-support-premium',
  1, 500.00, 500.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

-- Current renewal quote (with expansion)
INSERT INTO quote (
  id, status, currency, start_date, end_date, term_months, valid_until,
  created_by, created_at, updated_at
) VALUES
(
  'quote-renewal-001', 'priced', 'USD',
  '2026-03-01', '2027-02-28', 12, '2026-02-28',
  'rep.mike', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO quote_line (
  id, quote_id, product_id, quantity, unit_price, subtotal, created_at, updated_at
) VALUES
(
  'ql-renewal-001-1', 'quote-renewal-001', 'prod-plan-ent',
  100, 18.00, 1800.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
),
(
  'ql-renewal-001-2', 'quote-renewal-001', 'prod-support-premium',
  1, 500.00, 500.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
),
(
  'ql-renewal-001-3', 'quote-renewal-001', 'prod-onboarding',
  1, 5000.00, 5000.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO flow_state (
  id, quote_id, flow_type, current_step, step_number,
  required_fields_json, missing_fields_json, metadata_json, created_at, updated_at
) VALUES
(
  'fs-renewal-001', 'quote-renewal-001', 'renewal', 'validate_expansion', 3,
  '["prior_quote_id"]', '[]',
  '{"account_id":"acct-renewal-001","account_name":"Globex Industries","deal_id":"deal-renewal-001","deal_name":"Globex - Annual Renewal","policy_profile":"renewal","product_ids":["prod-plan-ent","prod-support-premium","prod-onboarding"],"product_names":["Enterprise Plan","Premium Support","Onboarding"],"prior_quote_id":"quote-renewal-prior-001","channel":"e2e"}',
  '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- DISCOUNT EXCEPTION QUOTE FLOW
-- ============================================

-- Quote requiring discount approval (>20% for mid-market scenario)
INSERT INTO quote (
  id, status, currency, start_date, end_date, term_months, valid_until,
  created_by, created_at, updated_at
) VALUES
(
  'quote-discount-001', 'approval', 'USD',
  '2026-03-15', '2027-03-14', 12, '2026-03-01',
  'rep.jenny', '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO quote_line (
  id, quote_id, product_id, quantity, unit_price, subtotal, created_at, updated_at
) VALUES
(
  'ql-discount-001-1', 'quote-discount-001', 'prod-plan-pro',
  50, 10.00, 500.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
),
(
  'ql-discount-001-2', 'quote-discount-001', 'prod-sso',
  50, 2.00, 100.00, '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO flow_state (
  id, quote_id, flow_type, current_step, step_number,
  required_fields_json, missing_fields_json, metadata_json, created_at, updated_at
) VALUES
(
  'fs-discount-001', 'quote-discount-001', 'discount_exception', 'awaiting_approval', 4,
  '["approval_decision"]', '["approval_decision"]',
  '{"account_id":"acct-discount-001","account_name":"Initech LLC","deal_id":"deal-discount-001","deal_name":"Initech - Expansion Deal","policy_profile":"discount_exception","product_ids":["prod-plan-pro","prod-sso"],"product_names":["Pro Plan","SSO Add-on"],"requested_discount_pct":25,"threshold_pct":20,"channel":"e2e"}',
  '2026-01-01T00:00:00+00:00', '2026-01-01T00:00:00+00:00'
)
ON CONFLICT (id) DO NOTHING;

-- ============================================
-- AUDIT EVENTS (for traceability)
-- ============================================
INSERT INTO audit_event (
  id, timestamp, actor, actor_type, quote_id, event_type,
  event_category, payload_json, metadata_json
) VALUES
(
  'ae-netnew-001', '2026-01-01T00:00:00+00:00',
  'rep.sarah', 'human', 'quote-netnew-001', 'quote.created', 'quote',
  '{"flow_type":"net_new"}', NULL
),
(
  'ae-renewal-001', '2026-01-01T00:00:00+00:00',
  'rep.mike', 'human', 'quote-renewal-001', 'quote.created', 'quote',
  '{"flow_type":"renewal","prior_quote_id":"quote-renewal-prior-001"}', NULL
),
(
  'ae-discount-001', '2026-01-01T00:00:00+00:00',
  'rep.jenny', 'human', 'quote-discount-001', 'quote.created', 'quote',
  '{"flow_type":"discount_exception","requested_discount_pct":25}', NULL
),
(
  'ae-discount-002', '2026-01-01T00:00:00+00:00',
  'system', 'system', 'quote-discount-001', 'approval.requested', 'approval',
  '{"approver_role":"sales_manager","discount_pct":25}', NULL
)
ON CONFLICT (id) DO NOTHING;
