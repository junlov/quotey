# ADR-008: Testing Architecture

**Status:** Accepted  
**Date:** 2026-02-23  
**Author:** Codex Agent  
**Related:** bd-256v.8 (Research Task)

## Context

Quotey is an async Rust application using tokio, sqlx, and Slack APIs. We need a testing strategy that:

1. Works well with async/await code
2. Supports database testing without external dependencies
3. Allows mocking external services (Slack, CRM, LLM)
4. Runs fast locally and in CI
5. Provides good developer experience

We evaluated multiple testing approaches and tools.

## Decision

We will use a **layered testing approach** with the following stack:

### Core Testing Stack

| Layer | Tool | Purpose |
|-------|------|---------|
| Test runtime | `tokio::test` | Async test execution |
| CI runner | `cargo-nextest` | Parallel test execution |
| Database | SQLite in-memory | Fast, isolated DB tests |
| HTTP mocking | `wiremock` | External API mocking |
| Trait mocking | `mockall` | Internal dependency mocking |
| Fixtures | `rstest` | Parameterized tests |

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Test Layers                            │
├─────────────────────────────────────────────────────────────┤
│  Unit Tests (70%)                                           │
│  ├── Inline with source (`#[cfg(test)]`)                    │
│  ├── Mock external deps with mockall                        │
│  └── Pure business logic, no I/O                            │
├─────────────────────────────────────────────────────────────┤
│  Integration Tests (25%)                                    │
│  ├── `tests/` directory                                     │
│  ├── SQLite in-memory with sqlx::test                       │
│  ├── Wiremock for HTTP services                             │
│  └── Repository + Service layer testing                     │
├─────────────────────────────────────────────────────────────┤
│  E2E Tests (5%)                                             │
│  ├── Full application bootstrap                             │
│  ├── Testcontainers if needed                               │
│  └── Critical user flows only                               │
└─────────────────────────────────────────────────────────────┘
```

## Consequences

### Positive

1. **Fast feedback loop** - SQLite in-memory tests run in milliseconds
2. **No external dependencies** - Tests work offline and in CI without Docker
3. **Real SQL validation** - Tests actual queries with sqlx compile-time checking
4. **Parallel execution** - nextest runs tests in parallel for speed
5. **Good isolation** - Each test gets fresh database, no shared state

### Negative

1. **SQLite ≠ PostgreSQL** - Minor SQL dialect differences if we migrate later
2. **Mock maintenance** - Mock expectations need updating when APIs change
3. **Test data setup** - Need fixtures/migrations for consistent test data

### Mitigations

- Use standard SQL that works on both SQLite and PostgreSQL
- Integration tests catch real DB issues before production
- Automated test data factories reduce boilerplate

## Alternatives Considered

### Alternative 1: PostgreSQL in Testcontainers

**Why rejected:**
- Slower test startup (Docker overhead)
- Requires Docker locally and in CI
- More complex test setup/teardown

### Alternative 2: Mock All Database Calls

**Why rejected:**
- Doesn't test actual SQL queries
- Mock setup is verbose and fragile
- Misses sqlx compile-time query validation

### Alternative 3: Use mockito instead of wiremock

**Why rejected:**
- mockito runs tests sequentially (slower)
- wiremock has better async support
- wiremock allows multiple mock servers per test

## Implementation Guidelines

### 1. Async Test Pattern

```rust
#[tokio::test]
async fn test_name() {
    // Test body
}

// Multi-threaded only when needed
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_test() {
    // Test concurrent operations
}
```

### 2. Database Test Pattern

```rust
#[sqlx::test(migrations = "./migrations")]
async fn test_with_db(pool: SqlitePool) {
    // Pool is pre-configured, migrations applied
    let repo = SqlQuoteRepository::new(pool);
    let result = repo.find_by_id("test").await;
    assert!(result.is_ok());
}
```

### 3. HTTP Mocking Pattern

```rust
#[tokio::test]
async fn test_slack_api() {
    let mock_server = MockServer::start().await;
    
    Mock::given(method("POST"))
        .and(path("/api/chat.postMessage"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;
    
    let client = SlackClient::new(&mock_server.uri());
    let result = client.post_message("#test", "Hello").await;
    
    assert!(result.is_ok());
}
```

### 4. Repository Pattern

```rust
// Define trait
#[async_trait]
pub trait QuoteRepository: Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<Quote>, Error>;
}

// Real implementation for production
pub struct SqlQuoteRepository { ... }

// Mock implementation for unit tests
#[automock]
#[async_trait]
pub trait QuoteRepository { ... }
```

## CI/CD Configuration

### Required Tools

```bash
# Install nextest
cargo install cargo-nextest --locked

# Install coverage tool (optional)
cargo install cargo-llvm-cov
```

### Test Command

```bash
# Local development
cargo test

# CI (parallel, with retries)
cargo nextest run --retries 2

# With coverage
cargo llvm-cov --lcov --output-path lcov.info
```

## References

- Research Report: `.planning/research/RCH-08-Async-Testing-Strategies.md`
- Bead: `bd-256v.8`
- Tokio Testing: https://tokio.rs/tokio/topics/testing
- Nextest: https://nexte.st/
