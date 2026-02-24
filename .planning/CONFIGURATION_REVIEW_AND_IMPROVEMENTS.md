# Quotey Configuration Review & Improvement Recommendations

**Review Date:** 2026-02-24  
**Reviewer:** ResearchAgent  
**Scope:** Project configuration, tooling, workflow, and integration gaps

---

## Executive Summary

Based on comprehensive research of the quotey codebase and the Dicklesworthstone ecosystem, this review identifies **22 improvement opportunities** across 6 categories. The project has a solid foundation but could significantly benefit from enhanced tooling integration, configuration hardening, and workflow optimizations.

### Top 5 Priority Improvements

| Priority | Improvement | Impact | Effort |
|----------|-------------|--------|--------|
| 1 | **Add GitHub Actions CI/CD** | Prevent broken main, automate quality gates | Medium |
| 2 | **Integrate cass-memory** | Cross-agent learning, institutional knowledge | Medium |
| 3 | **Add VS Code workspace settings** | Consistent dev experience, faster onboarding | Low |
| 4 | **Configure cargo-deny in CI** | License compliance, security audit | Low |
| 5 | **Add pre-commit hooks** | Prevent bad commits, automate beads sync | Low |

---

## 1. CI/CD Configuration (Missing)

### Current State
- ❌ No `.github/workflows/` directory
- ❌ No automated testing on PR/push
- ❌ No automated security audits
- ❌ No automated documentation deployment

### Research-Based Recommendations

Based on `RESEARCH_DICKLESWORTHSTONE_PROJECTS.md` and industry best practices:

#### 1.1 Add GitHub Actions Workflow (HIGH PRIORITY)

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          toolchain: stable
          components: rustfmt, clippy
      
      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
      
      - name: Check formatting
        run: cargo fmt --all -- --check
      
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings
      
      - name: Run tests
        run: cargo test --all-features
      
      - name: Run cargo-deny
        uses: EmbarkStudios/cargo-deny-action@v1
        with:
          command: check all
      
      - name: Verify beads synced
        run: |
          if [ -n "$(git status --porcelain .beads/)" ]; then
            echo "Error: .beads/ not synced. Run 'br sync --flush-only'"
            exit 1
          fi
```

**Rationale:**
- Prevents broken main branch
- Ensures `cargo fmt`, `clippy`, tests pass before merge
- Validates beads synchronization (per AGENTS.md requirements)
- Automates license/security auditing with cargo-deny

#### 1.2 Add Release Workflow

```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  release:
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, x86_64-apple-darwin]
    runs-on: ${{ matrix.target == 'x86_64-apple-darwin' && 'macos-latest' || 'ubuntu-latest' }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: softprops/action-gh-release@v1
        with:
          files: target/${{ matrix.target }}/release/quotey
```

---

## 2. Development Environment (Partial)

### Current State
- ✅ `rust-toolchain.toml` with stable channel
- ✅ `rustfmt.toml` configuration
- ❌ No `.vscode/settings.json` for IDE consistency
- ❌ No `.vscode/extensions.json` for recommended extensions
- ❌ No `.vscode/launch.json` for debugging

### Recommendations

#### 2.1 Add VS Code Workspace Settings

```json
// .vscode/settings.json
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.extraArgs": ["--all-targets", "--all-features"],
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.imports.granularity.group": "module",
  "rust-analyzer.imports.prefix": "crate",
  
  "editor.formatOnSave": true,
  "editor.defaultFormatter": "rust-lang.rust-analyzer",
  
  "files.watcherExclude": {
    "**/target/**": true,
    "**/.tmp/**": true,
    "**/.beads/*.db": true
  },
  
  "search.exclude": {
    "**/target": true,
    "**/.tmp": true,
    "**/Cargo.lock": true
  }
}
```

#### 2.2 Add Recommended Extensions

```json
// .vscode/extensions.json
{
  "recommendations": [
    "rust-lang.rust-analyzer",
    "vadimcn.vscode-lldb",
    "serayuzgur.crates",
    "tamasfe.even-better-toml",
    "usernamehw.errorlens"
  ]
}
```

#### 2.3 Add Debug Configuration

```json
// .vscode/launch.json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug CLI",
      "cargo": {
        "args": ["build", "--bin=quotey", "--package=quotey-cli"],
        "filter": {
          "name": "quotey",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}"
    },
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug Tests (core)",
      "cargo": {
        "args": ["test", "--no-run", "--package=quotey-core"],
        "filter": {
          "name": "quotey-core",
          "kind": "lib"
        }
      }
    }
  ]
}
```

---

## 3. Dicklesworthstone Stack Integration (Partial)

### Current State
- ✅ `beads_rust` (br) - Issue tracking integrated
- ✅ `mcp_agent_mail_rust` - MCP coordination integrated
- ❌ `cass-memory` - Not integrated
- ❌ `frankensearch` - Not integrated
- ❌ `frankentui` - Not integrated (low priority)

### Research-Based Recommendations

See `RESEARCH_DICKLESWORTHSTONE_PROJECTS.md` and `DICKLESWORTHSTONE_STACK.md` for detailed analysis.

#### 3.1 Integrate cass-memory (MEDIUM PRIORITY)

**Why:** Cross-agent learning, procedural memory for CPQ best practices

**Implementation:**

```bash
# Add to project setup scripts
cm onboard status --json || cm onboard sample --fill-gaps --json
```

**Add to AGENTS.md:**
```markdown
## Memory System: cass-memory (REQUIRED)

