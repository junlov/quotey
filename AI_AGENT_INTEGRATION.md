# AI Agent Integration Guide

Use Quotey headlessly with Pi Claude, Codex, or any AI agent via CLI commands or MCP.

---

## Overview

Quotey supports AI agent integration in two ways:

1. **CLI Commands** - Direct shell commands (simplest, universal)
2. **MCP (Model Context Protocol)** - Structured tool calling (recommended for agents)

Both approaches allow you to:
- Create quotes programmatically
- Check pricing and approvals
- Generate PDFs
- Query the catalog
- Manage the quote lifecycle

**No Slack required** for headless operation!

---

## Quick Start for AI Agents

### 1. Set Up Headless Mode

```bash
# Clone and build
git clone <repo-url> quotey
cd quotey
./scripts/setup-local.sh

# Create headless config (no Slack tokens needed!)
cat > config/quotey.headless.toml << 'EOF'
[database]
url = "sqlite://quotey.db"

[llm]
provider = "ollama"
base_url = "http://localhost:11434"
model = "llama3.1"

[server]
bind_address = "127.0.0.1"
health_check_port = 8080

[logging]
level = "info"
format = "compact"
EOF

# Run migrations
QUOTEY_CONFIG=config/quotey.headless.toml cargo run -p quotey-cli -- migrate

# Seed with demo data
cargo run -p quotey-cli -- seed
```

### 2. Test CLI Access

```bash
# List quotes
./target/release/quotey quote list --limit 5

# Search catalog
./target/release/quotey catalog search "pro plan"
```

---

## Method 1: CLI Commands (Universal)

Any AI agent can invoke Quotey through shell commands. All CLI commands return structured output.

### Quote Operations

```bash
# Create a quote
./target/release/quotey quote create \
  --account="Acme Corp" \
  --product="Pro Plan" \
  --quantity=50 \
  --term=12

# Returns JSON:
# {
#   "quote_id": "Q-2026-0001",
#   "account": "Acme Corp",
#   "status": "draft",
#   "total": 15000.00,
#   "line_items": [...]
# }

# Get quote details
./target/release/quotey quote get Q-2026-0001 --format=json

# List all quotes
./target/release/quotey quote list --format=json

# Generate PDF
./target/release/quotey quote pdf Q-2026-0001 --output=/tmp/quote.pdf
```

### Catalog Operations

```bash
# Search products
./target/release/quotey catalog search "enterprise" --format=json

# Import products
./target/release/quotey catalog import products.csv --format=csv
```

### Approval Workflow

```bash
# Check approval status
./target/release/quotey quote get Q-2026-0001 --format=json | jq '.approval_status'

# Submit for approval (if CLI supports this - check with `quotey quote --help`)
```

### Revenue Genome (Analytics)

```bash
# Analyze a deal
./target/release/quotey genome analyze Q-2026-0001 --format=json

# Query patterns
./target/release/quotey genome query "discounts > 20%" --format=json
```

---

## Method 2: MCP Server (Recommended for Agents)

Quotey includes an MCP (Model Context Protocol) server for structured tool calling.

### Start the MCP Server

```bash
# Run the MCP server (stdio mode for Claude Desktop, etc.)
./target/release/quotey-mcp

# Or with SSE transport (for remote agents)
./target/release/quotey-mcp --transport=sse --port=3001
```

### Available MCP Tools

| Tool | Description | Parameters |
|------|-------------|------------|
| `catalog_search` | Search products by name/description | `query: string` |
| `catalog_get` | Get product details by ID | `product_id: string` |
| `quote_create` | Create a new quote | `account: string, items: [...]` |
| `quote_get` | Get quote by ID | `quote_id: string` |
| `quote_price` | Calculate pricing for a configuration | `config: {...}` |
| `quote_list` | List quotes with filters | `status?: string, limit?: number` |
| `approval_request` | Submit quote for approval | `quote_id: string, notes?: string` |
| `approval_status` | Check approval status | `quote_id: string` |
| `approval_pending` | List pending approvals | `limit?: number` |
| `quote_pdf` | Generate PDF document | `quote_id: string` |

### MCP Configuration for Claude Desktop

