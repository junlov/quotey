/// Integration tests for NXT negotiation session/turn persistence (bd-3acq.2.2).
///
/// Exercises the SqlNegotiationRepository against real SQLite to verify:
///   N-001  Session create + find round-trip
///   N-002  Turn create + find-by-session round-trip
///   N-003  State transition validation (valid + invalid)
///   N-004  Idempotency constraint prevents duplicate sessions
///   N-005  Active session finder excludes terminal states
///   N-006  Turn count tracks correctly
use quotey_core::domain::negotiation::{
    NegotiationSession, NegotiationSessionId, NegotiationState, NegotiationTurn, NegotiationTurnId,
    TurnOutcome, TurnRequestType,
};
use quotey_core::domain::quote::{Quote, QuoteId, QuoteLine, QuoteStatus};
use quotey_db::repositories::negotiation::SqlNegotiationRepository;
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

async fn seed_quote(pool: &quotey_db::DbPool, id: &str) -> TestResult {
    let now = chrono::Utc::now();
    let quote = Quote {
        id: QuoteId(id.to_string()),
        version: 1,
        status: QuoteStatus::Draft,
        account_id: Some("acct-nxt".to_string()),
        deal_id: None,
        currency: "USD".to_string(),
        term_months: None,
        start_date: None,
        end_date: None,
        valid_until: None,
        notes: None,
        created_by: "nxt-test".to_string(),
        lines: vec![QuoteLine {
            product_id: quotey_core::domain::product::ProductId("plan-pro".to_string()),
            quantity: 5,
            unit_price: Decimal::new(10000, 2),
            discount_pct: 0.0,
            notes: None,
        }],
        created_at: now,
        updated_at: now,
    };
    let repo = SqlQuoteRepository::new(pool.clone());
    repo.save(quote).await.map_err(|e| format!("seed quote: {e}"))
}

fn make_session(id: &str, quote_id: &str) -> NegotiationSession {
    NegotiationSession {
        id: NegotiationSessionId(id.to_string()),
        quote_id: quote_id.to_string(),
        actor_id: "rep-alice".to_string(),
        state: NegotiationState::Draft,
        policy_version: "policy-v1".to_string(),
        pricing_version: "pricing-v1".to_string(),
        idempotency_key: format!("{quote_id}-key-1"),
        max_turns: 20,
        expires_at: None,
        created_at: "2026-03-06T00:00:00.000Z".to_string(),
        updated_at: "2026-03-06T00:00:00.000Z".to_string(),
    }
}

fn make_turn(id: &str, session_id: &str, turn_number: u32) -> NegotiationTurn {
    NegotiationTurn {
        id: NegotiationTurnId(id.to_string()),
        session_id: NegotiationSessionId(session_id.to_string()),
        turn_number,
        request_type: TurnRequestType::Counter,
        request_payload: r#"{"discount_pct":15}"#.to_string(),
        envelope_json: Some(r#"{"ranges":[]}"#.to_string()),
        plan_json: None,
        chosen_offer_id: None,
        outcome: TurnOutcome::Offered,
        boundary_json: None,
        transition_key: format!("txn-{session_id}-{turn_number}"),
        created_at: "2026-03-06T00:01:00.000Z".to_string(),
    }
}

// ── N-001: Session create + find round-trip ───────────────────────────

#[tokio::test]
async fn n001_session_round_trip() -> TestResult {
    let pool = setup_pool().await?;
    seed_quote(&pool, "Q-N001").await?;

    let session = make_session("NXT-001", "Q-N001");
    SqlNegotiationRepository::save_session(&pool, &session)
        .await
        .map_err(|e| format!("save: {e}"))?;

    let loaded = SqlNegotiationRepository::find_session_by_id(&pool, "NXT-001")
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("session not found")?;

    assert_eq!(loaded.id.0, "NXT-001");
    assert_eq!(loaded.quote_id, "Q-N001");
    assert_eq!(loaded.actor_id, "rep-alice");
    assert_eq!(loaded.state, NegotiationState::Draft);
    assert_eq!(loaded.policy_version, "policy-v1");
    assert_eq!(loaded.max_turns, 20);
    Ok(())
}

// ── N-002: Turn create + find round-trip ──────────────────────────────

#[tokio::test]
async fn n002_turn_round_trip() -> TestResult {
    let pool = setup_pool().await?;
    seed_quote(&pool, "Q-N002").await?;

    let session = make_session("NXT-002", "Q-N002");
    SqlNegotiationRepository::save_session(&pool, &session)
        .await
        .map_err(|e| format!("save session: {e}"))?;

    let turn = make_turn("T-002-1", "NXT-002", 1);
    SqlNegotiationRepository::save_turn(&pool, &turn)
        .await
        .map_err(|e| format!("save turn: {e}"))?;

    let turns = SqlNegotiationRepository::find_turns_by_session(&pool, "NXT-002")
        .await
        .map_err(|e| format!("find turns: {e}"))?;

    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].id.0, "T-002-1");
    assert_eq!(turns[0].turn_number, 1);
    assert_eq!(turns[0].request_type, TurnRequestType::Counter);
    assert_eq!(turns[0].outcome, TurnOutcome::Offered);
    assert!(turns[0].envelope_json.is_some());
    Ok(())
}

