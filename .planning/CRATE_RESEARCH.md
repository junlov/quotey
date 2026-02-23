# Crate Research & Selection Report

**Date:** 2026-02-23  
**Purpose:** Evaluate and select Rust crates for Quotey foundation  
**Approach:** Conservative, production-ready, battle-tested

---

## Executive Summary

| Category | Selected Crate | Version | Rationale |
|----------|---------------|---------|-----------|
| **Slack** | `slack-morphism` | 2.18.0 | Only mature Socket Mode library |
| **Database** | `sqlx` | 0.8.6 | Compile-time checked queries, stable |
| **LLM** | `async-openai` | 0.33.0 | Clean API, trait-friendly |
| **Web** | `axum` | 0.8.8 | Tokio-native, lightweight |
| **Config** | `config` + `envy` | 0.15 + 0.4 | Layered config, proven pattern |
| **Errors** | `thiserror` | 2.0.18 | Structured errors, no overhead |
| **HTTP** | `reqwest` | 0.13.2 | Standard, async-native |
| **Tracing** | `tracing` + `tracing-subscriber` | 0.1 + 0.3 | Ecosystem standard |
| **Money** | `rust_decimal` | 1.40.0 | Financial precision |
| **Secrets** | `secrecy` | 0.10.3 | Zero-on-drop, no Debug leak |
| **PDF** | `headless_chrome` | 1.0.21 | HTML→PDF, template flexibility |
| **Templates** | `minijinja` | 2.16.0 | Fast, minimal deps, Jinja2 compat |

---

## 1. Slack Integration

### Primary: `slack-morphism` v2.18.0

**Why:**
- ✅ **Only** Rust library with full Socket Mode support
- ✅ Type-safe Block Kit builders (no raw JSON)
- ✅ WebSocket reconnection built-in
- ✅ Actively maintained (Launchbadge, same team as sqlx)

**Feature flags needed:**
```toml
slack-morphism = { version = "2.18", features = ["socket-mode", "hyper"] }
```

**Architecture:**
```rust
// Socket Mode client
let client = SlackClientSocketMode::new(
    SlackSocketModeListener::new()
        .command_handler(my_command_handler)
        .event_handler(my_event_handler)
);
```

**Alternatives considered:**
- `slack-rs` - No Socket Mode, HTTP only
- Raw WebSocket + `serde_json` - Too much boilerplate, error-prone

**Risk:** Low. This is the de facto standard.

---

## 2. Database Layer

### Primary: `sqlx` v0.8.6 (STABLE)

**⚠️ CRITICAL: Use 0.8.6, NOT 0.9.0-alpha**

**Why:**
- ✅ Compile-time checked SQL queries (`query!` macro)
- ✅ Async-native with connection pooling
- ✅ SQLite-specific optimizations (WAL mode, etc.)
- ✅ Migration system built-in
- ✅ Zero-cost abstractions

**Why not 0.9.0-alpha:**
- ❌ Alpha status, API may change
- ❌ Requires Rust 1.86 (very new)
- ❌ Not battle-tested in production

**Feature flags:**
```toml
sqlx = { version = "0.8.6", features = [
    "runtime-tokio",
    "sqlite",
    "migrate",
    "uuid",
    "chrono",
    "json",
    "decimal"
] }
```

**Connection pooling:**
```rust
use sqlx::sqlite::SqlitePoolOptions;

let pool = SqlitePoolOptions::new()
    .max_connections(5)  // SQLite single-writer limit
    .acquire_timeout(Duration::from_secs(30))
    .connect("sqlite://quotey.db")
    .await?;
```

**Why not `deadpool-sqlite`?**
- `deadpool-sqlite` is for `rusqlite` (sync)
- `sqlx` has built-in pooling via `SqlitePool`
- One less dependency

**Migration strategy:**
```rust
// Embed migrations in binary
sqlx::migrate!("./migrations").run(&pool).await?;
```

**Risk:** Low. sqlx is production-proven at scale.

---

## 3. LLM Integration

### Primary: `async-openai` v0.33.0

