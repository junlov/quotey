# RCH-08: Async Rust Testing Strategies Research

**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** Codex Agent  
**Bead:** bd-256v.8

---

## Executive Summary

This research establishes testing patterns for Quotey's async Rust codebase. The key insight is that **testing async Rust requires careful attention to runtime selection, test isolation, and mocking strategies**. We recommend:

1. **tokio::test** as the primary test runner
2. **cargo-nextest** for CI/CD parallel execution
3. **Repository Pattern + Traits** for testable database code
4. **wiremock** for HTTP mocking
5. **sqlx::test** macro for database integration tests

---

## 1. Async Test Runtime Options

### 1.1 tokio::test (Recommended)

The standard approach for Tokio-based applications:

```rust
#[tokio::test]
async fn my_test() {
    let result = async_function().await;
    assert_eq!(result, expected);
}
```

**Flavors:**
- `#[tokio::test]` - Single-threaded (default)
- `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` - Multi-threaded

**Recommendation for Quotey:**
- Use single-threaded for most tests (deterministic, faster)
- Use multi-threaded only when testing concurrent behavior

### 1.2 Multi-threaded Considerations

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_test() {
    // Test concurrent operations
    let (a, b) = tokio::join!(
        async_operation_1(),
        async_operation_2(),
    );
    assert!(a.is_ok() && b.is_ok());
}
```

**Trade-offs:**
- Single-threaded: Deterministic, no race conditions, faster for simple tests
- Multi-threaded: Tests real concurrency, catches race conditions, slower

### 1.3 Time Control with tokio::time

For deterministic time-based tests:

```rust
#[tokio::test]
async fn test_with_time() {
    tokio::time::pause();
    
    let start = tokio::time::Instant::now();
    
    // Spawn a task that sleeps
    let handle = tokio::spawn(async {
        tokio::time::sleep(Duration::from_secs(5)).await;
        "completed"
    });
    
    // Advance time without waiting
    tokio::time::advance(Duration::from_secs(5)).await;
    
    let result = handle.await.unwrap();
    assert_eq!(result, "completed");
    
    // Verify actual elapsed time is near zero
    assert!(start.elapsed() < Duration::from_millis(10));
}
```

---

## 2. Test Runners: cargo test vs nextest

### 2.1 cargo test (Default)

```bash
# Run all tests
cargo test

# Run specific test
cargo test my_test_name

# Run with output
cargo test -- --nocapture

# Control parallelism
cargo test -- --test-threads=4
```

**Limitations:**
- Test binaries run serially
- Limited output formatting
- No test retries
- Harder to debug flaky tests

### 2.2 cargo-nextest (Recommended for CI/CD)

Nextest is a "next-generation test runner" that provides:

| Feature | cargo test | nextest |
|---------|-----------|---------|
| Parallel execution | Binary-level | Test-level |
| Output format | Limited | Rich, structured |
| Test retries | ❌ | ✅ |
| Flaky test detection | ❌ | ✅ |
| JUnit XML | ❌ | ✅ |
| Test filtering | Basic | Advanced |

**Installation:**
```bash
cargo install cargo-nextest
```

**Usage:**
```bash
# Run all tests
nextest run

# Run with retries
nextest run --retries 3

# Generate JUnit report
nextest run --format junit --output-file results.xml

