# Quotey Configuration Review & Improvement Recommendations

**Review Date:** 2026-02-24  
**Reviewer:** ResearchAgent  
**Scope:** Project configuration, tooling, workflow, and integration gaps

---

## Executive Summary

Based on comprehensive research of the quotey codebase and the Dicklesworthstone ecosystem, this review identifies **18 improvement opportunities** across 6 categories. The project has a solid foundation but could significantly benefit from enhanced tooling integration, configuration hardening, and workflow optimizations.

### Top 5 Priority Improvements

| Priority | Improvement | Impact | Effort |
|----------|-------------|--------|--------|
| 1 | **Enhanced local quality gates** | Prevent bad commits, automate verification | Medium |
| 2 | **Integrate cass-memory** | Cross-agent learning, institutional knowledge | Medium |
| 3 | **Add pre-commit hooks** | Automate quality checks before commit | Low |
| 4 | **Add VS Code workspace settings** | Consistent dev experience, faster onboarding | Low |
| 5 | **Configuration validation** | Catch config errors at startup | Medium |

---

## 1. Quality Gates & Automation (Needs Enhancement)

### Current State
- ✅ `deny.toml` for cargo-deny
- ✅ `quality-gates.sh` script exists
- ❌ No pre-commit hooks
- ❌ No automated benchmark tracking
- ❌ No code coverage reporting

### Research-Based Recommendations

#### 1.1 Enhanced Quality Gates Script (HIGH PRIORITY)

**Current `scripts/quality-gates.sh` is minimal. Expand it:**

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

# Security audit (if cargo-deny installed)
if command -v cargo-deny &> /dev/null; then
    echo "→ Running security audit..."
    cargo deny check advisories
    echo "→ Checking licenses..."
    cargo deny check licenses
else
    echo "⚠️  cargo-deny not installed. Skipping security/license audit."
    echo "   Install: cargo install cargo-deny"
fi

# Beads sync check
echo "→ Checking beads sync..."
if [ -n "$(git status --porcelain .beads/ 2>/dev/null)" ]; then
    echo "⚠️  Warning: .beads/ directory not synced"
    echo "   Run: br sync --flush-only"
    exit 1
fi

# UBS check (if available)
if command -v ubs &> /dev/null; then
    echo "→ Running UBS static analysis..."
    ubs .
fi

echo "✅ All quality gates passed"
```

**Usage:**
```bash
# Make executable
chmod +x scripts/quality-gates.sh

# Run before every commit
./scripts/quality-gates.sh
```

#### 1.2 Add Pre-Commit Hooks (MEDIUM PRIORITY)

Using [pre-commit](https://pre-commit.com/) framework:

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
        files: \.rs$
      
      - id: cargo-clippy
        name: Cargo clippy
        entry: cargo clippy --all-targets --all-features -- -D warnings
        language: system
        pass_filenames: false
        files: \.rs$
      
      - id: cargo-test
        name: Cargo test
        entry: cargo test --all-features
        language: system
        pass_filenames: false
        files: \.rs$
      
      - id: beads-sync
        name: Beads sync check
        entry: scripts/check-beads-sync.sh
        language: script
        pass_filenames: false
```

**Setup script:**
```bash
# scripts/setup-precommit.sh
#!/bin/bash
pip install pre-commit
pre-commit install
```

#### 1.3 Add Git Hook Directly (Alternative to pre-commit framework)

```bash
# .githooks/pre-commit
#!/bin/bash
# Local quality gates before commit

echo "Running quality gates..."

# Format check
if ! cargo fmt --all -- --check; then
    echo "❌ Formatting check failed. Run: cargo fmt --all"
    exit 1
fi

# Clippy
if ! cargo clippy --all-targets --all-features -- -D warnings; then
    echo "❌ Clippy check failed"
    exit 1
fi

# Tests (quick mode - skip integration tests)
if ! cargo test --lib; then
    echo "❌ Tests failed"
    exit 1
fi

# Beads sync check
if [ -n "$(git status --porcelain .beads/ 2>/dev/null)" ]; then
    echo "❌ .beads/ not synced. Run: br sync --flush-only"
    exit 1
fi

echo "✅ Quality gates passed"
```

