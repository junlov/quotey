# Quotey Quick Start Card

## 🚀 First Time Setup (15 minutes)

### 1. Install Prerequisites
```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 2. Clone & Build
```bash
git clone <your-repo-url> quotey
cd quotey
./scripts/setup-local.sh
```

### 3. Configure Slack App

Go to [api.slack.com/apps](https://api.slack.com/apps):

1. **Create New App** → **From scratch**
2. **Enable Socket Mode** → Generate token (save this - starts with `xapp-`)
3. **OAuth & Permissions** → Add Bot Token Scopes:
   - `app_mentions:read`
   - `chat:write`
   - `files:write`
   - `users:read`
4. **Install to Workspace** → Copy Bot Token (starts with `xoxb-`)

### 4. Add Tokens to Config
```bash
# Edit the config file
nano config/quotey.toml

# Or use environment variables
export SLACK_APP_TOKEN="xapp-your-token"
export SLACK_BOT_TOKEN="xoxb-your-token"
```

### 5. Run Migrations & Start
```bash
# Set up database
cargo run -p quotey-cli -- migrate

# Start the server
cargo run -p quotey-server
```

### 6. Test in Slack
```
/quote new for Acme Corp, Pro Plan, 50 seats, 12 months
```

---

## 🔧 Common Commands

```bash
# Development
cargo run -p quotey-server              # Start server
cargo run -p quotey-cli -- migrate      # Run DB migrations
cargo run -p quotey-cli -- seed         # Load demo data
cargo run -p quotey-cli -- doctor       # Run diagnostics
cargo run -p quotey-cli -- config       # View config

# Build & Test
cargo build --release                    # Build release binary
just test                                # Run tests
just quality                             # Run all quality gates

# Operations (Production)
sudo systemctl start quotey              # Start service
sudo systemctl stop quotey               # Stop service
sudo systemctl status quotey             # Check status
sudo journalctl -u quotey -f             # View logs
curl http://localhost:8080/health        # Health check
```

---

## 🐳 Docker Quick Start

```bash
# Copy and edit environment
cp .env.example .env
# Edit .env with your Slack tokens

# Build and run
docker-compose up -d

# View logs
docker-compose logs -f quotey

# Stop
docker-compose down
```

---

## 🖥️ Production Deployment (VPS)

```bash
# 1. Copy deployment script to server
scp scripts/deploy-systemd.sh user@your-server:/tmp/

# 2. SSH to server and run
ssh user@your-server
cd /opt
git clone <your-repo-url> quotey
cd quotey
./scripts/deploy-systemd.sh

# 3. Edit config with your tokens
sudo nano config/quotey.toml

# 4. Start service
sudo systemctl start quotey

# 5. Check status
sudo systemctl status quotey
```

---

## 🤖 LLM Configuration

### Option 1: Ollama (Local, Free)
```bash
# Install and run
curl -fsSL https://ollama.com/install.sh | sh
ollama pull llama3.1
ollama serve
```

Config:
```toml
[llm]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"
```

### Option 2: OpenAI
```toml
[llm]
provider = "openai"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"
```

---

## 🐛 Troubleshooting

| Problem | Solution |
|---------|----------|
| `slack.app_token must start with xapp-` | You swapped tokens. App token starts with `xapp-`, bot token with `xoxb-` |
| `unable to open database file` | Directory doesn't exist or not writable. Check permissions. |
| `llm.api_key is required` | Set OPENAI_API_KEY or switch to Ollama provider |
| Bot doesn't respond | Check logs: `sudo journalctl -u quotey -f`. Ensure tokens are correct. |
| Port already in use | Change port in config: `health_check_port = 8081` |

---

## 📁 Important Files

| File | Purpose |
|------|---------|
| `config/quotey.toml` | Main configuration |
| `.env` | Environment variables (don't commit!) |
| `quotey.db` | SQLite database |
| `target/release/quotey-server` | Production binary |

---

## 🔗 Documentation

- `DEPLOYMENT_GUIDE.md` - Complete setup & deployment guide
- `DEPLOYMENT_ARCHITECTURE.md` - Architecture diagrams
- `docs/docs/intro/getting-started.md` - Detailed getting started
- `docs/docs/guides/configuration.md` - Configuration reference

---

## 💡 Pro Tips

1. **Always source .env before running locally:**
   ```bash
   source .env && cargo run -p quotey-server
   ```

2. **Use `just` for common tasks:**
   ```bash
   just --list    # See all available commands
   ```

3. **Enable debug logging when troubleshooting:**
   ```bash
   RUST_LOG=debug cargo run -p quotey-server
   ```

4. **Backup your database regularly:**
   ```bash
   sqlite3 quotey.db ".backup 'quotey_backup.db'"
   ```

5. **The health check is your friend:**
   ```bash
   watch -n 5 curl -s http://localhost:8080/health
   ```
