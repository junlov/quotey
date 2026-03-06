# CLI Commands Reference

The Quotey CLI provides operational commands for managing the system.

## Global Options

```bash
quotey [OPTIONS] <COMMAND>

Options:
  -c, --config <PATH>     Path to configuration file
  -v, --verbose           Enable verbose output
  -h, --help              Print help
  -V, --version           Print version
```

## Commands

### `start`

Start the Quotey server.

```bash
quotey start [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--bind <ADDRESS>` | Bind address (default: 127.0.0.1) |
| `--port <PORT>` | Health check port (default: 8080) |

**Examples:**

```bash
# Start with default configuration
cargo run -p quotey-cli -- start

# Start with custom bind address
cargo run -p quotey-cli -- start --bind 0.0.0.0 --port 3000

# Start with specific config
cargo run -p quotey-cli -- --config config/prod.toml start
```

### `migrate`

Run database migrations.

```bash
quotey migrate [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--target <VERSION>` | Migrate to specific version |
| `--revert` | Revert last migration |
| `--status` | Show migration status |

**Examples:**

```bash
# Run all pending migrations
cargo run -p quotey-cli -- migrate

# Check migration status
cargo run -p quotey-cli -- migrate --status

# Revert last migration
cargo run -p quotey-cli -- migrate --revert
```

**Output:**
```
Migration Status
================

 Applied   Migration
─────────────────────────────
   ✓       0001_initial
   ✓       0002_emoji_approvals
   ✓       0003_configuration_fingerprints
   ...
   ○       0028_portal_push_subscription  (pending)

Run `quotey migrate` to apply pending migrations.
```

### `seed`

Load demo/fixture data into the database.

```bash
quotey seed [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--dataset <NAME>` | Dataset to load (default: demo) |
| `--clean` | Clean database before seeding |

**Examples:**

```bash
# Load demo data
cargo run -p quotey-cli -- seed

# Load specific dataset
cargo run -p quotey-cli -- seed --dataset=e2e-test

# Clean and re-seed
cargo run -p quotey-cli -- seed --clean
```

**Available Datasets:**

| Dataset | Description |
|---------|-------------|
| `demo` | Standard demo products and accounts |
| `e2e-test` | Data for E2E test scenarios |
| `minimal` | Minimal viable data |

### `smoke`

Run smoke tests to verify system health.

```bash
quotey smoke [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--verbose` | Show detailed test output |

**Examples:**

```bash
# Run smoke tests
cargo run -p quotey-cli -- smoke

# With verbose output
cargo run -p quotey-cli -- smoke --verbose
```

**Output:**
```
Smoke Tests
===========

✓ Database connection
✓ Migrations current
✓ LLM connectivity
✓ Slack API connectivity
✓ PDF generation

All tests passed!
```

### `config`

Display current configuration.

```bash
quotey config [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--format <FORMAT>` | Output format: table, json, toml |
| `--source` | Show configuration source |

**Examples:**

```bash
# Show config as table (default)
cargo run -p quotey-cli -- config

# Show as JSON
cargo run -p quotey-cli -- config --format=json

# Show with source information
cargo run -p quotey-cli -- config --source
```

**Output:**
```
Configuration
=============

Source: config/quotey.toml (with env overrides)

Database:
  URL: sqlite://quotey.db
  Max Connections: 5

Slack:
  App Token: xapp-**** (from env)
  Bot Token: xoxb-**** (from env)

LLM:
  Provider: ollama
  Base URL: http://localhost:11434
  Model: llama3.1
```

### `doctor`

Run diagnostics and troubleshooting.

```bash
quotey doctor [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--fix` | Attempt to fix issues automatically |
| `--category <CAT>` | Check specific category |

**Categories:**

| Category | Checks |
|----------|--------|
| `all` | All checks (default) |
| `database` | Database connectivity and schema |
| `slack` | Slack API tokens and connectivity |
| `llm` | LLM provider connectivity |
| `filesystem` | File permissions and paths |

**Examples:**

```bash
# Run all diagnostics
cargo run -p quotey-cli -- doctor

# Check database only
cargo run -p quotey-cli -- doctor --category=database

# Auto-fix issues
cargo run -p quotey-cli -- doctor --fix
```

**Output:**
```
Diagnostics
===========

Database:
  ✓ Connection successful
  ✓ Migrations current (28 applied)
  ✓ Write permissions OK

Slack:
  ✓ App token format valid
  ✓ Bot token format valid
  ✓ API connectivity OK
  ⚠ Socket Mode not tested (server not running)

LLM:
  ✓ Provider config valid
  ✓ Ollama server reachable
  ✓ Model 'llama3.1' available

Filesystem:
  ✓ Config directory readable
  ✓ Database directory writable
  ✓ Templates directory exists

1 warning, 0 errors
```

### `policy-packet`

Build a policy approval packet for human review.

```bash
quotey policy-packet <QUOTE_ID> [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--output <PATH>` | Output file path |
| `--format <FORMAT>` | Output format: json, pdf, markdown |

**Examples:**

