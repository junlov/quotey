# quotey

Planning-first Rust CPQ agent for Slack.

## 🚀 Getting Started

New to Quotey? Start here:

| Use Case | Start Here |
|----------|------------|
| **Quick Test** | [QUICK_START.md](QUICK_START.md) - Run locally in 15 minutes |
| **Production Deploy** | [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md) - VPS, Docker, systemd |
| **AI Agent Integration** | [AI_AGENT_INTEGRATION.md](AI_AGENT_INTEGRATION.md) - Pi Claude, Codex, headless |
| **Architecture** | [DEPLOYMENT_ARCHITECTURE.md](DEPLOYMENT_ARCHITECTURE.md) - System diagrams |

### AI Agent / Headless Mode (No Slack Required!)

Use Quotey with Pi Claude, Codex, or any AI agent via CLI or MCP:

```bash
# Deploy headless on VPS
sudo ./scripts/deploy-headless-vps.sh

# Or run locally
./scripts/setup-local.sh
cargo run -p quotey-cli -- quote list --format=json
```

See [AI_AGENT_INTEGRATION.md](AI_AGENT_INTEGRATION.md) for MCP server setup and tool calling.

### TL;DR - Run Locally

```bash
# 1. Setup (installs Rust, builds, creates config)
./scripts/setup-local.sh

# 2. Add your Slack tokens to config/quotey.toml
#    (see DEPLOYMENT_GUIDE.md for Slack setup)

# 3. Run migrations
cargo run -p quotey-cli -- migrate

# 4. Start the server
cargo run -p quotey-server

# 5. Test in Slack: /quote new for Acme Corp, Pro Plan
```

### TL;DR - Production Deploy

```bash
# Docker (easiest)
cp .env.example .env
# Edit .env with your tokens
docker-compose up -d

# Or VPS with systemd
./scripts/deploy-systemd.sh
```

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

## Skillshare Setup (Project + Global Bridge)

This repo includes project-mode skillshare config in `.skillshare/config.yaml`.

- Project source of truth: `.skillshare/skills/`
- Project targets: `.claude/skills`, `.agents/skills` (codex), `.crush/skills`, `.gemini/skills`
- Global bridge target: `~/.config/skillshare/skills` (copy mode)

Commands:

```bash
# Install skillshare if needed
curl -fsSL https://raw.githubusercontent.com/runkids/skillshare/main/install.sh | sh

# From repository root, preview then apply project sync
skillshare sync -p --dry-run
skillshare sync -p
```

Notes:

- Project mode and global mode are separate configs.
- This repo is configured so project sync also copies skills into the global skillshare directory.
- Treat `.skillshare/skills/` as source of truth to avoid drift.

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

## MCP Auth and Rate Limits

`quotey-mcp` supports API-key auth and request throttling via environment variables:

- `MCP_API_KEY`: enable single-key mode.
- `MCP_API_KEYS`: JSON array for multi-key mode, for example:
  `[{\"key\":\"agent-key\",\"name\":\"Agent\",\"requests_per_minute\":120}]`
- `MCP_RATE_LIMIT_WINDOW_SECS`: auth rate-limit window in seconds (default: `60`).
- `MCP_DEFAULT_REQUESTS_PER_MINUTE`: default RPM for `MCP_API_KEY` single-key mode (default: `60`).

Tool calls can supply keys through MCP request metadata using:

- `_meta.api_key`
- `_meta.x-api-key` (or `_meta.x_api_key`)
- `_meta.authorization` (`Bearer <token>`, `ApiKey <token>`, `Token <token>`, or raw key)

MCP auth denial payloads now include:

- `code`: canonical snake_case auth code from shared auth taxonomy.
  Current MCP auth denials emit: `missing_credential`, `invalid_credential`, `credential_revoked`, `rate_limited`.
- `http_status`: deterministic transport status mapping
- `retry_after`: included for rate-limited responses
- `error_code`: legacy compatibility field (`AUTHENTICATION_FAILED` or `RATE_LIMIT_EXCEEDED`)

MCP invocation audit events (`mcp.<tool>.received` / `mcp.<tool>.completed`) now include canonical auth metadata:

- payload `auth`: `channel`, `method`, `strength`, `principal`, optional token fingerprint/session
- metadata keys: `auth_channel`, `auth_method`, `auth_strength`, `auth_principal`
- completion events also include `outcome.auth_code` / `metadata.auth_error_code` when applicable

AI-agent smoke workflow test (catalog -> quote create/price -> approval -> PDF):

```bash
cargo test -p quotey-mcp test_ai_agent_quote_workflow_smoke -- --nocapture
```

## CRM Sync Monitoring API

`quotey-server` exposes CRM sync observability endpoints for monitoring and recovery:

- `GET /api/v1/crm/events` - sync history log with filters (`provider`, `direction`, `status`, `quote_id`)
- `GET /api/v1/crm/events/stats` - aggregate counts by status/provider/direction plus 24h success/failure totals
- `GET /api/v1/crm/events/alerts` - actionable alert feed (failed spikes, stale retries, retry-budget pressure)
- `POST /api/v1/crm/events/{event_id}/retry` - deterministic single-event replay
- `POST /api/v1/crm/sync/batch` and `POST /api/v1/crm/sync/inbound/batch` - bounded batch replay

The alerts feed is designed for Slack/operator surfaces and includes sample event IDs and suggested recovery actions.

Slack operator shortcut:
- `/quotey crm-status [failed=<n> stale=<n> near=<n>]` renders a compact CRM sync alert summary card with deterministic recovery actions.

## Portal Approval Auth

Portal approval actions support explicit auth method metadata (`biometric` or `password`) on
`POST /quote/{token}/approve` and `POST /quote/{token}/reject`.

- Legacy payloads without `authMethod` remain accepted for compatibility.
- Set `QUOTEY_PORTAL_APPROVAL_FALLBACK_PASSWORD` to enforce server-side verification when `authMethod=password`.
- Use `/settings` in the portal to register Face ID / Touch ID on-device and manage fallback credentials.
- Portal approval/rejection audit events now persist canonical auth context metadata (`auth_channel`, `auth_method`, `auth_strength`, `auth_principal`) in `audit_event.metadata_json`.
- Auth failures include canonical `code` values:
  `missing_credential` (required field missing), `invalid_credential` (wrong fallback password),
  `unsupported_method` (unsupported `authMethod`).

## Troubleshooting

### QA Gate Triage (Local + CI)

Use the deterministic quality-gate entrypoint:

```bash
scripts/quality-gates.sh
```

For targeted repro of a failing gate:

```bash
scripts/quality-gates.sh <build|fmt|clippy|tests|deny|doc>
```

Detailed local+CI triage workflow is documented in:

- `.planning/qa/QA_TRIAGE_RUNBOOK.md`
- `.planning/FOUNDATION_QUALITY_GATES.md`

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
