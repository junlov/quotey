# Dicklesworthstone Projects - Integration Research

**Research Agent:** ResearchAgent  
**Date:** 2026-02-24  
**Source:** https://github.com/Dicklesworthstone

---

## Overview

Dicklesworthstone (Jeffrey Emanuel) has created an extensive ecosystem of tools for AI agent coordination, async runtimes, and developer tooling. This research identifies the most relevant projects for integration with **quotey**.

---

## Top Integration Candidates

### 1. **beads_rust** (br) - Issue Tracker ⭐ HIGH PRIORITY
**Repository:** https://github.com/Dicklesworthstone/beads_rust

**What it is:**
- Rust port of Steve Yegge's beads issue tracker
- Local-first: SQLite + JSONL for git-friendly collaboration
- 20K lines of Rust (vs 276K in original Go)
- Non-invasive: never touches source code or auto-commits

**Key Features:**
| Feature | Benefit for Quotey |
|---------|-------------------|
| Works offline | CPQ work doesn't require internet |
| Lives in repo | Issues stay with code |
| Tracks dependencies | Blockers/dependencies for features |
| `--json` API | AI agent integration |
| Git-friendly sync | JSONL merges cleanly |

**Integration Potential:**
- **HIGH** - Quotey already uses beads (br) for issue tracking
- Could enhance bead integration with CPQ-specific workflows
- Link quotes to beads issues for full traceability

**Usage:**
```bash
br init                              # Initialize
br create "Fix pricing bug" -p 1     # Create issue
br ready                             # See actionable work
br sync --flush-only                 # Export for git
```

---

### 2. **mcp_agent_mail_rust** - Multi-Agent Coordination ⭐ HIGH PRIORITY
**Repository:** https://github.com/Dicklesworthstone/mcp_agent_mail_rust

**What it is:**
- Rust MCP server for multi-agent coordination
- 34 tools for identity, messaging, file reservations
- Git-backed archive, SQLite indexing
- 15-screen TUI operations console
- Replaces the original Python mcp_agent_mail (1,725 stars)

**Key Features:**
| Feature | Benefit for Quotey |
|---------|-------------------|
| Advisory file reservations | Prevent agents overwriting each other's CPQ work |
| Threaded inbox/outbox | Coordinate multiple CPQ agents |
| Searchable conversations | Find prior CPQ decisions |
| `--json` robot mode | Machine-readable for agents |
| Pre-commit guard | Block commits touching reserved files |

**Integration Potential:**
- **HIGH** - Quotey already has MCP Agent Mail integration
- Enhance with CPQ-specific message types
- Coordinate multiple CPQ agents on complex quotes

**Usage:**
```rust
// Agent coordination
ensure_project(project_key="/path/to/quotey")
register_agent(project_key, agent_name="CPQ-Agent-1")
file_reservation_paths(project_key, ["crates/cpq/**"], exclusive=true)
send_message(from, to, subject="Starting pricing refactor", thread_id="FEAT-123")
```

---

### 3. **frankensearch** - Hybrid Search ⭐ MEDIUM PRIORITY
**Repository:** https://github.com/Dicklesworthstone/frankensearch

**What it is:**
- Two-tier hybrid search: fast + quality refinement
- Combines lexical (Tantivy BM25) + semantic (vector cosine)
- Sub-millisecond initial results via potion-128M
- Quality refinement in 150ms via MiniLM-L6-v2
- CLI: `fsfs` with progressive delivery

**Key Features:**
| Feature | Benefit for Quotey |
|---------|-------------------|
| Fast semantic search | Find similar quotes instantly |
| BM25 lexical | Exact keyword matching for SKUs |
| Progressive delivery | UI stays responsive |
| Local-only | No external API dependencies |
| TOON format | Token-efficient for agents |

**Integration Potential:**
- **MEDIUM** - Enhance Deal DNA similarity search
- Search product catalog semantically
- Find similar historical quotes
- Index CPQ documentation

**Usage:**
```bash
fsfs index ./quotey                    # Index project
fsfs search "enterprise pricing"        # Search
fsfs search "discount policy" --stream  # Streaming for agents
```

---

### 4. **cass_memory_system** - Cross-Agent Memory ⭐ MEDIUM PRIORITY
**Repository:** https://github.com/Dicklesworthstone/cass_memory_system

**What it is:**
- Procedural memory for AI coding agents
- Three-layer architecture: Episodic → Working → Procedural
- Cross-agent learning: all agents feed unified playbook
- Confidence decay: rules fade without validation
- Anti-pattern learning: bad rules become warnings

