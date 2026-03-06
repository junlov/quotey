#!/bin/bash
# Deploy Quotey in headless mode on a VPS for AI agent access
# Usage: ./scripts/deploy-headless-vps.sh [DOMAIN] [EMAIL]

set -e

DOMAIN=${1:-}
EMAIL=${2:-$USER@$(hostname -f)}
QUOTEY_DIR="/opt/quotey"

echo "=========================================="
echo "Quotey Headless VPS Deployment"
echo "=========================================="
echo ""
echo "This will deploy Quotey for AI agent access:"
echo "  - MCP Server on port 3001"
echo "  - Health API on port 8080"
echo "  - Ollama LLM on port 11434"
echo ""
echo "No Slack required - pure AI agent integration"
echo ""

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root (use sudo)"
  exit 1
fi

# Install Docker if not present
if ! command -v docker &> /dev/null; then
  echo "Installing Docker..."
  curl -fsSL https://get.docker.com | sh
  systemctl enable docker
  systemctl start docker
fi

# Install Docker Compose
if ! command -v docker-compose &> /dev/null; then
  echo "Installing Docker Compose..."
  curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" \
    -o /usr/local/bin/docker-compose
  chmod +x /usr/local/bin/docker-compose
fi

# Create directory
echo ""
echo "Creating Quotey directory at $QUOTEY_DIR..."
mkdir -p $QUOTEY_DIR
cd $QUOTEY_DIR

# Clone or update repository
if [ -d ".git" ]; then
  echo "Updating existing repository..."
  git pull
else
  echo "Cloning repository..."
  # Replace with your actual repo URL
  git clone https://github.com/yourusername/quotey.git .
fi

# Create headless config
echo ""
echo "Creating headless configuration..."
cat > $QUOTEY_DIR/config/quotey.headless.toml << 'EOF'
[database]
url = "sqlite:///data/quotey.db"
max_connections = 5
timeout_secs = 30

[llm]
provider = "ollama"
base_url = "http://ollama:11434"
model = "llama3.1"
timeout_secs = 30
max_retries = 2

[server]
bind_address = "0.0.0.0"
health_check_port = 8080
graceful_shutdown_secs = 15

[logging]
level = "info"
format = "json"

[features]
enable_catalog_bootstrap = true
enable_quote_intelligence = true
enable_approval_workflow = true
enable_pdf_generation = true

[crm]
provider = "stub"
fixtures_path = "config/demo_fixtures"
EOF

# Create environment file
cat > $QUOTEY_DIR/.env << EOF
# Quotey Headless Environment
MCP_PORT=3001
API_PORT=8080

# Optional: MCP API Key for authentication
# MCP_API_KEY=$(openssl rand -hex 32)
EOF

# Pull and start services
echo ""
echo "Starting services..."
docker-compose -f docker-compose.headless.yml pull
docker-compose -f docker-compose.headless.yml up -d

# Wait for services to be ready
echo ""
echo "Waiting for services to start..."
sleep 10

# Run migrations
echo ""
echo "Running database migrations..."
docker-compose -f docker-compose.headless.yml exec -T quotey-api quotey migrate || true

# Seed demo data
echo ""
echo "Seeding demo data..."
docker-compose -f docker-compose.headless.yml exec -T quotey-api quotey seed || true

# Check health
echo ""
echo "Checking service health..."
curl -s http://localhost:8080/health || echo "Warning: Health check failed"

# Create systemd service for auto-start
echo ""
echo "Creating systemd service..."
cat > /etc/systemd/system/quotey-headless.service << EOF
[Unit]
Description=Quotey Headless (AI Agent Access)
Requires=docker.service
After=docker.service

[Service]
Type=oneshot
RemainAfterExit=yes
WorkingDirectory=$QUOTEY_DIR
ExecStart=/usr/local/bin/docker-compose -f docker-compose.headless.yml up -d
ExecStop=/usr/local/bin/docker-compose -f docker-compose.headless.yml down
TimeoutStartSec=0

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable quotey-headless.service

echo ""
echo "=========================================="
echo "Deployment Complete!"
echo "=========================================="
echo ""
echo "Services running:"
echo "  MCP Server:  http://$(hostname -I | awk '{print $1}'):3001"
echo "  Health API:  http://$(hostname -I | awk '{print $1}'):8080/health"
echo "  Ollama API:  http://$(hostname -I | awk '{print $1}'):11434"
echo ""
echo "Commands:"
echo "  View logs:    docker-compose -f docker-compose.headless.yml logs -f"
echo "  Stop:         docker-compose -f docker-compose.headless.yml down"
echo "  Restart:      docker-compose -f docker-compose.headless.yml restart"
echo "  Update:       cd $QUOTEY_DIR && git pull && docker-compose -f docker-compose.headless.yml up -d"
echo ""
echo "CLI access:"
echo "  docker-compose -f docker-compose.headless.yml exec quotey-api quotey quote list"
echo ""

# Optional: Setup Caddy reverse proxy with HTTPS
if [ -n "$DOMAIN" ]; then
  echo "Setting up HTTPS with Caddy..."
  
  # Install Caddy
  if ! command -v caddy &> /dev/null; then
    apt-get install -y debian-keyring debian-archive-keyring apt-transport-https
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
    curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' | tee /etc/apt/sources.list.d/caddy-stable.list
    apt-get update
    apt-get install -y caddy
  fi

  # Create Caddyfile
  cat > /etc/caddy/Caddyfile << EOF
$DOMAIN {
  reverse_proxy localhost:8080
  
  # MCP SSE endpoint
  handle /mcp/* {
    reverse_proxy localhost:3001
  }
}
EOF

  systemctl restart caddy
  echo "HTTPS enabled at https://$DOMAIN"
fi

echo ""
echo "AI Agent Configuration:"
echo "-----------------------"
echo "For Pi Claude / Codex, add to your MCP config:"
echo ""
echo '{'
echo '  "mcpServers": {'
echo '    "quotey": {'
echo '      "url": "http://'$(hostname -I | awk '{print $1}')':3001/sse"'
echo '    }'
echo '  }'
echo '}'
echo ""
