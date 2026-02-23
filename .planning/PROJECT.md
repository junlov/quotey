# Quotey

## What This Is

Quotey is a Rust-based, local-first CPQ (Configure, Price, Quote) agent that lives in Slack. It replaces rigid CPQ screens with natural language interaction — sales reps express intent, and the agent gathers context, configures products, runs deterministic pricing, manages approvals, and generates PDF quotes. Everything persists in local SQLite with a complete audit trail. It targets the hardest CPQ implementations: complex product configuration, multi-dimensional pricing, and enterprise approval workflows — problems that cause traditional CPQ deployments to fail.

## Core Value

Sales reps can create accurate, policy-compliant, fully-audited quotes through natural conversation in Slack — without touching a CPQ UI, without waiting days for approvals, and without the 6-18 month implementation cycle of traditional CPQ.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Slack Socket Mode bot that accepts `/quote` commands and manages quote lifecycle in threads
- [ ] Deterministic constraint-based product configuration engine with dependency/exclusion rules
- [ ] Database-driven pricing rules engine (price books, volume tiers, multi-dimensional pricing, formulas)
- [ ] Pricing trace and full audit log for every agent decision and price calculation
- [ ] Multi-level approval workflow with threshold-based routing via Slack interactive components
- [ ] PDF quote generation from HTML templates (tera/handlebars → wkhtmltopdf/headless Chrome)
- [ ] Three demo flows: net-new quote, renewal expansion, discount exception
- [ ] Quote lifecycle management (versioning, amendment, expiry, cloning)
- [ ] Pluggable LLM trait for NL → structured intent extraction (with cloud and local provider implementations)
- [ ] Catalog bootstrap agent that ingests unstructured data (CSV, spreadsheets, PDFs) to build product catalog
- [ ] Quote intelligence: parse RFPs, emails, and Slack threads to pre-populate quotes
- [ ] Composio REST client adapter for CRM integrations (alongside stubbed/CSV CRM for offline demo)
- [ ] SQLite-backed persistence for all data (products, price books, deals, quotes, approvals, audit log)
- [ ] CLI tools for admin, catalog management, and debugging
- [ ] Configuration rules and pricing policies stored in SQLite, manageable via CLI without recompilation

### Out of Scope

- Full product configurator UI (rely on Slack modals + NL intent parsing) — complexity vs. value for PoC
- Complex multi-currency taxation (stub or fixed tax table) — regulatory complexity, defer to v2
- Full contract lifecycle management (just PDF generation + storage) — CLM is a separate product
- Deep ERP integration (just CRM read/write via Composio) — ERP is high-friction, low-value for PoC
- Huge catalog search (start with 20-200 SKUs) — optimize for correctness first, scale later
- Real-time collaborative editing of quotes — adds collaboration complexity, Slack threads suffice
- Usage-based / consumption pricing models — requires billing system integration, defer to v2
- Ramp pricing (multi-period escalating/de-escalating) — significant billing complexity, defer to v2
- Web admin dashboard (axum UI) — CLI is sufficient for alpha
- Multi-tenancy — single-tenant local-first for alpha, multi-tenant architecture for v2

## Context

**Market opportunity:** Salesforce CPQ entered End-of-Sale (March 2025) with projected EOL 2029-2030. Thousands of enterprises need to migrate, creating a 3-year window for alternatives that don't require big-bang migrations. Most alternatives (DealHub, Conga, PROS) are cloud-first platforms with long implementation cycles.

**Why CPQ implementations fail:** 43% of manufacturers still use spreadsheets for quoting. The #1 blocker is data readiness — product catalogs are scattered across spreadsheets, ERPs, and tribal knowledge. The #2 blocker is pricing complexity (thousands of customer-specific exceptions). The #3 blocker is approval cycle time killing deals.

**Agent-first differentiation:** Traditional CPQ tools operate on structured data through rigid UI. Quotey's agent-first approach uniquely enables: (1) catalog bootstrap from unstructured sources, (2) quote pre-population from RFPs/emails, (3) intelligent approval routing with context, and (4) natural language interaction that eliminates the CPQ training burden.

**The safety principle:** The LLM is strictly a translator (NL → structured intent, structured data → human-friendly text). It never becomes the source of truth for prices, configuration validity, or policy decisions. Deterministic engines decide; the LLM proposes, summarizes, and extracts.

**Technical approach:** Rust for performance and safety guarantees. SQLite for local-first persistence with zero ops burden. Slack Socket Mode for zero-infrastructure deployment (runs on a laptop). Composio for pluggable CRM integration. Pluggable LLM trait for provider flexibility.

## Constraints

- **Language**: Rust — chosen for performance, safety, and single-binary deployment
- **Database**: SQLite via sqlx — local-first, zero-ops, portable
- **Interface**: Slack Socket Mode primary, CLI secondary — no web UI in v1
- **LLM**: Behind pluggable trait — must work with cloud APIs (OpenAI/Anthropic) and local (Ollama)
- **CRM**: Composio REST + stubbed CSV — real CRM integration via Composio, offline demo via fixtures
- **Deployment**: Single binary + SQLite file + config — someone else can install and run against their Slack workspace
- **Rules storage**: SQLite — all configuration rules, pricing policies, and approval thresholds stored in database, manageable via CLI

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Constraint-based configuration over rules-based | Rules-based systems suffer combinatorial explosion at scale; constraints reduce thousands of rules to hundreds of constraints (Tacton case study). More sustainable for complex products. | — Pending |
| Database-driven rules over YAML/code | Rules must be manageable without recompilation for enterprise deployability. SQLite keeps everything local and inspectable. CLI management is sufficient for alpha. | — Pending |
| LLM as translator, never source of truth | Enterprise CPQ requires deterministic, auditable pricing. LLM hallucination of prices = contractual liability. Strict separation eliminates this risk class entirely. | — Pending |
| Composio for CRM integration | No Rust SDK exists, but REST API is callable via reqwest. Handles OAuth complexity for 90+ integrations. Worth the external dependency for integration breadth. | — Pending |
| HTML → PDF over native Rust PDF | Better output quality, easier templating (tera/handlebars), faster to iterate on quote design. External converter (wkhtmltopdf) is acceptable for alpha. | — Pending |
| Slack-first over web UI | Meets reps where they work. Socket Mode eliminates infrastructure requirements. Interactive components (buttons, modals) provide structured input when NL is insufficient. | — Pending |
| Catalog bootstrap as differentiator | The #1 CPQ implementation blocker is data readiness. Agent-powered catalog ingestion from unstructured sources (CSV, PDF, spreadsheets) is a unique value prop that traditional CPQ cannot match. | — Pending |

---
*Last updated: 2026-02-23 after initialization*
