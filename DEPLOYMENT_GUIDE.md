# Quotey Setup & Deployment Guide

A complete guide to running Quotey on your personal computer and deploying to production servers.

---

## Table of Contents

1. [Local Development Setup](#local-development-setup)
2. [Production Deployment Options](#production-deployment-options)
3. [Slack App Configuration](#slack-app-configuration)
4. [LLM Configuration](#llm-configuration)
5. [Troubleshooting](#troubleshooting)

---

## Local Development Setup

### Prerequisites

| Requirement | Version | Installation |
|-------------|---------|--------------|
| **Rust** | 1.75+ | [rustup.rs](https://rustup.rs) |
| **SQLite** | 3.x | Usually included with Rust |
| **Git** | Any | [git-scm.com](https://git-scm.com) |

**Optional (for development tools):**
- `cargo-sqlx` - Database migrations
- `cargo-nextest` - Better test runner
- `cargo-deny` - Security audit
- `just` - Task runner (like Make)

### Step 1: Install Rust

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify
rustc --version  # Should be 1.75.0 or later
cargo --version
```

### Step 2: Clone and Build

```bash
# Clone the repository
git clone <your-repo-url> quotey
cd quotey

# Build the project (this takes a few minutes the first time)
cargo build --release
```

### Step 3: Create Your Configuration

Quotey needs Slack tokens to connect to your Slack workspace. Here's how to set it up:

#### Option A: Using Environment Variables (Simplest for Local)

```bash
# Create a .env file
cp .env.example .env

# Edit .env with your favorite editor
nano .env  # or vim, code, etc.
```

Fill in your Slack tokens (see [Slack App Configuration](#slack-app-configuration) below):

```bash
QUOTEY_SLACK_APP_TOKEN=xapp-your-app-token-here
QUOTEY_SLACK_BOT_TOKEN=xoxb-your-bot-token-here
QUOTEY_DATABASE_URL=sqlite://quotey.db
QUOTEY_LLM_PROVIDER=ollama
```

Then load it:
```bash
source .env
```

#### Option B: Using Config File

```bash
# Copy the example config
cp config/quotey.example.toml config/quotey.toml

# Edit with your settings
nano config/quotey.toml
```

### Step 4: Set Up the Database

```bash
# Run migrations
cargo run -p quotey-cli -- migrate

# (Optional) Seed with demo data for testing
cargo run -p quotey-cli -- seed
```

### Step 5: Start the Server

```bash
# Run the server
cargo run -p quotey-server

# Or if you built with --release
./target/release/quotey-server
```

You should see:
```
INFO  quotey_server::bootstrap > Starting Quotey server v0.1.2
INFO  quotey_server::bootstrap > Configuration loaded
INFO  quotey_server::slack      > Connecting to Slack Socket Mode...
INFO  quotey_server::slack      > Slack connection established
INFO  quotey_server::health     > Health check server listening on 127.0.0.1:8080
```

### Step 6: Test in Slack

In any channel in your Slack workspace, type:
```
/quote new for Acme Corp, Pro Plan, 50 seats, 12 months
```

You should see a response from the Quotey bot!

---

## Production Deployment Options

Since Quotey uses **Slack Socket Mode**, you don't need a public-facing URL or static IP. The bot connects outbound to Slack's servers, making deployment much simpler.

### Option 1: VPS/Cloud Server (Recommended)

Best for: Production, always-on bot

**Providers:** DigitalOcean, Linode, AWS EC2, Google Cloud, Azure, Hetzner

**Recommended specs:**
- 1-2 vCPUs
- 1-2 GB RAM
- 10-20 GB SSD storage
- Ubuntu 22.04 LTS or Debian 12

#### Deployment Steps:

1. **Provision a server** and SSH into it

2. **Install Rust:**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

3. **Clone and build:**
```bash
git clone <your-repo-url> quotey
cd quotey
cargo build --release
```

4. **Create config directory:**
```bash
sudo mkdir -p /etc/quotey
sudo nano /etc/quotey/quotey.toml
```

5. **Create systemd service** (so it runs automatically):

```bash
sudo nano /etc/systemd/system/quotey.service
```

Paste this (replace `your-user` with your actual username):
```ini
[Unit]
Description=Quotey Slack Bot
After=network.target

[Service]
Type=simple
User=your-user
WorkingDirectory=/home/your-user/quotey
Environment=QUOTEY_CONFIG=/etc/quotey/quotey.toml
Environment=RUST_LOG=info
ExecStart=/home/your-user/quotey/target/release/quotey-server
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

6. **Start the service:**
```bash
sudo systemctl daemon-reload
sudo systemctl enable quotey
sudo systemctl start quotey

# Check status
sudo systemctl status quotey

# View logs
sudo journalctl -u quotey -f
```

### Option 2: Docker Deployment

Best for: Containerized environments, easy scaling

**Create a Dockerfile:**

```dockerfile
# Dockerfile
FROM rust:1.75-slim as builder

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/quotey-server /usr/local/bin/
COPY --from=builder /app/target/release/quotey /usr/local/bin/
COPY config/ config/

ENV QUOTEY_CONFIG=/app/config/quotey.toml

EXPOSE 8080

CMD ["quotey-server"]
```

**Build and run:**
```bash
# Build the image
docker build -t quotey:latest .

# Run with environment variables
docker run -d \
  --name quotey \
  -e QUOTEY_SLACK_APP_TOKEN=xapp-your-token \
  -e QUOTEY_SLACK_BOT_TOKEN=xoxb-your-token \
  -e QUOTEY_DATABASE_URL=sqlite:///data/quotey.db \
  -v quotey-data:/data \
  -p 8080:8080 \
  quotey:latest
```

**Using Docker Compose:**

```yaml
# docker-compose.yml
version: '3.8'

services:
  quotey:
    build: .
    container_name: quotey
    restart: unless-stopped
    environment:
      - QUOTEY_SLACK_APP_TOKEN=${SLACK_APP_TOKEN}
      - QUOTEY_SLACK_BOT_TOKEN=${SLACK_BOT_TOKEN}
      - QUOTEY_DATABASE_URL=sqlite:///data/quotey.db
      - QUOTEY_LLM_PROVIDER=ollama
      - QUOTEY_LLM_BASE_URL=http://ollama:11434
    volumes:
      - ./data:/data
    ports:
      - "8080:8080"
    depends_on:
      - ollama

  ollama:
    image: ollama/ollama:latest
    container_name: ollama
    volumes:
      - ollama-data:/root/.ollama
    # Uncomment for GPU support:
    # deploy:
    #   resources:
    #     reservations:
    #       devices:
    #         - driver: nvidia
    #           count: 1
    #           capabilities: [gpu]

volumes:
  ollama-data:
```

Run with:
```bash
docker-compose up -d
```

### Option 3: Running on Your Personal Computer (Always-On)

Best for: Small teams, testing, or if you have a computer that's always on

**Using `screen` or `tmux` (simple but not auto-restarting):**

```bash
# Install screen
sudo apt-get install screen  # Ubuntu/Debian
brew install screen          # macOS

# Start a new screen session
screen -S quotey

# Run the server
cd ~/quotey
source .env
cargo run -p quotey-server

# Detach: Press Ctrl+A, then D
# Reattach: screen -r quotey
```

**Using PM2 (Node.js process manager, works great for any process):**

```bash
# Install PM2
npm install -g pm2

# Create ecosystem file
pm2 init
```

Edit `ecosystem.config.js`:
```javascript
module.exports = {
  apps: [{
    name: 'quotey',
    cwd: '/home/your-user/quotey',
    script: './target/release/quotey-server',
    env: {
      QUOTEY_CONFIG: '/home/your-user/quotey/config/quotey.toml',
      RUST_LOG: 'info'
    },
    autorestart: true,
    max_restarts: 5,
    min_uptime: '10s'
  }]
};
```

```bash
pm2 start ecosystem.config.js
pm2 save
pm2 startup  # Configure to start on boot
```

---

## Slack App Configuration

### 1. Create Your Slack App

1. Go to [https://api.slack.com/apps](https://api.slack.com/apps)
2. Click **"Create New App"** → **"From scratch"**
3. Name it "Quotey" and select your workspace

### 2. Enable Socket Mode

1. In the left sidebar, click **"Socket Mode"**
2. Toggle **"Enable Socket Mode"** to On
3. Generate an app-level token with scope: `connections:write`
4. **Copy this token** — it starts with `xapp-` (this is your `SLACK_APP_TOKEN`)

### 3. Add Bot Token Scopes

1. Go to **"OAuth & Permissions"** in the sidebar
2. Scroll to **"Scopes"** → **"Bot Token Scopes"**
3. Add these scopes:
   - `app_mentions:read` — Detect @mentions
   - `channels:history` — Read channel messages
   - `chat:write` — Send messages
   - `files:write` — Upload PDFs
   - `groups:history` — Read private channel messages
   - `im:history` — Read direct messages
   - `mpim:history` — Read group direct messages
   - `reactions:write` — Add reactions
   - `users:read` — Get user info

### 4. Install to Workspace

1. Click **"Install to Workspace"**
2. Authorize the permissions
3. **Copy the "Bot User OAuth Token"** — it starts with `xoxb-` (this is your `SLACK_BOT_TOKEN`)

### 5. Subscribe to Events

1. Go to **"Event Subscriptions"**
2. Toggle **"Enable Events"** to On
3. Subscribe to these **bot events**:
   - `message.channels`
   - `message.groups`
   - `message.im`
   - `message.mpim`
   - `app_mention`

### 6. Add Slash Commands

1. Go to **"Slash Commands"**
2. Click **"Create New Command"**

Create these commands:

| Command | Description |
|---------|-------------|
| `/quote` | Create and manage quotes |
| `/quote-status` | Check quote status |
| `/quote-list` | List your quotes |

For each command:
- **Request URL**: Leave blank (Socket Mode handles this)
- **Short Description**: Brief description
- **Usage Hint**: `[new|status|list]`

---

## LLM Configuration

Quotey needs an LLM for natural language understanding. You have three options:

### Option 1: Ollama (Local - Free, Private)

**Best for:** Privacy, no API costs, offline capability

```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Pull a model (llama3.1 is recommended)
ollama pull llama3.1

# Start Ollama server
ollama serve
```

**Configuration:**
```toml
[llm]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"
timeout_secs = 30
max_retries = 2
```

### Option 2: OpenAI

**Best for:** Best performance, reliable

```toml
[llm]
provider = "openai"
base_url = "https://api.openai.com/v1"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"
timeout_secs = 30
max_retries = 2
```

Set environment variable:
```bash
export OPENAI_API_KEY="sk-your-key-here"
```

### Option 3: Anthropic (Claude)

**Best for:** High-quality reasoning

```toml
[llm]
provider = "anthropic"
base_url = "https://api.anthropic.com"
model = "claude-3-5-sonnet-20241022"
api_key = "${ANTHROPIC_API_KEY}"
timeout_secs = 30
max_retries = 2
```

---

## Troubleshooting

### "slack.app_token must start with xapp-"

You've swapped the tokens:
- `SLACK_APP_TOKEN` should start with `xapp-` (from Socket Mode)
- `SLACK_BOT_TOKEN` should start with `xoxb-` (from OAuth)

### "unable to open database file"

The database directory doesn't exist or isn't writable:
```bash
# Create directory if needed
mkdir -p $(dirname /path/to/quotey.db)

# Fix permissions
chmod 755 /path/to/parent-directory
```

### Bot doesn't respond in Slack

1. Check logs: `sudo journalctl -u quotey -f`
2. Verify tokens are correct
3. Ensure the bot is invited to the channel
4. Check that Event Subscriptions are enabled

### "configuration validation failed: llm.api_key is required"

If using OpenAI/Anthropic, you need an API key:
```bash
export OPENAI_API_KEY="sk-your-key"
```

Or switch to Ollama (no key needed):
```bash
export QUOTEY_LLM_PROVIDER="ollama"
```

### Server crashes on startup

1. Run the doctor command:
```bash
cargo run -p quotey-cli -- doctor
```

2. Check configuration:
```bash
cargo run -p quotey-cli -- config
```

### Health check fails

Test the health endpoint:
```bash
curl http://localhost:8080/health
```

Should return: `{"status":"healthy"}`

---

## Quick Reference Commands

```bash
# Build release binary
cargo build --release

# Run migrations
cargo run -p quotey-cli -- migrate

# Seed demo data
cargo run -p quotey-cli -- seed

# Check configuration
cargo run -p quotey-cli -- config

# Run diagnostics
cargo run -p quotey-cli -- doctor

# Generate a quote PDF
cargo run -p quotey-cli -- quote pdf Q-2026-0001

# Run tests
just test
# or
cargo test

# Run quality gates (same as CI)
just quality
# or
./scripts/quality-gates.sh
```

---

## Security Checklist

Before going to production:

- [ ] Change default config paths
- [ ] Use environment variables for secrets (don't commit tokens)
- [ ] Restrict config file permissions: `chmod 600 config/quotey.toml`
- [ ] Enable firewall (allow only necessary ports)
- [ ] Set up log rotation: `sudo logrotate`
- [ ] Configure backups for the SQLite database
- [ ] Use HTTPS for health check endpoint (if exposed publicly)
- [ ] Review and tighten Slack app scopes

---

## Need Help?

- Check the logs: `sudo journalctl -u quotey -f`
- Run diagnostics: `cargo run -p quotey-cli -- doctor`
- Enable debug logging: `RUST_LOG=debug cargo run -p quotey-server`
- Review documentation in `docs/`