Add to your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json` on macOS):

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

### MCP Configuration for Codex

For OpenAI Codex or other MCP clients:

```json
{
  "mcpServers": {
    "quotey": {
      "command": "/path/to/quotey/target/release/quotey-mcp",
      "transport": "stdio",
      "env": {
        "QUOTEY_CONFIG": "/path/to/quotey/config/quotey.headless.toml"
      }
    }
  }
}
```

### Example Agent Prompts

With MCP configured, your AI agent can:

**Create a quote:**
```
Create a quote for Acme Corp with:
- 50 seats of Pro Plan
- 12 month term
- 15% discount requested
```

The agent will:
1. Call `catalog_search` to find "Pro Plan"
2. Call `quote_create` with the configuration
3. Return the quote number and pricing

**Check approval status:**
```
What's the status of quote Q-2026-0042? Can it be approved?
```

The agent will:
1. Call `quote_get` for Q-2026-0042
2. Check `approval_status`
3. Report if policy violations exist

---

## Docker Setup for VPS

Deploy Quotey on a VPS for 24/7 AI agent access.

### docker-compose.headless.yml

```yaml
version: '3.8'

services:
  quotey-mcp:
    build: .
    container_name: quotey-mcp
    restart: unless-stopped
    environment:
      - QUOTEY_DATABASE_URL=sqlite:///data/quotey.db
      - QUOTEY_LLM_PROVIDER=ollama
      - QUOTEY_LLM_BASE_URL=http://ollama:11434
      - QUOTEY_LLM_MODEL=llama3.1
      - RUST_LOG=info
    volumes:
      - quotey-data:/data
      - ./config/quotey.headless.toml:/app/config/quotey.toml:ro
    ports:
      - "3001:3001"  # MCP SSE transport
    command: ["quotey-mcp", "--transport=sse", "--port=3001", "--host=0.0.0.0"]
    depends_on:
      - ollama

  ollama:
    image: ollama/ollama:latest
    container_name: quotey-ollama
    restart: unless-stopped
    volumes:
      - ollama-data:/root/.ollama

  quotey-api:
    build: .
    container_name: quotey-api
    restart: unless-stopped
    environment:
      - QUOTEY_DATABASE_URL=sqlite:///data/quotey.db
      - QUOTEY_LLM_PROVIDER=ollama
      - QUOTEY_LLM_BASE_URL=http://ollama:11434
    volumes:
      - quotey-data:/data
      - ./config/quotey.headless.toml:/app/config/quotey.toml:ro
    ports:
      - "8080:8080"  # Health/REST API
    command: ["quotey-server"]
    depends_on:
      - ollama

volumes:
  quotey-data:
  ollama-data:
```

### Deploy to VPS

```bash
# On your VPS:
git clone <repo-url> quotey
cd quotey

# Create headless config
mkdir -p config
cat > config/quotey.headless.toml << 'EOF'
[database]
url = "sqlite:///data/quotey.db"

[llm]
provider = "ollama"
base_url = "http://ollama:11434"
model = "llama3.1"

[server]
bind_address = "0.0.0.0"
health_check_port = 8080

[logging]
level = "info"
format = "json"
EOF

# Start services
docker-compose -f docker-compose.headless.yml up -d

# Check status
docker-compose -f docker-compose.headless.yml ps
docker-compose -f docker-compose.headless.yml logs -f

# Run migrations
docker-compose -f docker-compose.headless.yml exec quotey-api quotey migrate

# Seed data
docker-compose -f docker-compose.headless.yml exec quotey-api quotey seed
```

### Access from Your Local Agent

Once deployed, your local AI agent can connect:

```bash
# Test MCP connection
curl http://your-vps-ip:3001/mcp/list

# Or use the CLI remotely via SSH
ssh your-vps "docker-compose -f /opt/quotey/docker-compose.headless.yml exec -T quotey-api quotey quote list"
```

---

## AI Agent Agnostic Design

Quotey is designed to work with any AI agent:

### Pi Claude
- Use MCP server with Claude Desktop
- Or invoke CLI commands via bash tool

### OpenAI Codex
- Use MCP with Codex's tool system
- Or call CLI and parse JSON output

### Custom Agents
- REST API on port 8080
- MCP over SSE on port 3001
- CLI invocation over SSH

### LangChain/LangGraph
```python
from langchain.tools import ShellTool

quotey_tool = ShellTool(
    name="quotey",
    description="Create and manage quotes. Input should be a quotey CLI command.",
    args_schema={"command": "string"}
)
```

---

## Example Workflows

### Workflow 1: Automated Quote Generation

```python
# Agent receives: "Customer wants 100 seats of Enterprise plan for 24 months"