# Partition tests (for CI sharding)
nextest run --partition count:1/3
```

**Why nextest for Quotey:**
1. **Faster CI**: Tests run in parallel at test level, not binary level
2. **Reliability**: Automatic retries for flaky tests
3. **Debugging**: Better output and failure isolation
4. **Reporting**: JUnit output for CI integration

**Architecture insight:** Nextest uses Tokio internally for its runner loop, making it a good fit for Tokio-based projects like Quotey.

---

## 3. Database Testing Strategies

### 3.1 SQLite In-Memory (Primary)

For Quotey's SQLite-based architecture, in-memory databases are ideal:

```rust
#[sqlx::test]
async fn test_with_db(pool: SqlitePool) {
    // Pool is automatically created with in-memory database
    let result = sqlx::query!("SELECT 1 as val")
        .fetch_one(&pool)
        .await
        .unwrap();
    
    assert_eq!(result.val, Some(1));
}
```

**Configuration:**
```rust
// test_helpers.rs
pub async fn test_db() -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect(":memory:")
        .await
        .expect("Failed to create test database")
}
```

**Migration handling:**
```rust
#[sqlx::test(migrations = "./migrations")]
async fn test_with_migrations(pool: SqlitePool) {
    // Migrations run automatically before test
    let result = my_query(&pool).await.unwrap();
    assert!(result.is_some());
}
```

### 3.2 Connection Pool Considerations

**Important:** SQLite in-memory databases are per-connection by default. For sharing:

```rust
// Use shared in-memory database
let pool = SqlitePoolOptions::new()
    .max_connections(5)
    .connect("file::memory:?cache=shared")
    .await
    .unwrap();
```

**Recommendation:** Use `sqlx::test` macro which handles pool management correctly.

### 3.3 Repository Pattern for Testability

```rust
// Define repository trait
#[async_trait::async_trait]
pub trait QuoteRepository: Send + Sync {
    async fn find_by_id(&self, id: &str) -> Result<Option<Quote>, Error>;
    async fn create(&self, quote: &Quote) -> Result<Quote, Error>;
    async fn update(&self, quote: &Quote) -> Result<Quote, Error>;
}

// SQLx implementation
pub struct SqlQuoteRepository {
    pool: SqlitePool,
}

#[async_trait::async_trait]
impl QuoteRepository for SqlQuoteRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Quote>, Error> {
        sqlx::query_as!(Quote, "SELECT * FROM quotes WHERE id = ?", id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::from)
    }
    // ...
}

// In-memory mock for unit tests
pub struct InMemoryQuoteRepository {
    data: Arc<RwLock<Vec<Quote>>>,
}

#[async_trait::async_trait]
impl QuoteRepository for InMemoryQuoteRepository {
    async fn find_by_id(&self, id: &str) -> Result<Option<Quote>, Error> {
        let data = self.data.read().await;
        Ok(data.iter().find(|q| q.id == id).cloned())
    }
    // ...
}
```

### 3.4 Test Isolation Strategies

| Approach | Speed | Realism | Complexity | Use Case |
|----------|-------|---------|------------|----------|
| In-memory SQLite | ⭐⭐⭐ Fast | ⭐⭐ Good | ⭐ Low | Unit/integration tests |
| Real SQLite (temp file) | ⭐⭐ Medium | ⭐⭐⭐ Full | ⭐⭐ Medium | Integration tests |
| Mock repository | ⭐⭐⭐ Fast | ⭐ Low | ⭐⭐ Medium | Pure unit tests |

**Recommendation for Quotey:**
- **Primary**: In-memory SQLite with `sqlx::test`
- **Secondary**: Mock repositories for pure unit tests of business logic
- **CI**: Same as local (SQLite is fast enough)

---

## 4. HTTP Mocking

### 4.1 wiremock (Recommended)

For testing Slack API and CRM integrations:

```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path, header};

#[tokio::test]
async fn test_slack_integration() {
    // Start mock server
    let mock_server = MockServer::start().await;
    
    // Configure mock
    Mock::given(method("POST"))
        .and(path("/api/chat.postMessage"))
        .and(header("Authorization", "Bearer xoxb-test"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({
                "ok": true,
                "ts": "1234567890.123456"
            })))
        .expect(1) // Expect exactly one call
        .mount(&mock_server)
        .await;
    
    // Test code using mock_server.uri()
    let slack_client = SlackClient::new(&mock_server.uri(), "xoxb-test");
    let result = slack_client.post_message("#general", "Hello").await;
    
    assert!(result.is_ok());
    // Mock is automatically verified on drop
}
```

**Key Features:**
- Parallel test execution (each test gets isolated mock)
- Request matching (method, path, headers, body)
- Response templating
- Expectation verification (automatic on drop)

### 4.2 Comparison: wiremock vs mockito vs httpmock

| Feature | wiremock | mockito | httpmock |
|---------|----------|---------|----------|
| Execution | Parallel | Sequential | Parallel |
| Runtime | Async (tokio/async-std) | Sync | Async/Sync |
| Multiple APIs | ✅ Yes | ❌ No | ✅ Yes |
| Request matching | ✅ Extensive | ✅ Basic | ✅ Extensive |
| Spying | ✅ Yes | ✅ Yes | ✅ Yes |

**Recommendation:** Use `wiremock` for Quotey's async codebase.

---

## 5. Mocking Strategies

### 5.1 Trait-Based Mocking with mockall

For mocking internal dependencies:

```rust
use mockall::automock;

