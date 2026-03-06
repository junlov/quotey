# Configuration Guide

Quotey uses a layered configuration system that allows flexibility while maintaining sensible defaults.

## Configuration Precedence

Configuration values are resolved in this order (highest to lowest):

1. **CLI arguments** — Runtime overrides
2. **Environment variables** — `QUOTEY_*` prefixed vars
3. **Config file** — TOML file at specified path
4. **Built-in defaults** — Hardcoded fallbacks

## Configuration File

### Location

By default, Quotey looks for configuration at:
- `config/quotey.toml` (relative to working directory)
- Or path specified by `--config` flag
- Or path in `QUOTEY_CONFIG` environment variable

### Example Configuration

```toml
# quotey.toml — Main Quotey Configuration

[database]
# SQLite connection string
url = "sqlite://quotey.db"

# Connection pool settings
max_connections = 5
timeout_secs = 30

[slack]
# App-Level Token (starts with xapp-)
# Get this from Slack API > Your App > Socket Mode
app_token = "${SLACK_APP_TOKEN}"

# Bot User OAuth Token (starts with xoxb-)
# Get this from Slack API > Your App > OAuth & Permissions
bot_token = "${SLACK_BOT_TOKEN}"

# Default channel for notifications (optional)
default_channel = "#quotes"

[llm]
# Provider: "openai", "anthropic", or "ollama"
provider = "ollama"

# Base URL for the LLM API
# For Ollama: http://localhost:11434
# For OpenAI: https://api.openai.com/v1
base_url = "http://localhost:11434"

# Model name
# For Ollama: llama3.1, mistral, etc.
# For OpenAI: gpt-4o, gpt-4o-mini, etc.
# For Anthropic: claude-3-5-sonnet-20241022, etc.
model = "llama3.1"

# Request timeout in seconds
timeout_secs = 30

# Maximum retries for failed requests
max_retries = 2

# API key (required for OpenAI/Anthropic, not for Ollama)
# api_key = "${OPENAI_API_KEY}"

[server]
# Bind address for health check server
bind_address = "127.0.0.1"

# Port for health check endpoint
health_check_port = 8080

# Graceful shutdown timeout
graceful_shutdown_secs = 15

[logging]
# Log level: trace, debug, info, warn, error
level = "info"

# Log format: pretty, compact, json
format = "pretty"

# Optional: log to file instead of stdout
# file = "/var/log/quotey.log"

[features]
# Enable/disable specific features
enable_catalog_bootstrap = true
enable_quote_intelligence = true
enable_approval_workflow = true
enable_pdf_generation = true

[crm]
# CRM provider: "stub" or "composio"
provider = "stub"

[crm.stub]
# Path to CSV fixtures
fixtures_path = "config/demo_fixtures"

[crm.composio]
# Composio API key
api_key = "${COMPOSIO_API_KEY}"

# Default CRM integration
integration = "hubspot"  # or "salesforce"
```

## Environment Variables

All configuration values can be overridden via environment variables. The naming convention is:

1. Prefix with `QUOTEY_`
2. Use uppercase
3. Use underscores for nesting

### Examples

| Config Path | Environment Variable | Example Value |
|-------------|---------------------|---------------|
| `database.url` | `QUOTEY_DATABASE_URL` | `sqlite://quotey.db` |
| `slack.app_token` | `QUOTEY_SLACK_APP_TOKEN` | `xapp-1-...` |
| `slack.bot_token` | `QUOTEY_SLACK_BOT_TOKEN` | `xoxb-...` |
| `llm.provider` | `QUOTEY_LLM_PROVIDER` | `ollama` |
| `llm.api_key` | `QUOTEY_LLM_API_KEY` | `sk-...` |
| `logging.level` | `QUOTEY_LOG_LEVEL` | `debug` |

### Environment Variable Interpolation

You can use `${VAR_NAME}` syntax in the config file to reference environment variables:

```toml
[slack]
app_token = "${SLACK_APP_TOKEN}"
bot_token = "${SLACK_BOT_TOKEN}"

[llm]
api_key = "${OPENAI_API_KEY}"
```

This is useful for:
- Keeping secrets out of config files
- Different values per environment
- CI/CD pipelines

## Environment-Specific Configs

You can have multiple configuration files for different environments:

```bash
config/
├── quotey.toml           # Base config (always loaded)
├── quotey.dev.toml       # Development overrides
├── quotey.staging.toml   # Staging overrides
└── quotey.prod.toml      # Production overrides
```

To use a specific environment:

```bash
# Using environment variable
export QUOTEY_ENV=staging
cargo run -p quotey-server

# Or using explicit path
cargo run -p quotey-server -- --config config/quotey.prod.toml
```

## Configuration Validation

Quotey validates configuration on startup and fails fast if there are problems:

### Validation Rules

| Field | Validation |
|-------|-----------|
| `database.url` | Must be valid SQLite URL |
| `slack.app_token` | Must start with `xapp-` |
| `slack.bot_token` | Must start with `xoxb-` |
| `llm.provider` | Must be "openai", "anthropic", or "ollama" |
| `llm.api_key` | Required for OpenAI/Anthropic |
| `server.health_check_port` | Must be valid port number |
| `logging.level` | Must be valid log level |