Before starting complex CPQ tasks, retrieve relevant context:

```bash
# Get task-specific memory
cm context "implement pricing approval workflow" --json
```

This returns:
- **relevantBullets**: CPQ best practices for your task
- **antiPatterns**: Pitfalls to avoid (e.g., "Don't cache approval decisions")
- **historySnippets**: Past sessions that solved similar CPQ problems

Protocol:
1. **START**: Run `cm context "<task>" --json` before non-trivial CPQ work
2. **WORK**: Reference rule IDs when following them
3. **END**: Finish work. Learning happens automatically.
```

#### 3.2 Integrate frankensearch (MEDIUM PRIORITY)

**Why:** Semantic search for Deal DNA similarity matching

**Configuration:**

```toml
# config/quotey.toml (add section)
[search]
index_path = "./.quotey/search"
enable_semantic = true
embedding_model = "potion-128M"
```

**Add to scripts:**
```bash
# scripts/index-quotey.sh
#!/bin/bash
fsfs index ./crates --db ./.quotey/search/index.db
fsfs index ./migrations --db ./.quotey/search/index.db
```

**Integration in Deal DNA:**
```rust
// crates/core/src/ml/deal_dna.rs
use frankensearch::Index;

pub struct SemanticDealSearcher {
    index: Index,
}

impl SemanticDealSearcher {
    pub fn find_similar_deals(&self, deal: &DealDna) -> Vec<SimilarDeal> {
        let query = format!(
            "{} {} seats ${}",
            deal.customer.industry,
            deal.metrics.total_seats,
            deal.metrics.total_value
        );
        self.index.search(&query).limit(5).execute()
    }
}
```

---

## 4. Configuration Management (Needs Hardening)

### Current State
- ✅ `config/quotey.example.toml` exists
- ❌ No configuration validation
- ❌ No environment-specific configs
- ❌ No secrets management beyond env vars
- ❌ No configuration documentation

### Recommendations

#### 4.1 Add Configuration Schema Validation

```rust
// crates/core/src/config/mod.rs
use serde::Deserialize;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct Config {
    #[validate(nested)]
    pub database: DatabaseConfig,
    
    #[validate(nested)]
    pub slack: SlackConfig,
    
    #[validate(nested)]
    pub llm: LlmConfig,
}

#[derive(Debug, Deserialize, Validate)]
pub struct DatabaseConfig {
    #[validate(url)]
    pub url: String,
    
    #[validate(range(min = 1, max = 100))]
    pub max_connections: u32,
    
    #[validate(range(min = 1, max = 300))]
    pub timeout_secs: u64,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config/quotey"))
            .add_source(config::Environment::with_prefix("QUOTEY"))
            .build()?;
        
        let cfg: Config = config.try_deserialize()?;
        cfg.validate()?;
        Ok(cfg)
    }
}
```

#### 4.2 Add Environment-Specific Configs

```
config/
├── quotey.toml              # Base config
├── quotey.development.toml  # Dev overrides
├── quotey.staging.toml      # Staging overrides
├── quotey.production.toml   # Production overrides
└── README.md                # Config documentation
```

#### 4.3 Add Configuration Documentation

```markdown
<!-- config/README.md -->
# Quotey Configuration

## Environment Variables

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `QUOTEY_DATABASE_URL` | SQLite database URL | No | `sqlite://quotey.db` |
| `SLACK_APP_TOKEN` | Slack App-level token | Yes | - |
| `SLACK_BOT_TOKEN` | Slack Bot token | Yes | - |
| `QUOTEY_LLM_PROVIDER` | LLM provider (ollama/openai) | No | `ollama` |

