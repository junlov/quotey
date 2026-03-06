use sqlx::{Row, SqlitePool};

use quotey_core::domain::negotiation::{
    NegotiationSession, NegotiationSessionId, NegotiationState, NegotiationTurn, NegotiationTurnId,
    TurnOutcome, TurnRequestType,
};

use super::RepositoryError;

pub struct SqlNegotiationRepository;

impl SqlNegotiationRepository {
    // -----------------------------------------------------------------------
    // Session CRUD
    // -----------------------------------------------------------------------

    pub async fn save_session(
        pool: &SqlitePool,
        session: &NegotiationSession,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO negotiation_session
                (id, quote_id, actor_id, state, policy_version, pricing_version,
                 idempotency_key, max_turns, expires_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&session.id.0)
        .bind(&session.quote_id)
        .bind(&session.actor_id)
        .bind(session.state.as_str())
        .bind(&session.policy_version)
        .bind(&session.pricing_version)
        .bind(&session.idempotency_key)
        .bind(session.max_turns)
        .bind(&session.expires_at)
        .bind(&session.created_at)
        .bind(&session.updated_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_session_by_id(
        pool: &SqlitePool,
        id: &str,
    ) -> Result<Option<NegotiationSession>, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, quote_id, actor_id, state, policy_version, pricing_version,
                    idempotency_key, max_turns, expires_at, created_at, updated_at
             FROM negotiation_session WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.and_then(|r| row_to_session(&r)))
    }

    pub async fn find_sessions_by_quote(
        pool: &SqlitePool,
        quote_id: &str,
    ) -> Result<Vec<NegotiationSession>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, quote_id, actor_id, state, policy_version, pricing_version,
                    idempotency_key, max_turns, expires_at, created_at, updated_at
             FROM negotiation_session WHERE quote_id = ? ORDER BY created_at DESC",
        )
        .bind(quote_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.iter().filter_map(row_to_session).collect())
    }

    /// Advance session state. Returns error if the transition is invalid per lifecycle contract.
    pub async fn advance_session_state(
        pool: &SqlitePool,
        session_id: &str,
        new_state: NegotiationState,
    ) -> Result<(), RepositoryError> {
        let current = Self::find_session_by_id(pool, session_id).await?;
        let current =
            current.ok_or_else(|| RepositoryError::Decode("session not found".to_string()))?;

        if !current.state.can_transition_to(&new_state) {
            return Err(RepositoryError::Decode(format!(
                "invalid transition from {} to {}",
                current.state.as_str(),
                new_state.as_str()
            )));
        }

        sqlx::query(
            "UPDATE negotiation_session SET state = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(new_state.as_str())
        .bind(session_id)
        .execute(pool)
        .await?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Turn CRUD
    // -----------------------------------------------------------------------

    pub async fn save_turn(
        pool: &SqlitePool,
        turn: &NegotiationTurn,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "INSERT INTO negotiation_turn
                (id, session_id, turn_number, request_type, request_payload,
                 envelope_json, plan_json, chosen_offer_id, outcome,
                 boundary_json, transition_key, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&turn.id.0)
        .bind(&turn.session_id.0)
        .bind(turn.turn_number)
        .bind(turn.request_type.as_str())
        .bind(&turn.request_payload)
        .bind(&turn.envelope_json)
        .bind(&turn.plan_json)
        .bind(&turn.chosen_offer_id)
        .bind(turn.outcome.as_str())
        .bind(&turn.boundary_json)
        .bind(&turn.transition_key)
        .bind(&turn.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_turns_by_session(
        pool: &SqlitePool,
        session_id: &str,
    ) -> Result<Vec<NegotiationTurn>, RepositoryError> {
        let rows = sqlx::query(
            "SELECT id, session_id, turn_number, request_type, request_payload,
                    envelope_json, plan_json, chosen_offer_id, outcome,
                    boundary_json, transition_key, created_at
             FROM negotiation_turn WHERE session_id = ? ORDER BY turn_number ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.iter().filter_map(row_to_turn).collect())
    }

    /// Count turns for a session (used for max-turn enforcement).
    pub async fn count_turns(pool: &SqlitePool, session_id: &str) -> Result<i64, RepositoryError> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM negotiation_turn WHERE session_id = ?")
                .bind(session_id)
                .fetch_one(pool)
                .await?;
        Ok(count)
    }

    /// Find active (non-terminal) sessions for a quote. Useful for preventing duplicate sessions.
    pub async fn find_active_session_for_quote(
        pool: &SqlitePool,
        quote_id: &str,
    ) -> Result<Option<NegotiationSession>, RepositoryError> {
        let row = sqlx::query(
            "SELECT id, quote_id, actor_id, state, policy_version, pricing_version,
                    idempotency_key, max_turns, expires_at, created_at, updated_at
             FROM negotiation_session
             WHERE quote_id = ? AND state NOT IN ('accepted','rejected','expired','cancelled')
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(quote_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.and_then(|r| row_to_session(&r)))
    }
}

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

fn row_to_session(row: &sqlx::sqlite::SqliteRow) -> Option<NegotiationSession> {
    Some(NegotiationSession {
        id: NegotiationSessionId(row.try_get("id").ok()?),
        quote_id: row.try_get("quote_id").ok()?,
        actor_id: row.try_get("actor_id").ok()?,
        state: NegotiationState::parse_label(&row.try_get::<String, _>("state").ok()?)?,
        policy_version: row.try_get("policy_version").ok()?,
        pricing_version: row.try_get("pricing_version").ok()?,
        idempotency_key: row.try_get("idempotency_key").ok()?,
        max_turns: row.try_get::<u32, _>("max_turns").ok()?,
        expires_at: row.try_get("expires_at").ok()?,
        created_at: row.try_get("created_at").ok()?,
        updated_at: row.try_get("updated_at").ok()?,
    })
}

fn row_to_turn(row: &sqlx::sqlite::SqliteRow) -> Option<NegotiationTurn> {
    Some(NegotiationTurn {
        id: NegotiationTurnId(row.try_get("id").ok()?),
        session_id: NegotiationSessionId(row.try_get("session_id").ok()?),
        turn_number: row.try_get::<u32, _>("turn_number").ok()?,
        request_type: TurnRequestType::parse_label(
            &row.try_get::<String, _>("request_type").ok()?,
        )?,
        request_payload: row.try_get("request_payload").ok()?,
        envelope_json: row.try_get("envelope_json").ok()?,
        plan_json: row.try_get("plan_json").ok()?,
        chosen_offer_id: row.try_get("chosen_offer_id").ok()?,
        outcome: TurnOutcome::parse_label(&row.try_get::<String, _>("outcome").ok()?)?,
        boundary_json: row.try_get("boundary_json").ok()?,
        transition_key: row.try_get("transition_key").ok()?,
        created_at: row.try_get("created_at").ok()?,
    })
}
