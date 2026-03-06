# Getting Started

This guide will help you get Quotey up and running locally in minutes.

## Prerequisites

Before you begin, ensure you have the following installed:

| Requirement | Version | Purpose |
|-------------|---------|---------|
| **Rust** | 1.75+ | Core language runtime |
| **SQLite** | 3.x | Local database (usually included with Rust) |
| **cargo-sqlx** | latest | Database migrations |
| **cargo-nextest** | latest | Better test runner |
| **cargo-deny** | latest | Security audit |

### Installing Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Verify your installation:

```bash
rustc --version  # Should be 1.75.0 or later
cargo --version
```

### Installing Cargo Tools

```bash
# SQLx CLI for database migrations
cargo install sqlx-cli --no-default-features --features sqlite

# Nextest for better test output
cargo install cargo-nextest

# Deny for security auditing
cargo install cargo-deny
```

## Clone and Build

```bash
# Clone the repository
git clone https://github.com/quotey/quotey.git
cd quotey

# Run quality gates to verify everything works
./scripts/quality-gates.sh

# Build the project
cargo build --release
```

The quality gates script checks:
- Code formatting (`cargo fmt`)
- Linting (`cargo clippy`)
- Tests (`cargo test`)
- Security audit (`cargo deny`)

## Configuration

Quotey uses a layered configuration system with the following precedence (highest to lowest):

1. CLI/runtime overrides
2. `QUOTEY_*` environment variables
3. TOML config file
4. Built-in defaults

### Create Your Config File

Copy the example configuration:

```bash
cp config/quotey.example.toml config/quotey.toml
```

Edit `config/quotey.toml` with your settings:

```toml
[database]
url = "sqlite://quotey.db"
max_connections = 5
timeout_secs = 30

[slack]
app_token = "${SLACK_APP_TOKEN}"      # Starts with xapp-
bot_token = "${SLACK_BOT_TOKEN}"      # Starts with xoxb-

[llm]
provider = "ollama"                    # or "openai", "anthropic"
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
format = "pretty"                      # or "json"
```

### Environment Variables

Instead of editing the config file, you can use environment variables:

```bash
export SLACK_APP_TOKEN="xapp-your-app-token"
export SLACK_BOT_TOKEN="xoxb-your-bot-token"
export QUOTEY_DATABASE_URL="sqlite://quotey.db"
export QUOTEY_LLM_PROVIDER="ollama"
```

Environment variables use the `QUOTEY_` prefix and uppercase with underscores. Nested config values use underscores:

| Config Path | Environment Variable |
|-------------|---------------------|
| `database.url` | `QUOTEY_DATABASE_URL` |
| `slack.app_token` | `QUOTEY_SLACK_APP_TOKEN` |
| `llm.provider` | `QUOTEY_LLM_PROVIDER` |

## Database Setup

### Run Migrations

```bash
# Using the CLI
cargo run -p quotey-cli -- migrate

# Or directly with sqlx
sqlx migrate run --database-url sqlite://quotey.db
```

### Seed Demo Data (Optional)

For development and testing, you can load demo fixtures:

```bash
# Seed the database with demo products, price books, and accounts
cargo run -p quotey-cli -- seed

# Or use the E2E bootstrap script
./scripts/e2e_bootstrap.sh
```

The E2E bootstrap script creates a reproducible database for testing:

```bash
# Creates a fresh database with demo data
CLEAN_BEFORE_BOOTSTRAP=1 ./scripts/e2e_bootstrap.sh

# Use a custom database path
QUOTEY_E2E_DB_PATH=/path/to/my.db ./scripts/e2e_bootstrap.sh
```

## Slack App Setup

To use Quotey in Slack, you need to create a Slack app:

### 1. Create a New App

