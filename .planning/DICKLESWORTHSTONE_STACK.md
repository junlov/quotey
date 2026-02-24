# The Dicklesworthstone Stack

**A Comprehensive Guide to Jeffrey Emanuel's AI-Native Developer Tool Ecosystem**

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [The Ecosystem Philosophy](#the-ecosystem-philosophy)
3. [Core Stack Components](#core-stack-components)
   - beads_rust (Issue Tracking)
   - mcp_agent_mail_rust (Multi-Agent Coordination)
   - frankensearch (Hybrid Search)
   - cass_memory (Cross-Agent Memory)
4. [Supporting Infrastructure](#supporting-infrastructure)
   - asupersync (Async Runtime)
   - frankentui (Terminal UI)
   - fastmcp_rust (MCP Framework)
5. [Integration Matrix for Quotey](#integration-matrix-for-quotey)
6. [Architecture Patterns](#architecture-patterns)
7. [Usage Examples](#usage-examples)
8. [Decision Guide](#decision-guide)

---

## Executive Summary

The Dicklesworthstone Stack is a cohesive ecosystem of tools designed for **AI-first development workflows**. Built primarily in Rust with a focus on local-first operation, deterministic behavior, and AI agent integration, these tools form a complete pipeline for:

- **Issue Tracking** (beads_rust)
- **Agent Coordination** (mcp_agent_mail_rust)
- **Knowledge Retrieval** (frankensearch, cass_memory)
- **User Interface** (frankentui)
- **Async Execution** (asupersync)

All tools share common design principles:
- `--json` robot mode for machine-readable I/O
- SQLite + JSONL for git-friendly storage
- Deterministic, auditable behavior
- Local-first (works offline)
- Never auto-destructive

---

## The Ecosystem Philosophy

### Design Principles

| Principle | Implementation |
|-----------|---------------|
| **Local-First** | SQLite backends, JSONL serialization, offline operation |
| **Agent-Native** | All tools support `--json` for programmatic access |
| **Git-Friendly** | JSONL appends for conflict-free merges |
| **Deterministic** | Structured concurrency, cancel-correctness, replayable tests |
| **Non-Destructive** | Never auto-commits, never deletes without explicit approval |

### The ACE Pipeline (cass_memory)

```
┌─────────────────────────────────────────────────────────────────────┐
│                    EPISODIC MEMORY (cass)                           │
│   Raw session logs from all agents — the "ground truth"             │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ cass search
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    WORKING MEMORY (Diary)                           │
│   Structured session summaries bridging raw logs to rules           │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ reflect + curate
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    PROCEDURAL MEMORY (Playbook)                     │
│   Distilled rules with confidence tracking                          │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Core Stack Components

---

### 1. beads_rust (br) — Issue Tracking

**Repository:** https://github.com/Dicklesworthstone/beads_rust  
**Language:** Rust  
**Stars:** 592  
**Status:** Production Ready

**What it is:**
Rust port of Steve Yegge's beads issue tracker. Local-first issue tracking that lives in your repo, never touches your source code, and works offline.

**Key Features:**
- **Non-invasive:** JSONL files in `.beads/`, never touches source
- **Git-friendly:** Append-only JSONL merges cleanly
- **Dependency tracking:** Blockers, dependents, cycles detected
- **AI-native:** `--json` flag for all commands
- **Offline-first:** SQLite + JSONL, no cloud required

**Command Reference:**
```bash
# Initialize
br init

# Create issues
br create "Fix pricing bug" -t bug -p 1 --json
br create "Add discount feature" -t feature -p 2 --deps br-123 --json

# View work
br ready --json                    # Unblocked, actionable work
br blocked --json                  # Issues with unresolved deps
br triage --json                   # Stale issues needing attention

# Update progress
br update br-42 --status in_progress --json
br update br-42 --priority 1 --json
br close br-42 --reason "Fixed in #123" --json

# Sync with git
br sync --flush-only               # Export issues.jsonl
```

**Data Model:**
```rust
struct Issue {
    id: String,              // "br-42"
    title: String,
    status: Status,          // open | in_progress | done | wontfix
    priority: u8,            // 0=critical, 4=backlog
    deps: Vec<String>,       // ["br-123", "discovered-from:br-456"]
    created_at: DateTime,
    updated_at: DateTime,
}
```

**Quotey Integration:**
- ✅ Already integrated (using `br` for all issue tracking)
- Link quotes to beads issues for full traceability
- Track CPQ feature dependencies
- Priority-based CPQ workflow routing

---

### 2. mcp_agent_mail_rust — Multi-Agent Coordination

**Repository:** https://github.com/Dicklesworthstone/mcp_agent_mail_rust  
**Language:** Rust  
**Stars:** 18  
**Status:** Production Ready

**What it is:**
MCP (Model Context Protocol) server for multi-agent coordination. Enables agents to send messages, reserve files, and coordinate work without stepping on each other.

**Key Features:**
- **Advisory file reservations:** Prevent conflicting edits
- **Threaded messaging:** Inbox/outbox with searchable threads
- **Git-backed archive:** All conversations in git
- **15-screen TUI:** Operations console for monitoring
- **34 MCP tools:** Complete coordination API

**Core Tools:**
```rust
// Identity & Projects
ensure_project(project_key: String)
register_agent(project_key, agent_name, capabilities)

// File Reservations (Advisory)
file_reservation_paths(project_key, agent, paths, ttl, exclusive)
file_reservation_release(project_key, reservation_id)
query_reservations(project_key, path_pattern)

// Messaging
send_message(from, to, subject, body, thread_id)
fetch_inbox(project_key, agent_name)
acknowledge_message(message_id)

// Macros
macro_start_session(project_key, agent_name, task)
macro_file_reservation_cycle(project_key, agent, paths)
macro_contact_handshake(from_agent, to_agent, task)
```

**Resource URIs:**
```
resource://inbox/{Agent}?project=/path/to/quotey&limit=20
resource://thread/{id}?project=/path/to/quotey&include_bodies=true
resource://agent/{Agent}?project=/path/to/quotey
```

**File Reservation Conflict Resolution:**
```rust
// Agent A reserves crates/cpq/**
file_reservation_paths("/quotey", "Agent-A", ["crates/cpq/**"], 3600, true)

// Agent B tries to reserve overlapping path
file_reservation_paths("/quotey", "Agent-B", ["crates/cpq/pricing.rs"], 3600, true)
// → FILE_RESERVATION_CONFLICT error
// Solutions: wait for expiry, adjust patterns, or use exclusive=false
```

**Quotey Integration:**
- ✅ Already integrated via MCP
- Coordinate multiple CPQ agents on complex quotes
- Reserve CPQ modules during pricing engine work
- Thread-based discussion of approval policies

---

### 3. frankensearch — Hybrid Search

**Repository:** https://github.com/Dicklesworthstone/frankensearch  
**Language:** Rust  
**Stars:** 31  
**Status:** Production Ready

**What it is:**
Two-tier hybrid search combining fast lexical search (Tantivy BM25) with quality semantic search (MiniLM embeddings). Sub-millisecond initial results, ~150ms refinement.

**Key Features:**
- **Tier 1 (Fast):** Tantivy BM25 lexical search
- **Tier 2 (Quality):** potion-128M semantic embeddings
- **Tier 3 (Precision):** MiniLM-L6-v2 cross-encoding
- **Progressive delivery:** Results stream as quality improves
- **Local-only:** No external APIs required
- **TOON format:** Token-efficient for agents

**Architecture:**
```
┌─────────────────────────────────────────────────────────────┐
│  User Query: "enterprise pricing with volume discounts"      │
└───────────────────────────┬─────────────────────────────────┘
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  TIER 1: Tantivy BM25 (lexical)                             │
│  → "enterprise", "pricing", "volume", "discounts"           │
│  Results in <1ms                                            │
└───────────────────────────┬─────────────────────────────────┘
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  TIER 2: potion-128M semantic (cosine similarity)           │
│  → Query embedding vs document embeddings                   │
│  Results in ~50ms                                           │
└───────────────────────────┬─────────────────────────────────┘
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  TIER 3: MiniLM-L6-v2 cross-encoder (re-ranking)            │
│  → Precise relevance scoring                                │
│  Final results in ~150ms                                    │
└─────────────────────────────────────────────────────────────┘
```

**CLI Usage:**
```bash
# Index a directory
fsfs index ./quotey --db ./search.db

# Search
fsfs search "enterprise pricing" --db ./search.db
fsfs search "volume discount policy" --stream --json

# TOON format (token-efficient for agents)
fsfs search "approval workflow" --toon --json

# Meta search (which index to use)
fsfs meta-search "pricing engine" --db-dir ~/.frankensearch
```

**TOON Format (Token-Optimized Object Notation):**
```json
{
  "r": [                      // results
    {
      "p": "src/cpq/pricing.rs",  // path
      "s": 0.95,                   // score
      "t": "fn calculate_enterprise_price"  // text snippet
    }
  ],
  "q": "enterprise pricing"   // query
}
```

**Quotey Integration:**
- 🔶 **HIGH POTENTIAL** - Not yet integrated
- Enhance Deal DNA similarity search with semantic matching
- Search product catalog by natural language
- Find similar historical quotes
- Index CPQ documentation for agent context

**Proposed Integration:**
```rust
// In quotey core - Deal DNA similarity
pub struct SemanticDealSearcher {
    index: frankensearch::Index,
}

impl SemanticDealSearcher {
    pub fn find_similar_deals(&self, deal: &DealDna) -> Vec<SimilarDeal> {
        // Hybrid search: lexical (customer name, industry) + semantic (deal characteristics)
        self.index.search(&deal.to_search_query())
    }
}
```

---

### 4. cass_memory — Cross-Agent Memory

**Repository:** https://github.com/Dicklesworthstone/cass_memory_system  
**Language:** TypeScript  
**Stars:** 241  
**Status:** Production Ready

**What it is:**
Procedural memory system for AI coding agents. Transforms raw session logs into actionable rules with confidence tracking. Cross-agent learning means Cursor discoveries help Claude Code.

**Key Features:**
- **Cross-agent learning:** Unified playbook from all agents
- **Three-layer memory:** Episodic (cass) → Working (Diary) → Procedural (Playbook)
- **Confidence decay:** Rules fade without validation (90-day half-life)
- **Anti-pattern learning:** Bad rules become warnings
- **Scientific validation:** New rules require historical evidence

**The ACE Pipeline:**
```
┌─────────────────────────────────────────────────────────────────────┐
│  EPISODIC MEMORY (cass)                                             │
│  Raw session logs from: Claude Code, Cursor, Codex, Aider, etc.     │
│  → Searchable ground truth                                          │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ cass search "pricing bug"
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│  WORKING MEMORY (Diary)                                             │
│  Structured session summaries                                       │
│  accomplishments │ decisions │ challenges │ outcomes                │
└───────────────────────────┬─────────────────────────────────────────┘
                            │ reflect + curate
                            ▼
┌─────────────────────────────────────────────────────────────────────┐
│  PROCEDURAL MEMORY (Playbook)                                       │
│  Distilled rules with confidence tracking                           │
│  Rules │ Anti-patterns │ Feedback │ Decay                           │
└─────────────────────────────────────────────────────────────────────┘
```

**Confidence Decay System:**
```
Rule: "Always validate discount thresholds before approval"

Helpful marks: +8 (over 6 months)
Harmful marks: 0
Confidence: 0.95 (proven)

After 90 days without validation:
Confidence: 0.475 (established)

After 180 days:
Confidence: 0.237 (candidate)

After harmful mark (mistake found):
Confidence drops 4x faster
→ May invert to anti-pattern: "PITFALL: Don't skip discount validation"
```

**CLI Usage:**
```bash
# Get task-specific memory
cm context "implement approval workflow" --json

# Returns:
{
  "relevantBullets": [
    {
      "id": "rule-123",
      "text": "Always check approval thresholds before pricing",
      "confidence": 0.95,
      "maturity": "proven"
    }
  ],
  "antiPatterns": [
    {
      "id": "anti-456",
      "text": "PITFALL: Don't cache approval decisions without TTL",
      "confidence": 0.88
    }
  ],
  "historySnippets": [
    {
      "session": "2026-01-15.jsonl",
      "relevance": 0.92,
      "excerpt": "Fixed approval bug by adding threshold validation..."
    }
  ]
}

# Onboarding workflow
cm onboard status --json              # Check what's indexed
cm onboard sample --fill-gaps --json  # Build playbook from history
cm onboard read /path/to/session.jsonl --template --json
cm onboard mark-done /path/to/session.jsonl

# Quickstart
cm quickstart --json
```

**Quotey Integration:**
- 🔶 **HIGH POTENTIAL** - Not yet integrated
- Build CPQ institutional memory
- Remember pricing strategies that worked
- Learn from past quote negotiations
- Share knowledge across CPQ agent sessions

**Proposed Integration:**
```rust
// Before starting CPQ task
let memory = cm_context("price enterprise software deal").await;

// Apply learned rules
for rule in memory.relevant_bullets {
    if rule.confidence > 0.8 {
        apply_pricing_strategy(&rule.text);
    }
}

// Avoid known pitfalls
for anti in memory.anti_patterns {
    println!("WARNING: {}", anti.text);
}
```

---

## Supporting Infrastructure

---

### 5. asupersync — Async Runtime

**Repository:** https://github.com/Dicklesworthstone/asupersync  
**Language:** Rust  
**Stars:** 147  
**Status:** Nightly Required (Rust 2024)

**What it is:**
Structured concurrency runtime for Rust. Cancel-correct async, deterministic testing via LabRuntime, budget-based resource management.

**Key Concepts:**
| Concept | Description |
|---------|-------------|
| **Structured Concurrency** | Tasks belong to regions; no orphan tasks |
| **Cancel-Correctness** | All blocking ops have cancellation points |
| **Budgets** | Deadline + poll quota + cost quota (product semiring) |
| **LabRuntime** | Deterministic testing with synthetic time |

**Critical Constraint:** Requires Rust 2024 edition + nightly compiler.

**See:** `.planning/RESEARCH_ASUPERSYNC.md` for detailed analysis.

**Quotey Integration:**
- 🔴 **HIGH EFFORT** - Would replace Tokio entirely
- Cancel-correct async for CPQ operations
- Deterministic testing for pricing engine
- Budget-based timeouts for approval workflows

**Migration Complexity:**
```
Current:  tokio = { version = "1", features = ["full"] }
Future:   asupersync = { path = "../asupersync" }

Changes Required:
- 97 test files need async runtime update
- All spawn!() calls need region contexts
- All async fns need checkpoint() calls
- Cargo.toml toolchain: nightly + 2024 edition
- CI/CD updates for nightly Rust
```

**Recommendation:** Defer until quotey has:
- Stable production release
- Comprehensive test coverage
- Bandwidth for major refactoring

---

### 6. frankentui — Terminal UI

**Repository:** https://github.com/Dicklesworthstone/frankentui  
**Language:** Rust  
**Stars:** 189  
**Status:** Production Ready (Crates.io: ftui-core, ftui-layout, ftui-i18n)

**What it is:**
Minimal, high-performance TUI kernel with diff-based rendering, inline mode, and RAII cleanup. Deterministic output for testing.

**Key Features:**
- **Inline mode:** UI at top/bottom, logs scroll above
- **Deterministic rendering:** Buffer → Diff → Presenter → ANSI
- **One-writer rule:** `TerminalWriter` owns all stdout writes
- **RAII cleanup:** Terminal restored even on panic
- **Elm architecture:** Model → Update → View → Runtime

**Comparison:**
| Feature | FrankenTUI | Ratatui |
|---------|-----------|---------|
| Inline mode w/ scrollback | ✅ First-class | ⚠️ App-specific |
| Deterministic buffer diff | ✅ Kernel-level | ✅ Yes |
| One-writer rule | ✅ Enforced | ⚠️ App-specific |
| RAII teardown | ✅ TerminalSession | ⚠️ App-specific |
| Snapshot/time-travel | ✅ Built-in | ❌ No |

**Quotey Integration:**
- 🔶 **LOW PRIORITY** - Quotey is Slack-first
- Could enhance CLI with better UI
- Not critical for CPQ workflows

---

### 7. fastmcp_rust — MCP Framework

**Repository:** https://github.com/Dicklesworthstone/fastmcp_rust  
**Language:** Rust  
**Stars:** 12  
**Status:** Alpha

**What it is:**
MCP server framework with cancel-correct async, attribute macros, and structured concurrency.

**Key Features:**
- `#[tool]`, `#[resource]`, `#[prompt]` macros
- Four-valued `Outcome<T, E>`: Ok/Err/Cancelled/Panicked
- Budget-based timeouts
- asupersync runtime

**Quotey Integration:**
- 🔶 **LOW PRIORITY** - mcp_agent_mail_rust already integrated
- Consider if building custom MCP server

---

## Integration Matrix for Quotey

### Current State

| Component | Status | Usage | Priority |
|-----------|--------|-------|----------|
| beads_rust | ✅ Integrated | Issue tracking via `br` | Core |
| mcp_agent_mail_rust | ✅ Integrated | Agent coordination | Core |

### Integration Candidates

| Component | Effort | Impact | Priority | Timeline |
|-----------|--------|--------|----------|----------|
| frankensearch | Medium | High | ⭐⭐⭐ HIGH | Next Month |
| cass_memory | Medium | High | ⭐⭐⭐ HIGH | Next Month |
| asupersync | Very High | Very High | ⭐⭐ MEDIUM | Future |
| frankentui | High | Low | ⭐ LOW | Deferred |
| fastmcp_rust | Medium | Low | ⭐ LOW | Deferred |

### Integration Roadmap

```
Phase 1: NOW (Already Integrated)
├── beads_rust ──────────────●───── Issue tracking
└── mcp_agent_mail_rust ─────●───── Agent coordination

Phase 2: NEXT MONTH (High Value)
├── frankensearch ───────────○───── Deal DNA semantic search
│   └── Index: products, quotes, policies
│   └── Search: "similar enterprise deals"
└── cass_memory ─────────────○───── CPQ institutional memory
    └── Learn: pricing strategies
    └── Remember: approval patterns

Phase 3: FUTURE (Strategic)
└── asupersync ──────────────○───── Cancel-correct CPQ engine
    └── Structured concurrency
    └── Deterministic testing
```

---

## Architecture Patterns

### Pattern 1: Git-Friendly Storage

All tools use **append-only JSONL** for git compatibility:

```
.beads/
├── issues.jsonl          # {"id":"br-1",...}\n{"id":"br-2",...}
├── issues.jsonl.idx      # SQLite index for queries
└── config.json           # {"counter": 42}

.mcp_agent_mail/
├── archive/
│   └── 2026-01/
│       └── thread-*.jsonl
└── index.db              # SQLite for fast queries

.cass/
├── playbook.jsonl        # Distilled rules
├── diary/
│   └── 2026-01-15.jsonl  # Session summaries
└── config.toml
```

**Why JSONL:**
- Append-only → No merge conflicts
- Line-based → Git diff friendly
- Streamable → Process large files incrementally
- Human-readable → Debuggable

### Pattern 2: Robot Mode

All tools support `--json` for programmatic access:

```bash
# Human mode (TUI/output)
br ready

# Robot mode (JSON)
br ready --json
# → {"issues":[{"id":"br-42",...}]}
```

**Benefits:**
- Agents parse output deterministically
- No regex/screen scraping
- Versioned JSON schemas
- Errors in JSON format

### Pattern 3: Advisory Reservations

File reservations are **advisory, not mandatory**:

```rust
// Agent A reserves
file_reservation_paths("/quotey", "Agent-A", ["crates/cpq/**"], 3600, true)

// Agent B can still edit (but shouldn't)
// Pre-commit hook warns: "Agent-A has reservation on crates/cpq/pricing.rs"
```

**Design Philosophy:**
- Trust agents to cooperate
- Don't block emergency fixes
- Audit trail of conflicts

### Pattern 4: Local-First with Optional Sync

```
┌─────────────────────────────────────┐
│  LOCAL (SQLite + JSONL)             │
│  ├── beads.db                       │
│  ├── mcp_mail.db                    │
│  └── cass.db                        │
└─────────────────────────────────────┘
           │
           │ Optional: Sync to cloud
           ▼
┌─────────────────────────────────────┐
│  REMOTE (Optional)                  │
│  ├── GitHub/GitLab (JSONL)          │
│  └── S3/R2 (Backups)                │
└─────────────────────────────────────┘
```

### Pattern 5: Deterministic Testing

```rust
// asupersync LabRuntime
#[lab_test]
fn test_pricing_engine() {
    let mut lab = LabRuntime::new();
    lab.set_time(Instant::from_secs(1000));
    
    let result = lab.run(async {
        // Deterministic: same input → same output
        pricing_engine.calculate(deal).await
    });
    
    assert_snapshot!(result);
}
```

---

## Usage Examples

### Example 1: Starting a CPQ Session

```bash
# 1. Check for actionable work
br ready --json | jq '.issues[] | select(.priority <= 1)'

# 2. Claim the issue
br update br-123 --status in_progress --json

# 3. Get relevant memory
cm context "implement volume discount pricing" --json

# 4. Reserve CPQ module
mcp-tool file_reservation_paths \
  --project_key /data/projects/quotey \
  --agent_name CPQ-Agent \
  --paths '["crates/cpq/src/pricing.rs"]' \
  --ttl_seconds 3600 \
  --exclusive true

# 5. Implement with learned rules in mind
# ... coding ...

# 6. Run quality gates
cargo test
cargo clippy -- -D warnings
ubs .

# 7. Close issue
br update br-123 --status done --reason "Implemented in #456"

# 8. Release reservation
mcp-tool file_reservation_release --reservation_id res-789
```

### Example 2: Finding Similar Deals

```rust
use frankensearch::Index;

pub struct DealSearcher {
    index: Index,
}

impl DealSearcher {
    pub fn find_similar(&self, deal: &DealDna) -> Vec<SimilarDeal> {
        // Hybrid search query
        let query = format!(
            "{} enterprise {} seats {} discount",
            deal.customer.industry,
            deal.metrics.total_seats,
            deal.pricing.discount_percent
        );
        
        // Tier 1: Lexical (BM25)
        // Tier 2: Semantic (embeddings)
        // Tier 3: Re-rank (cross-encoder)
        self.index.search(&query)
            .limit(5)
            .min_score(0.7)
            .execute()
    }
}
```

### Example 3: Agent Coordination Thread

```rust
// Agent A: Pricing specialist
mcp_tool!("send_message", json!({
    "from_agent": "Pricing-Agent",
    "to_agent": "Approval-Agent",
    "subject": "Discount threshold exceeded",
    "body": "Deal ACME-2026 requires 35% discount, above 30% threshold",
    "thread_id": "QUOTE-ACME-2026"
}));

// Agent B: Approval specialist
let inbox = mcp_tool!("fetch_inbox", json!({
    "project_key": "/data/projects/quotey",
    "agent_name": "Approval-Agent"
}));

// Process and respond
mcp_tool!("send_message", json!({
    "from_agent": "Approval-Agent",
    "to_agent": "Pricing-Agent",
    "subject": "RE: Discount threshold exceeded",
    "body": "Approved with VP override. See thread for details.",
    "thread_id": "QUOTE-ACME-2026"
}));
```

### Example 4: Building CPQ Memory

```bash
# After completing complex pricing negotiation...

# 1. Index session for cass
cm onboard read /data/sessions/2026-02-24.jsonl --template --json

# 2. Extract rules
cm onboard sample --fill-gaps --json
# → Extracts: "For enterprise deals >$100K, lead with 20% discount anchor"

# 3. Future agent benefits
cm context "price enterprise deal over $100K" --json
# → Returns the learned rule with 95% confidence
```

---

## Decision Guide

### When to Use Each Component

```
Need issue tracking? ──► beads_rust
  ├─ Local-first ✓
  ├─ Git-friendly ✓
  └─ AI-native ✓

Need agent coordination? ──► mcp_agent_mail_rust
  ├─ File reservations ✓
  ├─ Threaded messaging ✓
  └─ Git-backed ✓

Need semantic search? ──► frankensearch
  ├─ Hybrid BM25 + embeddings ✓
  ├─ Local-only ✓
  └─ Progressive delivery ✓

Need institutional memory? ──► cass_memory
  ├─ Cross-agent learning ✓
  ├─ Confidence tracking ✓
  └─ Anti-pattern detection ✓

Need cancel-correct async? ──► asupersync
  ├─ Structured concurrency ✓
  ├─ Deterministic testing ✓
  └─ Budget-based timeouts ✓
  ⚠️ Requires Rust 2024 + nightly

Need TUI? ──► frankentui
  ├─ Inline mode ✓
  ├─ Deterministic rendering ✓
  └─ RAII cleanup ✓

Need MCP server framework? ──► fastmcp_rust
  ├─ Attribute macros ✓
  ├─ Cancel-correct ✓
  └─ asupersync runtime ✓
```

### Migration Decision Tree

```
Is quotey in production?
├── NO ──► Can experiment with asupersync
└── YES ──► Defer asupersync migration

Do you need semantic search now?
├── YES ──► Integrate frankensearch (medium effort)
└── NO ──► Defer

Do you have multiple agents working?
├── YES ──► Use mcp_agent_mail (already integrated)
└── NO ──► Still useful for future-proofing

Want to capture institutional knowledge?
├── YES ──► Integrate cass_memory (medium effort)
└── NO ──► Defer until scaling team
```

---

## Summary

The Dicklesworthstone Stack provides a **complete toolchain for AI-native development**:

1. **beads_rust** - Track work locally, git-friendly
2. **mcp_agent_mail_rust** - Coordinate agents without conflict
3. **frankensearch** - Find relevant code and docs intelligently
4. **cass_memory** - Learn from every session
5. **asupersync** - Reliable, testable async (future)
6. **frankentui** - Beautiful, correct terminal UI (optional)

For **quotey specifically**:

| Component | Recommendation |
|-----------|---------------|
| beads_rust | ✅ Continue using |
| mcp_agent_mail_rust | ✅ Continue using |
| frankensearch | 🔶 Integrate for Deal DNA search |
| cass_memory | 🔶 Integrate for CPQ memory |
| asupersync | ⏸️ Defer until post-production |
| frankentui | ⏸️ Low priority (Slack-first) |

**Next Actions:**
1. ✅ Maintain beads/mcp_agent_mail integration
2. 🔶 Evaluate frankensearch for Deal DNA (next month)
3. 🔶 Evaluate cass_memory for CPQ knowledge (next month)
4. ⏸️ Monitor asupersync for future migration

---

## References

- **beads_rust:** https://github.com/Dicklesworthstone/beads_rust
- **mcp_agent_mail_rust:** https://github.com/Dicklesworthstone/mcp_agent_mail_rust
- **frankensearch:** https://github.com/Dicklesworthstone/frankensearch
- **cass_memory:** https://github.com/Dicklesworthstone/cass_memory_system
- **asupersync:** https://github.com/Dicklesworthstone/asupersync
- **frankentui:** https://github.com/Dicklesworthstone/frankentui
- **fastmcp_rust:** https://github.com/Dicklesworthstone/fastmcp_rust

**Related Documents:**
- `.planning/RESEARCH_ASUPERSYNC.md` - Detailed asupersync analysis
- `.planning/RESEARCH_DICKLESWORTHSTONE_PROJECTS.md` - Initial research

---

*Document Version: 1.0*  
*Stack Version: 2026-02*  
*Research Agent: ResearchAgent*
