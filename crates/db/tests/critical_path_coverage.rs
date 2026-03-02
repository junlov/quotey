/// Integration tests for critical-path coverage gaps identified in
/// `.planning/qa/CRITICAL_PATH_MATRIX.md` (quotey-115.2).
///
/// These tests exercise the real SQLite stack (migrations, FK constraints,
/// WAL mode) to verify that domain logic round-trips correctly through
/// persistence.
///
/// Coverage targets:
///   G-001  Pricing engine real-DB round-trip
///   G-002  Constraint engine real-DB validation
///   G-003  Audit event real-DB persistence
///   G-004  Product FTS search real-DB test
use quotey_core::cpq::constraints::{
    ConstraintEngine, ConstraintInput, DeterministicConstraintEngine,
};
use quotey_core::cpq::pricing::{
    price_quote_with_trace, DeterministicPricingEngine, PricingEngine,
};
use quotey_core::domain::product::{Product, ProductId};
use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
use quotey_db::repositories::{QuoteRepository, SqlQuoteRepository};
use rust_decimal::Decimal;

type TestResult<T = ()> = Result<T, String>;

async fn setup_pool() -> TestResult<quotey_db::DbPool> {
    let pool = quotey_db::connect_with_settings("sqlite::memory:", 1, 30)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    quotey_db::migrations::run_pending(&pool).await.map_err(|e| format!("migrations: {e}"))?;
    Ok(pool)
}