1. Go to [https://api.slack.com/apps](https://api.slack.com/apps)
2. Click "Create New App" → "From scratch"
3. Name it "Quotey" and select your workspace

### 2. Enable Socket Mode

1. Go to "Socket Mode" in the left sidebar
2. Toggle "Enable Socket Mode" to On
3. Generate an app-level token with `connections:write` scope
4. Save this token — it's your `SLACK_APP_TOKEN` (starts with `xapp-`)

### 3. Add Bot Token Scopes

1. Go to "OAuth & Permissions"
2. Under "Scopes" → "Bot Token Scopes", add:
   - `app_mentions:read`
   - `channels:history`
   - `chat:write`
   - `files:write`
   - `groups:history`
   - `im:history`
   - `mpim:history`
   - `reactions:write`
   - `users:read`

### 4. Install to Workspace

1. Click "Install to Workspace"
2. Authorize the permissions
3. Copy the "Bot User OAuth Token" — it's your `SLACK_BOT_TOKEN` (starts with `xoxb-`)

### 5. Subscribe to Events

1. Go to "Event Subscriptions"
2. Enable events
3. Subscribe to bot events:
   - `message.channels`
   - `message.groups`
   - `message.im`
   - `message.mpim`

### 6. Add Slash Commands

Go to "Slash Commands" and add:

| Command | Request URL | Description |
|---------|-------------|-------------|
| `/quote` | (Socket Mode handles this) | Create and manage quotes |
| `/quote-status` | (Socket Mode handles this) | Check quote status |
| `/quote-list` | (Socket Mode handles this) | List your quotes |

## Running Quotey

### Start the Server

```bash
# Using cargo
cargo run -p quotey-server

# Or if you built with --release
./target/release/quotey-server
```

You should see output like:

```
INFO  quotey_server::bootstrap > Starting Quotey server v0.1.2
INFO  quotey_server::bootstrap > Configuration loaded from: config/quotey.toml
INFO  quotey_server::bootstrap > Database connected: sqlite://quotey.db
INFO  quotey_server::slack      > Connecting to Slack Socket Mode...
INFO  quotey_server::slack      > Slack connection established
INFO  quotey_server::health     > Health check server listening on 127.0.0.1:8080
```

### Verify It's Working

1. In Slack, type `/quote new test` in any channel
2. You should see a response from the Quotey bot
3. Check the logs for any errors

## Your First Quote

Let's create your first quote to verify everything works:

```
/quote new for Acme Corp, Pro Plan, 50 seats, 12 months
```

The bot will respond with:
- A quote number (e.g., `Q-2026-0001`)
- Line items with pricing
- Missing fields that need to be filled
- Action buttons to continue

Follow the prompts to complete the quote and generate a PDF.

## Troubleshooting

### "slack.app_token must start with xapp-"

You've swapped the tokens. Ensure:
- `SLACK_APP_TOKEN` starts with `xapp-` (App-Level Token from Socket Mode)
- `SLACK_BOT_TOKEN` starts with `xoxb-` (Bot User OAuth Token)

### "unable to open database file"

The database directory may not exist or be writable:

```bash
# Create the directory if using a relative path
mkdir -p $(dirname quotey.db)

# Or use an absolute path
export QUOTEY_DATABASE_URL="sqlite:///absolute/path/to/quotey.db"
```

### "configuration validation failed: llm.api_key is required"

OpenAI/Anthropic providers require an API key:

```bash
# Set the API key
export QUOTEY_LLM_API_KEY="sk-..."

# Or switch to Ollama (local, no API key needed)
export QUOTEY_LLM_PROVIDER="ollama"
```

### Debug Logging

Enable debug logging to see more details:

```bash
RUST_LOG=debug cargo run -p quotey-server

# Or for specific modules
RUST_LOG=quotey_slack=trace,quotey_agent=debug cargo run -p quotey-server
```

### Check Database State

```bash
# Using the CLI
cargo run -p quotey-cli -- config

# Or query directly
sqlite3 quotey.db ".schema"
```

## Next Steps

- [Quick Start Guide](./quick-start-guide) — Learn the core workflows
- [Key Concepts](./key-concepts) — Understand Quotey's fundamental ideas
- [Architecture Overview](../architecture/overview) — Dive into how it works