**Key Features:**
| Feature | Benefit for Quotey |
|---------|-------------------|
| Cross-agent learning | CPQ knowledge shared across sessions |
| Procedural memory | Distilled CPQ best practices |
| Confidence tracking | Know which rules are reliable |
| `cm context` | Get relevant memory before task |

**Integration Potential:**
- **MEDIUM** - Build CPQ institutional memory
- Remember pricing strategies that worked
- Learn from past quote negotiations
- Share knowledge across CPQ agent sessions

**Usage:**
```bash
cm context "implement approval workflow" --json  # Get relevant rules
cm onboard status --json                          # Check playbook
cm onboard sample --fill-gaps --json              # Build playbook
```

---

### 5. **frankentui** - Terminal UI ⭐ LOW PRIORITY
**Repository:** https://github.com/Dicklesworthstone/frankentui

**What it is:**
- Minimal, high-performance TUI kernel
- Diff-based rendering, inline mode, RAII cleanup
- Elm-architecture runtime (Bubble Tea style)
- Deterministic output for testing

**Key Features:**
| Feature | Benefit for Quotey |
|---------|-------------------|
| Inline mode | UI at top, logs scroll below |
| Deterministic rendering | Testable TUI output |
| RAII cleanup | Terminal restored on panic |
| Composable crates | Use only what you need |

**Integration Potential:**
- **LOW** - Quotey is Slack-first, not TUI-first
- Could enhance CLI with better UI
- Not a priority for CPQ workflows

---

### 6. **fastmcp_rust** - MCP Framework ⭐ LOW PRIORITY
**Repository:** https://github.com/Dicklesworthstone/fastmcp_rust

**What it is:**
- Rust framework for building MCP servers
- Cancel-correct async via asupersync
- `#[tool]`, `#[resource]`, `#[prompt]` macros
- Four-valued Outcome: Ok/Err/Cancelled/Panicked

**Key Features:**
| Feature | Benefit for Quotey |
|---------|-------------------|
| Cancel-correct | Clean MCP tool cancellation |
| Attribute macros | Easy tool definitions |
| Budget-based timeouts | Better resource control |

**Integration Potential:**
- **LOW** - Quotey already uses mcp_agent_mail
- Could migrate if building custom MCP server
- Not immediately needed

---

## Summary Matrix

| Project | Stars | Lang | Integration Priority | Effort | Impact |
|---------|-------|------|---------------------|--------|--------|
| beads_rust | 592 | Rust | ⭐⭐⭐ HIGH | Low | High |
| mcp_agent_mail_rust | 18 | Rust | ⭐⭐⭐ HIGH | Low | High |
| frankensearch | 31 | Rust | ⭐⭐ MEDIUM | Medium | Medium |
| cass_memory | 241 | TypeScript | ⭐⭐ MEDIUM | Medium | Medium |
| frankentui | 189 | Rust | ⭐ LOW | High | Low |
| fastmcp_rust | 12 | Rust | ⭐ LOW | High | Low |

---

## Recommended Integration Path

### Phase 1: Immediate (Already Using)
1. **beads_rust** - Continue using for issue tracking
2. **mcp_agent_mail_rust** - Enhance agent coordination

### Phase 2: Near-term (Next Month)
3. **frankensearch** - Enhance Deal DNA with semantic search
4. **cass_memory** - Build CPQ procedural memory

### Phase 3: Future (Longer Term)
5. **frankentui** - If building rich CLI UI
6. **asupersync** - If migrating from Tokio (high effort)

---

## Other Notable Projects

| Project | Description | Relevance |
|---------|-------------|-----------|
| **asupersync** | Cancel-correct async runtime | High effort, high reward (already researched) |
| **frankenterm** | Terminal hypervisor for AI swarms | Terminal-focused, not CPQ |
| **ntm** | Named Tmux Manager | Terminal coordination, not CPQ |
| **charmed_rust** | TUI framework port | Alternative to frankentui |
| **source_to_prompt_tui** | Source → LLM prompt | Developer tooling |
| **franken_engine** | Deterministic extension runtime | Advanced, high effort |
| **frankensqlite** | SQLite reimplementation | Overkill for Quotey |
| **beads_viewer** | Graph-aware TUI for beads | Visualization for beads |

---

## Conclusion

The **highest-value integrations** for quotey are:

1. **beads_rust** - Already integrated, continue using
2. **mcp_agent_mail_rust** - Already integrated, enhance coordination
3. **frankensearch** - Could significantly enhance Deal DNA similarity search
4. **cass_memory** - Could build CPQ institutional memory

These tools align with quotey's architecture:
- Local-first (SQLite)
- AI agent friendly (`--json` API)
- Git-friendly collaboration
- Deterministic and auditable

---

*Document Version: 1.0*  
*Research Agent: ResearchAgent*  
*Status: Complete*