# Step 1: Search catalog
products = run("./quotey catalog search 'enterprise' --format=json")
# Returns: [{"id": "ENT-001", "name": "Enterprise Plan", ...}]

# Step 2: Create quote
quote = run(f"""
  ./quotey quote create 
    --account="New Customer"
    --product="{products[0]['id']}"
    --quantity=100
    --term=24
    --format=json
""")
# Returns: {"quote_id": "Q-2026-0105", "total": 480000, ...}

# Step 3: Check if approval needed
if quote['discount'] > 10:
    approval = run(f"./quotey policy-packet {quote['quote_id']} --format=json")
    return f"Quote {quote['quote_id']} created. Approval required: {approval['violations']}"

# Step 4: Generate PDF
run(f"./quotey quote pdf {quote['quote_id']} --output=/tmp/quote.pdf")
return f"Quote {quote['quote_id']} ready: /tmp/quote.pdf"
```

### Workflow 2: Deal Analysis

```python
# Agent receives: "Analyze why we lost the Acme Corp deal"

# Step 1: Find the quote
quotes = run("./quotey quote list --account='Acme Corp' --status=lost --format=json")

# Step 2: Run genome analysis
for quote in quotes:
    analysis = run(f"./quotey genome analyze {quote['quote_id']} --format=json")
    insights.append(analysis)

# Step 3: Summarize
return generate_summary(insights)
```

### Workflow 3: Batch Processing

```bash
#!/bin/bash
# Process a CSV of quote requests

while IFS="," read -r account product quantity term; do
  ./quotey quote create \
    --account="$account" \
    --product="$product" \
    --quantity="$quantity" \
    --term="$term" \
    --format=json
    
done < quote_requests.csv
```

---

## Security for AI Agent Access

### API Key Authentication (MCP)

Set API keys for MCP access:

```bash
export MCP_API_KEY="your-secret-key"
./target/release/quotey-mcp --transport=sse --port=3001
```

Rate limiting:
```bash
export MCP_DEFAULT_REQUESTS_PER_MINUTE=120
./target/release/quotey-mcp
```

### SSH Key Authentication (CLI)

For remote CLI access:

```bash
# On VPS, create restricted user
useradd -m -s /bin/bash quotey-agent

# Add SSH key
mkdir -p /home/quotey-agent/.ssh
cat > /home/quotey-agent/.ssh/authorized_keys << 'EOF'
restrict,command="/opt/quotey/bin/quotey-wrapper" ssh-rsa AAAAB3...
EOF

# Create wrapper script
cat > /opt/quotey/bin/quotey-wrapper << 'EOF'
#!/bin/bash
# Only allow read-only operations
ALLOWED_PATTERN='^(quote list|quote get|catalog search|genome analyze)'
if [[ "$SSH_ORIGINAL_COMMAND" =~ $ALLOWED_PATTERN ]]; then
  cd /opt/quotey && ./target/release/quotey $SSH_ORIGINAL_COMMAND
else
  echo "Command not allowed: $SSH_ORIGINAL_COMMAND"
  exit 1
fi
EOF
chmod +x /opt/quotey/bin/quotey-wrapper
```

---

## Troubleshooting

### "Cannot connect to MCP server"

```bash
# Check if MCP server is running
curl http://localhost:3001/health

# Check logs
docker-compose logs quotey-mcp
```

### "CLI returns no output"

```bash
# Check if database is initialized
./target/release/quotey doctor

# Run migrations
./target/release/quotey migrate
```

### "LLM not responding"

```bash
# Check Ollama
curl http://localhost:11434/api/tags

# Pull model if needed
ollama pull llama3.1
```

---

## Integration Checklist

- [ ] Quotey built and database initialized
- [ ] Headless config created (no Slack tokens)
- [ ] LLM provider configured (Ollama recommended)
- [ ] MCP server running OR CLI accessible
- [ ] API keys configured (if remote access)
- [ ] Test quote creation works
- [ ] Agent can parse JSON output
- [ ] Error handling implemented

---

## Next Steps

1. **Deploy on VPS** using `docker-compose.headless.yml`
2. **Configure your AI agent** (Pi Claude, Codex, etc.) with MCP
3. **Test basic operations** (quote create, catalog search)
4. **Build custom workflows** for your use case

See also:
- `DEPLOYMENT_GUIDE.md` - General deployment instructions
- `docs/docs/api/cli-commands.md` - Complete CLI reference
