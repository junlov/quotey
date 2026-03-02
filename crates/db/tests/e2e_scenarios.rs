/// End-to-end scenario tests for the quotey CPQ lifecycle (quotey-115.4).
///
/// These tests exercise the full business flow stack — persistence,
/// domain engines, flow state machine, audit trail — against a real
/// SQLite database (in-memory).
///
/// Scenarios:
///   S-001  Happy path: create → price → finalize → deliver (no approval)
///   S-002  Approval path: create → price → policy violation → approve → deliver
///   S-003  Rejection path: policy violation → approval denied → quote rejected
///   S-004  Deterministic replay: same inputs produce identical outcomes
///   S-005  Audit trail completeness: every transition emits trackable events
use chrono::Utc;
use quotey_core::audit::{AuditCategory, AuditContext, AuditOutcome, InMemoryAuditSink};
use quotey_core::cpq::constraints::{
    ConstraintEngine, ConstraintInput, DeterministicConstraintEngine,
};
use quotey_core::cpq::policy::{evaluate_policy_input, PolicyInput};
use quotey_core::cpq::pricing::price_quote_with_trace;
use quotey_core::domain::approval::{ApprovalId, ApprovalRequest, ApprovalStatus};
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
use quotey_core::flows::engine::{FlowEngine, NetNewFlow};
use quotey_core::flows::states::{FlowContext, FlowEvent, FlowState};
use quotey_db::repositories::audit::SqlAuditEventRepository;
use quotey_db::repositories::{
    ApprovalRepository, ProductRepository, QuoteRepository, SqlApprovalRepository,
    SqlProductRepository, SqlQuoteRepository,
};
use rust_decimal::Decimal;

type TestResult<T = ()> = Result<T, String>;

// ── Test infrastructure ─────────────────────────────────────────────────

async fn setup_pool() -> TestResult<quotey_db::DbPool> {
    let pool = quotey_db::connect_with_settings("sqlite::memory:", 1, 30)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    quotey_db::migrations::run_pending(&pool).await.map_err(|e| format!("migrations: {e}"))?;
    Ok(pool)
}

fn make_quote(id: &str, lines: Vec<QuoteLine>) -> Quote {
    let now = Utc::now();
    Quote {
        id: QuoteId(id.to_string()),
        version: 1,
        status: QuoteStatus::Draft,
        account_id: Some("acct-e2e".to_string()),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        end_date: None,
        valid_until: None,
        notes: None,
        created_by: "e2e-harness".to_string(),
        lines,
        created_at: now,
        updated_at: now,
    }
}

fn line(product_id: &str, qty: u32, unit_price_cents: i64) -> QuoteLine {
    QuoteLine {
        product_id: ProductId(product_id.to_string()),
        quantity: qty,
        unit_price: Decimal::new(unit_price_cents, 2),
        discount_pct: 0.0,
        notes: None,
    }
}

async fn seed_product(
    repo: &SqlProductRepository,
    id: &str,
    sku: &str,
    name: &str,
    price_cents: i64,
) -> TestResult {
    let mut p = Product::simple(id, sku, name);
    p.base_price = Some(Decimal::new(price_cents, 2));
    repo.save(p).await.map_err(|e| format!("seed product {id}: {e}"))
}

/// Helper: persist audit events from an InMemoryAuditSink into the SQL audit table.
async fn flush_audit_to_db(
    sink: &InMemoryAuditSink,
    audit_repo: &SqlAuditEventRepository,
) -> TestResult {
    for event in sink.events() {
        audit_repo.save(&event).await.map_err(|e| format!("flush audit: {e}"))?;
    }
    Ok(())
}

// ── S-001: Happy path (no approval required) ────────────────────────────