## Profiles

Run with specific profile:
```bash
QUOTEY_PROFILE=production cargo run
```
```

---

## 5. Quality Gates & Tooling (Partial)

### Current State
- ✅ `deny.toml` for cargo-deny
- ✅ `quality-gates.sh` script exists
- ❌ No pre-commit hooks
- ❌ No automated benchmark tracking
- ❌ No code coverage reporting

### Recommendations

#### 5.1 Add Pre-Commit Hooks

```yaml
# .pre-commit-config.yaml
repos:
  - repo: local
    hooks:
      - id: cargo-fmt
        name: Cargo fmt
        entry: cargo fmt --all -- --check
        language: system
        pass_filenames: false
      
      - id: cargo-clippy
        name: Cargo clippy
        entry: cargo clippy --all-targets --all-features -- -D warnings
        language: system
        pass_filenames: false
      
      - id: cargo-test
        name: Cargo test
        entry: cargo test
        language: system
        pass_filenames: false
      
      - id: beads-sync
        name: Beads sync check
        entry: scripts/check-beads-sync.sh
        language: script
        pass_filenames: false
      
      - id: ubs-check
        name: UBS static analysis
        entry: ubs
        language: system
        files: \.rs$
```

**Setup script:**
```bash
# scripts/setup-precommit.sh
#!/bin/bash
pip install pre-commit
pre-commit install
```

#### 5.2 Enhanced Quality Gates Script

```bash
#!/bin/bash
# scripts/quality-gates.sh (enhanced)

set -e

echo "=== Quotey Quality Gates ==="

# Format check
echo "→ Checking formatting..."
cargo fmt --all -- --check

# Clippy
echo "→ Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# Tests
echo "→ Running tests..."
cargo test --all-features

# Documentation tests
echo "→ Testing documentation..."
cargo test --doc

# Security audit
echo "→ Running security audit..."
cargo deny check advisories

# License check
echo "→ Checking licenses..."
cargo deny check licenses

# Beads sync check
echo "→ Checking beads sync..."
if [ -n "$(git status --porcelain .beads/ 2>/dev/null)" ]; then
    echo "⚠️  Warning: .beads/ directory not synced"
    echo "   Run: br sync --flush-only"
fi

# UBS check (if available)
if command -v ubs &> /dev/null; then
    echo "→ Running UBS static analysis..."
    ubs .
fi

echo "✅ All quality gates passed"
```

#### 5.3 Add Code Coverage

```yaml
# .github/workflows/coverage.yml
name: Coverage

on:
  push:
    branches: [main]

jobs:
  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      
      - name: Install tarpaulin
        run: cargo install cargo-tarpaulin
      
      - name: Generate coverage
        run: cargo tarpaulin --all-features --workspace --timeout 120 --out xml
      
      - name: Upload to Codecov
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
```

---

## 6. Project Metadata & Documentation (Good)

### Current State
- ✅ `README.md` exists
- ✅ `AGENTS.md` comprehensive
- ✅ `CLAUDE.md` focused
- ✅ `.planning/` with research documents
- ❌ No `CONTRIBUTING.md`
- ❌ No `CHANGELOG.md`
- ❌ No security policy

### Recommendations

#### 6.1 Add CONTRIBUTING.md

```markdown
# Contributing to Quotey

## Development Setup

1. Install Rust (1.75+)
2. Install beads: `brew install dicklesworthstone/tap/br`
3. Install cass-memory: `brew install dicklesworthstone/tap/cm`
4. Run quality gates: `./scripts/quality-gates.sh`

## Workflow

1. Find work: `br ready --json`
2. Claim: `br update <id> --status in_progress`
3. Implement with tests
4. Run quality gates
5. Close: `br close <id> --reason "Implemented"`
6. Commit `.beads/` with code

## Research

Before implementing, check:
- `.planning/RESEARCH_*.md` for relevant research
- `cm context "<task>" --json` for institutional memory
```

#### 6.2 Add CHANGELOG.md

```markdown
# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Policy evaluation persistence model (bd-imtv)
- Dicklesworthstone Stack integration guide

### Changed
- Enhanced AGENTS.md with cass-memory instructions

## [0.1.1] - 2026-02-20
...
```

---

## 7. Security & Compliance (Needs Work)

### Current State
- ✅ `secrecy` crate in dependencies
- ✅ cargo-deny for license checking
- ❌ No `SECURITY.md`
- ❌ No dependency vulnerability scanning in CI
- ❌ No secrets scanning

### Recommendations

#### 7.1 Add SECURITY.md

```markdown
# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅ Yes |

## Reporting a Vulnerability

Please report vulnerabilities to: security@quotey.dev

## Security Measures

- All pricing decisions are deterministic and auditable
- LLMs do not make financial decisions
- All database access is through parameterized queries (sqlx)
- Secrets use the `secrecy` crate for zeroization
```

#### 7.2 Add Dependency Vulnerability Scanning

```yaml
# .github/workflows/security.yml
name: Security Audit

on:
  schedule:
    - cron: '0 0 * * *'  # Daily
  push:
    paths:
      - '**/Cargo.toml'
      - '**/Cargo.lock'

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
```

---

## 8. Crate Configuration Review

### 8.1 Core Crate (crates/core/Cargo.toml)

**Current:**
```toml
[dependencies]
async-trait.workspace = true
chrono.workspace = true
hmac = "0.12"
rust_decimal.workspace = true
...
```

**Recommendations:**
- ✅ Good: Uses workspace dependencies for consistency
- ✅ Good: Minimal dependencies (hmac, sha2 only for crypto)
- ⚠️ Consider: Add `tracing` for structured logging

### 8.2 DB Crate (crates/db/Cargo.toml)

**Current:**
```toml
[dependencies]
sqlx.workspace = true
...
```

**Recommendations:**
- ✅ Good: sqlx with all required features
- ⚠️ Consider: Add connection pooling metrics

### 8.3 Feature Flags Strategy

**Recommendation:** Add feature flags for optional functionality:

```toml
# crates/core/Cargo.toml
[features]
default = ["std"]
std = []
ml = ["linfa", "smartcore"]  # Machine learning (Deal DNA)
full = ["std", "ml"]
```

---

## 9. Documentation Website (Future)

### Recommendation: Use frankentui for CLI docs

Based on `RESEARCH_DICKLESWORTHSTONE_PROJECTS.md`, frankentui provides:
- Inline TUI mode (UI at top, logs scroll)
- Deterministic rendering
- Perfect for CLI documentation viewer

**Future enhancement:**
```bash
quotey docs --tui  # Open interactive documentation viewer
```

---

## Implementation Priority Matrix

| Priority | Category | Improvement | Effort | Impact | Owner |
|----------|----------|-------------|--------|--------|-------|
| P0 | CI/CD | GitHub Actions workflow | M | 🔥 High | DevOps |
| P0 | Quality | Pre-commit hooks | L | 🔥 High | Any |
| P1 | DevEnv | VS Code settings | L | Medium | Any |
| P1 | Tooling | cass-memory integration | M | 🔥 High | Research |
| P1 | Config | Config validation | M | Medium | Core |
| P2 | Security | SECURITY.md | L | Medium | Any |
| P2 | Security | Vulnerability scanning | L | Medium | DevOps |
| P2 | Docs | CONTRIBUTING.md | L | Low | Any |
| P3 | Tooling | frankensearch integration | M | Medium | ML |
| P3 | Docs | CHANGELOG.md | L | Low | Any |

---

## Summary Checklist

### Must Do (P0)
- [ ] Add `.github/workflows/ci.yml`
- [ ] Add `.pre-commit-config.yaml`
- [ ] Enhance `scripts/quality-gates.sh`

### Should Do (P1)
- [ ] Add `.vscode/settings.json`
- [ ] Add `.vscode/extensions.json`
- [ ] Add configuration validation (`validator` crate)
- [ ] Document cass-memory integration in AGENTS.md

### Could Do (P2)
- [ ] Add `SECURITY.md`
- [ ] Add `CONTRIBUTING.md`
- [ ] Add `CHANGELOG.md`
- [ ] Add code coverage workflow

### Future (P3)
- [ ] Integrate frankensearch for Deal DNA
- [ ] Add frankentui-based CLI docs viewer
- [ ] Consider asupersync migration (post-production)

---

## Appendix: Configuration Files Reference

| File | Purpose | Status |
|------|---------|--------|
| `Cargo.toml` | Workspace definition | ✅ Good |
| `rust-toolchain.toml` | Rust version | ✅ Good |
| `rustfmt.toml` | Formatting rules | ✅ Good |
| `deny.toml` | License/security audit | ✅ Good |
| `config/quotey.example.toml` | Example config | ⚠️ Needs validation |
| `.planning/config.json` | Planning mode | ✅ Good |
| `AGENTS.md` | Agent instructions | ✅ Comprehensive |
| `CLAUDE.md` | Claude-specific | ✅ Focused |

---

*Document Version: 1.0*  
*Review Date: 2026-02-24*  
*Next Review: After Wave 2 completion*