#[automock]
#[async_trait::async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;
}

#[tokio::test]
async fn test_agent_with_mock_llm() {
    let mut mock_llm = MockLlmClient::new();
    
    mock_llm
        .expect_complete()
        .with(eq("extract intent"))
        .returning(|_| Ok("{\"intent\": \"new_quote\"}".to_string()));
    
    let agent = Agent::new(Box::new(mock_llm));
    let result = agent.process("Create a new quote").await;
    
    assert!(matches!(result.intent, Intent::NewQuote));
}
```

**Limitations:**
- Generic methods require manual mock definition
- Complex trait bounds can be challenging

### 5.2 Manual Mock Implementation

For complex cases:

```rust
pub struct MockLlmClient {
    responses: Arc<RwLock<Vec<String>>>,
    call_count: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError> {
        let count = self.call_count.fetch_add(1, Ordering::SeqCst);
        let responses = self.responses.read().await;
        
        responses
            .get(count)
            .cloned()
            .ok_or_else(|| LlmError::NoMoreResponses)
    }
}
```

### 5.3 Test Fixtures with rstest

For parameterized tests:

```rust
use rstest::rstest;

#[rstest]
#[case("Pro Plan", vec!["Pro Plan", "Enterprise Plan"], "plan_pro")]
#[case("SSO Add-on", vec!["SSO", "Single Sign-On"], "sso_addon")]
#[tokio::test]
async fn test_product_matching(
    #[case] input: &str,
    #[case] candidates: Vec<&str>,
    #[case] expected: &str,
) {
    let matcher = ProductMatcher::new();
    let result = matcher.match_product(input, &candidates).await;
    
    assert_eq!(result, expected);
}
```

---

## 6. Integration Test Structure

### 6.1 Recommended Project Structure

```
quotey/
├── src/
│   └── ...
├── tests/
│   ├── integration/
│   │   ├── mod.rs           # Integration test helpers
│   │   ├── quote_flow.rs    # End-to-end quote tests
│   │   ├── slack_commands.rs
│   │   └── pricing.rs
│   └── fixtures/
│       ├── products.json
│       └── price_books.json
└── Cargo.toml
```

### 6.2 Integration Test Helpers

```rust
// tests/integration/mod.rs
use quotey::Application;

pub async fn test_app() -> Application {
    let config = Config::test_config();
    Application::bootstrap(config).await.unwrap()
}

pub async fn with_test_db<F, Fut>(test: F)
where
    F: FnOnce(SqlitePool) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();
    
    // Run test
    test(pool).await;
}
```

### 6.3 Example Integration Test

```rust
// tests/integration/quote_flow.rs
use super::*;

#[tokio::test]
async fn test_full_quote_lifecycle() {
    with_test_db(|pool| async move {
        // Setup
        let quote_repo = SqlQuoteRepository::new(pool.clone());
        let product_repo = SqlProductRepository::new(pool.clone());
        
        // Seed test data
        seed_products(&pool).await;
        
        // Execute: Create quote
        let quote = Quote::new("Acme Corp", "net_new");
        let quote = quote_repo.create(&quote).await.unwrap();
        
        // Add line item
        let line = QuoteLine::new(&quote.id, "plan_pro", 100);
        quote_repo.add_line(&line).await.unwrap();
        
        // Verify
        let retrieved = quote_repo.find_by_id(&quote.id).await.unwrap();
        assert_eq!(retrieved.lines.len(), 1);
        assert_eq!(retrieved.lines[0].quantity, 100);
    }).await;
}
```

---

## 7. Testing Best Practices

### 7.1 Golden Rules

1. **Use `#[tokio::test]` for all async tests**
   - No manual runtime setup/teardown
   - Consistent runtime configuration