#[tokio::test]
async fn s001_happy_path_create_price_finalize_deliver() -> TestResult {
    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());
    let product_repo = SqlProductRepository::new(pool.clone());
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    // Step 1: Seed catalog
    seed_product(&product_repo, "PROD-HP-1", "SKU-HP-A", "Standard Widget", 4999).await?;
    seed_product(&product_repo, "PROD-HP-2", "SKU-HP-B", "Widget Accessory", 1999).await?;

    // Step 2: Create quote with lines
    let quote =
        make_quote("Q-E2E-HP-001", vec![line("PROD-HP-1", 5, 4999), line("PROD-HP-2", 10, 1999)]);
    quote_repo.save(quote).await.map_err(|e| format!("save quote: {e}"))?;

    // Step 3: Load and validate constraints
    let loaded = quote_repo
        .find_by_id(&QuoteId("Q-E2E-HP-001".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    let constraint_engine = DeterministicConstraintEngine;
    let constraints =
        constraint_engine.validate(&ConstraintInput { quote_lines: loaded.lines.clone() });
    assert!(constraints.valid, "quote should pass constraints: {:?}", constraints.violations);

    // Step 4: Price the quote
    let pricing = price_quote_with_trace(&loaded, "USD");
    // 5 × $49.99 = $249.95 + 10 × $19.99 = $199.90 = $449.85
    let expected_total = Decimal::new(44985, 2);
    assert_eq!(pricing.total, expected_total, "pricing total mismatch");

    // Step 5: Policy check — 0% discount = no approval needed
    let policy = evaluate_policy_input(&PolicyInput {
        requested_discount_pct: Decimal::ZERO,
        deal_value: pricing.total,
        minimum_margin_pct: Decimal::new(25, 0),
    });
    assert!(!policy.approval_required, "0% discount should not require approval");

    // Step 6: Drive the flow state machine with audit
    let engine = FlowEngine::new(NetNewFlow);
    let sink = InMemoryAuditSink::default();
    let audit_ctx = AuditContext::new(
        Some(QuoteId("Q-E2E-HP-001".to_string())),
        Some("1730000000.0001".to_string()),
        "e2e-s001",
        "e2e-harness",
    );
    let ctx = FlowContext::default();

    let mut state = engine.initial_state();
    assert_eq!(state, FlowState::Draft);

    // Draft → Validated
    let outcome = engine
        .apply_with_audit(&state, &FlowEvent::RequiredFieldsCollected, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("transition 1: {e}"))?;
    assert_eq!(outcome.to, FlowState::Validated);
    state = outcome.to;

    // Validated → Priced
    let outcome = engine
        .apply_with_audit(&state, &FlowEvent::PricingCalculated, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("transition 2: {e}"))?;
    assert_eq!(outcome.to, FlowState::Priced);
    state = outcome.to;

    // Priced → Finalized (policy clear)
    let outcome = engine
        .apply_with_audit(&state, &FlowEvent::PolicyClear, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("transition 3: {e}"))?;
    assert_eq!(outcome.to, FlowState::Finalized);
    state = outcome.to;

    // Finalized → Sent
    let outcome = engine
        .apply_with_audit(&state, &FlowEvent::QuoteDelivered, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("transition 4: {e}"))?;
    assert_eq!(outcome.to, FlowState::Sent);

    // Step 7: Verify audit trail
    let events = sink.events();
    assert_eq!(events.len(), 4, "expected 4 audit events for happy path");
    assert!(events.iter().all(|e| e.outcome == AuditOutcome::Success));
    assert!(events.iter().all(|e| e.correlation_id == "e2e-s001"));

    // Persist audit to DB and verify round-trip
    flush_audit_to_db(&sink, &audit_repo).await?;
    let db_events = audit_repo
        .find_by_quote_id(&QuoteId("Q-E2E-HP-001".to_string()))
        .await
        .map_err(|e| format!("query audit: {e}"))?;
    assert_eq!(db_events.len(), 4, "expected 4 audit events in DB");

    Ok(())
}

// ── S-002: Approval path (discount triggers policy violation) ───────────

#[tokio::test]
async fn s002_approval_path_policy_violation_approve_deliver() -> TestResult {
    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());
    let approval_repo = SqlApprovalRepository::new(pool.clone());
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    // Create a quote: 10 × $99.99 = $999.90
    let quote = make_quote("Q-E2E-AP-001", vec![line("prod-ap", 10, 9999)]);
    quote_repo.save(quote).await.map_err(|e| format!("save: {e}"))?;

    let loaded = quote_repo
        .find_by_id(&QuoteId("Q-E2E-AP-001".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("not found")?;

    // Price it
    let pricing = price_quote_with_trace(&loaded, "USD");
    assert_eq!(pricing.total, Decimal::new(99990, 2));

    // Policy: 25% discount requires sales_manager approval
    let policy = evaluate_policy_input(&PolicyInput {
        requested_discount_pct: Decimal::new(25, 0),
        deal_value: pricing.total,
        minimum_margin_pct: Decimal::new(75, 0),
    });
    assert!(policy.approval_required);
    assert!(policy.violations.iter().any(|v| v.policy_id == "discount-cap"));
    let required_role = policy.violations[0].required_approval.as_deref().unwrap_or("unknown");

    // Drive flow through approval path
    let engine = FlowEngine::new(NetNewFlow);
    let sink = InMemoryAuditSink::default();
    let audit_ctx = AuditContext::new(
        Some(QuoteId("Q-E2E-AP-001".to_string())),
        None,
        "e2e-s002",
        "e2e-harness",
    );
    let ctx = FlowContext::default();

    let mut state = engine.initial_state();

    // Draft → Validated → Priced
    state = engine
        .apply_with_audit(&state, &FlowEvent::RequiredFieldsCollected, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("t1: {e}"))?
        .to;
    state = engine
        .apply_with_audit(&state, &FlowEvent::PricingCalculated, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("t2: {e}"))?
        .to;
    assert_eq!(state, FlowState::Priced);

    // Priced → Approval (policy violation detected)
    state = engine
        .apply_with_audit(&state, &FlowEvent::PolicyViolationDetected, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("t3: {e}"))?
        .to;
    assert_eq!(state, FlowState::Approval);

    // Persist the approval request in the DB
    let now = Utc::now();
    let approval = ApprovalRequest {
        id: ApprovalId("APR-E2E-001".to_string()),
        quote_id: QuoteId("Q-E2E-AP-001".to_string()),
        approver_role: required_role.to_string(),
        reason: "Discount exceeds 20% threshold".to_string(),
        justification: "Strategic deal — competitor pricing pressure".to_string(),
        status: ApprovalStatus::Pending,
        requested_by: "e2e-harness".to_string(),
        expires_at: Some(now + chrono::Duration::hours(4)),
        created_at: now,
        updated_at: now,
    };
    approval_repo.save(approval).await.map_err(|e| format!("save approval: {e}"))?;

    // Simulate approval granted
    let mut approved = approval_repo
        .find_by_id(&ApprovalId("APR-E2E-001".to_string()))
        .await
        .map_err(|e| format!("find approval: {e}"))?
        .ok_or("approval not found")?;
    approved.status = ApprovalStatus::Approved;
    approved.updated_at = Utc::now();
    approval_repo.save(approved).await.map_err(|e| format!("update approval: {e}"))?;

    // Verify approval is now in Approved state
    let final_approval = approval_repo
        .find_by_id(&ApprovalId("APR-E2E-001".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("not found")?;
    assert_eq!(final_approval.status, ApprovalStatus::Approved);

    // Approval → Approved → Sent
    state = engine
        .apply_with_audit(&state, &FlowEvent::ApprovalGranted, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("t4: {e}"))?
        .to;
    assert_eq!(state, FlowState::Approved);

    state = engine
        .apply_with_audit(&state, &FlowEvent::QuoteDelivered, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("t5: {e}"))?
        .to;
    assert_eq!(state, FlowState::Sent);

    // Verify audit: 5 transitions total
    let events = sink.events();
    assert_eq!(events.len(), 5, "expected 5 audit events for approval path");

    // Persist and verify DB round-trip
    flush_audit_to_db(&sink, &audit_repo).await?;
    let db_events = audit_repo
        .find_by_quote_id(&QuoteId("Q-E2E-AP-001".to_string()))
        .await
        .map_err(|e| format!("query: {e}"))?;
    assert_eq!(db_events.len(), 5);

    Ok(())
}

// ── S-003: Rejection path ───────────────────────────────────────────────

#[tokio::test]
async fn s003_rejection_path_policy_violation_denied() -> TestResult {
    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());
    let approval_repo = SqlApprovalRepository::new(pool.clone());
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    let quote = make_quote("Q-E2E-RJ-001", vec![line("prod-rj", 5, 20000)]);
    quote_repo.save(quote).await.map_err(|e| format!("save: {e}"))?;

    // 35% discount → requires vp_finance
    let pricing_total = Decimal::new(100000, 2); // 5 × $200.00
    let policy = evaluate_policy_input(&PolicyInput {
        requested_discount_pct: Decimal::new(35, 0),
        deal_value: pricing_total,
        minimum_margin_pct: Decimal::new(65, 0),
    });
    assert!(policy.approval_required);
    assert!(policy
        .violations
        .iter()
        .any(|v| v.required_approval == Some("vp_finance".to_string())));

    // Drive flow to Approval state
    let engine = FlowEngine::new(NetNewFlow);
    let sink = InMemoryAuditSink::default();
    let audit_ctx = AuditContext::new(
        Some(QuoteId("Q-E2E-RJ-001".to_string())),
        None,
        "e2e-s003",
        "e2e-harness",
    );
    let ctx = FlowContext::default();

    let mut state = engine.initial_state();
    state = engine
        .apply(&state, &FlowEvent::RequiredFieldsCollected, &ctx)
        .map_err(|e| format!("{e}"))?
        .to;
    state =
        engine.apply(&state, &FlowEvent::PricingCalculated, &ctx).map_err(|e| format!("{e}"))?.to;
    state = engine
        .apply(&state, &FlowEvent::PolicyViolationDetected, &ctx)
        .map_err(|e| format!("{e}"))?
        .to;
    assert_eq!(state, FlowState::Approval);

    // Create and reject the approval
    let now = Utc::now();
    let approval = ApprovalRequest {
        id: ApprovalId("APR-E2E-RJ-001".to_string()),
        quote_id: QuoteId("Q-E2E-RJ-001".to_string()),
        approver_role: "vp_finance".to_string(),
        reason: "Discount exceeds 30% hard cap".to_string(),
        justification: "Attempted strategic pricing".to_string(),
        status: ApprovalStatus::Rejected,
        requested_by: "e2e-harness".to_string(),
        expires_at: Some(now + chrono::Duration::hours(4)),
        created_at: now,
        updated_at: now,
    };
    approval_repo.save(approval).await.map_err(|e| format!("save: {e}"))?;

    // Approval → Rejected
    state = engine
        .apply_with_audit(&state, &FlowEvent::ApprovalDenied, &ctx, &sink, &audit_ctx)
        .map_err(|e| format!("deny: {e}"))?
        .to;
    assert_eq!(state, FlowState::Rejected);

    // Verify audit captures the rejection
    let events = sink.events();
    assert_eq!(events.len(), 1, "expected 1 audit event from denial transition");
    assert_eq!(events[0].outcome, AuditOutcome::Success); // transition succeeded (to Rejected)
    assert_eq!(events[0].metadata.get("to").map(|s| s.as_str()), Some("Rejected"));

    // Persist and verify
    flush_audit_to_db(&sink, &audit_repo).await?;
    let db_count = audit_repo
        .count_by_quote_id(&QuoteId("Q-E2E-RJ-001".to_string()))
        .await
        .map_err(|e| format!("count: {e}"))?;
    assert_eq!(db_count, 1);

    Ok(())
}

// ── S-004: Deterministic replay ─────────────────────────────────────────

#[tokio::test]
async fn s004_deterministic_replay_produces_identical_outcomes() -> TestResult {
    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());

    // Create and persist the same quote
    let quote = make_quote("Q-E2E-DET-001", vec![line("prod-det", 7, 14299)]);
    quote_repo.save(quote).await.map_err(|e| format!("save: {e}"))?;

    let loaded = quote_repo
        .find_by_id(&QuoteId("Q-E2E-DET-001".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("not found")?;

    // Run pricing twice from the same DB-loaded data
    let result_1 = price_quote_with_trace(&loaded, "USD");
    let result_2 = price_quote_with_trace(&loaded, "USD");

    assert_eq!(result_1.subtotal, result_2.subtotal, "subtotal mismatch across runs");
    assert_eq!(result_1.total, result_2.total, "total mismatch across runs");
    assert_eq!(result_1.discount_total, result_2.discount_total, "discount mismatch across runs");
    assert_eq!(result_1.tax_total, result_2.tax_total, "tax mismatch across runs");
    assert_eq!(
        result_1.approval_required, result_2.approval_required,
        "approval flag mismatch across runs"
    );

    // Policy decisions should also be identical
    let policy_input = PolicyInput {
        requested_discount_pct: Decimal::new(15, 0),
        deal_value: result_1.total,
        minimum_margin_pct: Decimal::new(30, 0),
    };
    let policy_1 = evaluate_policy_input(&policy_input);
    let policy_2 = evaluate_policy_input(&policy_input);

    assert_eq!(policy_1.approval_required, policy_2.approval_required);
    assert_eq!(policy_1.violations.len(), policy_2.violations.len());

    // Flow engine replay should produce same final state
    let engine = FlowEngine::new(NetNewFlow);
    let ctx = FlowContext::default();
    let events = [
        FlowEvent::RequiredFieldsCollected,
        FlowEvent::PricingCalculated,
        FlowEvent::PolicyClear,
        FlowEvent::QuoteDelivered,
    ];

    let replay = |run_label: &str| -> Result<FlowState, String> {
        let mut state = engine.initial_state();
        for (i, event) in events.iter().enumerate() {
            state = engine
                .apply(&state, event, &ctx)
                .map_err(|e| format!("{run_label} step {i}: {e}"))?
                .to;
        }
        Ok(state)
    };

    let final_1 = replay("run-1")?;
    let final_2 = replay("run-2")?;
    assert_eq!(final_1, final_2, "replay produced different final states");
    assert_eq!(final_1, FlowState::Sent);

    Ok(())
}

// ── S-005: Audit trail completeness ─────────────────────────────────────

#[tokio::test]
async fn s005_audit_trail_captures_every_transition_with_correlation() -> TestResult {
    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    let quote = make_quote("Q-E2E-AUD-001", vec![line("prod-aud", 3, 7500)]);
    quote_repo.save(quote).await.map_err(|e| format!("save: {e}"))?;

    let engine = FlowEngine::new(NetNewFlow);
    let sink = InMemoryAuditSink::default();
    let audit_ctx = AuditContext::new(
        Some(QuoteId("Q-E2E-AUD-001".to_string())),
        Some("1730000000.0500".to_string()),
        "e2e-s005",
        "e2e-harness",
    );
    let ctx = FlowContext::default();

    // Full happy-path transitions
    let transitions: Vec<(FlowEvent, FlowState)> = vec![
        (FlowEvent::RequiredFieldsCollected, FlowState::Validated),
        (FlowEvent::PricingCalculated, FlowState::Priced),
        (FlowEvent::PolicyClear, FlowState::Finalized),
        (FlowEvent::QuoteDelivered, FlowState::Sent),
    ];

    let mut state = engine.initial_state();
    for (event, expected_to) in &transitions {
        let outcome = engine
            .apply_with_audit(&state, event, &ctx, &sink, &audit_ctx)
            .map_err(|e| format!("transition to {expected_to:?}: {e}"))?;
        assert_eq!(&outcome.to, expected_to);
        state = outcome.to.clone();
    }

    // Verify in-memory audit events
    let events = sink.events();
    assert_eq!(events.len(), transitions.len());

    for (i, event) in events.iter().enumerate() {
        // Every event has the correct quote_id
        assert_eq!(
            event.quote_id.as_ref().map(|q| q.0.as_str()),
            Some("Q-E2E-AUD-001"),
            "event {i} missing quote_id"
        );
        // Every event has correlation_id
        assert_eq!(event.correlation_id, "e2e-s005", "event {i} correlation mismatch");
        // Every event has thread_id
        assert_eq!(
            event.thread_id.as_deref(),
            Some("1730000000.0500"),
            "event {i} thread_id mismatch"
        );
        // Every event is a flow transition
        assert_eq!(event.event_type, "flow.transition_applied");
        assert_eq!(event.category, AuditCategory::Flow);
        // Every event has from/to metadata
        assert!(event.metadata.contains_key("from"), "event {i} missing 'from'");
        assert!(event.metadata.contains_key("to"), "event {i} missing 'to'");
    }

    // Verify state progression in metadata
    assert_eq!(events[0].metadata["from"], "Draft");
    assert_eq!(events[0].metadata["to"], "Validated");
    assert_eq!(events[1].metadata["from"], "Validated");
    assert_eq!(events[1].metadata["to"], "Priced");
    assert_eq!(events[2].metadata["from"], "Priced");
    assert_eq!(events[2].metadata["to"], "Finalized");
    assert_eq!(events[3].metadata["from"], "Finalized");
    assert_eq!(events[3].metadata["to"], "Sent");

    // Persist to SQL and verify completeness
    flush_audit_to_db(&sink, &audit_repo).await?;
    let db_events = audit_repo
        .find_by_quote_id(&QuoteId("Q-E2E-AUD-001".to_string()))
        .await
        .map_err(|e| format!("query: {e}"))?;

    assert_eq!(db_events.len(), 4, "audit DB should have 4 events");
    for (i, db_event) in db_events.iter().enumerate() {
        assert_eq!(db_event.correlation_id, "e2e-s005", "DB event {i} correlation");
        assert_eq!(db_event.event_type, "flow.transition_applied", "DB event {i} type");
    }

    Ok(())
}

// ── S-006: Invalid transition rejected with audit ───────────────────────

#[tokio::test]
async fn s006_invalid_transition_emits_rejection_audit() -> TestResult {
    let pool = setup_pool().await?;
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    let engine = FlowEngine::new(NetNewFlow);
    let sink = InMemoryAuditSink::default();
    let audit_ctx = AuditContext::new(None, None, "e2e-s006", "e2e-harness");
    let ctx = FlowContext::default();

    // Try an invalid transition: Draft → QuoteDelivered (not allowed)
    let result = engine.apply_with_audit(
        &FlowState::Draft,
        &FlowEvent::QuoteDelivered,
        &ctx,
        &sink,
        &audit_ctx,
    );

    assert!(result.is_err(), "invalid transition should fail");

    // Audit should capture the rejection
    let events = sink.events();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "flow.transition_rejected");
    assert_eq!(events[0].outcome, AuditOutcome::Rejected);
    assert!(events[0].metadata.contains_key("error"));

    // Persist and verify DB
    flush_audit_to_db(&sink, &audit_repo).await?;
    let db_events = audit_repo
        .find_by_type("flow.transition_rejected")
        .await
        .map_err(|e| format!("query: {e}"))?;
    assert!(
        db_events.iter().any(|e| e.correlation_id == "e2e-s006"),
        "expected rejection event in DB"
    );

    Ok(())
}

// ── S-007: Cancellation from any state ──────────────────────────────────

#[tokio::test]
async fn s007_cancellation_from_priced_state() -> TestResult {
    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());

    let quote = make_quote("Q-E2E-CAN-001", vec![line("prod-can", 1, 5000)]);
    quote_repo.save(quote).await.map_err(|e| format!("save: {e}"))?;

    let engine = FlowEngine::new(NetNewFlow);
    let ctx = FlowContext::default();

    // Drive to Priced
    let mut state = engine.initial_state();
    state = engine
        .apply(&state, &FlowEvent::RequiredFieldsCollected, &ctx)
        .map_err(|e| format!("{e}"))?
        .to;
    state =
        engine.apply(&state, &FlowEvent::PricingCalculated, &ctx).map_err(|e| format!("{e}"))?.to;
    assert_eq!(state, FlowState::Priced);

    // Cancel from Priced
    let outcome = engine
        .apply(&state, &FlowEvent::CancelRequested, &ctx)
        .map_err(|e| format!("cancel: {e}"))?;
    assert_eq!(outcome.to, FlowState::Cancelled);

    // Verify cancelled quote can be loaded from DB with original data intact
    let loaded = quote_repo
        .find_by_id(&QuoteId("Q-E2E-CAN-001".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("not found")?;
    assert_eq!(loaded.lines.len(), 1, "cancelled quote should still have lines");

    Ok(())
}
