# Foundation Quickstart

**Start here after reading `FOUNDATION_IMPLEMENTATION_PLAN.md`**

## Task Sequence (DO NOT SKIP)

```
bd-3d8.1 → bd-3d8.2 → bd-3d8.3 → bd-3d8.4 → bd-3d8.5 → bd-3d8.10
```

## Current Task Command

```bash
# Start the current task
br ready  # See what's unblocked
br update <task-id> --status in_progress
```

## Phase 0 Tooling Gate (`bd-3d8.12`)

Run these before scaffold coding starts:

```bash
# Minimum compiler requirement: rustc >= 1.75
rustc --version | awk '{split($2,v,"."); exit !((v[1] > 1) || (v[1] == 1 && v[2] >= 75))}'

# Required tooling availability
cargo fmt --version
cargo clippy --version
cargo sqlx --version
cargo nextest --version
cargo deny --version
```

If a command is missing:

```bash
cargo install --locked sqlx-cli --no-default-features --features rustls,sqlite
cargo install --locked cargo-nextest
cargo install --locked cargo-deny
```

Policy files now used by the repository:
- `rust-toolchain.toml` for toolchain + rustfmt/clippy components
- `rustfmt.toml` for formatter behavior
- `.cargo/config.toml` for strict lint/test aliases
- `deny.toml` for dependency auditing rules

## Config Bootstrap Checks (`bd-3d8.2`)

```bash
# Validate typed config + fail-fast behavior from CLI
cargo run -p quotey-cli -- config
cargo run -p quotey-cli -- doctor
```

Useful env overrides:
- `QUOTEY_DATABASE_URL`
- `QUOTEY_SLACK_APP_TOKEN`
- `QUOTEY_SLACK_BOT_TOKEN`
- `QUOTEY_LLM_PROVIDER`
- `QUOTEY_LOGGING_LEVEL`

## Daily Development Loop

```bash
# 1. Verify clean state
cargo fmt-check
cargo lint

# 2. Run tests
cargo test-all

# 3. Make changes
# ... edit files ...

# 4. Verify again
cargo fmt-check
cargo lint
cargo test-all
cargo deny check

# 5. Commit
git add .
git commit -m "<task-id>: <description>"
```

## Key Commands

| Command | Purpose |
|---------|---------|
| `cargo build --workspace` | Build all crates |
| `cargo test --workspace` | Run all tests |
| `cargo clippy --workspace -- -D warnings` | Lint (zero warnings policy) |
| `cargo fmt` | Format code |
| `cargo fmt-check` | Enforced formatter check (alias from `.cargo/config.toml`) |
| `cargo lint` | Enforced clippy check (alias from `.cargo/config.toml`) |
| `cargo test-all` | Run tests with nextest (alias from `.cargo/config.toml`) |
| `cargo deny check` | Dependency/license/advisory audit |
| `cargo sqlx migrate run` | Apply DB migrations |
| `cargo sqlx prepare` | Prepare for offline builds |
| `cargo doc --workspace` | Build documentation |

## Architecture Decisions (Frozen)

| Decision | Rationale |
|----------|-----------|
| Workspace with 6 crates | Clean boundaries, parallel builds |
| SQLite + sqlx | Local-first, zero-ops, compile-time checked |
| Socket Mode only | No infrastructure, runs on laptop |
| LLM behind trait | Swappable implementations |
| Secrets via `secrecy` | Security best practice |
| Money as `Decimal` | No floating-point financial calculations |

## Testing Strategy

- **Unit tests:** In same file as code (`#[cfg(test)]`)
- **Integration tests:** In `tests/integration/`
- **Mocks:** Implement traits with in-memory versions
- **Property tests:** For constraint engine (optional)

## Common Patterns

### Adding a New Repository

```rust
// 1. Define trait in core
crates/core/src/ports/repository.rs:
#[async_trait]
pub trait QuoteRepository: Send + Sync {
    async fn find_by_id(&self, id: &QuoteId) -> Result<Option<Quote>, RepositoryError>;
}

// 2. Implement in db crate
crates/db/src/repositories/quote.rs:
pub struct SqlQuoteRepository { pool: DbPool }

#[async_trait]
impl QuoteRepository for SqlQuoteRepository { ... }

// 3. Add mock for testing
crates/db/src/repositories/mock.rs:
pub struct InMemoryQuoteRepository { ... }
```

### Adding a New Slack Handler

```rust
// 1. Implement EventHandler trait
crates/slack/src/events/my_handler.rs:
pub struct MyHandler;

#[async_trait]
impl EventHandler for MyHandler {
    fn event_type() -> SlackEventType { ... }
    async fn handle(&self, event: &SlackEvent, ctx: &EventContext) { ... }
}

// 2. Register in router
crates/slack/src/events/mod.rs:
router.register(MyHandler::event_type(), MyHandler::new());
```

## Blockers & Escalation

If you find yourself:
- Changing a previously "done" task → Review dependency assumptions
- Adding a new crate → Discuss with team (cognitive overhead)
- Bypassing a trait for "quick fix" → Stop, implement properly
- Adding `unwrap()` in production code → Use `?` and proper error types

## Definition of Done (Per Task)

- [ ] Code compiles with zero warnings
- [ ] Tests pass (unit + integration)
- [ ] Documentation comments on public APIs
- [ ] CHANGELOG.md updated (if user-visible)
- [ ] Task closed in beads: `br update <id> --status done`

## Next Phase Gate

Foundation is complete when `bd-3d8.10` passes:

```bash
# Run the gate check
cargo build --release --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt -- --check
cargo deny check

# Integration test
cargo test --test integration -- --ignored
```

All must pass before starting Deal DNA (bd-70d.1).
