# Quotey Research Document Index

**Maintainer:** ResearchAgent (kimi-k2)  
**Last Updated:** 2026-02-24  
**Total Documents:** 29

---

## Research Documents by Category

### Core Architecture Research

| Document | Description | Size |
|----------|-------------|------|
| `RESEARCH_DOCUMENT.md` | Comprehensive architecture analysis covering all crates, domain models, CPQ engines, flows | 26KB |
| `RESEARCH_SUMMARY.md` | Session summary of research activities and findings | 7KB |

### Technical Specifications

| Document | Description | Size |
|----------|-------------|------|
| `TECHNICAL_SPEC.md` | API specifications, data models, configuration, error handling | 26KB |

### Wave 1 Feature Specifications

| Document | Feature | Description | Size |
|----------|---------|-------------|------|
| `W1_SIM_DEAL_FLIGHT_SIMULATOR_SPEC.md` | SIM | Scenario generation and comparison | 6KB |
| `W1_SAN_RULE_SANDBOX_SPEC.md` | SAN | Policy replay and blast radius analysis | 6KB |
| `W1_REN_RENEWAL_DELTA_SPEC.md` | REN | Renewal intelligence and diff analysis | 6KB |
| `W1_MEM_ADAPTIVE_EXTRACTION_SPEC.md` | MEM | Learning terminology corrections | 6KB |
| `W1_APP_APPROVAL_PACKET_SPEC.md` | APP | Auto-assembled approval packets | 6KB |
| `W1_HLT_QUOTE_HEALTH_SPEC.md` | HLT | Health scoring and fix recommendations | 6KB |
| `W1_EXP_EXPLAIN_ANY_NUMBER_SPEC.md` | EXP | Explain any number | 5KB |
| `W1_PRE_PRECEDENT_INTELLIGENCE_GRAPH_SPEC.md` | PRE | Precedent intelligence graph | 5KB |
| `W1_REL_EXECUTION_QUEUE_SPEC.md` | REL | Resilient execution queue | 4KB |
| `W1_FIX_CONSTRAINT_AUTOREPAIR_TRADEOFF_SPEC.md` | FIX | Constraint auto-repair | 6KB |

### Wave 2 Feature Specifications

| Document | Feature | Description | Size |
|----------|---------|-------------|------|
| `W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_SPEC.md` | CLO | Closed-loop, replay-gated, human-approved policy optimization | 10KB |

### AI-Native CPQ Feature Research (FEAT-XX)

| Document | Feature | Description | Size |
|----------|---------|-------------|------|
| `RESEARCH_DEAL_DNA.md` | FEAT-01 | MinHash/SimHash algorithms, fingerprinting, similarity search | 17KB |
| `RESEARCH_CONVERSATIONAL_CONSTRAINT_SOLVER.md` | FEAT-02 | NLU architecture, dialogue state management, intent extraction | 31KB |
| `RESEARCH_EMOJI_MICRO_APPROVALS.md` | FEAT-03 | Slack emoji reactions, authority verification, audit trail | 20KB |
| `RESEARCH_EXPLAINABLE_POLICY.md` | FEAT-05 | Policy rule schema, explanation templates, resolution paths | 21KB |
| `RESEARCH_IMMUTABLE_LEDGER.md` | FEAT-08 | Cryptographic hash chains, Merkle trees, tamper detection | 22KB |

### Foundation Documentation

| Document | Description | Size |
|----------|-------------|------|
| `PROJECT.md` | Master project specification (2000+ lines) | 84KB |
| `ARCHITECTURE_DECISION_RESEARCH.md` | Architecture decision records | 141KB |
| `FOUNDATION_IMPLEMENTATION_PLAN.md` | Foundation phase implementation | 15KB |
| `FOUNDATION_QUALITY_GATES.md` | Quality gates and verification | 2KB |
| `FOUNDATION_QUICKSTART.md` | Quick start guide | 6KB |
| `CRATE_RESEARCH.md` | Crate structure research | 15KB |
| `INSPIRATION_RESEARCH.md` | Market and competitor research | 16KB |
| `RESEARCH_AGENT_DOCUMENTATION.md` | Agent documentation (auto-generated) | 12KB |
| `W1_REL_EXECUTION_QUEUE_DEMO_CHECKLIST.md` | Demo checklist for REL feature | 4KB |

