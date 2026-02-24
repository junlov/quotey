# quotey

Planning-first Rust CPQ agent for Slack.

## Foundation Start

Before scaffold implementation, read:
- `AGENTS.md`
- `.planning/config.json`
- `.planning/PROJECT.md`
- `.planning/FOUNDATION_IMPLEMENTATION_PLAN.md`
- `.planning/FOUNDATION_QUICKSTART.md`

Phase 0 tooling baseline (`bd-3d8.12`) is documented in `.planning/FOUNDATION_QUICKSTART.md`,
including minimum Rust version checks and required `cargo sqlx`, `cargo nextest`,
and `cargo deny` verification commands.

## Workspace Boundaries

`bd-3d8.1` introduces a six-crate scaffold with explicit ownership boundaries:

- `crates/core`:
  deterministic domain primitives and engine seams (`domain`, `cpq`, `flows`, `audit`).
- `crates/db`:
  SQLite connection, migration runner seam, and repository layer.
- `crates/slack`:
  Slack command/event/block/socket integration seams.
- `crates/agent`:
  runtime orchestration, tool registry, and guardrail policy seams.
- `crates/cli`:
  operator command surface (`start`, `migrate`, `seed`, `smoke`, `config`, `doctor`).
- `crates/server`:
  executable bootstrap/wiring entrypoint for runtime components.

Dependency intent:
- `core` has no runtime/network/database dependencies.
- `db`, `slack`, and `agent` are adapter/service crates that depend on `core`.
- `server` composes runtime crates and owns process startup.

## Foundation Contracts

Current scaffold contracts from completed foundation beads:

- `crates/core/src/flows`:
  deterministic state-machine contracts (`FlowState`, `FlowEvent`, `FlowEngine`, `NetNewFlow`)
  with typed rejection errors and replay-stability tests.
- `crates/core/src/cpq`:
  explicit `ConstraintEngine` / `PricingEngine` / `PolicyEngine` interfaces,
  plus `DeterministicCpqRuntime` orchestration and pricing trace output.
- `crates/core/src/dna`:
  deterministic `FingerprintGenerator` producing 128-bit SimHash signatures and
  vector representation for configuration similarity workflows.
- `crates/core/src/errors.rs`:
  layered error taxonomy (`DomainError` → `ApplicationError` → `InterfaceError`) with
  explicit conversion boundaries and user-safe interface messages.
- `crates/core/src/audit.rs`:
  structured audit event model with `quote_id`, `thread_id`, and `correlation_id` fields,
  plus sink interface for emission.
- `crates/core/src/domain/execution.rs` and `migrations/0012_execution_queue_rel.*`:
  durable execution-queue persistence primitives (queue tasks, idempotency ledger, transition
  audit trail) for resilient retry/recovery workflows.
- `crates/slack/src/socket.rs` and `crates/server/src/*`:
  structured tracing baseline including `correlation_id`, `quote_id`, and `thread_id`
  fields at ingress/bootstrap/runtime boundaries.

## Deterministic E2E Bootstrap

Use the E2E bootstrap script to create a local seeded database for CI-reproducible
scenario runs:

```bash
./scripts/e2e_bootstrap.sh
```

Optional overrides:

- `CLEAN_BEFORE_BOOTSTRAP=0` to reuse an existing file.
- `QUOTEY_E2E_DB_PATH=...` to choose the SQLite path.

The script appends `mode=rwc` to the `sqlite://` URL so the file-backed database is
created on first run before running migrations and seed loading.

## Configuration Contract

`quotey-core` now provides typed startup configuration (`crates/core/src/config.rs`) with
deterministic load precedence:

1. built-in defaults
2. optional TOML config file
3. `QUOTEY_*` environment variables
4. explicit CLI/runtime overrides

Environment interpolation is supported in config files via `${ENV_VAR}` syntax.

Example `quotey.toml`:

```toml
[database]
url = "sqlite://quotey.db"
max_connections = 5
timeout_secs = 30

[slack]
app_token = "${SLACK_APP_TOKEN}"
bot_token = "${SLACK_BOT_TOKEN}"

[llm]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"
timeout_secs = 30
max_retries = 2

[server]
bind_address = "127.0.0.1"
health_check_port = 8080
graceful_shutdown_secs = 15

[logging]
level = "info"
format = "pretty"
```

Validation is fail-fast and actionable. Startup fails if critical fields are invalid
(for example missing Slack token prefixes, invalid SQLite URL, or invalid timeout ranges).

Reference file: `config/quotey.example.toml`.

## Troubleshooting

### Common Errors

**Error: `slack.app_token must start with `xapp-``**
- You may have swapped the tokens. `QUOTEY_SLACK_APP_TOKEN` should start with `xapp-` (App-Level Token)
- `QUOTEY_SLACK_BOT_TOKEN` should start with `xoxb-` (Bot User OAuth Token)
- Get these from https://api.slack.com/apps > Your App > OAuth & Permissions

**Error: `unable to open database file` (SQLite)**
- The database directory may not exist or be writable
- For relative paths, ensure the working directory is correct
- Try using an absolute path: `QUOTEY_DATABASE_URL=sqlite:///absolute/path/to/quotey.db`

**Error: `configuration validation failed: llm.api_key is required`**
- OpenAI/Anthropic providers require an API key
- Set `QUOTEY_LLM_API_KEY` or switch to Ollama: `QUOTEY_LLM_PROVIDER=ollama`

### Debugging

Enable debug logging:
```bash
QUOTEY_LOG_LEVEL=debug cargo run
```

Run with structured JSON logs:
```bash
QUOTEY_LOG_FORMAT=json cargo run
```

### Migration Recovery

If migrations fail:
```bash
# Check current schema version
./target/debug/quotey config

# Reset database (WARNING: destroys data)
rm quotey.db
./target/debug/quotey migrate
```