### Error Messages

If validation fails, you'll see a clear error message:

```
Error: Configuration validation failed

Caused by:
    0: slack.app_token must start with "xapp-"
    1: You may have swapped the tokens:
       - SLACK_APP_TOKEN should start with "xapp-" (from Socket Mode)
       - SLACK_BOT_TOKEN should start with "xoxb-" (from OAuth)
       Get these from https://api.slack.com/apps
```

## Viewing Current Configuration

To see the effective configuration (after all overrides):

```bash
cargo run -p quotey-cli -- config
```

Output:
```
Quotey Configuration
====================

Database:
  URL: sqlite:///home/user/quotey/quotey.db
  Max Connections: 5

Slack:
  App Token: xapp-**** (masked)
  Bot Token: xoxb-**** (masked)

LLM:
  Provider: ollama
  Base URL: http://localhost:11434
  Model: llama3.1

Server:
  Bind Address: 127.0.0.1
  Health Port: 8080

Logging:
  Level: info
  Format: pretty
```

## LLM Configuration Examples

### Using Ollama (Local)

```toml
[llm]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"
# No API key needed
```

Prerequisites:
```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model
ollama pull llama3.1

# Start Ollama server
ollama serve
```

### Using OpenAI

```toml
[llm]
provider = "openai"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"
```

Environment:
```bash
export OPENAI_API_KEY="sk-..."
```

### Using Anthropic

```toml
[llm]
provider = "anthropic"
base_url = "https://api.anthropic.com"
model = "claude-3-5-sonnet-20241022"
api_key = "${ANTHROPIC_API_KEY}"
```

Environment:
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

## Database Configuration

### Local SQLite File

```toml
[database]
url = "sqlite://quotey.db"
```

Creates `quotey.db` in the current working directory.

### Absolute Path

```toml
[database]
url = "sqlite:///var/lib/quotey/quotey.db"
```

### In-Memory (Testing Only)

```toml
[database]
url = "sqlite::memory:"
```

**Note:** In-memory databases are destroyed when the process exits.

### Connection Pool Tuning

```toml
[database]
url = "sqlite://quotey.db"
max_connections = 10     # Increase for high concurrency
timeout_secs = 60        # Increase for slow disks
```

SQLite handles concurrency via WAL mode, so 5-10 connections is usually sufficient.

## Logging Configuration

### Log Levels

| Level | Description |
|-------|-------------|
| `trace` | Very verbose, includes spans |
| `debug` | Detailed debug information |
| `info` | General operational information |
| `warn` | Warning conditions |
| `error` | Error conditions |

### Per-Module Log Levels

You can set different levels for different modules:

```bash
# Debug for agent, info for everything else
RUST_LOG=quotey_agent=debug,info cargo run -p quotey-server

# Trace for database queries
RUST_LOG=sqlx=trace,quotey=debug cargo run -p quotey-server
```

### Log Formats

**Pretty** (default, development):
```
2026-03-06T10:30:00.123Z  INFO  quotey_server::bootstrap > Starting Quotey server v0.1.2
```

**Compact** (production, single line):
```
2026-03-06T10:30:00.123Z INFO quotey_server::bootstrap: Starting Quotey server v0.1.2
```

**JSON** (machine parsing):
```json
{"timestamp":"2026-03-06T10:30:00.123Z","level":"INFO","target":"quotey_server::bootstrap","message":"Starting Quotey server v0.1.2"}
```

## Security Considerations

### Protecting Secrets

1. **Never commit secrets to git:**
   ```bash
   echo "config/quotey.prod.toml" >> .gitignore
   echo "*.env" >> .gitignore
   ```

2. **Use environment variables for secrets:**
   ```toml
   [slack]
   app_token = "${SLACK_APP_TOKEN}"
   ```

3. **Use a secrets manager in production:**
   ```bash
   # Load from secrets manager
   export SLACK_APP_TOKEN=$(vault read -field=app_token secret/quotey/slack)
   ```

### Configuration File Permissions

Restrict access to config files:

```bash
chmod 600 config/quotey.prod.toml
```

## Troubleshooting

### "unable to open database file"

The directory for the database doesn't exist:

```bash
# Create the directory
mkdir -p $(dirname /path/to/quotey.db)

# Ensure write permissions
chmod 755 /path/to
```

### "slack.app_token must start with xapp-"

You've swapped the tokens:

```bash
# Check your tokens
echo $SLACK_APP_TOKEN  # Should start with xapp-
echo $SLACK_BOT_TOKEN  # Should start with xoxb-

# Fix if swapped
export SLACK_APP_TOKEN="xapp-correct-token"
export SLACK_BOT_TOKEN="xoxb-correct-token"
```

### "configuration validation failed: llm.api_key is required"

OpenAI/Anthropic requires an API key:

```bash
# Set the API key
export OPENAI_API_KEY="sk-your-key"

# Or switch to Ollama (no key needed)
export QUOTEY_LLM_PROVIDER="ollama"
```

## Next Steps

- [Slack Setup](./slack-setup) — Configure your Slack app
- [LLM Configuration](./llm-configuration) — Choose and configure your LLM
- [Database Migrations](./database-migrations) — Manage database schema