2. **Prefer in-memory SQLite over mocks for database tests**
   - Tests actual SQL queries
   - Fast enough for most cases
   - No mock maintenance

3. **Mock external services (Slack, CRM, LLM)**
   - Tests shouldn't depend on external APIs
   - Use wiremock for HTTP services
   - Use mockall for trait-based dependencies

4. **Run tests in parallel**
   - Use `cargo nextest` for CI
   - Ensure test isolation (no shared state)
   - Random port allocation for any servers

5. **Use timeouts for async operations**
   ```rust
   use tokio::time::{timeout, Duration};
   
   #[tokio::test]
   async fn test_with_timeout() {
       let result = timeout(
           Duration::from_secs(5),
           async_operation()
       ).await;
       
       assert!(result.is_ok(), "Operation timed out");
   }
   ```

### 7.2 Test Organization

| Test Type | Location | Scope | Speed |
|-----------|----------|-------|-------|
| Unit tests | Inline (`#[cfg(test)]`) | Single function | < 10ms |
| Integration | `tests/` directory | Module/crate | 10-100ms |
| E2E | `tests/e2e/` | Full application | 100ms-1s |

### 7.3 Debugging Failed Tests

```bash
# Run single test with output
cargo test test_name -- --nocapture

# Run with tracing
cargo test test_name --features tracing

# Run with nextest and immediate output
nextest run test_name --no-capture

# Run with debugger (requires setup)
rust-gdb --args cargo test test_name
```

---

## 8. CI/CD Configuration

### 8.1 GitHub Actions Example

```yaml
name: Test

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - uses: dtolnay/rust-cache@v2
      
      - name: Install nextest
        uses: taiki-e/install-action@nextest
      
      - name: Run tests
        run: cargo nextest run
      
      - name: Generate coverage
        run: cargo llvm-cov --lcov --output-path lcov.info
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info
```

### 8.2 Test Matrix

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    rust: [stable, beta]
```

---

## 9. Summary & Recommendations

### 9.1 Testing Stack for Quotey

| Component | Tool | Justification |
|-----------|------|---------------|
| Test runner | `tokio::test` | Standard, works with our stack |
| CI runner | `cargo-nextest` | Faster, better reporting |
| DB tests | `sqlx::test` + SQLite | Real queries, fast, isolated |
| HTTP mocking | `wiremock` | Async, parallel, feature-rich |
| Trait mocking | `mockall` | Standard, works with async_trait |
| Fixtures | `rstest` | Parameterized tests |

### 9.2 Key Decisions

1. **Use `#[tokio::test]` everywhere** - No exceptions
2. **SQLite in-memory for most tests** - Only use mocks for external services
3. **Repository pattern with traits** - Enables both real and mock implementations
4. **Wiremock for Slack/CRM tests** - Isolated, parallel, realistic
5. **Nextest in CI** - Performance and reliability

### 9.3 Testing Pyramid for Quotey

```
       /\
      /  \  E2E (5%) - Full flows via Slack
     /____\ 
    /      \  Integration (25%) - Repos, services
   /________\
  /          \  Unit (70%) - Pure functions, business logic
 /____________\
```

---

## References

1. Tokio Testing Guide: https://tokio.rs/tokio/topics/testing
2. Nextest Documentation: https://nexte.st/
3. Wiremock Documentation: https://docs.rs/wiremock/
4. Mockall Documentation: https://docs.rs/mockall/
5. SQLx Testing: https://docs.rs/sqlx/
6. "Zero to Production in Rust" - Luca Palmieri (Chapter on testing)

---

*End of Research Report*
