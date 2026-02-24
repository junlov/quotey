# Contributing to Quotey

Thank you for your interest in contributing to Quotey! This document provides guidelines and setup instructions for contributors.

## Development Setup

### Prerequisites

- **Rust** 1.75 or later
- **SQLite** (usually included with Rust sqlx)
- **beads** (issue tracking): `brew install dicklesworthstone/tap/br`
- **cass-memory** (cross-agent memory): `brew install dicklesworthstone/tap/cm`

### Quick Start

```bash
# Clone the repository
git clone https://github.com/junlov/quotey.git
cd quotey

# Run quality gates
./scripts/quality-gates.sh

# Build the project
cargo build --release
```

## Development Workflow

### Finding Work

```bash
# See actionable (unblocked) work
br ready --json

# See full issue list
br list --json | jq '.[] | {id, title, status, priority}'
```

### Claiming Work

```bash
# Claim an issue
br update <id> --status in_progress --json

# Example:
br update br-123 --status in_progress --json
```

### Before Starting Complex Work

```bash
# Get cross-agent memory for your task
cm context "implement pricing approval workflow" --json
```

This returns:
- **relevantBullets**: CPQ best practices
- **antiPatterns**: Pitfalls to avoid
- **historySnippets**: Past sessions that solved similar problems

### Making Changes

1. **Research First**: Check `.planning/RESEARCH_*.md` for relevant research
2. **Write Tests**: All new functionality must include tests
3. **Run Quality Gates**: Execute `./scripts/quality-gates.sh`
4. **Update Documentation**: Update relevant docs in `.planning/`

### Quality Gates

Before committing, ensure:

```bash
# Formatting
cargo fmt --all -- --check

# Linting
cargo clippy --all-targets --all-features -- -D warnings

# Tests
cargo test --all-features

# Security audit (requires cargo-deny)
cargo deny check advisories
```

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
br close <id> --reason "Implemented in #<PR-number>" --json
```

## Architecture Guidelines

### Key Principles

1. **Deterministic CPQ**: All pricing, policy, and approval decisions must be deterministic
2. **Auditability**: Every decision must be traceable to its source data
3. **Local-First**: Works offline, SQLite-backed
4. **Agent-Native**: Support `--json` flags for programmatic access

### Module Structure

```
crates/
├── core/       # Domain logic, deterministic engines
├── db/         # SQLite repositories, migrations
├── slack/      # Slack bot, Block Kit UI
├── agent/      # Agent runtime, intent extraction
├── cli/        # Command-line interface
└── server/     # HTTP server (future)
```

### Safety Principle

> LLMs may translate natural language into structured intent and summaries.  
> LLMs do **not** decide prices, configuration validity, policy compliance, or approval routing.  
> Deterministic engines and rules in SQLite are always the source of truth.

## Research Documents

Before implementing, review relevant research:

| Document | Topic |
|----------|-------|
| `RESEARCH_DEAL_DNA.md` | Deal similarity, ML scoring |
| `RESEARCH_POLICY_EVALUATION_PERSISTENCE.md` | Policy snapshot strategies |
| `RESEARCH_PRICING_SNAPSHOT_PERSISTENCE.md` | Pricing trace storage |
| `RESEARCH_EXPLAINABLE_POLICY.md` | Policy explanation engine |
| `DICKLESWORTHSTONE_STACK.md` | External tool integration |

## Testing

### Unit Tests

```bash
# Run all tests
cargo test

# Run specific crate tests
cargo test -p quotey-core

# Run with output
cargo test -- --nocapture
```

### Integration Tests

```bash
# Run all integration tests
cargo test --all-features
```

### Test Database

Tests use in-memory SQLite by default. For persistent test database:

```bash
export TEST_DATABASE_URL=sqlite://test.db
cargo test
```

## Debugging

### VS Code

Recommended `launch.json` configurations are provided in `.vscode/launch.json`:

- **Debug CLI**: Debug the CLI binary
- **Debug Tests**: Debug test execution

### Logging

```bash
# Enable debug logging
RUST_LOG=debug cargo run

# Enable trace logging for specific module
RUST_LOG=quotey_core::cpq=trace cargo run
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
- Table names: snake_case, plural
- Column names: snake_case

### Documentation

- Keep `AGENTS.md` updated with agent workflows
- Document architectural decisions in `.planning/`
- Update `CHANGELOG.md` for user-facing changes

## Communication

### Issue Tracking

- Use **beads** (br) for all issue tracking
- Create issues for bugs, features, and research tasks
- Link related issues with dependencies
- Use `--json` flag for programmatic access

### Agent Coordination

- Use **mcp_agent_mail** for multi-agent coordination
- Reserve files before editing
- Use thread IDs for related work

## Questions?

- Check existing research in `.planning/RESEARCH_*.md`
- Review `AGENTS.md` for workflow details
- Run `cm context "<your question>" --json` for institutional memory

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
