# Quotey Setup Summary for Your Use Case

**Your Requirements:**
- ✅ Local testing on personal computer
- ✅ VPS deployment for production
- ✅ Docker support
- ✅ Slack workspace (you'll create)
- ✅ Pi Claude / Codex as AI agent runtime
- ✅ Headless CLI commands
- ✅ AI agent agnostic

---

## 📁 New Files Created

### Documentation
| File | Purpose |
|------|---------|
| `QUICK_START.md` | One-page reference for getting started |
| `DEPLOYMENT_GUIDE.md` | Complete local + VPS + Docker guide |
| `DEPLOYMENT_ARCHITECTURE.md` | Visual architecture & security |
| `AI_AGENT_INTEGRATION.md` | Headless AI agent integration |
| `AGENT_QUICK_REFERENCE.md` | One-page agent commands |
| `SETUP_SUMMARY.md` | This file - your roadmap |

### Deployment Files
| File | Purpose |
|------|---------|
| `Dockerfile` | Multi-stage build for Docker |
| `docker-compose.yml` | Full stack with Slack + Ollama |
| `docker-compose.headless.yml` | Headless mode (no Slack) |
| `config/quotey.headless.toml` | Headless configuration template |

### Scripts
| File | Purpose |
|------|---------|
| `scripts/setup-local.sh` | Automated local setup |
| `scripts/deploy-systemd.sh` | VPS deployment with systemd |
| `scripts/deploy-headless-vps.sh` | One-command headless VPS deploy |
| `scripts/agent-ssh-wrapper.sh` | Secure SSH agent access |

---

## 🛠️ Step-by-Step Setup Plan

### Phase 1: Local Testing (Your Computer)

```bash
# 1. Clone repo
git clone <your-repo-url> quotey
cd quotey

# 2. Run automated setup
./scripts/setup-local.sh
# This installs Rust, builds, creates configs

# 3. Create headless config for AI testing
cp config/quotey.headless.toml config/quotey.local.toml

# 4. Run migrations
QUOTEY_CONFIG=config/quotey.local.toml cargo run -p quotey-cli -- migrate

# 5. Seed demo data
cargo run -p quotey-cli -- seed

# 6. Test CLI
./target/release/quotey quote list --format=json
./target/release/quotey catalog search "pro" --format=json
```

**Result:** Quotey running locally, accessible via CLI

---

### Phase 2: AI Agent Integration (Local)

Option A: **CLI Commands** (Universal, any agent)
```python
# Your agent calls:
result = run_shell("./target/release/quotey quote create --account='Test' --product='Pro' --quantity=10 --format=json")
quote = json.loads(result)
```

Option B: **MCP Server** (Native tool calling)
```bash
# Start MCP server
./target/release/quotey-mcp
```

Configure Pi Claude / Codex with MCP:
```json
{
  "mcpServers": {
    "quotey": {
      "command": "/path/to/quotey/target/release/quotey-mcp",
      "env": {
        "QUOTEY_CONFIG": "/path/to/quotey/config/quotey.local.toml"
      }
    }
  }
}
```

**Result:** AI agent can create quotes, search catalog, check approvals

---

### Phase 3: Create Slack Workspace

1. Go to https://slack.com/create
2. Create a new workspace (free)
3. Go to https://api.slack.com/apps
4. Create New App → From scratch
5. Follow `DEPLOYMENT_GUIDE.md` "Slack App Configuration" section

**Key steps:**
- Enable Socket Mode → save `xapp-` token
- Add Bot Token Scopes → install → save `xoxb-` token
- Subscribe to bot events

---

### Phase 4: VPS Production Deploy

```bash
# On your VPS (Ubuntu/Debian)

# Option 1: Headless (AI agents only)
curl -fsSL https://raw.githubusercontent.com/youruser/quotey/main/scripts/deploy-headless-vps.sh | sudo bash

# Option 2: Full (Slack bot + AI agents)
git clone <repo> /opt/quotey
cd /opt/quotey
sudo ./scripts/deploy-systemd.sh
```

**Docker alternative:**
```bash
# On VPS
git clone <repo> /opt/quotey
cd /opt/quotey

# Copy and edit environment
cp .env.example .env
nano .env  # Add your tokens

# Start
docker-compose up -d
```

**Result:** 24/7 running service accessible via:
- SSH + CLI
- MCP over HTTP (if headless)
- Slack (if configured)

---

### Phase 5: Connect Your AI Agent to VPS

**Via SSH (secure, simple):**
```bash
# On VPS, setup restricted SSH access
sudo ./scripts/agent-ssh-wrapper.sh

# From your local machine
ssh quotey-agent@vps-ip "quote list --format=json"
```

**Via MCP (native tool calling):**
```bash
# MCP config for Pi Claude / Codex
{
  "mcpServers": {
    "quotey": {
      "url": "http://vps-ip:3001/sse"
    }
  }
}
```

**Result:** Your AI agent can control Quotey remotely

---

## 🔧 Configuration Files

### Local Development
```
config/quotey.local.toml     # Your local config (gitignored)
.env                         # Environment variables (gitignored)
quotey.db                    # SQLite database
```

### VPS Production
```
/opt/quotey/config/quotey.toml    # Production config
/opt/quotey/.env                  # Production env vars
/opt/quotey/quotey.db             # Production database
/etc/systemd/system/quotey.service # Systemd service
```

---

## 🧪 Testing Checklist

### Local Testing
- [ ] `./scripts/setup-local.sh` completes without errors
- [ ] `cargo run -p quotey-cli -- doctor` shows all green
- [ ] `./target/release/quotey quote list` returns empty array `[]`
- [ ] `./target/release/quotey catalog search "pro"` returns products
- [ ] Can create a quote via CLI

### AI Agent Testing
- [ ] MCP server starts: `./target/release/quotey-mcp`
- [ ] Agent can call `catalog_search` tool
- [ ] Agent can call `quote_create` tool
- [ ] Agent receives JSON responses

### Slack Testing
- [ ] Created Slack workspace
- [ ] Created Slack app
- [ ] Obtained `xapp-` and `xoxb-` tokens
- [ ] Bot responds to `/quote new test` in Slack

### VPS Testing
- [ ] Deployed to VPS
- [ ] `curl http://vps-ip:8080/health` returns `{"status":"healthy"}`
- [ ] Can SSH and run CLI commands
- [ ] (Optional) MCP accessible at `http://vps-ip:3001`

---

## 🚀 Next Actions for You

### Right Now (5 minutes)
1. Run `./scripts/setup-local.sh` on your computer
2. Verify it builds successfully

### Today (30 minutes)
1. Create Slack workspace at https://slack.com/create
2. Create Slack app at https://api.slack.com/apps
3. Get your tokens (save in 1Password/password manager)
4. Add tokens to `config/quotey.toml`
5. Test: `cargo run -p quotey-server` then type `/quote new test` in Slack

### This Week (1-2 hours)
1. Provision a VPS (DigitalOcean, Linode, etc. - $5-10/month)
2. Deploy with `sudo ./scripts/deploy-headless-vps.sh`
3. Configure Pi Claude / Codex with MCP or SSH access
4. Test end-to-end: Agent → VPS → Quote Created

---

## 💡 Pro Tips

1. **Start headless** - You don't need Slack for AI agent testing
2. **Use Ollama locally** - Free, private LLM for development
3. **Keep configs separate** - `quotey.local.toml` vs `quotey.prod.toml`
4. **Backup your database** - SQLite is just a file, easy to backup
5. **Use JSON output** - `--format=json` for easy parsing by agents

---

## 📚 Documentation Index

| Question | Read This |
|----------|-----------|
| "How do I run this locally?" | `QUICK_START.md` |
| "How do I deploy to a server?" | `DEPLOYMENT_GUIDE.md` |
| "How do I use it with Pi Claude?" | `AI_AGENT_INTEGRATION.md` |
| "What are all the CLI commands?" | `docs/docs/api/cli-commands.md` |
| "How does the architecture work?" | `DEPLOYMENT_ARCHITECTURE.md` |
| "Quick command reference?" | `AGENT_QUICK_REFERENCE.md` |

---

## 🆘 Getting Help

If something doesn't work:

1. **Check logs:** `sudo journalctl -u quotey -f` (VPS) or just look at terminal output (local)
2. **Run doctor:** `cargo run -p quotey-cli -- doctor`
3. **Enable debug:** `RUST_LOG=debug cargo run -p quotey-server`
4. **Check health:** `curl http://localhost:8080/health`

---

## ✅ Summary

You now have:
- ✅ Complete documentation for your use case
- ✅ Automated setup scripts
- ✅ Docker deployment configs
- ✅ AI agent integration guides
- ✅ Security hardening (SSH wrapper, API keys)

**Your next step:** Run `./scripts/setup-local.sh` and start testing!