**Why:**
- ✅ Clean, idiomatic Rust API
- ✅ Easy to wrap behind trait
- ✅ Supports all OpenAI features (functions, streaming, etc.)
- ✅ Compatible with Ollama (OpenAI-compatible API)

**Trait abstraction:**
```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String, LlmError>;
    async fn extract_structured<T: DeserializeOwned>(
        &self, 
        prompt: &str
    ) -> Result<T, LlmError>;
}

// async-openai wrapper
pub struct OpenAiClient {
    inner: async_openai::Client<OpenAIConfig>,
}

// Ollama wrapper (same interface)
pub struct OllamaClient {
    inner: async_openai::Client<OpenAIConfig>, // Uses Ollama base URL
}
```

**Alternatives considered:**
- `rig-core` (v0.31.0): More opinionated, higher-level, adds complexity we don't need
- Raw `reqwest`: Too much boilerplate for OpenAI API

**Feature flags:**
```toml
async-openai = "0.33"
```

**Risk:** Low. Simple wrapper around REST API.

---

## 4. Web Framework (Health Checks)

### Primary: `axum` v0.8.8

**Why:**
- ✅ Tokio-native (same runtime as slack-morphism)
- ✅ Minimal overhead (just health checks)
- ✅ Excellent error handling integration
- ✅ Tower ecosystem compatibility

**Use case:**
```rust
// Single health check endpoint
let app = Router::new()
    .route("/health", get(health_check));

// Runs on localhost:8080
axum::serve(tokio::net::TcpListener::bind("127.0.0.1:8080").await?, app).await?;
```

**Why not `actix-web`?**
- Different runtime model (actix-rt vs tokio)
- Heavier for our minimal use case
- Axum integrates better with Tower middleware

**Why not `rocket`?**
- Requires nightly Rust (unacceptable for production)
- Heavy compile times
- More than we need

**Risk:** Very low. Axum is the emerging standard.

---

## 5. Configuration Management

### Primary: `config` v0.15.19 + `envy` v0.4.2

**Pattern:** Layered configuration
1. Defaults (compiled)
2. Config file (`quotey.toml`)
3. Environment variables (`QUOTEY_*`)
4. CLI arguments

**Implementation:**
```rust
use config::{Config, File, Environment};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct AppConfig {
    pub database: DatabaseConfig,
    pub slack: SlackConfig,
}

let cfg = Config::builder()
    .add_source(File::with_name("quotey").required(false))
    .add_source(Environment::with_prefix("QUOTEY"))
    .build()?
    .try_deserialize::<AppConfig>()?;
```

**Secrets handling:**
```rust
use secrecy::SecretString;

#[derive(Deserialize)]
pub struct SlackConfig {
    pub bot_token: SecretString,  // Not visible in Debug
}
```

**Risk:** Low. Well-established pattern.

---

## 6. Error Handling

### Primary: `thiserror` v2.0.18

**Why:**
- ✅ Zero runtime overhead (just derive macros)
- ✅ Automatic `std::error::Error` implementation
- ✅ Clean source error chains
- ✅ No allocation in hot paths

