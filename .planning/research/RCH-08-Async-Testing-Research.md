# RCH-08: Async Rust Testing Strategies Research

**Research Task:** bd-256v.8  
**Status:** Complete  
**Date:** 2026-02-23

---

## Executive Summary

Async Rust testing recommendations for Quotey:

- **Unit tests:** Use `tokio::test` with mockall for mocking
- **Integration tests:** Use `sqlx::test` with in-memory SQLite
- **HTTP testing:** Use wiremock for external APIs
- **Test runner:** cargo-nextest for speed

---

## 1. Testing Stack

| Layer | Tool | Purpose |
|-------|------|---------|
| Async runtime | `tokio::test` | Async test execution |
| Mocking | `mockall` | Trait mocking |
| HTTP mocking | `wiremock` | External API simulation |
| DB testing | `sqlx::test` | Test transactions |
| Fixtures | `fake` / factories | Test data generation |

---

## 2. Patterns

### 2.1 Mocking Repository

```rust
#[mockall::automock]
#[async_trait]
trait QuoteRepository {
    async fn find_by_id(&self, id: &QuoteId) -> Result<Option<Quote>>;
}

#[tokio::test]
async fn test_pricing_service() {
    let mut mock_repo = MockQuoteRepository::new();
    mock_repo.expect_find_by_id()
        .returning(|_| Ok(Some(test_quote())));
    
    let service = PricingService::new(mock_repo);
    let result = service.price_quote(&id).await;
    assert!(result.is_ok());
}
```

### 2.2 Database Testing

```rust
#[sqlx::test]
async fn test_quote_creation(pool: SqlitePool) {
    let repo = QuoteRepository::new(pool);
    let quote = repo.create(&test_quote()).await.unwrap();
    assert_eq!(quote.status, QuoteStatus::Draft);
}
```

---

## 3. Test Organization

```
tests/
├── unit/              # Fast, isolated tests
├── integration/       # Database + API tests
└── e2e/               # Full workflow tests
```

---

## 4. Recommendations

1. Use `mockall` for all trait mocking
2. Use in-memory SQLite for integration tests
3. Use `wiremock` for Slack API simulation
4. Target 80%+ code coverage
5. Run tests in CI with nextest
