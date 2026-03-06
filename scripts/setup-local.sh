#!/bin/bash
# Quotey local development setup script
# Usage: ./scripts/setup-local.sh

set -e

echo "=========================================="
echo "Quotey Local Development Setup"
echo "=========================================="
echo ""

QUOTEY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$QUOTEY_DIR"

# Check Rust installation
 echo "Checking Rust installation..."
if ! command -v rustc &> /dev/null; then
    echo "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    source "$HOME/.cargo/env"
fi

echo "Rust version: $(rustc --version)"
echo "Cargo version: $(cargo --version)"

# Check minimum Rust version
RUST_VERSION=$(rustc --version | cut -d' ' -f2)
MIN_VERSION="1.75.0"

if [ "$(printf '%s\n' "$MIN_VERSION" "$RUST_VERSION" | sort -V | head -n1)" != "$MIN_VERSION" ]; then
    echo "Error: Rust $MIN_VERSION or later is required. You have $RUST_VERSION"
    echo "Please update Rust: rustup update"
    exit 1
fi

# Install cargo tools
 echo ""
echo "Installing development tools..."
echo "  - cargo-deny (security audit)"
echo "  - cargo-nextest (test runner)"
echo "  - sqlx-cli (database migrations)"

cargo install cargo-deny cargo-nextest --locked 2>/dev/null || true
cargo install sqlx-cli --no-default-features --features sqlite 2>/dev/null || true

# Build project
 echo ""
echo "Building Quotey (this may take a few minutes)..."
cargo build --release

# Create config from example
 echo ""
if [ ! -f "config/quotey.toml" ]; then
    echo "Creating configuration file from example..."
    cp config/quotey.example.toml config/quotey.toml
    echo "Created: config/quotey.toml"
else
    echo "Configuration file already exists: config/quotey.toml"
fi

# Create .env from example
if [ ! -f ".env" ]; then
    echo "Creating .env file from example..."
    cp .env.example .env
    echo "Created: .env"
else
    echo ".env file already exists"
fi

# Run database migrations
 echo ""
echo "Setting up database..."
cargo run -p quotey-cli -- migrate

echo ""
echo "=========================================="
echo "Setup Complete!"
echo "=========================================="
echo ""
echo "Next steps:"
echo ""
echo "1. Set up your Slack app:"
echo "   - Go to https://api.slack.com/apps"
echo "   - Create a new app from scratch"
echo "   - Enable Socket Mode"
echo "   - Add bot token scopes (see DEPLOYMENT_GUIDE.md)"
echo "   - Install to workspace"
echo ""
echo "2. Add your Slack tokens to config/quotey.toml or .env:"
echo "   SLACK_APP_TOKEN=xapp-your-token"
echo "   SLACK_BOT_TOKEN=xoxb-your-token"
echo ""
echo "3. (Optional) Set up Ollama for local LLM:"
echo "   curl -fsSL https://ollama.com/install.sh | sh"
echo "   ollama pull llama3.1"
echo "   ollama serve"
echo ""
echo "4. Start the server:"
echo "   cargo run -p quotey-server"
echo "   # or with .env:"
echo "   source .env && cargo run -p quotey-server"
echo ""
echo "5. Test in Slack:"
echo "   /quote new for Acme Corp, Pro Plan, 50 seats"
echo ""
echo "See DEPLOYMENT_GUIDE.md for complete documentation."