fn make_quote(id: &str, lines: Vec<QuoteLine>) -> Quote {
    let now = chrono::Utc::now();
    Quote {
        id: QuoteId(id.to_string()),
        version: 1,
        status: QuoteStatus::Draft,
        account_id: Some("acct-test".to_string()),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        end_date: None,
        valid_until: None,
        notes: None,
        created_by: "test-harness".to_string(),
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

// ── G-001: Pricing engine real-DB round-trip ────────────────────────────

#[tokio::test]
async fn g001_pricing_round_trip_single_line() -> TestResult {
    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    // Save a quote with one line: 10 × $99.99
    let quote = make_quote("Q-G001-001", vec![line("prod-a", 10, 9999)]);
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    // Load it back from the real DB
    let loaded = repo
        .find_by_id(&QuoteId("Q-G001-001".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found after save")?;

    // Run the deterministic pricing engine on the DB-loaded quote
    let result = price_quote_with_trace(&loaded, &loaded.currency);

    // Expected: 10 × $99.99 = $999.90
    assert_eq!(
        result.subtotal,
        Decimal::new(99990, 2),
        "subtotal mismatch: expected $999.90, got {}",
        result.subtotal
    );
    assert_eq!(result.discount_total, Decimal::ZERO);
    assert_eq!(result.tax_total, Decimal::ZERO);
    assert_eq!(result.total, result.subtotal);
    assert!(!result.approval_required);

    // Verify trace has the expected steps
    assert!(
        result.trace.steps.len() >= 4,
        "expected at least 4 trace steps, got {}",
        result.trace.steps.len()
    );
    assert_eq!(result.trace.quote_id, loaded.id);
    assert_eq!(result.trace.currency, "USD");

    Ok(())
}

#[tokio::test]
async fn g001_pricing_round_trip_multi_line() -> TestResult {
    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    // Save a quote with multiple lines:
    //   3 × $19.99 = $59.97
    //   1 × $50.00 = $50.00
    //   5 × $100.00 = $500.00
    let quote = make_quote(
        "Q-G001-002",
        vec![line("prod-b", 3, 1999), line("prod-c", 1, 5000), line("prod-d", 5, 10000)],
    );
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&QuoteId("Q-G001-002".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    assert_eq!(loaded.lines.len(), 3, "expected 3 lines after round-trip");

    let result = price_quote_with_trace(&loaded, "USD");

    // Expected total: $59.97 + $50.00 + $500.00 = $609.97
    let expected = Decimal::new(60997, 2);
    assert_eq!(result.total, expected, "total mismatch: expected {expected}, got {}", result.total);

    Ok(())
}

#[tokio::test]
async fn g001_pricing_engine_trait_matches_free_fn() -> TestResult {
    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    let quote = make_quote("Q-G001-003", vec![line("prod-e", 2, 7500)]);
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&QuoteId("Q-G001-003".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    // Both paths should produce identical results
    let from_fn = price_quote_with_trace(&loaded, "USD");
    let engine = DeterministicPricingEngine;
    let from_trait = engine.price(&loaded, "USD");

    assert_eq!(from_fn.total, from_trait.total);
    assert_eq!(from_fn.subtotal, from_trait.subtotal);

    Ok(())
}

#[tokio::test]
async fn g001_pricing_snapshot_persist_and_read_back() -> TestResult {
    use quotey_core::explanation::PricingSnapshotProvider;
    use quotey_db::repositories::pricing_snapshot::SqlPricingSnapshotRepository;

    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    // Create and save a quote: 4 × $25.00 = $100.00
    let quote = make_quote("Q-G001-SNAP", vec![line("prod-snap", 4, 2500)]);
    repo.save(quote.clone()).await.map_err(|e| format!("save quote: {e}"))?;

    // Load and price it
    let loaded = repo
        .find_by_id(&QuoteId("Q-G001-SNAP".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    let pricing = price_quote_with_trace(&loaded, "USD");

    // Build the PersistedPricingTrace JSON in the format the snapshot repo expects.
    // The calculation_steps use a different schema than PricingTrace.steps, so we
    // map them with the correct field names.
    let trace_json = serde_json::json!({
        "line_items": [{
            "line_id": "Q-G001-SNAP-ql-1",
            "product_id": "prod-snap",
            "product_name": "prod-snap",
            "quantity": 4,
            "unit_price": "25.00",
            "discount_percent": "0",
            "discount_amount": "0",
            "line_subtotal": "100.00"
        }],
        "calculation_steps": pricing.trace.steps.iter().enumerate().map(|(i, s)| {
            serde_json::json!({
                "step_order": i as i32 + 1,
                "step_name": s.stage,
                "input_values": {},
                "output_value": s.amount.to_string(),
                "formula": null
            })
        }).collect::<Vec<_>>()
    })
    .to_string();

    let ts = chrono::Utc::now().to_rfc3339();

    // Insert ledger entry (required for snapshot FK)
    sqlx::query(
        "INSERT INTO quote_ledger (entry_id, quote_id, version_number, content_hash, prev_hash, actor_id, action_type, timestamp, signature, metadata_json)
         VALUES ('led-snap-1', 'Q-G001-SNAP', 1, 'hash-snap-1', NULL, 'test', 'price', ?, 'sig', '{}')",
    )
    .bind(&ts)
    .execute(&pool)
    .await
    .map_err(|e| format!("insert ledger: {e}"))?;

    // Insert the pricing snapshot
    sqlx::query(
        "INSERT INTO quote_pricing_snapshot (id, quote_id, version, ledger_entry_id, ledger_content_hash, subtotal, discount_total, tax_total, total, currency, pricing_trace_json, priced_at, priced_by)
         VALUES ('snap-g001', 'Q-G001-SNAP', 1, 'led-snap-1', 'hash-snap-1', ?, ?, ?, ?, 'USD', ?, ?, 'test')",
    )
    .bind(pricing.subtotal.to_string())
    .bind(pricing.discount_total.to_string())
    .bind(pricing.tax_total.to_string())
    .bind(pricing.total.to_string())
    .bind(&trace_json)
    .bind(&ts)
    .execute(&pool)
    .await
    .map_err(|e| format!("insert snapshot: {e}"))?;

    // Read it back via the repository
    let snap_repo = SqlPricingSnapshotRepository::new(pool.clone());
    let snapshot = snap_repo
        .get_snapshot(&QuoteId("Q-G001-SNAP".to_string()), 1)
        .await
        .map_err(|e| format!("get snapshot: {e}"))?;

    assert_eq!(snapshot.quote_id, QuoteId("Q-G001-SNAP".to_string()));
    assert_eq!(snapshot.version, 1);
    // 4 × $25.00 = $100.00
    assert_eq!(snapshot.total, Decimal::new(10000, 2));
    assert_eq!(snapshot.line_items.len(), 1);
    assert_eq!(snapshot.line_items[0].product_id, "prod-snap");
    assert_eq!(snapshot.line_items[0].quantity, 4);

    Ok(())
}

// ── G-002: Constraint engine real-DB validation ─────────────────────────

#[tokio::test]
async fn g002_valid_quote_passes_constraints_after_round_trip() -> TestResult {
    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    let quote =
        make_quote("Q-G002-VALID", vec![line("prod-ok-1", 5, 2000), line("prod-ok-2", 1, 5000)]);
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&QuoteId("Q-G002-VALID".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    let engine = DeterministicConstraintEngine;
    let result = engine.validate(&ConstraintInput { quote_lines: loaded.lines.clone() });

    assert!(result.valid, "expected valid quote, got violations: {:?}", result.violations);
    assert!(result.violations.is_empty());

    Ok(())
}

#[tokio::test]
async fn g002_zero_quantity_detected_after_round_trip() -> TestResult {
    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    // Save a quote with zero-quantity line
    let quote = make_quote("Q-G002-ZERO", vec![line("prod-zero", 0, 1000)]);
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&QuoteId("Q-G002-ZERO".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    let engine = DeterministicConstraintEngine;
    let result = engine.validate(&ConstraintInput { quote_lines: loaded.lines.clone() });

    assert!(!result.valid);
    assert!(
        result.violations.iter().any(|v| v.code == "ZERO_QUANTITY"),
        "expected ZERO_QUANTITY violation, got: {:?}",
        result.violations
    );

    Ok(())
}

#[tokio::test]
async fn g002_duplicate_product_detected_after_round_trip() -> TestResult {
    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    // Save a quote with duplicate product IDs
    let quote =
        make_quote("Q-G002-DUP", vec![line("prod-dup", 2, 3000), line("prod-dup", 1, 3000)]);
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&QuoteId("Q-G002-DUP".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    let engine = DeterministicConstraintEngine;
    let result = engine.validate(&ConstraintInput { quote_lines: loaded.lines.clone() });

    assert!(!result.valid);
    assert!(
        result.violations.iter().any(|v| v.code == "DUPLICATE_PRODUCT_ID"),
        "expected DUPLICATE_PRODUCT_ID, got: {:?}",
        result.violations
    );

    Ok(())
}

#[tokio::test]
async fn g002_empty_quote_fails_constraints() -> TestResult {
    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    // Save a quote with zero lines
    let quote = make_quote("Q-G002-EMPTY", vec![]);
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&QuoteId("Q-G002-EMPTY".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    assert!(loaded.lines.is_empty(), "expected 0 lines after round-trip");

    let engine = DeterministicConstraintEngine;
    let result = engine.validate(&ConstraintInput { quote_lines: loaded.lines.clone() });

    assert!(!result.valid);
    assert!(
        result.violations.iter().any(|v| v.code == "EMPTY_QUOTE"),
        "expected EMPTY_QUOTE, got: {:?}",
        result.violations
    );

    Ok(())
}

#[tokio::test]
async fn g002_full_cpq_evaluation_round_trip() -> TestResult {
    use quotey_core::cpq::constraints::ConstraintEngine;
    use quotey_core::cpq::policy::{evaluate_policy_input, PolicyInput};

    let pool = setup_pool().await?;
    let repo = SqlQuoteRepository::new(pool.clone());

    // Quote: 10 × $99.99 = $999.90 with 25% discount request
    let quote = make_quote("Q-G002-CPQ", vec![line("prod-cpq", 10, 9999)]);
    repo.save(quote.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&QuoteId("Q-G002-CPQ".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("quote not found")?;

    // Step 1: Constraints
    let engine = DeterministicConstraintEngine;
    let constraints = engine.validate(&ConstraintInput { quote_lines: loaded.lines.clone() });
    assert!(constraints.valid, "quote should pass constraints");

    // Step 2: Pricing
    let pricing = price_quote_with_trace(&loaded, "USD");
    assert_eq!(pricing.total, Decimal::new(99990, 2));

    // Step 3: Policy with 25% discount (should trigger sales_manager approval)
    let policy = evaluate_policy_input(&PolicyInput {
        requested_discount_pct: Decimal::new(25, 0),
        deal_value: pricing.total,
        minimum_margin_pct: Decimal::new(75, 0),
    });

    assert!(policy.approval_required, "25% discount should require approval");
    assert!(
        policy.violations.iter().any(|v| v.policy_id == "discount-cap"),
        "expected discount-cap violation"
    );

    Ok(())
}

// ── G-004: Product FTS search real-DB test ──────────────────────────────

#[tokio::test]
async fn g004_product_save_and_search_via_fts() -> TestResult {
    use quotey_db::repositories::{ProductRepository, SqlProductRepository};

    let pool = setup_pool().await?;
    let repo = SqlProductRepository::new(pool.clone());

    // Save three products
    let mut p1 = Product::simple("PROD-FTS-1", "SKU-WIDGET-A", "Enterprise Widget Pro");
    p1.base_price = Some(Decimal::new(9999, 2));
    p1.description = Some("Advanced widget for enterprise deployments".to_string());
    repo.save(p1).await.map_err(|e| format!("save p1: {e}"))?;

    let mut p2 = Product::simple("PROD-FTS-2", "SKU-GADGET-B", "Basic Gadget Lite");
    p2.base_price = Some(Decimal::new(2500, 2));
    p2.description = Some("Lightweight gadget for small teams".to_string());
    repo.save(p2).await.map_err(|e| format!("save p2: {e}"))?;

    let mut p3 = Product::simple("PROD-FTS-3", "SKU-WIDGET-C", "Widget Connector Pack");
    p3.base_price = Some(Decimal::new(1500, 2));
    p3.description = Some("Connector bundle for widget integrations".to_string());
    repo.save(p3).await.map_err(|e| format!("save p3: {e}"))?;

    // Search for "widget" — should match p1 and p3
    let results =
        repo.search("widget", true, 10).await.map_err(|e| format!("search 'widget': {e}"))?;

    assert!(results.len() >= 2, "expected at least 2 results for 'widget', got {}", results.len());
    let ids: Vec<&str> = results.iter().map(|p| p.id.0.as_str()).collect();
    assert!(ids.contains(&"PROD-FTS-1"), "missing PROD-FTS-1 in {ids:?}");
    assert!(ids.contains(&"PROD-FTS-3"), "missing PROD-FTS-3 in {ids:?}");

    // Search for "gadget" — should match only p2
    let results =
        repo.search("gadget", true, 10).await.map_err(|e| format!("search 'gadget': {e}"))?;

    assert_eq!(results.len(), 1, "expected 1 result for 'gadget'");
    assert_eq!(results[0].id.0, "PROD-FTS-2");

    // Search by SKU prefix
    let results = repo
        .search("SKU-WIDGET", true, 10)
        .await
        .map_err(|e| format!("search 'SKU-WIDGET': {e}"))?;

    assert!(
        results.len() >= 2,
        "expected at least 2 results for SKU prefix, got {}",
        results.len()
    );

    Ok(())
}

#[tokio::test]
async fn g004_product_search_empty_query_lists_all() -> TestResult {
    use quotey_db::repositories::{ProductRepository, SqlProductRepository};

    let pool = setup_pool().await?;
    let repo = SqlProductRepository::new(pool.clone());

    // Save two products
    let mut p1 = Product::simple("PROD-LIST-1", "SKU-LIST-A", "Alpha Product");
    p1.base_price = Some(Decimal::new(1000, 2));
    repo.save(p1).await.map_err(|e| format!("save p1: {e}"))?;

    let mut p2 = Product::simple("PROD-LIST-2", "SKU-LIST-B", "Beta Product");
    p2.base_price = Some(Decimal::new(2000, 2));
    repo.save(p2).await.map_err(|e| format!("save p2: {e}"))?;

    // Empty query should list all active products
    let results = repo.search("", true, 10).await.map_err(|e| format!("search empty: {e}"))?;

    assert!(results.len() >= 2, "expected at least 2 products, got {}", results.len());

    Ok(())
}

#[tokio::test]
async fn g004_product_round_trip_preserves_fields() -> TestResult {
    use quotey_db::repositories::{ProductRepository, SqlProductRepository};

    let pool = setup_pool().await?;
    let repo = SqlProductRepository::new(pool.clone());

    let mut product = Product::simple("PROD-RT-1", "SKU-RT-001", "Round Trip Widget");
    product.base_price = Some(Decimal::new(4299, 2));
    product.description = Some("Test product for round-trip".to_string());

    repo.save(product.clone()).await.map_err(|e| format!("save: {e}"))?;

    let loaded = repo
        .find_by_id(&ProductId("PROD-RT-1".to_string()))
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("product not found")?;

    assert_eq!(loaded.id.0, "PROD-RT-1");
    assert_eq!(loaded.sku, "SKU-RT-001");
    assert_eq!(loaded.name, "Round Trip Widget");
    assert_eq!(loaded.base_price, Some(Decimal::new(4299, 2)));
    assert_eq!(loaded.description.as_deref(), Some("Test product for round-trip"));
    assert!(loaded.active);

    Ok(())
}

// ── G-003: Audit event real-DB persistence ──────────────────────────────

#[tokio::test]
async fn g003_audit_event_save_and_read_back() -> TestResult {
    use quotey_core::audit::{AuditCategory, AuditEvent, AuditOutcome};
    use quotey_db::repositories::SqlAuditEventRepository;

    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    // Insert a quote first (FK constraint on audit_event.quote_id)
    let quote = make_quote("Q-G003-001", vec![line("prod-audit", 1, 1000)]);
    quote_repo.save(quote).await.map_err(|e| format!("save quote: {e}"))?;

    // Emit and persist an audit event
    let event = AuditEvent::new(
        Some(QuoteId("Q-G003-001".to_string())),
        Some("1730000000.0100".to_string()),
        "req-g003-1",
        "flow.transition_applied",
        AuditCategory::Flow,
        "flow-engine",
        AuditOutcome::Success,
    )
    .with_metadata("from", "Draft")
    .with_metadata("to", "Validated");

    audit_repo.save(&event).await.map_err(|e| format!("save audit: {e}"))?;

    // Read it back
    let events = audit_repo
        .find_by_quote_id(&QuoteId("Q-G003-001".to_string()))
        .await
        .map_err(|e| format!("find audit: {e}"))?;

    assert_eq!(events.len(), 1, "expected 1 audit event");
    assert_eq!(events[0].event_id, event.event_id);
    assert_eq!(events[0].event_type, "flow.transition_applied");
    assert_eq!(events[0].correlation_id, "req-g003-1");
    assert_eq!(events[0].thread_id.as_deref(), Some("1730000000.0100"));
    assert_eq!(events[0].actor, "flow-engine");
    assert_eq!(events[0].category, AuditCategory::Flow);
    assert_eq!(events[0].outcome, AuditOutcome::Success);
    assert_eq!(events[0].metadata.get("from").map(|s| s.as_str()), Some("Draft"));
    assert_eq!(events[0].metadata.get("to").map(|s| s.as_str()), Some("Validated"));

    Ok(())
}

#[tokio::test]
async fn g003_audit_multiple_events_for_quote() -> TestResult {
    use quotey_core::audit::{AuditCategory, AuditEvent, AuditOutcome};
    use quotey_db::repositories::SqlAuditEventRepository;

    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    let quote = make_quote("Q-G003-002", vec![line("prod-audit2", 2, 5000)]);
    quote_repo.save(quote).await.map_err(|e| format!("save quote: {e}"))?;

    // Emit several events in sequence — simulating a flow lifecycle
    let e1 = AuditEvent::new(
        Some(QuoteId("Q-G003-002".to_string())),
        None,
        "req-g003-2a",
        "flow.transition_applied",
        AuditCategory::Flow,
        "flow-engine",
        AuditOutcome::Success,
    );
    let e2 = AuditEvent::new(
        Some(QuoteId("Q-G003-002".to_string())),
        None,
        "req-g003-2b",
        "pricing.calculated",
        AuditCategory::Pricing,
        "pricing-engine",
        AuditOutcome::Success,
    )
    .with_metadata("total", "100.00");
    let e3 = AuditEvent::new(
        Some(QuoteId("Q-G003-002".to_string())),
        None,
        "req-g003-2c",
        "policy.evaluated",
        AuditCategory::Policy,
        "policy-engine",
        AuditOutcome::Rejected,
    )
    .with_metadata("reason", "discount exceeds cap");

    audit_repo.save(&e1).await.map_err(|e| format!("save e1: {e}"))?;
    audit_repo.save(&e2).await.map_err(|e| format!("save e2: {e}"))?;
    audit_repo.save(&e3).await.map_err(|e| format!("save e3: {e}"))?;

    let events = audit_repo
        .find_by_quote_id(&QuoteId("Q-G003-002".to_string()))
        .await
        .map_err(|e| format!("find audits: {e}"))?;

    assert_eq!(events.len(), 3, "expected 3 audit events");

    // Verify ordering by timestamp
    assert_eq!(events[0].event_type, "flow.transition_applied");
    assert_eq!(events[1].event_type, "pricing.calculated");
    assert_eq!(events[2].event_type, "policy.evaluated");

    // Verify rejected outcome survives round-trip
    assert_eq!(events[2].outcome, AuditOutcome::Rejected);
    assert_eq!(events[2].metadata.get("reason").map(|s| s.as_str()), Some("discount exceeds cap"));

    // Count should match
    let count = audit_repo
        .count_by_quote_id(&QuoteId("Q-G003-002".to_string()))
        .await
        .map_err(|e| format!("count: {e}"))?;
    assert_eq!(count, 3);

    Ok(())
}

#[tokio::test]
async fn g003_audit_find_by_event_type() -> TestResult {
    use quotey_core::audit::{AuditCategory, AuditEvent, AuditOutcome};
    use quotey_db::repositories::SqlAuditEventRepository;

    let pool = setup_pool().await?;
    let quote_repo = SqlQuoteRepository::new(pool.clone());
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    let quote = make_quote("Q-G003-003", vec![line("prod-audit3", 1, 3000)]);
    quote_repo.save(quote).await.map_err(|e| format!("save quote: {e}"))?;

    // Two flow events, one pricing event
    let e1 = AuditEvent::new(
        Some(QuoteId("Q-G003-003".to_string())),
        None,
        "req-a",
        "flow.transition_applied",
        AuditCategory::Flow,
        "flow-engine",
        AuditOutcome::Success,
    );
    let e2 = AuditEvent::new(
        Some(QuoteId("Q-G003-003".to_string())),
        None,
        "req-b",
        "pricing.calculated",
        AuditCategory::Pricing,
        "pricing-engine",
        AuditOutcome::Success,
    );

    audit_repo.save(&e1).await.map_err(|e| format!("save: {e}"))?;
    audit_repo.save(&e2).await.map_err(|e| format!("save: {e}"))?;

    // Query by type
    let flow_events = audit_repo
        .find_by_type("flow.transition_applied")
        .await
        .map_err(|e| format!("find by type: {e}"))?;

    assert!(flow_events.iter().any(|e| e.event_id == e1.event_id), "expected to find flow event");
    assert!(
        !flow_events.iter().any(|e| e.event_id == e2.event_id),
        "pricing event should not appear in flow query"
    );

    Ok(())
}

#[tokio::test]
async fn g003_audit_event_without_quote_id() -> TestResult {
    use quotey_core::audit::{AuditCategory, AuditEvent, AuditOutcome};
    use quotey_db::repositories::SqlAuditEventRepository;

    let pool = setup_pool().await?;
    let audit_repo = SqlAuditEventRepository::new(pool.clone());

    // System-level audit event with no quote association
    let event = AuditEvent::new(
        None,
        None,
        "req-system-1",
        "system.startup",
        AuditCategory::System,
        "server",
        AuditOutcome::Success,
    )
    .with_metadata("version", "0.1.0");

    audit_repo.save(&event).await.map_err(|e| format!("save: {e}"))?;

    // Verify we can find it by type
    let events =
        audit_repo.find_by_type("system.startup").await.map_err(|e| format!("find: {e}"))?;

    assert_eq!(events.len(), 1);
    assert!(events[0].quote_id.is_none());
    assert_eq!(events[0].category, AuditCategory::System);
    assert_eq!(events[0].metadata.get("version").map(|s| s.as_str()), Some("0.1.0"));

    Ok(())
}
