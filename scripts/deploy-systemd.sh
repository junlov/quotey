#!/bin/bash
# Quotey systemd deployment script
# Usage: ./scripts/deploy-systemd.sh [user]

set -e

USER=${1:-$USER}
QUOTEY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE_NAME="quotey"

 echo "=========================================="
echo "Quotey Systemd Deployment"
echo "=========================================="
echo ""
echo "This script will:"
echo "  1. Build Quotey in release mode"
echo "  2. Create systemd service file"
echo "  3. Enable and start the service"
echo ""
echo "Quotey directory: $QUOTEY_DIR"
echo "Service user: $USER"
echo ""
read -p "Continue? (y/n) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Aborted."
    exit 1
fi

# Build release binary
echo ""
echo "Building Quotey (release mode)..."
cd "$QUOTEY_DIR"
cargo build --release

# Check if config exists
if [ ! -f "$QUOTEY_DIR/config/quotey.toml" ]; then
    echo ""
    echo "Warning: config/quotey.toml not found!"
    echo "Creating from example..."
    cp "$QUOTEY_DIR/config/quotey.example.toml" "$QUOTEY_DIR/config/quotey.toml"
    echo "Please edit config/quotey.toml with your Slack tokens before starting."
fi

# Create systemd service file
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"

echo ""
echo "Creating systemd service file at $SERVICE_FILE..."

sudo tee "$SERVICE_FILE" > /dev/null <<EOF
[Unit]
Description=Quotey Slack Bot
Documentation=https://github.com/quotey/quotey
After=network.target
Wants=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$QUOTEY_DIR
Environment=QUOTEY_CONFIG=$QUOTEY_DIR/config/quotey.toml
Environment=RUST_LOG=info
Environment=QUOTEY_DATABASE_URL=sqlite://$QUOTEY_DIR/quotey.db
ExecStart=$QUOTEY_DIR/target/release/quotey-server
ExecReload=/bin/kill -HUP \$MAINPID
Restart=always
RestartSec=5
StartLimitInterval=60s
StartLimitBurst=3

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=$QUOTEY_DIR
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd
 echo ""
echo "Reloading systemd..."
sudo systemctl daemon-reload

# Enable service
 echo ""
echo "Enabling service to start on boot..."
sudo systemctl enable "$SERVICE_NAME"

echo ""
echo "=========================================="
echo "Deployment Complete!"
echo "=========================================="
echo ""
echo "Service: $SERVICE_NAME"
echo ""
echo "Commands:"
echo "  Start:   sudo systemctl start $SERVICE_NAME"
echo "  Stop:    sudo systemctl stop $SERVICE_NAME"
echo "  Status:  sudo systemctl status $SERVICE_NAME"
echo "  Logs:    sudo journalctl -u $SERVICE_NAME -f"
echo "  Restart: sudo systemctl restart $SERVICE_NAME"
echo ""
echo "Don't forget to:"
echo "  1. Edit config/quotey.toml with your Slack tokens"
echo "  2. Start the service: sudo systemctl start $SERVICE_NAME"
echo ""

# Ask to start now
read -p "Start the service now? (y/n) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    sudo systemctl start "$SERVICE_NAME"
    sleep 2
    sudo systemctl status "$SERVICE_NAME"
fi
