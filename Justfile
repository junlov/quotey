# Quotey - Justfile for common development tasks
# Install just: https://github.com/casey/just

# Default recipe - show available commands
default:
    @just --list

# Setup development environment (install required tools)
setup:
    @echo "Installing required tools..."
    cargo install cargo-deny cargo-nextest sqlx-cli --locked
    @echo "Setup complete!"

# Run all tests (uses nextest if available, falls back to cargo test)
test:
    #!/usr/bin/env bash
    if command -v cargo-nextest &> /dev/null; then
        cargo nextest run --all-targets --all-features
    else
        cargo test --all-targets --all-features
    fi

# Run tests for a specific crate
test-crate crate:
    cargo test -p {{crate}}

# Run E2E bootstrap script
e2e:
    ./scripts/e2e_bootstrap.sh

# Run all quality gates (same as CI)
quality:
    ./scripts/quality-gates.sh

# Run clippy on all targets
clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Check code formatting
fmt-check:
    cargo fmt -- --check

# Format code
fmt:
    cargo fmt

# Run cargo-deny security audit
audit:
    cargo deny check

# Build release binary
build:
    cargo build --release

# Run the CLI with development config
run *ARGS:
    QUOTEY_CONFIG=config/quotey.dev.toml cargo run -- {{ARGS}}

# Run database migrations
migrate:
    cargo run -- migrate

# Seed the database with E2E fixtures
seed:
    cargo run -- seed

# Run smoke tests
smoke:
    cargo run -- smoke

# Clean build artifacts
clean:
    cargo clean
    rm -rf target/

# Run the doctor command
doctor:
    cargo run -- doctor

# Generate test coverage report (requires cargo-tarpaulin)
coverage:
    cargo tarpaulin --out Html --output-dir coverage/

# Prepare sqlx offline data (run after schema changes)
prepare:
    cargo sqlx prepare --workspace

# Watch for changes and run tests
watch:
    cargo watch -x test
