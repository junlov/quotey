# AI Agent Quick Reference

One-page reference for integrating Quotey with Pi Claude, Codex, or custom agents.

---

## 🎯 Three Ways to Connect

| Method | Best For | Complexity |
|--------|----------|------------|
| **CLI Commands** | Universal, quick start | Low |
| **MCP Server** | Native tool calling | Medium |
| **REST API** | Custom integrations | Medium |

---

## 📋 Essential CLI Commands

```bash
# Setup (run once)
./scripts/setup-local.sh

# Quotes
./target/release/quotey quote create --account="Acme" --product="Pro" --quantity=10
cargo run -p quotey-cli -- quote list --format=json
cargo run -p quotey-cli -- quote get Q-2026-0001 --format=json

# Catalog
cargo run -p quotey-cli -- catalog search "enterprise" --format=json

# System
cargo run -p quotey-cli -- doctor
cargo run -p quotey-cli -- smoke
```

---

## 🔌 MCP Server (Recommended)

### Start Server

```bash
# Local (stdio - for Claude Desktop)
./target/release/quotey-mcp

# Remote (SSE - for cloud agents)
./target/release/quotey-mcp --transport=sse --port=3001
```

### Claude Desktop Config

```json
{
  "mcpServers": {
    "quotey": {
      "command": "/path/to/quotey/target/release/quotey-mcp",
      "env": {
        "QUOTEY_CONFIG": "/path/to/quotey/config/quotey.headless.toml"
      }
    }
  }
}
```

### Available Tools

- `catalog_search` - Find products
- `catalog_get` - Get product details
- `quote_create` - Create quote
- `quote_get` - Get quote details
- `quote_price` - Calculate pricing
- `quote_list` - List quotes
- `approval_request` - Submit for approval
- `approval_status` - Check status
- `quote_pdf` - Generate PDF

---

## 🐳 Docker VPS Deploy

```bash
# 1. On your VPS
git clone <repo> /opt/quotey && cd /opt/quotey

# 2. Deploy
sudo ./scripts/deploy-headless-vps.sh

# 3. Agent connects to:
#    MCP: http://vps-ip:3001
#    API: http://vps-ip:8080
```

---

## 🤖 Agent Prompts

**Create a quote:**
```
Create a quote for [Customer Name] with [Product], [Quantity] seats, [Term] months.
Return the quote ID and total price.
```

**Check catalog:**
```
What products are available? Search for "enterprise" plans.
```

**Approval workflow:**
```
Check if quote Q-2026-0042 can be approved. What's blocking it?
```

**Batch processing:**
```
Process this list of quote requests from the CSV and return all quote IDs.
```

---

## 🔒 Security Quick Setup

```bash
# Create agent user with restricted SSH
useradd -m -s /bin/bash quotey-agent

# Add SSH key (restrict to quotey commands)
echo 'restrict,command="/opt/quotey/bin/agent-ssh-wrapper" ssh-rsa AAA...' \
  > /home/quotey-agent/.ssh/authorized_keys

# Agent connects via:
# ssh quotey-agent@vps-ip "quote list --format=json"
```

---

## 🐛 Quick Troubleshooting

| Issue | Fix |
|-------|-----|
| "Database not found" | Run `cargo run -p quotey-cli -- migrate` |
| "Ollama not responding" | Check `curl http://localhost:11434/api/tags` |
| "Permission denied" | Check file ownership: `chown -R $USER:$USER quotey.db` |
| "Port already in use" | Change port in config or kill existing process |

---

## 📊 JSON Output Format

All CLI commands support `--format=json`:

```json
{
  "quote_id": "Q-2026-0001",
  "account": "Acme Corp",
  "status": "draft",
  "total": 15000.00,
  "line_items": [
    {
      "product": "Pro Plan",
      "quantity": 10,
      "unit_price": 1500.00,
      "total": 15000.00
    }
  ],
  "approval_status": "not_required"
}
```

Parse with: `jq '.quote_id'`

---

## 🔗 Files to Know

| File | Purpose |
|------|---------|
| `AI_AGENT_INTEGRATION.md` | Full integration guide |
| `docker-compose.headless.yml` | VPS deployment |
| `config/quotey.headless.toml` | Headless config |
| `scripts/deploy-headless-vps.sh` | One-command VPS deploy |
| `scripts/agent-ssh-wrapper.sh` | SSH access control |
