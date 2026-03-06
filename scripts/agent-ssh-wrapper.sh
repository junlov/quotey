#!/bin/bash
# SSH Command Wrapper for AI Agent Access
# Place this in /opt/quotey/bin/quotey-agent-wrapper
# Configure in authorized_keys: restrict,command="/opt/quotey/bin/quotey-agent-wrapper"

set -e

QUOTEY_DIR="/opt/quotey"
QUOTEY_BIN="$QUOTEY_DIR/target/release/quotey"
LOG_FILE="/var/log/quotey-agent.log"

# Log the request
echo "$(date '+%Y-%m-%d %H:%M:%S') - User: $SSH_USER - Command: $SSH_ORIGINAL_COMMAND" >> "$LOG_FILE"

# Define allowed commands (read-only by default)
# Modify patterns to allow write operations if needed
ALLOWED_PATTERNS=(
  '^quote list.*'
  '^quote get .*'
  '^catalog search .*'
  '^catalog get .*'
  '^genome analyze .*'
  '^genome query .*'
  '^config.*'
  '^doctor.*'
  '^smoke.*'
  # Uncomment below to enable write operations:
  # '^quote create .*'
  # '^quote update .*'
  # '^approval_request .*'
)

# Check if command is allowed
is_allowed=false
for pattern in "${ALLOWED_PATTERNS[@]}"; do
  if [[ "$SSH_ORIGINAL_COMMAND" =~ $pattern ]]; then
    is_allowed=true
    break
  fi
done

if [ "$is_allowed" = false ]; then
  echo "Error: Command not allowed: $SSH_ORIGINAL_COMMAND"
  echo "Allowed commands: quote list/get, catalog search/get, genome analyze/query, config, doctor, smoke"
  exit 1
fi

# Execute the command
cd "$QUOTEY_DIR"
export QUOTEY_CONFIG="$QUOTEY_DIR/config/quotey.headless.toml"
exec "$QUOTEY_BIN" $SSH_ORIGINAL_COMMAND