**Enable git hooks:**
```bash
git config core.hooksPath .githooks
chmod +x .githooks/pre-commit
```

#### 1.4 Add Code Coverage Tracking

```bash
# scripts/coverage.sh
#!/bin/bash

# Install tarpaulin if needed
if ! command -v cargo-tarpaulin &> /dev/null; then
    cargo install cargo-tarpaulin
fi

# Generate coverage report
cargo tarpaulin --all-features --workspace --timeout 120 --out Html --output-dir ./coverage

echo "Coverage report generated at: ./coverage/tarpaulin-report.html"
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

## 5. Security & Compliance (Needs Work)

### Current State
- ✅ `secrecy` crate in dependencies
- ✅ cargo-deny for license checking
- ❌ No `SECURITY.md`
- ❌ No dependency vulnerability scanning
- ❌ No secrets scanning

### Recommendations

#### 5.1 Add SECURITY.md

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

#### 5.2 Add Local Security Auditing

```bash
# scripts/security-audit.sh
#!/bin/bash

echo "=== Security Audit ==="

# Install cargo-deny if needed
if ! command -v cargo-deny &> /dev/null; then
    echo "Installing cargo-deny..."
    cargo install cargo-deny
fi

# Check advisories
echo "→ Checking security advisories..."
cargo deny check advisories

# Check licenses
echo "→ Checking licenses..."
cargo deny check licenses

# Check bans
echo "→ Checking banned crates..."
cargo deny check bans

echo "✅ Security audit complete"
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

### Recommendations

#### 6.1 Add CONTRIBUTING.md

(See CONTRIBUTING.md already added to repo)

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

## 7. Crate Configuration Review

### 7.1 Core Crate (crates/core/Cargo.toml)

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

### 7.2 DB Crate (crates/db/Cargo.toml)

**Current:**
```toml
[dependencies]
sqlx.workspace = true
...
```

**Recommendations:**
- ✅ Good: sqlx with all required features
- ⚠️ Consider: Add connection pooling metrics

### 7.3 Feature Flags Strategy

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

## Implementation Priority Matrix

| Priority | Category | Improvement | Effort | Impact | Owner |
|----------|----------|-------------|--------|--------|-------|
| P0 | Quality | Enhanced quality-gates.sh | M | 🔥 High | Any |
| P0 | Quality | Git pre-commit hooks | L | 🔥 High | Any |
| P1 | DevEnv | VS Code settings | L | Medium | Any |
| P1 | Tooling | cass-memory integration | M | 🔥 High | Research |
| P1 | Config | Config validation | M | Medium | Core |
| P2 | Security | SECURITY.md | L | Medium | Any |
| P2 | Security | security-audit.sh | L | Medium | Any |
| P2 | Docs | CHANGELOG.md | L | Low | Any |
| P3 | Tooling | frankensearch integration | M | Medium | ML |
| P3 | Tooling | frankentui CLI docs | H | Low | Future |

---

## Summary Checklist

### Must Do (P0)
- [x] Add enhanced `scripts/quality-gates.sh`
- [ ] Add `.githooks/pre-commit`
- [ ] Document git hooks setup in CONTRIBUTING.md

### Should Do (P1)
- [x] Add `.vscode/settings.json` ✅
- [x] Add `.vscode/extensions.json` ✅
- [ ] Add configuration validation (`validator` crate)
- [ ] Document cass-memory integration in AGENTS.md

### Could Do (P2)
- [ ] Add `SECURITY.md`
- [ ] Add `scripts/security-audit.sh`
- [ ] Add `CHANGELOG.md`
- [ ] Add code coverage script

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
| `CONTRIBUTING.md` | Contribution guide | ✅ Added |
| `.vscode/settings.json` | VS Code settings | ✅ Added |

---

*Document Version: 1.1 (GitHub Actions removed)*  
*Review Date: 2026-02-24*  
*Next Review: After Wave 2 completion*
