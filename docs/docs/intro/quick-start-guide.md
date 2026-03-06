# Quick Start Guide

Get up and running with Quotey in 5 minutes.

## Prerequisites

- Rust 1.75+
- SQLite (usually included)
- A Slack workspace where you can create apps

## Step 1: Clone and Build

```bash
git clone https://github.com/quotey/quotey.git
cd quotey
cargo build --release
```

## Step 2: Create Slack App

1. Go to [https://api.slack.com/apps](https://api.slack.com/apps)
2. Click "Create New App" → "From scratch"
3. Name it "Quotey" and select your workspace

### Enable Socket Mode

1. Go to "Socket Mode" in the left sidebar
2. Toggle "Enable Socket Mode" to On
3. Generate an app-level token with `connections:write` scope
4. **Save this token** — you'll need it (starts with `xapp-`)

### Add Bot Token Scopes

1. Go to "OAuth & Permissions"
2. Add these Bot Token Scopes:
   - `app_mentions:read`
   - `chat:write`
   - `files:write`
   - `reactions:write`
   - `users:read`

3. Click "Install to Workspace"
4. **Copy the Bot User OAuth Token** (starts with `xoxb-`)

## Step 3: Configure

Create a `.env` file:

```bash
export SLACK_APP_TOKEN="xapp-your-token-here"
export SLACK_BOT_TOKEN="xoxb-your-token-here"
export QUOTEY_DATABASE_URL="sqlite://quotey.db"
export QUOTEY_LLM_PROVIDER="ollama"
```

Or copy and edit the config:

```bash
cp config/quotey.example.toml config/quotey.toml
# Edit config/quotey.toml with your tokens
```

## Step 4: Run Migrations

```bash
cargo run -p quotey-cli -- migrate
```

## Step 5: Seed Demo Data

```bash
cargo run -p quotey-cli -- seed
```

This loads sample products, price books, and accounts.

## Step 6: Start the Server

```bash
# Option 1: Using cargo
cargo run -p quotey-server

# Option 2: Using the built binary
./target/release/quotey-server
```

You should see:
```
INFO  quotey_server::bootstrap > Starting Quotey server v0.1.2
INFO  quotey_server::slack      > Slack connection established
INFO  quotey_server::health     > Health check on :8080
```

## Step 7: Test in Slack

In any channel in your Slack workspace:

```
/quote new for Acme Corp, Pro Plan, 50 seats, 12 months
```

You should see a response from the Quotey bot with:
- A quote number (e.g., `Q-2026-0001`)
- Line items with pricing
- Missing fields
- Action buttons

## Common Commands

### Check Server Health

```bash
curl http://localhost:8080/health
```

### View Configuration

```bash
cargo run -p quotey-cli -- config
```

### Run Diagnostics

```bash
cargo run -p quotey-cli -- doctor
```

### Generate a Quote PDF

```bash
cargo run -p quotey-cli -- quote pdf Q-2026-0001
```

## Next Steps

- [Configuration Guide](../guides/configuration) — Customize your setup
- [Slack Setup](../guides/slack-setup) — Advanced Slack configuration
- [Key Concepts](./key-concepts) — Understand how Quotey works