```bash
# Build packet for quote
cargo run -p quotey-cli -- policy-packet Q-2026-0042

# Save to file
cargo run -p quotey-cli -- policy-packet Q-2026-0042 --output=packet.json

# Generate PDF
cargo run -p quotey-cli -- policy-packet Q-2026-0042 --format=pdf
```

**Output:**
```json
{
  "quote_id": "Q-2026-0042",
  "account": "Acme Corp",
  "deal_value": 20400.00,
  "discount_requested": 15.0,
  "policies_evaluated": [
    {
      "policy": "discount_cap_smb",
      "result": "violation",
      "threshold": 10.0,
      "actual": 15.0,
      "approver_role": "sales_manager"
    }
  ],
  "precedents": [
    {
      "quote_id": "Q-2026-0018",
      "account": "Acme Corp",
      "discount": 12.0,
      "outcome": "approved"
    }
  ],
  "margin_analysis": {
    "revenue": 20400.00,
    "cost": 8160.00,
    "margin_pct": 60.0,
    "margin_after_discount": 51.0
  }
}
```

### `genome`

Revenue genome analysis commands.

```bash
quotey genome <SUBCOMMAND>
```

**Subcommands:**

#### `genome analyze`

Analyze a deal for autopsy insights.

```bash
quotey genome analyze <QUOTE_ID> [OPTIONS]
```

**Examples:**

```bash
# Analyze a won deal
cargo run -p quotey-cli -- genome analyze Q-2026-0042

# Analyze with counterfactual
cargo run -p quotey-cli -- genome analyze Q-2026-0042 --what-if="10% discount"
```

**Output:**
```
Deal Autopsy: Q-2026-0042
=========================

Outcome: Won
Close Date: 2026-02-15

Attribution:
  • Discount Level: +25% influence
  • Competitive Position: +20% influence
  • Product Fit: +30% influence
  • Sales Engagement: +25% influence

Counterfactual Analysis:
  If discount was 10% instead of 15%:
    - Win probability: 85% → 72%
    - Expected value: $20,400 → $18,700
```

#### `genome query`

Query the revenue genome for patterns.

```bash
quotey genome query <QUERY>
```

**Examples:**

```bash
# Query for patterns
cargo run -p quotey-cli -- genome query "discounts > 20% in Q4"
cargo run -p quotey-cli -- genome query "renewal rates by segment"
```

#### `genome export`

Export genome data for external analysis.

```bash
quotey genome export [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--format <FORMAT>` | csv, json, parquet |
| `--since <DATE>` | Export data since date |
| `--output <PATH>` | Output file |

### `quote`

Manage quotes via CLI.

```bash
quotey quote <SUBCOMMAND>
```

**Subcommands:**

#### `quote list`

List quotes.

```bash
quotey quote list [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--status <STATUS>` | Filter by status |
| `--account <ACCOUNT>` | Filter by account |
| `--limit <N>` | Limit results |

**Examples:**

```bash
# List all quotes
cargo run -p quotey-cli -- quote list

# List pending approval
cargo run -p quotey-cli -- quote list --status=approval

# List for specific account
cargo run -p quotey-cli -- quote list --account="Acme Corp"
```

#### `quote get`

Get quote details.

```bash
quotey quote get <QUOTE_ID>
```

**Examples:**

```bash
cargo run -p quotey-cli -- quote get Q-2026-0042
```

#### `quote pdf`

Generate PDF for a quote.

```bash
quotey quote pdf <QUOTE_ID> [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--output <PATH>` | Output file path |
| `--template <NAME>` | Template to use |

**Examples:**

```bash
# Generate PDF
cargo run -p quotey-cli -- quote pdf Q-2026-0042

# Custom output path
cargo run -p quotey-cli -- quote pdf Q-2026-0042 --output=/tmp/quote.pdf
```

### `catalog`

Manage product catalog.

```bash
quotey catalog <SUBCOMMAND>
```

**Subcommands:**

#### `catalog import`

Import products from file.

```bash
quotey catalog import <FILE> [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--format <FORMAT>` | csv, xlsx, pdf |
| `--dry-run` | Validate without importing |

**Examples:**

```bash
# Import CSV
cargo run -p quotey-cli -- catalog import products.csv --format=csv

# Dry run first
cargo run -p quotey-cli -- catalog import products.csv --dry-run
```

#### `catalog search`

Search products.

```bash
quotey catalog search <QUERY>
```

**Examples:**

```bash
cargo run -p quotey-cli -- catalog search "pro plan"
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid arguments |
| 3 | Configuration error |
| 4 | Database error |
| 5 | Network error |
| 6 | Validation error |

## Environment Variables

All commands respect these environment variables:

| Variable | Description |
|----------|-------------|
| `QUOTEY_CONFIG` | Path to config file |
| `QUOTEY_DATABASE_URL` | Database URL |
| `RUST_LOG` | Log level filter |

## Next Steps

- [Configuration Guide](../guides/configuration) — Configure the CLI
- [Database Migrations](../guides/database-migrations) — Manage schema
- [Testing Guide](../contributing/testing-guide) — Test your changes