// ── N-003: State transition validation ────────────────────────────────

#[tokio::test]
async fn n003_valid_state_transition() -> TestResult {
    let pool = setup_pool().await?;
    seed_quote(&pool, "Q-N003").await?;

    let session = make_session("NXT-003", "Q-N003");
    SqlNegotiationRepository::save_session(&pool, &session)
        .await
        .map_err(|e| format!("save: {e}"))?;

    // draft -> active: valid
    SqlNegotiationRepository::advance_session_state(&pool, "NXT-003", NegotiationState::Active)
        .await
        .map_err(|e| format!("advance to active: {e}"))?;

    let loaded = SqlNegotiationRepository::find_session_by_id(&pool, "NXT-003")
        .await
        .map_err(|e| format!("find: {e}"))?
        .ok_or("not found")?;
    assert_eq!(loaded.state, NegotiationState::Active);

    Ok(())
}

#[tokio::test]
async fn n003_invalid_state_transition_rejected() -> TestResult {
    let pool = setup_pool().await?;
    seed_quote(&pool, "Q-N003b").await?;

    let session = make_session("NXT-003b", "Q-N003b");
    SqlNegotiationRepository::save_session(&pool, &session)
        .await
        .map_err(|e| format!("save: {e}"))?;

    // draft -> approved: invalid
    let result = SqlNegotiationRepository::advance_session_state(
        &pool,
        "NXT-003b",
        NegotiationState::Approved,
    )
    .await;
    assert!(result.is_err(), "draft->approved should be rejected");
    Ok(())
}

// ── N-004: Idempotency constraint ────────────────────────────────────

#[tokio::test]
async fn n004_duplicate_session_idempotency_key_rejected() -> TestResult {
    let pool = setup_pool().await?;
    seed_quote(&pool, "Q-N004").await?;

    let session1 = make_session("NXT-004a", "Q-N004");
    SqlNegotiationRepository::save_session(&pool, &session1)
        .await
        .map_err(|e| format!("save first: {e}"))?;

    // Same quote_id + actor_id + idempotency_key → unique constraint violation
    let mut session2 = make_session("NXT-004b", "Q-N004");
    session2.idempotency_key = session1.idempotency_key.clone();
    let result = SqlNegotiationRepository::save_session(&pool, &session2).await;
    assert!(result.is_err(), "duplicate idempotency key should fail");
    Ok(())
}

// ── N-005: Active session finder excludes terminal states ─────────────

#[tokio::test]
async fn n005_active_session_excludes_terminal() -> TestResult {
    let pool = setup_pool().await?;
    seed_quote(&pool, "Q-N005").await?;

    let session = make_session("NXT-005", "Q-N005");
    SqlNegotiationRepository::save_session(&pool, &session)
        .await
        .map_err(|e| format!("save: {e}"))?;

    // While draft, it should be findable as active
    let active = SqlNegotiationRepository::find_active_session_for_quote(&pool, "Q-N005")
        .await
        .map_err(|e| format!("find active: {e}"))?;
    assert!(active.is_some());

    // Advance to active then cancel
    SqlNegotiationRepository::advance_session_state(&pool, "NXT-005", NegotiationState::Active)
        .await
        .map_err(|e| format!("to active: {e}"))?;
    SqlNegotiationRepository::advance_session_state(&pool, "NXT-005", NegotiationState::Cancelled)
        .await
        .map_err(|e| format!("to cancelled: {e}"))?;

    let active = SqlNegotiationRepository::find_active_session_for_quote(&pool, "Q-N005")
        .await
        .map_err(|e| format!("find active after cancel: {e}"))?;
    assert!(active.is_none(), "cancelled session should not be returned as active");
    Ok(())
}

// ── N-006: Turn count ─────────────────────────────────────────────────

#[tokio::test]
async fn n006_turn_count_tracks_correctly() -> TestResult {
    let pool = setup_pool().await?;
    seed_quote(&pool, "Q-N006").await?;

    let session = make_session("NXT-006", "Q-N006");
    SqlNegotiationRepository::save_session(&pool, &session)
        .await
        .map_err(|e| format!("save session: {e}"))?;

    let count_0 = SqlNegotiationRepository::count_turns(&pool, "NXT-006")
        .await
        .map_err(|e| format!("count: {e}"))?;
    assert_eq!(count_0, 0);

    for i in 1..=3 {
        let turn = make_turn(&format!("T-006-{i}"), "NXT-006", i);
        SqlNegotiationRepository::save_turn(&pool, &turn)
            .await
            .map_err(|e| format!("save turn {i}: {e}"))?;
    }

    let count_3 = SqlNegotiationRepository::count_turns(&pool, "NXT-006")
        .await
        .map_err(|e| format!("count after 3: {e}"))?;
    assert_eq!(count_3, 3);
    Ok(())
}
