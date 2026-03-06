# Contributing to Quotey

Thank you for your interest in contributing to Quotey! This guide will help you get started.

## Quick Start

```bash
# 1. Clone the repository
git clone https://github.com/quotey/quotey.git
cd quotey

# 2. Install dependencies
cargo install sqlx-cli --no-default-features --features sqlite
cargo install cargo-nextest
cargo install cargo-deny

# 3. Run quality gates
./scripts/quality-gates.sh

# 4. Build the project
cargo build --release
```

## Development Workflow

### Finding Work

Quotey uses **beads** (br) for issue tracking:

```bash
# See actionable (unblocked) work
br ready --json

# See full issue list
br list --json | jq '.[] | {id, title, status, priority}'
```

### Claiming Work

```bash
# Claim an issue
br update br-123 --status in_progress --json
```

### Before Starting Complex Work

```bash
# Get cross-agent memory for your task
cm context "implement pricing approval workflow" --json
```

### Making Changes

1. **Research First** — Check `.planning/RESEARCH_*.md` for relevant research
2. **Write Tests** — All new functionality must include tests
3. **Run Quality Gates** — Execute `./scripts/quality-gates.sh`
4. **Update Documentation** — Update relevant docs in `docs/`

### Committing Changes

```bash
# 1. Sync beads
br sync --flush-only

# 2. Stage changes
git add .

# 3. Commit (include beads changes with code changes)
git commit -m "feat: your change description"

# 4. Push
git push
```

### Closing Issues

```bash
# When work is complete
br close br-123 --reason "Implemented in #<PR-number>" --json
```

## Architecture Guidelines

### Key Principles

1. **Deterministic CPQ** — All pricing, policy, and approval decisions must be deterministic
2. **Auditability** — Every decision must be traceable to its source data
3. **Local-First** — Works offline, SQLite-backed
4. **Agent-Native** — Support `--json` flags for programmatic access

### The Safety Principle

> LLMs may translate natural language into structured intent and summaries.  
> LLMs do **not** decide prices, configuration validity, policy compliance, or approval routing.  
> Deterministic engines and rules in SQLite are always the source of truth.

This is the most important architectural decision. When in doubt, err on the side of determinism.

### Code Organization

```
crates/
├── core/       # Domain logic, deterministic engines
├── db/         # SQLite repositories, migrations
├── slack/      # Slack bot, Block Kit UI
├── agent/      # Agent runtime, intent extraction
├── cli/        # Command-line interface
├── server/     # HTTP server bootstrap
└── mcp/        # MCP server for AI agents
```

### Dependency Rules

- `core` has no runtime/network/database dependencies
- `db`, `slack`, and `agent` are adapter crates that depend on `core`
- `server` composes runtime crates and owns process startup

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run with nextest (better output)
cargo nextest run

# Run specific crate tests
cargo test -p quotey-core

# Run with output
cargo test -- --nocapture
```

### Test Categories

| Category | Location | Purpose |
|----------|----------|---------|
| Unit tests | In source files (`#[cfg(test)]`) | Test individual functions |
| Integration tests | `tests/` directories | Test across modules |
| E2E tests | `tests/e2e/` | Full workflow tests |

### Writing Tests

```rust
#[test]
fn pricing_calculates_correctly() {
    // Arrange
    let engine = PricingEngine::new(/* test data */);
    let quote = test_quote();
    
    // Act
    let result = engine.price(&quote);
    
    // Assert
    assert_eq!(result.total, dec!(1000.00));
    assert!(result.trace.is_complete());
}

#[tokio::test]
async fn quote_creation_persists() {
    let db = test_db().await;
    let repo = SqliteQuoteRepository::new(db);
    
    let quote = test_quote();
    let created = repo.create(&quote).await.unwrap();
    
    assert_eq!(created.id, quote.id);
}
```

## Code Style

### Rust

- Use `rustfmt` for formatting (enforced in CI)
- Follow `clippy` recommendations (enforced in CI)
- Document public APIs with `///`
- Use `thiserror` for error types
- Use `tracing` for logging

### SQL

- Use parameterized queries (sqlx handles this)
- Migrations must be reversible (up/down)
- Table names: `snake_case`, plural
- Column names: `snake_case`

### Documentation

- Keep `AGENTS.md` updated with agent workflows
- Document architectural decisions in `.planning/`
- Update `docs/` for user-facing changes

## Pre-Commit Checklist

Before committing, ensure:

- [ ] Code compiles without warnings
- [ ] `cargo fmt` has been run
- [ ] `cargo clippy` passes
- [ ] Tests pass (`cargo test`)
- [ ] Documentation is updated
- [ ] Beads are synced (`br sync --flush-only`)

## Git Hooks

The project includes a pre-commit hook:

```bash
# Run manually
.githooks/pre-commit

# Skip clippy during commit (faster)
QUOTEY_PRE_COMMIT_CLIPPY=0 git commit -m "your message"
```

## Getting Help

- Check existing research in `.planning/RESEARCH_*.md`
- Review `AGENTS.md` for workflow details
- Run `cm context "<your question>" --json` for institutional memory

## Code of Conduct

Be respectful, constructive, and collaborative. We're building this together.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