**Pattern:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PricingError {
    #[error("product not found: {0}")]
    ProductNotFound(String),
    
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

**When to use `anyhow`:**
- Application boundary (CLI main, HTTP handlers)
- Where specific error types don't matter
- Quick prototyping

**Risk:** None. Industry standard.

---

## 7. HTTP Client

### Primary: `reqwest` v0.13.2

**Why:**
- ✅ Async-native
- ✅ Connection pooling
- ✅ Built-in JSON support
- ✅ Standard for Rust HTTP

**Use cases:**
- Composio REST API calls
- Slack API calls (if not using slack-morphism for some)
- LLM API calls (if using something other than OpenAI)

**Feature flags:**
```toml
reqwest = { version = "0.13", features = ["json", "rustls-tls"] }
```

**Why `rustls-tls` over `native-tls`?**
- Pure Rust (no OpenSSL dependency)
- Cross-compilation friendly
- Modern TLS versions

**Risk:** None. De facto standard.

---

## 8. Observability

### Primary: `tracing` v0.1.44 + `tracing-subscriber` v0.3.22

**Why:**
- ✅ Structured logging (not just strings)
- ✅ Span-based tracing (request lifecycle)
- ✅ Multiple output formats (pretty, JSON, compact)
- ✅ OpenTelemetry compatible (future-proof)

**Configuration:**
```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "quotey=info".into())
    )
    .with(tracing_subscriber::fmt::layer())
    .init();
```

**Usage:**
```rust
#[tracing::instrument(skip(self))]
pub async fn price_quote(&self, quote_id: &QuoteId) -> Result<PricingResult, PricingError> {
    tracing::info!(%quote_id, "pricing quote");
    
    let result = self.do_pricing(quote_id).await?;
    
    tracing::info!(total = %result.total, "pricing complete");
    Ok(result)
}
```

**Risk:** None. Industry standard.

---

## 9. Financial Calculations

### Primary: `rust_decimal` v1.40.0

**Why:**
- ✅ Decimal arithmetic (no float rounding errors)
- ✅ SQLx integration (via feature flag)
- ✅ Serde support
- ✅ Fast enough for our use case

**Critical for:**
- Unit prices
- Discounts
- Totals
- Tax calculations

**Why not `f64`?**
```rust
// Float - DANGEROUS for money
let a: f64 = 0.1;
let b: f64 = 0.2;
assert_ne!(a + b, 0.3);  // FAILS!

// Decimal - CORRECT
use rust_decimal::Decimal;
let a = Decimal::from_f64(0.1).unwrap();
let b = Decimal::from_f64(0.2).unwrap();
assert_eq!(a + b, Decimal::from_f64(0.3).unwrap());  // PASSES
```

**Feature flags:**
```toml
rust_decimal = { version = "1.40", features = ["db-sqlx", "serde"] }
```

**Risk:** None. Essential for correctness.

---

## 10. Secret Management

### Primary: `secrecy` v0.10.3

**Why:**
- ✅ Zero-on-drop (memory cleared when dropped)
- ✅ No `Debug` exposure (secrets don't leak in logs)
- ✅ No `Display` implementation
- ✅ Explicit extraction via `expose_secret()`

**Pattern:**
```rust
use secrecy::{SecretString, ExposeSecret};

#[derive(Debug)]  // Debug shows [REDACTED]
pub struct Config {
    pub api_key: SecretString,
}

// Usage
let key = config.api_key.expose_secret();  // Explicit!
client.call_api(key).await?;
// key is zeroed in memory when dropped
```

**Risk:** Low. Defense in depth.

---

## 11. PDF Generation

### Primary: `headless_chrome` v1.0.21

**Why:**
- ✅ HTML → PDF (use templates!)
- ✅ Full CSS support
- ✅ Handlebars/Tera integration
- ✅ Page headers/footers

**Trade-offs:**
- Requires Chrome/Chromium installed (80MB+ dependency)
- Alternative: `wkhtmltopdf` (deprecated, no longer maintained)
- Alternative: `genpdf` (pure Rust, limited CSS)

**Template → PDF flow:**
```rust
use headless_chrome::{Browser, LaunchOptions};
use minijinja::Environment;

// 1. Render HTML template
let mut env = Environment::new();
env.add_template("quote", include_str!("templates/quote.html"))?;
let template = env.get_template("quote")?;
let html = template.render(context! { quote })?;

// 2. Convert to PDF
let browser = Browser::new(LaunchOptions::default())?;
let tab = browser.new_tab()?;
tab.navigate_to("data:text/html;base64,...")?;
let pdf = tab.print_to_pdf(None)?;
```

**Risk:** Medium. External binary dependency, but best quality output.

**Alternative for v1:** Generate HTML, let user print-to-PDF. Add `headless_chrome` in v2.

---

## 12. Template Engine

### Primary: `minijinja` v2.16.0

**Why:**
- ✅ Jinja2-compatible syntax (familiar to Python devs)
- ✅ Fast (Rust-native, no Python)
- ✅ Minimal dependencies
- ✅ Sandboxed (no arbitrary code execution)

**Why not `tera`?**
- Tera v2 is alpha (similar to sqlx situation)
- MiniJinja is more actively maintained
- Similar feature set, smaller footprint

**Why not `handlebars`?**
- Handlebars is logic-less (too limiting for complex quotes)
- Jinja2 has better expressions and filters

**Example:**
```jinja2
{# templates/quote.html #}
<h1>Quote for {{ quote.customer.name }}</h1>

<table>
  {% for line in quote.lines %}
  <tr>
    <td>{{ line.product.name }}</td>
    <td>{{ line.quantity }}</td>
    <td>${{ line.unit_price }}</td>
    <td>${{ line.total }}</td>
  </tr>
  {% endfor %}
</table>

<p><strong>Total: ${{ quote.total }}</strong></p>
```

**Risk:** Low. Stable, well-maintained.

---

## 13. Additional Utilities

### UUID Generation: `uuid` v1.21.0
```toml
uuid = { version = "1.21", features = ["v4", "serde"] }
```
- v4 = random UUIDs
- serde = JSON serialization

### Date/Time: `chrono` v0.4.43
```toml
chrono = { version = "0.4", features = ["serde"] }
```
- SQLx integration built-in
- Timezone support (store everything as UTC)

### Async Runtime: `tokio` v1.49.0
```toml
tokio = { version = "1.49", features = ["full"] }
```
- Full features = rt-multi-thread, macros, sync, time, etc.

### Serialization: `serde` v1.0.228
```toml
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```
- Standard for Rust serialization

### Async Traits: `async-trait` v0.1.89
```toml
async-trait = "0.1"
```
- Required for trait methods that are async
- Native async traits coming to stable Rust soon

### Testing: `mockall` v0.14.0
```toml
mockall = "0.14"
```
- Automatic mock generation for traits
- Essential for testing repositories

---

## 14. Workspace Cargo.toml

```toml
[workspace]
members = ["crates/*"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"  # Required for async traits
authors = ["Quotey Contributors"]
license = "MIT OR Apache-2.0"

[workspace.dependencies]
# Async runtime
tokio = { version = "1.49", features = ["full"] }
async-trait = "0.1.89"

# Web/HTTP
axum = "0.8.8"
reqwest = { version = "0.13", features = ["json", "rustls-tls"] }

# Slack
slack-morphism = { version = "2.18", features = ["socket-mode", "hyper"] }

# Database
sqlx = { version = "0.8.6", features = [
    "runtime-tokio",
    "sqlite",
    "migrate",
    "uuid",
    "chrono",
    "json",
    "decimal"
] }

# LLM
async-openai = "0.33"

# Configuration
config = "0.15"
envy = "0.4"

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Observability
tracing = "0.1.44"
tracing-subscriber = "0.3.22"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Financial
rust_decimal = { version = "1.40", features = ["db-sqlx", "serde"] }

# Secrets
secrecy = "0.10"

# IDs
uuid = { version = "1.21", features = ["v4", "serde"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# Templates
minijinja = "2.16"

# PDF (optional for v1)
# headless_chrome = "1.0"

# Testing
mockall = "0.14"
tokio-test = "0.4"
```

---

## Risk Assessment Summary

| Risk Level | Crates | Mitigation |
|------------|--------|------------|
| **None** | tokio, serde, thiserror, anyhow, tracing, reqwest, chrono, uuid | Industry standard, millions of downloads |
| **Low** | sqlx (0.8.6), axum, slack-morphism, async-openai, rust_decimal, secrecy, minijinja | Production-proven, active maintenance |
| **Medium** | headless_chrome | External binary dependency; consider HTML-only for v1 |
| **High** | sqlx 0.9.0-alpha | **DO NOT USE** - Wait for stable release |

---

## Recommendation

**Proceed with confidence.** All selected crates are production-ready, actively maintained, and widely used in the Rust ecosystem. The combination represents a conservative, proven stack that prioritizes correctness and maintainability over bleeding-edge features.

**Exception:** Defer `headless_chrome` to v2 if binary size/deployment complexity is a concern. Generate HTML quotes initially; users can print-to-PDF.