---

## Quick Reference by Feature

### Power Capabilities Wave 1 (W1)

| Feature | Code | Spec | Research | Status |
|---------|------|------|----------|--------|
| Resilient Execution Queue | REL | ✅ | ⏳ | In Progress |
| Explain Any Number | EXP | ✅ | ⏳ | Open |
| Constraint Auto-Repair | FIX | ✅ | ⏳ | In Progress |
| Precedent Intelligence | PRE | ✅ | ⏳ | Open |
| Deal Flight Simulator | SIM | ✅ | ⏳ | Open |
| Rule Sandbox | SAN | ✅ | ⏳ | Open |
| Renewal Delta | REN | ✅ | ⏳ | Open |
| Adaptive Extraction | MEM | ✅ | ⏳ | Open |
| Approval Packet | APP | ✅ | ⏳ | Open |
| Quote Health | HLT | ✅ | ⏳ | Open |

### AI-Native CPQ Enhancements (FEAT)

| Feature | Code | Research | Status |
|---------|------|----------|--------|
| Deal DNA - Fingerprint Matching | FEAT-01 | ✅ | Open |
| Conversational Constraint Solver | FEAT-02 | ✅ | Open |
| Emoji-Based Micro-Approvals | FEAT-03 | ✅ | Open |
| Ghost Quotes | FEAT-04 | ⏳ | Open |
| Explainable Policy Engine | FEAT-05 | ✅ | Open |
| Multi-User Sessions | FEAT-06 | ⏳ | Open |
| Win Probability | FEAT-07 | ⏳ | Open |
| Immutable Quote Ledger | FEAT-08 | ✅ | Open |
| Configuration Archaeology | FEAT-09 | ⏳ | Open |
| Smart Thread Routing | FEAT-10 | ⏳ | Open |

---

## Research Coverage

### Completed Research Areas

✅ **Core Architecture**
- 6-crate workspace structure
- Domain models (Quote, Product, Customer, Approval)
- CPQ engines (Constraint, Pricing, Policy)
- Flow engine state machine
- Database layer and repositories

✅ **Advanced Capabilities**
- Deal DNA / Fingerprinting (SimHash)
- Immutable Ledger (SHA-256, HMAC, Merkle trees)
- Ghost Quotes (signal detection)
- Archaeology (dependency graphs)

✅ **Integration Patterns**
- Slack Socket Mode
- Block Kit UI components
- CLI commands
- Configuration system

✅ **Wave 1 Specifications**
- All 10 W1 features specified with:
  - Scope definitions
  - KPI contracts
  - Interface boundaries
  - Risk registers

### Remaining Research Opportunities

⏳ **Implementation Research**
- Composio REST API specification
- LLM provider implementations (OpenAI, Anthropic, Ollama)
- PDF generation pipeline (wkhtmltopdf)
- SQL repository implementations

⏳ **Feature Research**
- FEAT-04: Ghost Quotes (predictive opportunity creation)
- FEAT-06: Multi-user collaborative sessions
- FEAT-07: Win probability ML models
- FEAT-09: Configuration archaeology (forensics)
- FEAT-10: Smart thread routing (approval routing)

---

## Document Usage Guide

### For New Developers
1. Start with `RESEARCH_DOCUMENT.md` for architecture overview
2. Read `TECHNICAL_SPEC.md` for API details
3. Check feature-specific research docs for implementation details

### For Feature Implementation
1. Check `W1_*_SPEC.md` for scope and acceptance criteria
2. Review `RESEARCH_*.md` for technical deep-dives
3. Reference `PROJECT.md` for context and constraints

### For Architecture Decisions
1. Consult `ARCHITECTURE_DECISION_RESEARCH.md`
2. Review `FOUNDATION_IMPLEMENTATION_PLAN.md`
3. Check relevant feature research for patterns

---

## Statistics

| Metric | Value |
|--------|-------|
| Total Documents | 29 |
| Research Documents | 7 |
| Spec Documents | 11 |
| Foundation Docs | 8 |
| Total Lines | ~15,000+ |
| Total Size | ~500KB+ |

---

## Maintenance Notes

- All documents are in `.planning/` directory
- Markdown format for readability
- Code examples in Rust
- Database schemas in SQL
- Keep in sync with codebase changes

---

*Index maintained by ResearchAgent for the quotey project.*
