# Quotey

## What This Is

Quotey is a Rust-based, local-first CPQ (Configure, Price, Quote) agent that lives in Slack.
It replaces rigid CPQ screens with natural language interaction — sales reps express intent,
and the agent gathers context, configures products, runs deterministic pricing, manages
approvals, and generates PDF quotes. Everything persists in local SQLite with a complete
audit trail.

Quotey targets the hardest CPQ implementations: complex product configuration with
constraint-based engines, multi-dimensional pricing with database-driven rules, enterprise
approval workflows with multi-level routing, and — critically — the data readiness problem
that causes most CPQ deployments to fail before they start. It does this through three
agent-first capabilities that traditional CPQ cannot match: catalog bootstrap from
unstructured data, quote intelligence that pre-populates from RFPs and emails, and
natural language interaction that eliminates the CPQ training burden.

The target user is a sales rep or deal desk analyst in a mid-to-enterprise organization
who currently quotes using spreadsheets, manual CRM workflows, or a legacy CPQ tool
that's too slow, too rigid, or too expensive to maintain. Quotey meets them where they
already work — Slack — and lets them express what they need in plain language.

---

## Core Value

Sales reps can create accurate, policy-compliant, fully-audited quotes through natural
conversation in Slack — without touching a CPQ UI, without waiting days for approvals,
and without the 6-18 month implementation cycle of traditional CPQ.

If everything else fails, this must work: a rep types a quote request in Slack, the system
prices it deterministically and correctly, and a PDF comes back with a complete audit trail
proving exactly how every number was calculated.

---

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

---

## Context

### Market Opportunity

Salesforce CPQ entered End-of-Sale (March 2025) with projected EOL 2029-2030. Thousands
of enterprises need to migrate, and the migration path to Salesforce Revenue Cloud takes
18-24 months — many CIOs are outright rejecting it. This creates a 3-year market window
for alternatives that:

1. Don't require big-bang migrations
2. Can be adopted incrementally alongside existing CRM/ERP
3. Provide value from Day 1 on simple use cases while growing into complex ones
4. Don't lock organizations into another cloud platform

Most alternatives (DealHub, Conga, PROS) are cloud-first platforms with their own long
implementation cycles. A local-first, agent-first tool that runs on a laptop and connects
to existing systems via Composio occupies a genuinely different position.

### Why CPQ Implementations Fail

Research across industry sources reveals consistent failure patterns:

**1. Data readiness is the silent killer.** 43% of global manufacturers still rely on
spreadsheets for quoting despite growing product complexity. Most enterprises do not have
a clean, centralized product catalog when they begin a CPQ project. Product data is
scattered across spreadsheets, ERP systems, tribal knowledge in sales engineers' heads,
and legacy pricing sheets. CPQ requires a single source of truth — and creating that
source of truth is often harder than the CPQ implementation itself.

**2. Scope creep from pricing complexity.** What starts as "we have 3 pricing tiers"
turns into thousands of customer-specific price books, negotiated rates, contractual
overrides, and regional variations. Each exception becomes a custom rule that compounds
implementation time.

**3. Integration stalls.** CPQ must read/write to CRM (opportunity data), ERP (inventory,
cost, fulfillment), billing (invoicing, revenue recognition), and often engineering systems
(BOMs, feasibility). These integrations are rarely clean. Systems that "live comfortably
inside CRM struggle with ERP, eCommerce, or engineering tools, requiring custom code or
middleware to sync pricing, BOMs, or availability data."

**4. User adoption failure.** Sales reps revert to spreadsheets when the CPQ system is
slower than their existing process. Salesforce CPQ's JavaScript-based quote calculator
engine hits performance ceilings with quotes exceeding 500 lines. When sales reps spend
time watching loading screens and navigating slow, rigid workflows, they stop using
the tool.

**5. Organizational misalignment.** CPQ touches sales, sales ops, finance, legal, and
product management. Without executive sponsorship spanning all these groups, each team
optimizes for their own workflow, and the result is a system nobody loves.

### Agent-First Differentiation

Traditional CPQ tools operate on structured data through rigid UI workflows. Quotey's
agent-first approach uniquely enables capabilities that traditional CPQ architecturally
cannot match:

**Catalog bootstrap from unstructured sources.** An agent that can ingest disparate data
sources (spreadsheets, PDFs, ERP exports, legacy system dumps, even sales engineer tribal
knowledge captured in emails and docs) and construct/maintain a normalized product catalog
with configuration rules. This is a high-value, high-tolerance-for-imperfection task —
80% automated catalog construction saves months of implementation time.

**Quote intelligence from unstructured context.** CPQ tools operate on structured data,
but the context for a deal lives in emails, call recordings, RFP documents, and Slack
threads. An agent that ingests an RFP document or customer email thread, extracts
requirements, maps them to product configurations, identifies ambiguities that need
clarification, and pre-populates a quote. The rep reviews and adjusts rather than
building from scratch.

**Intelligent approval routing with context.** An agent that understands deal context
holistically (customer history, competitive situation, pipeline pressure, margin impact,
precedent deals) and can either auto-approve with explanation or pre-package the approval
request with all relevant context so human approvers can decide in seconds rather than
hours.

**Natural language interaction.** The rep doesn't operate CPQ screens; they express intent,
and the agent gathers context, fills in slots, executes tools, moves the deterministic
workflow forward, and asks only for missing or risky items. This eliminates the CPQ
training burden entirely.

### The Safety Principle

This is the most important architectural decision in the entire system.

The LLM is strictly a translator. It converts:
- Natural language → structured intent (slot extraction)
- Fuzzy product names → product IDs (catalog matching)
- Structured data → human-friendly summaries
- Deal context → approval justification text

The LLM **never** becomes the source of truth for:
- Prices (deterministic pricing engine decides)
- Configuration validity (constraint engine decides)
- Policy compliance (rules engine decides)
- Approval routing (threshold matrix decides)
- Discount authorization (approval workflow decides)

This separation is not a limitation — it's the feature. Enterprise CPQ requires
deterministic, auditable pricing. An LLM hallucinating a price is a contractual
liability. The strict separation eliminates this risk class entirely while still
getting the benefits of natural language interaction.

### Competitive Landscape Analysis

**Salesforce CPQ (sunsetting)**
- Strength: Deep CRM integration, massive ecosystem
- Weakness: Performance ceilings at 500+ line quotes, entering End-of-Sale
- Quotey advantage: No platform lock-in, local-first performance, agent-first UX

**DealHub**
- Strength: Unified quote-to-revenue, fast implementation
- Weakness: Less mature for deep manufacturing configuration
- Quotey advantage: Constraint-based configuration, fully local, open architecture

**Conga (formerly Apttus)**
- Strength: Document generation and CLM
- Weakness: Implementation complexity comparable to Salesforce CPQ
- Quotey advantage: Agent-first catalog bootstrap, NL interaction, zero-ops deployment

**PROS**
- Strength: AI-powered dynamic pricing, genuine ML sophistication
- Weakness: Requires dedicated revenue operations and data science involvement, high cost
- Quotey advantage: Local-first simplicity, self-serve deployment, open LLM layer

**Tacton**
- Strength: True constraint-based configuration engine
- Weakness: Narrow focus on manufacturing, not a full CPQ platform
- Quotey advantage: Full CPQ flow (configure + price + quote + approve + generate),
  agent-first interaction layer, broader applicability

### What "Agentic CPQ" Actually Means (vs. Hype)

In 2025-2026, "agentic" is used loosely by vendors. Here's what's real vs. hype:

**Real and valuable:**
- Natural language configuration ("I need a heavy-duty crane for offshore use with
  15-ton capacity" → valid configuration)
- Predictive deal scoring (historical win/loss data → optimal discount recommendation)
- Document generation from quote data (proposals, SOWs, executive summaries)
- Quote anomaly detection (unusual discounting patterns, missing components, pricing errors)

**Mostly hype (for now):**
- Fully autonomous quoting without human involvement for complex deals
- AI replacing Deal Desk judgment on deal structuring
- "AI-first" meaning the AI decides prices (this is dangerous, not innovative)

**Quotey's position:** Agent-assisted workflows with deterministic guardrails. The agent
handles the tedious work (data gathering, slot filling, calculation, formatting) while
humans make the judgment calls (deal strategy, exception approval, customer relationships).
This is the right level of autonomy for enterprise CPQ.

---

## Constraints

- **Language**: Rust — chosen for performance, safety, and single-binary deployment
- **Database**: SQLite via sqlx — local-first, zero-ops, portable
- **Interface**: Slack Socket Mode primary, CLI secondary — no web UI in v1
- **LLM**: Behind pluggable trait — must work with cloud APIs (OpenAI/Anthropic) and local (Ollama)
- **CRM**: Composio REST + stubbed CSV — real CRM integration via Composio, offline demo via fixtures
- **Deployment**: Single binary + SQLite file + config — someone else can install and run against their Slack workspace
- **Rules storage**: SQLite — all configuration rules, pricing policies, and approval thresholds stored in database, manageable via CLI

---

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

# DETAILED ARCHITECTURE

## High-Level Architecture (6 Boxes)

The system is composed of 6 major subsystems that communicate through well-defined
interfaces. Each subsystem is independently testable and has clear responsibilities.

```
┌─────────────────────────────────────────────────────────────────┐
│                    SLACK BOT INTERFACE                          │
│  Socket Mode listener · Slash commands · Message events         │
│  Interactive components (buttons/modals) · File uploads         │
├─────────────────────────────────────────────────────────────────┤
│                    AGENT RUNTIME                                │
│  Intent extraction · Slot filling · Action selection            │
│  Guardrails · Tool permissions · Conversation management        │
├──────────────────┬──────────────────┬───────────────────────────┤
│  DETERMINISTIC   │   CPQ CORE       │   TOOL ADAPTERS           │
│  FLOW ENGINE     │                  │                           │
│                  │  Product catalog  │  slack.* (post/update/    │
│  State machine   │  Constraint       │          upload)          │
│  Required fields │    engine         │  crm.* (sync/read/write) │
│  Allowed         │  Pricing engine   │  doc.* (render/attach)   │
│    transitions   │  Discount         │  composio.* (REST)       │
│  "What happens   │    policies       │  catalog.* (bootstrap/   │
│   next"          │  Approval         │           ingest)         │
│                  │    thresholds     │  intelligence.*           │
│                  │                  │    (parse/extract)        │
├──────────────────┴──────────────────┴───────────────────────────┤
│                    SQLITE DATA STORE                            │
│  Products · Price books · Deals · Quotes · Approvals            │
│  Configuration rules · Pricing policies · Audit log             │
│  Slack thread mapping · CRM sync state                          │
└─────────────────────────────────────────────────────────────────┘
```

### Box 1: Slack Bot Interface

The Slack Bot Interface is the primary entry point for all user interaction. It uses
Slack Socket Mode, which means the bot connects to Slack via WebSocket — no public URL,
no ngrok, no cloud deployment required. It runs on a laptop.

**Responsibilities:**
- Listen for slash commands (`/quote new`, `/quote status`, `/quote list`)
- Listen for message events in quote threads (natural language follow-ups)
- Render interactive components (buttons for Confirm/Edit/Add line/Request approval/Generate PDF)
- Handle modal submissions (structured input for complex data)
- Upload files (PDF quotes, CSV exports)
- Manage thread context (every quote gets its own thread)

**Implementation:**
- `slack-morphism` crate for Socket Mode support
- Slash command handler that parses initial intent
- Message event handler that feeds into the agent runtime
- Interactive component handler that feeds into the flow engine
- Rate limiting and error handling for Slack API calls

**Key design principle:** The Slack layer is thin. It translates Slack events into
domain events and domain responses into Slack messages. No business logic lives here.

### Box 2: Agent Runtime

The Agent Runtime is the "brain" of the system — but a carefully constrained brain. It
translates between the natural language world (Slack messages) and the deterministic
world (flow engine, CPQ core).

**Responsibilities:**
- Turn Slack messages into structured "Quote Intent" objects
- Choose next action based on flow state (ask question, run pricing, request approval, render PDF)
- Enforce strict guardrails and tool permissions
- Manage conversation context across multiple messages in a thread
- Generate human-friendly summaries and responses

**The Agent Loop (practical, constrained):**

```
1. Load current quote draft + flow step from SQLite
2. Attempt structured extraction from user message
   - Primary: LLM-based slot extraction (via pluggable trait)
   - Fallback: Regex/pattern matching for common patterns
3. Validate extracted data against deterministic schema
   - Missing required fields?
   - Constraint violations?
   - Policy violations?
4. Decide next action DETERMINISTICALLY:
   - If missing fields → ask for them (generate NL question)
   - If constraints violated → explain why and suggest alternatives
   - If all fields present → run pricing
   - If pricing complete + policy clean → offer to generate PDF
   - If pricing complete + policy violation → request approval
   - If approved → generate PDF
5. Execute action via tool adapter
6. Log everything to audit_event
7. Update flow state in SQLite
8. Respond to user in Slack thread
```

**Critical constraint:** Step 4 is always deterministic. The flow engine decides what
happens next, not the LLM. The LLM is used in Steps 2 (extraction) and 8 (response
generation) only.

**What the LLM does:**
- Extract structured fields from natural language ("12 months" → term_months: 12)
- Map fuzzy product names to product IDs ("Pro plan" → product_id: "plan_pro_v2")
- Generate human-friendly summaries ("Here's what I built for Acme...")
- Draft approval justification text with deal context
- Parse RFPs and emails for quote intelligence

**What the LLM does NOT do:**
- Decide prices (pricing engine decides)
- Validate configurations (constraint engine decides)
- Approve discounts (approval workflow decides)
- Choose workflow steps (flow engine decides)
- Make any decision that affects the quote's financial content

### Box 3: Deterministic Flow Engine

The flow engine is a state machine that owns "what happens next." It defines the
required fields, allowed transitions, and completion criteria for every step of the
quote lifecycle.

**Flow States:**

```
                    ┌──────────┐
                    │  DRAFT   │
                    └────┬─────┘
                         │ all required fields present
                         ▼
                    ┌──────────┐
                    │VALIDATED │
                    └────┬─────┘
                         │ pricing complete
                         ▼
                    ┌──────────┐
                 ┌──│  PRICED  │──┐
                 │  └──────────┘  │
    no policy    │                │ policy violation
    violation    │                │
                 ▼                ▼
           ┌──────────┐   ┌───────────┐
           │FINALIZED │   │ APPROVAL  │
           └────┬─────┘   └─────┬─────┘
                │               │ approved
                │               ▼
                │         ┌──────────┐
                │         │ APPROVED │
                │         └────┬─────┘
                │              │
                ▼              ▼
           ┌──────────────────────┐
           │       SENT           │
           │  (PDF generated +    │
           │   delivered)         │
           └──────────────────────┘
```

**Additional states for lifecycle:**
- `EXPIRED` — quote past its validity date
- `REVISED` — superseded by a newer version
- `REJECTED` — approval denied
- `CANCELLED` — manually cancelled by rep

**Flow Steps for Each Quote Type:**

**Net-New Quote Flow:**
1. Identify customer (lookup or create account)
2. Gather deal context (opportunity type, competitive situation)
3. Gather required inputs (term, start date, billing country, currency, plan tier)
4. Build line items (products, quantities, attributes)
5. Validate configuration (constraint engine)
6. Run pricing (pricing engine)
7. Policy checks (discount caps, margin floors, required approvals)
8. If approval needed → route to approvers
9. Generate PDF document
10. Deliver to rep + write to CRM

**Renewal Expansion Flow:**
1. Load existing deal/contract context from CRM or local data
2. Identify what's changing (added seats, new products, term extension)
3. Apply renewal-specific pricing (loyalty discounts, price locks, uplift caps)
4. Build amended line items
5. Validate configuration against existing contract
6. Run pricing with renewal context
7. Policy checks (renewal discount limits, churn risk assessment)
8. If approval needed → route with renewal context
9. Generate PDF with renewal-specific template
10. Deliver + update CRM with renewal data

**Discount Exception Flow:**
1. Load existing quote (may already be priced)
2. Identify requested discount and justification
3. Evaluate against discount policy matrix
4. Determine required approval level(s)
5. Draft approval request with full context:
   - Customer segment, deal size, competitive landscape
   - Historical discount precedents for this customer
   - Margin impact analysis
   - Justification summary
6. Route to appropriate approver(s) via Slack
7. Handle approval/rejection/revision cycle
8. If approved → update pricing with authorized discount
9. Re-generate PDF
10. Deliver + log exception

**At each step, the engine determines:**
- What fields are missing (and prompts for them)
- What transitions are allowed (and blocks invalid ones)
- Whether approvals are required (and routes appropriately)
- What the next action should be (deterministic decision tree)

**Implementation:** Rust enums + match statements for the state machine. Each flow
type is a separate module that implements a common `Flow` trait. The flow engine stores
its state in SQLite (`flow_state` table) so it survives restarts and can be resumed.

### Box 4: CPQ Core

The CPQ Core is the heart of the deterministic engine. It contains four sub-engines:

#### 4a. Product Catalog

The product catalog stores all configurable products, their attributes, valid options,
bundles, and the constraints that govern valid configurations.

**Product model:**
- Products have a type (simple, configurable, bundle)
- Configurable products have attributes (key-value pairs with types and allowed values)
- Bundles contain components (required and optional)
- Products can have relationships (requires, excludes, recommends)

**Catalog bootstrap (agent-first differentiator):**
The catalog bootstrap agent can ingest:
- CSV files with product data (columns mapped to product fields)
- PDF product sheets (parsed via LLM to extract structured data)
- Spreadsheets with pricing matrices
- Unstructured text descriptions of products

The agent:
1. Parses the input source
2. Extracts structured product data (name, SKU, description, attributes, pricing hints)
3. Identifies relationships between products (bundles, dependencies, exclusions)
4. Normalizes data into the product schema
5. Presents the extracted catalog for human review
6. Loads confirmed data into SQLite

This is intentionally high-tolerance-for-imperfection. 80% automated catalog construction
saves months of manual data entry. The remaining 20% is human review and correction,
which is far faster than building from scratch.

#### 4b. Constraint-Based Configuration Engine

This is the architectural choice that differentiates Quotey from rules-based CPQ tools.

**Why constraint-based over rules-based:**

Rules-based systems use if-then logic:
- "If customer selects Option A, then Option B is required"
- "If customer selects Option A, then Option C is excluded"
- For N options, you need O(N²) rules to cover all combinations

Constraint-based systems define what must be true:
- "Motor voltage must match enclosure voltage rating"
- "Enclosure thermal rating must exceed motor heat output"
- A few constraints replace hundreds of rules

**Constraint types in Quotey:**

1. **Requires** — Product A requires Product B
   - Example: "SSO add-on requires Enterprise tier or above"

2. **Excludes** — Product A is incompatible with Product B
   - Example: "Basic support excludes 24/7 SLA"

3. **Attribute constraints** — Attribute values must satisfy conditions
   - Example: "If billing_country is EU, then currency must be EUR"

4. **Quantity constraints** — Minimum/maximum quantities
   - Example: "Enterprise tier requires minimum 50 seats"

5. **Bundle constraints** — Bundle composition rules
   - Example: "Starter bundle must include exactly 1 base plan + 1 support tier"

6. **Cross-product constraints** — Constraints that span multiple line items
   - Example: "Total seat count across all line items must not exceed license cap"

**Constraint evaluation:**
The engine evaluates all constraints against the current configuration and returns:
- Valid/invalid status
- List of violated constraints with human-readable explanations
- Suggested fixes (alternative valid configurations)

The constraint engine is deterministic — same inputs always produce same outputs.
All constraints are stored in SQLite and manageable via CLI.

**Constraint storage schema:**

```sql
constraint_rule (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    constraint_type TEXT NOT NULL,  -- 'requires', 'excludes', 'attribute', 'quantity', 'bundle', 'cross_product'
    source_product_id TEXT,         -- NULL for global constraints
    target_product_id TEXT,         -- NULL for non-product constraints
    condition_json TEXT NOT NULL,   -- structured condition definition
    error_message TEXT,             -- human-readable violation message
    suggestion_json TEXT,           -- suggested fix template
    priority INTEGER DEFAULT 0,    -- evaluation order
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

#### 4c. Pricing Engine

The pricing engine computes the price for a validated configuration. It is deterministic,
auditable, and produces a complete trace of every calculation step.

**Pricing pipeline:**

```
1. Select price book(s)
   → Based on: customer segment, region, currency, deal type
   → Multiple price books can apply (with priority ordering)

2. Look up base prices
   → Each line item gets its unit price from the selected price book
   → If no price book entry exists, flag as error (never guess)

3. Apply volume tiers
   → Quantity-based price breaks within a single line item
   → Example: 1-49 seats = $10/seat, 50-99 = $8/seat, 100+ = $6/seat

4. Apply bundle discounts
   → Discounts for purchasing certain product combinations
   → Example: "Buy base plan + SSO + premium support = 15% bundle discount"

5. Apply formulas
   → Custom pricing formulas stored in database
   → Example: "unit_price * quantity * (term_months / 12) * segment_factor"

6. Apply requested discounts
   → Rep-requested discounts (flat % or per-line)
   → Capped by policy (see policy engine)

7. Compute subtotals
   → Per-line subtotals: unit_price * quantity - discounts
   → Quote subtotal: sum of all line subtotals

8. Apply tax (stub for v1)
   → Fixed tax rate table by region
   → Real tax integration deferred to v2

9. Compute total
   → Subtotal + tax = total

10. Generate pricing trace
    → JSON document recording every step, every input, every calculation
    → This is the audit trail that proves the price is correct
```

**Pricing trace format (example):**

```json
{
  "quote_id": "Q-2026-0042",
  "priced_at": "2026-02-23T14:30:00Z",
  "currency": "USD",
  "price_book_id": "pb_enterprise_us",
  "price_book_selection_reason": "customer_segment=enterprise, region=US",
  "lines": [
    {
      "line_id": "ql_001",
      "product_id": "plan_pro_v2",
      "product_name": "Pro Plan",
      "quantity": 150,
      "base_unit_price": 10.00,
      "base_price_source": "price_book_entry:pbe_042",
      "volume_tier_applied": {
        "tier": "100+",
        "tier_unit_price": 8.00,
        "discount_from_base": 2.00
      },
      "unit_price_after_tiers": 8.00,
      "formula_applied": {
        "formula_id": "f_annual_commitment",
        "expression": "unit_price * quantity * (term_months / 12)",
        "inputs": {"unit_price": 8.00, "quantity": 150, "term_months": 12},
        "result": 14400.00
      },
      "subtotal_before_discount": 14400.00,
      "discount_applied": {
        "type": "percentage",
        "requested": 10.0,
        "authorized": 10.0,
        "amount": 1440.00,
        "policy_check": "PASS (10% <= 15% segment cap)"
      },
      "line_total": 12960.00
    }
  ],
  "subtotal": 12960.00,
  "discount_total": 1440.00,
  "tax": {
    "rate": 0.0,
    "region": "US",
    "amount": 0.00,
    "note": "Tax stub - real tax integration in v2"
  },
  "total": 12960.00,
  "policy_flags": [],
  "approval_required": false
}
```

This trace is stored in `quote_pricing_snapshot.pricing_trace_json` and is immutable
once created. If the quote is re-priced, a new snapshot is created (the old one is
retained for audit history).

**Price book structure:**

```
Price Book: "Enterprise US 2026"
  Segment: Enterprise
  Region: US
  Currency: USD
  Priority: 1 (highest)
  Valid from: 2026-01-01
  Valid to: 2026-12-31

  Entries:
    Pro Plan:     $10.00/seat/month
    Enterprise:   $18.00/seat/month
    SSO Add-on:   $2.00/seat/month
    Premium Support: $500.00/month (flat)
    Onboarding:   $5,000.00 (one-time)

Volume Tiers (per product):
    Pro Plan:
      1-49:   $10.00
      50-99:  $8.00
      100+:   $6.00
    Enterprise:
      1-49:   $18.00
      50-99:  $15.00
      100+:   $12.00
```

#### 4d. Policy Engine

The policy engine evaluates business rules that determine whether a quote can proceed
without approval, requires approval, or is outright blocked.

**Policy types:**

1. **Discount caps** — Maximum discount by segment/product/deal size
   - Example: "SMB segment: max 10% without approval"
   - Example: "Enterprise segment: max 20% without approval"
   - Example: "Any segment: max 40% with VP approval, >40% blocked"

2. **Margin floors** — Minimum acceptable margin
   - Example: "Margin must be >= 60% for SaaS products"
   - Example: "Margin must be >= 40% for professional services"

3. **Deal size thresholds** — Approval required above certain deal values
   - Example: "Deals > $100K require Deal Desk review"
   - Example: "Deals > $500K require VP Sales approval"

4. **Product-specific policies** — Special rules for certain products
   - Example: "Custom SLA terms require Legal review"
   - Example: "Free onboarding requires manager approval"

5. **Temporal policies** — Time-based rules
   - Example: "End-of-quarter deals with >15% discount require extra scrutiny"
   - Example: "Quotes valid for max 30 days"

**Policy evaluation output:**

```json
{
  "quote_id": "Q-2026-0042",
  "evaluated_at": "2026-02-23T14:30:01Z",
  "status": "APPROVAL_REQUIRED",
  "policies_evaluated": 12,
  "policies_passed": 11,
  "policies_failed": 1,
  "violations": [
    {
      "policy_id": "pol_discount_cap_smb",
      "policy_name": "SMB Discount Cap",
      "severity": "approval_required",
      "description": "15% discount exceeds 10% cap for SMB segment",
      "threshold": 10.0,
      "actual": 15.0,
      "required_approver_role": "sales_manager",
      "suggested_action": "Request sales manager approval or reduce discount to 10%"
    }
  ],
  "auto_approved_policies": [
    "pol_margin_floor_saas",
    "pol_deal_size_threshold",
    "pol_product_sso_addon"
  ]
}
```

All policies are stored in SQLite and manageable via CLI. The policy engine is
deterministic — same quote always triggers the same policy evaluation.

### Box 5: Tool Adapters

Tool adapters are the interface between the agent runtime and external systems. Even
though everything is local in the PoC, defining tools cleanly keeps the agent safe,
testable, and extensible.

**Design principle:** Each tool adapter implements a trait. The agent runtime calls
tools through the trait interface, never directly. This enables:
- Stubbed implementations for testing
- Composio implementations for real CRM
- CLI implementations for admin tools
- Future implementations for new integrations

#### Slack Tools

```
slack.post_message(channel, thread_ts, blocks) → message_ts
slack.update_message(channel, ts, blocks) → ok
slack.open_modal(trigger_id, view_json) → ok
slack.upload_file(channel, thread_ts, file_path, title) → file_id
slack.add_reaction(channel, ts, emoji) → ok
slack.get_thread_messages(channel, thread_ts) → Vec<Message>
```

#### CRM Tools

```
crm.lookup_account(name_or_domain) → Option<Account>
crm.get_deal(deal_id) → Option<Deal>
crm.create_deal(account_id, deal_data) → Deal
crm.update_deal(deal_id, updates) → Deal
crm.write_quote(deal_id, quote_summary, pdf_url_or_attachment_id) → ok
crm.sync_incremental() → SyncResult  // runs in background / on timer
crm.search_contacts(query) → Vec<Contact>
```

**Implementations:**
- `StubCrmAdapter` — loads from CSV fixtures, writes to SQLite
- `ComposioCrmAdapter` — calls Composio REST API for real CRM operations

#### CPQ Core Tools

```
cpq.search_products(query) → Vec<Product>
cpq.get_product(product_id) → Option<Product>
cpq.validate_configuration(line_items) → ValidationResult
cpq.add_line(quote_id, product_id, qty, attributes) → QuoteLine
cpq.update_line(quote_id, line_id, updates) → QuoteLine
cpq.remove_line(quote_id, line_id) → ok
cpq.price_quote(quote_id) → PricingResult  // returns totals + policy flags + trace
cpq.validate(quote_id) → ValidationResult  // missing fields + constraint violations
```

#### Approval Tools

```
approval.request(quote_id, reason, approver_role) → ApprovalRequest
approval.get_pending(approver_id) → Vec<ApprovalRequest>
approval.record_decision(request_id, approved, comment) → ok
approval.check_status(request_id) → ApprovalStatus
approval.escalate(request_id, new_approver_role) → ok
approval.delegate(approver_id, delegate_id, duration) → ok
```

#### Document Tools

```
doc.render_pdf(quote_id, template_id) → FilePath + Checksum
doc.list_templates() → Vec<Template>
doc.preview_html(quote_id, template_id) → HtmlString
```

#### Catalog Bootstrap Tools

```
catalog.ingest_csv(file_path, column_mapping) → IngestionResult
catalog.ingest_pdf(file_path) → IngestionResult  // uses LLM for extraction
catalog.ingest_spreadsheet(file_path) → IngestionResult
catalog.review_pending() → Vec<PendingProduct>  // products awaiting human review
catalog.confirm(product_ids) → ok  // move from pending to active catalog
catalog.reject(product_ids, reason) → ok
```

#### Quote Intelligence Tools

```
intelligence.parse_rfp(file_path_or_text) → ExtractedRequirements
intelligence.parse_email(email_text) → ExtractedRequirements
intelligence.parse_slack_thread(channel, thread_ts) → ExtractedRequirements
intelligence.match_requirements_to_products(requirements) → Vec<ProductMatch>
intelligence.generate_draft_quote(matches, account_id) → DraftQuote
```

#### Composio Integration Tools

```
composio.authenticate(integration_id) → AuthResult
composio.execute_action(app, action, params) → ActionResult
composio.list_connections() → Vec<Connection>
composio.get_connection_status(connection_id) → ConnectionStatus
```

### Box 6: SQLite Data Store

Everything persists locally in a single SQLite file. The schema is designed to be
comprehensive enough for enterprise credibility while staying manageable for a PoC.

#### Complete Schema

**Reference Data:**

```sql
-- Products: the catalog of things you can sell
product (
    id TEXT PRIMARY KEY,
    sku TEXT UNIQUE NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    product_type TEXT NOT NULL,        -- 'simple', 'configurable', 'bundle'
    category TEXT,
    attributes_json TEXT,              -- {"key": {"type": "string", "values": ["a","b"], "default": "a"}}
    active BOOLEAN DEFAULT TRUE,
    source TEXT,                       -- 'manual', 'csv_import', 'pdf_import', 'composio_sync'
    source_ref TEXT,                   -- external ID from source system
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Product relationships: constraints between products
product_relationship (
    id TEXT PRIMARY KEY,
    source_product_id TEXT NOT NULL REFERENCES product(id),
    target_product_id TEXT NOT NULL REFERENCES product(id),
    relationship_type TEXT NOT NULL,   -- 'requires', 'excludes', 'recommends'
    condition_json TEXT,               -- optional: conditions when this relationship applies
    description TEXT,
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL
)

-- Price books: named collections of prices for specific segments/regions
price_book (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'USD',
    segment TEXT,                      -- 'smb', 'mid_market', 'enterprise'
    region TEXT,                       -- 'us', 'eu', 'apac'
    priority INTEGER DEFAULT 0,       -- higher = selected first when multiple match
    valid_from TEXT,
    valid_to TEXT,
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Price book entries: individual product prices within a price book
price_book_entry (
    id TEXT PRIMARY KEY,
    price_book_id TEXT NOT NULL REFERENCES price_book(id),
    product_id TEXT NOT NULL REFERENCES product(id),
    unit_price REAL NOT NULL,
    pricing_unit TEXT DEFAULT 'each',  -- 'each', 'per_seat', 'per_month', 'flat'
    minimum_quantity INTEGER DEFAULT 1,
    rules_json TEXT,                   -- additional pricing rules specific to this entry
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(price_book_id, product_id)
)

-- Volume tiers: quantity-based price breaks
volume_tier (
    id TEXT PRIMARY KEY,
    price_book_entry_id TEXT NOT NULL REFERENCES price_book_entry(id),
    min_quantity INTEGER NOT NULL,
    max_quantity INTEGER,              -- NULL = unlimited
    unit_price REAL NOT NULL,
    created_at TEXT NOT NULL
)

-- Pricing formulas: custom calculation expressions
pricing_formula (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    expression TEXT NOT NULL,          -- e.g., "unit_price * quantity * (term_months / 12)"
    variables_json TEXT NOT NULL,      -- describes expected input variables
    applies_to_json TEXT,              -- product IDs, categories, or segments this applies to
    priority INTEGER DEFAULT 0,
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Bundles: product groupings with composition rules
bundle (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    components_json TEXT NOT NULL,     -- [{"product_id": "x", "required": true, "default_qty": 1}]
    discount_type TEXT,                -- 'percentage', 'fixed', 'none'
    discount_value REAL DEFAULT 0,
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Constraint rules: configuration validation rules
constraint_rule (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    constraint_type TEXT NOT NULL,     -- 'requires', 'excludes', 'attribute', 'quantity', 'bundle', 'cross_product'
    source_product_id TEXT,
    target_product_id TEXT,
    condition_json TEXT NOT NULL,
    error_message TEXT,
    suggestion_json TEXT,
    priority INTEGER DEFAULT 0,
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Discount policies: business rules for discount authorization
discount_policy (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    segment TEXT,                      -- NULL = applies to all segments
    product_category TEXT,             -- NULL = applies to all products
    max_discount_auto REAL NOT NULL,   -- max % that can be auto-approved
    max_discount_with_approval REAL,   -- max % that can be approved (NULL = no hard cap)
    required_approver_role TEXT,       -- role needed when exceeding auto threshold
    deal_size_min REAL,               -- NULL = no minimum deal size filter
    deal_size_max REAL,               -- NULL = no maximum deal size filter
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Approval thresholds: multi-level approval routing
approval_threshold (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    threshold_type TEXT NOT NULL,      -- 'discount_pct', 'deal_value', 'margin_pct', 'product_specific'
    threshold_value REAL NOT NULL,
    comparison TEXT NOT NULL,          -- 'gt', 'gte', 'lt', 'lte'
    required_role TEXT NOT NULL,       -- 'sales_manager', 'vp_sales', 'deal_desk', 'cfo', 'legal'
    segment TEXT,
    product_category TEXT,
    priority INTEGER DEFAULT 0,       -- higher priority thresholds evaluated first
    active BOOLEAN DEFAULT TRUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

**Deals and Customers (synced or stubbed):**

```sql
-- Accounts: customer organizations
account (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    domain TEXT,
    segment TEXT,                      -- 'smb', 'mid_market', 'enterprise'
    region TEXT,
    industry TEXT,
    annual_revenue REAL,
    employee_count INTEGER,
    crm_ref TEXT,                      -- external CRM ID
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Contacts: people at customer organizations
contact (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES account(id),
    name TEXT NOT NULL,
    email TEXT,
    phone TEXT,
    title TEXT,
    role TEXT,                         -- 'decision_maker', 'influencer', 'user', 'champion'
    crm_ref TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Deals: sales opportunities
deal (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES account(id),
    name TEXT NOT NULL,
    stage TEXT,                        -- 'prospecting', 'qualification', 'proposal', 'negotiation', 'closed_won', 'closed_lost'
    deal_type TEXT,                    -- 'net_new', 'renewal', 'expansion', 'upsell'
    amount REAL,
    currency TEXT DEFAULT 'USD',
    close_date TEXT,
    owner TEXT,                        -- sales rep name/ID
    competitor TEXT,
    notes TEXT,
    crm_ref TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

**Quotes (the core output):**

```sql
-- Quotes: the primary work product
quote (
    id TEXT PRIMARY KEY,               -- format: Q-YYYY-NNNN
    deal_id TEXT REFERENCES deal(id),
    account_id TEXT NOT NULL REFERENCES account(id),
    version INTEGER DEFAULT 1,         -- version number within a quote series
    parent_quote_id TEXT,              -- previous version, NULL for first version
    status TEXT NOT NULL DEFAULT 'draft',  -- draft, validated, priced, approval, approved, rejected, finalized, sent, expired, cancelled, revised
    currency TEXT NOT NULL DEFAULT 'USD',
    start_date TEXT,
    end_date TEXT,
    term_months INTEGER,
    billing_frequency TEXT,            -- 'monthly', 'quarterly', 'annual', 'upfront'
    billing_country TEXT,
    requested_discount REAL,           -- overall requested discount %
    payment_terms TEXT,                -- 'net_30', 'net_60', 'net_90', 'upfront'
    valid_until TEXT,                  -- quote expiry date
    notes TEXT,
    created_by TEXT NOT NULL,          -- Slack user ID
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    finalized_at TEXT,
    sent_at TEXT,
    expired_at TEXT
)

-- Quote lines: individual items on a quote
quote_line (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    product_id TEXT NOT NULL REFERENCES product(id),
    quantity INTEGER NOT NULL DEFAULT 1,
    unit_price REAL,                   -- set by pricing engine, NULL before pricing
    discount_pct REAL DEFAULT 0,
    discount_amount REAL DEFAULT 0,
    subtotal REAL,                     -- set by pricing engine
    attributes_json TEXT,              -- line-specific configuration attributes
    sort_order INTEGER NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Quote pricing snapshots: immutable record of each pricing calculation
quote_pricing_snapshot (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    version INTEGER NOT NULL,          -- matches quote version
    subtotal REAL NOT NULL,
    discount_total REAL NOT NULL,
    tax_total REAL NOT NULL,
    total REAL NOT NULL,
    currency TEXT NOT NULL,
    price_book_id TEXT REFERENCES price_book(id),
    pricing_trace_json TEXT NOT NULL,  -- complete calculation trace (see format above)
    policy_evaluation_json TEXT,       -- policy engine output
    priced_at TEXT NOT NULL,
    priced_by TEXT NOT NULL            -- 'system' or user ID who triggered repricing
)
```

**Workflow and Approvals:**

```sql
-- Flow state: current position in the quote workflow
flow_state (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    flow_type TEXT NOT NULL,           -- 'net_new', 'renewal', 'discount_exception'
    current_step TEXT NOT NULL,        -- step identifier
    step_number INTEGER NOT NULL,
    required_fields_json TEXT,         -- fields needed to advance
    missing_fields_json TEXT,          -- fields still needed
    last_prompt TEXT,                  -- last message sent to user
    last_user_input TEXT,              -- last message received from user
    metadata_json TEXT,                -- flow-specific state data
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- Approval requests: multi-level approval tracking
approval_request (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    status TEXT NOT NULL DEFAULT 'pending',  -- pending, approved, rejected, escalated, expired, delegated
    requested_by TEXT NOT NULL,         -- Slack user ID of requester
    approver_role TEXT NOT NULL,        -- required role (sales_manager, vp_sales, deal_desk, cfo, legal)
    approver_id TEXT,                   -- specific Slack user ID if assigned
    delegate_id TEXT,                   -- delegated approver if original is unavailable
    reason TEXT NOT NULL,               -- why approval is needed
    context_json TEXT,                  -- deal context, precedents, margin analysis
    justification TEXT,                 -- LLM-drafted justification (editable by rep)
    decision_comment TEXT,              -- approver's comment on their decision
    policy_violation_ids TEXT,          -- comma-separated policy IDs that triggered this
    slack_message_ts TEXT,              -- Slack message with approval buttons
    slack_channel_id TEXT,              -- channel where approval was posted
    escalation_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    decided_at TEXT,
    expires_at TEXT                     -- auto-escalate if not decided by this time
)

-- Approval chain: for multi-level sequential/parallel approvals
approval_chain (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    chain_type TEXT NOT NULL,          -- 'sequential', 'parallel'
    steps_json TEXT NOT NULL,          -- ordered list of approval steps
    current_step INTEGER DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active',  -- active, complete, cancelled
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

**Audit and Observability:**

```sql
-- Audit events: complete history of every action and decision
audit_event (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    actor TEXT NOT NULL,                -- 'system', 'agent', Slack user ID, 'pricing_engine', etc.
    actor_type TEXT NOT NULL,           -- 'human', 'system', 'agent', 'llm'
    quote_id TEXT REFERENCES quote(id),
    event_type TEXT NOT NULL,           -- see event types below
    event_category TEXT NOT NULL,       -- 'quote', 'pricing', 'approval', 'configuration', 'catalog', 'crm', 'system'
    payload_json TEXT NOT NULL,         -- event-specific data
    metadata_json TEXT                  -- additional context
)

-- Event types:
-- quote.created, quote.updated, quote.versioned, quote.finalized, quote.sent, quote.expired, quote.cancelled
-- line.added, line.updated, line.removed
-- pricing.calculated, pricing.recalculated
-- policy.evaluated, policy.violation_detected, policy.auto_approved
-- approval.requested, approval.approved, approval.rejected, approval.escalated, approval.delegated, approval.expired
-- config.validated, config.constraint_violation, config.constraint_resolved
-- catalog.product_ingested, catalog.product_confirmed, catalog.product_rejected
-- crm.synced, crm.account_created, crm.deal_updated, crm.quote_written
-- agent.intent_extracted, agent.slot_filled, agent.action_selected, agent.llm_called
-- system.started, system.migration_run, system.error

-- Slack thread mapping: links quotes to Slack threads
slack_thread_map (
    id TEXT PRIMARY KEY,
    quote_id TEXT NOT NULL REFERENCES quote(id),
    channel_id TEXT NOT NULL,
    thread_ts TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(channel_id, thread_ts)
)

-- CRM sync state: tracks incremental sync position
crm_sync_state (
    id TEXT PRIMARY KEY,
    integration TEXT NOT NULL,         -- 'hubspot', 'salesforce', 'composio'
    entity_type TEXT NOT NULL,         -- 'account', 'deal', 'contact', 'product'
    last_sync_at TEXT,
    last_sync_cursor TEXT,             -- provider-specific cursor/token
    last_sync_count INTEGER DEFAULT 0,
    status TEXT DEFAULT 'idle',        -- 'idle', 'syncing', 'error'
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)

-- LLM interaction log: every LLM call for audit and debugging
llm_interaction_log (
    id TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL,
    provider TEXT NOT NULL,            -- 'openai', 'anthropic', 'ollama'
    model TEXT NOT NULL,
    purpose TEXT NOT NULL,             -- 'intent_extraction', 'slot_filling', 'product_matching', 'summary_generation', 'catalog_parsing', 'rfp_parsing'
    input_text TEXT NOT NULL,
    output_text TEXT NOT NULL,
    input_tokens INTEGER,
    output_tokens INTEGER,
    latency_ms INTEGER,
    quote_id TEXT REFERENCES quote(id),
    success BOOLEAN NOT NULL,
    error_message TEXT
)
```

---

## Rust Tech Stack

### Runtime and Async

| Crate | Purpose | Why |
|-------|---------|-----|
| `tokio` | Async runtime | Industry standard, required by most async crates |
| `tracing` | Structured logging | Excellent for observability, supports spans and fields |
| `tracing-subscriber` | Log output | Configurable formatting and filtering |
| `anyhow` | Error handling | Ergonomic error chaining for application code |
| `thiserror` | Error types | Derive macro for library-style error enums |

### Database

| Crate | Purpose | Why |
|-------|---------|-----|
| `sqlx` | SQLite async driver | Compile-time checked queries, migration support |
| Feature: `sqlite` | SQLite backend | Local-first, zero-ops |
| Feature: `migrate` | Schema migrations | Version-controlled schema changes |

### Slack

| Crate | Purpose | Why |
|-------|---------|-----|
| `slack-morphism` | Slack SDK | Socket Mode support, interactive components, file upload |

### HTTP Client

| Crate | Purpose | Why |
|-------|---------|-----|
| `reqwest` | HTTP client | For Composio REST API, LLM API calls, CRM APIs |
| Feature: `json` | JSON support | Serde integration for request/response bodies |

### Serialization

| Crate | Purpose | Why |
|-------|---------|-----|
| `serde` | Serialization framework | Derive macros for JSON/TOML/etc |
| `serde_json` | JSON handling | Schema definitions, API payloads, trace data |
| `toml` | Config files | Human-readable configuration |

### PDF Generation

| Crate | Purpose | Why |
|-------|---------|-----|
| `tera` | HTML templating | Jinja2-like syntax, excellent for quote templates |
| External: `wkhtmltopdf` | HTML→PDF conversion | Best output quality, widely available |

### CLI

| Crate | Purpose | Why |
|-------|---------|-----|
| `clap` | CLI argument parsing | Derive macro, subcommands, completion generation |

### LLM Integration

| Crate | Purpose | Why |
|-------|---------|-----|
| `async-trait` | Trait async methods | Required for pluggable LLM trait |
| `reqwest` | API calls | OpenAI/Anthropic/Ollama HTTP APIs |

### Catalog Bootstrap

| Crate | Purpose | Why |
|-------|---------|-----|
| `csv` | CSV parsing | Product catalog import |
| `calamine` | Spreadsheet parsing | .xlsx/.xls/.ods import for price matrices |
| `pdf-extract` or `lopdf` | PDF text extraction | Basic text extraction before LLM processing |

### Testing

| Crate | Purpose | Why |
|-------|---------|-----|
| `tokio::test` | Async test runtime | Test async functions |
| `sqlx::test` | Database test fixtures | Isolated test databases |
| `mockall` | Mock generation | Mock trait implementations for unit tests |
| `assert_json_diff` | JSON comparison | Validate pricing traces |
| `insta` | Snapshot testing | Capture and compare complex outputs |

---

## Project Layout

```
quotey/
  Cargo.toml
  Cargo.lock
  README.md
  AGENTS.md
  LICENSE

  # Database migrations (sqlx)
  migrations/
    001_initial_schema.sql          # Reference data tables
    002_deals_customers.sql         # Account, contact, deal tables
    003_quotes.sql                  # Quote, quote_line, pricing_snapshot
    004_workflow_approvals.sql      # Flow state, approval tables
    005_audit_observability.sql     # Audit events, Slack mapping, CRM sync, LLM log
    006_seed_demo_data.sql          # Demo products, price books, policies for PoC

  # HTML templates for PDF generation
  templates/
    quotes/
      standard.html.tera            # Standard quote template
      renewal.html.tera             # Renewal-specific template
      executive_summary.html.tera   # One-page summary for approvers
    styles/
      quote.css                     # Shared styles for PDF output

  # Configuration
  config/
    quotey.toml                     # Main configuration file
    demo_fixtures/
      products.csv                  # Demo product catalog
      price_books.csv               # Demo price book entries
      accounts.csv                  # Demo customer accounts
      deals.csv                     # Demo deals/opportunities

  # Source code
  src/
    main.rs                         # Entry point: CLI parsing, service startup
    config.rs                       # Configuration loading and validation
    error.rs                        # Application-wide error types

    # Database layer
    db/
      mod.rs                        # Re-exports, connection pool management
      models.rs                     # Rust structs for all database tables
      repo.rs                       # Repository trait + SQLite implementation
      migrate.rs                    # Migration runner
      fixtures.rs                   # Demo data loader

    # Slack integration
    slack/
      mod.rs                        # Re-exports
      socket_mode.rs                # WebSocket connection + event loop
      commands.rs                   # Slash command handlers (/quote new, /quote status, etc.)
      events.rs                     # Message event handlers (thread replies)
      interactive.rs                # Button/modal interaction handlers
      blocks.rs                     # Slack Block Kit message builders
      handlers.rs                   # Unified event dispatch

    # CRM integration
    crm/
      mod.rs                        # CrmAdapter trait definition
      stub.rs                       # StubCrmAdapter (CSV fixtures → SQLite)
      composio.rs                   # ComposioCrmAdapter (Composio REST API)
      sync.rs                       # Background incremental sync logic
      models.rs                     # CRM-specific data models

    # CPQ Core Engine
    cpq/
      mod.rs                        # Re-exports
      catalog.rs                    # Product catalog operations
      constraints.rs                # Constraint-based configuration engine
      pricing.rs                    # Pricing pipeline (price books → tiers → formulas → discounts)
      policy.rs                     # Policy engine (discount caps, margin floors, deal thresholds)
      rules.rs                      # Rule evaluation engine (database-driven)
      trace.rs                      # Pricing trace builder (audit trail)
      formulas.rs                   # Formula parser and evaluator

    # Flow engine
    flows/
      mod.rs                        # Flow trait definition + flow registry
      engine.rs                     # State machine logic, transition validation
      net_new.rs                    # Net-new quote flow implementation
      renewal.rs                    # Renewal expansion flow implementation
      discount_exception.rs         # Discount exception flow implementation
      states.rs                     # Flow state enum and transition rules

    # Agent runtime
    agent/
      mod.rs                        # Re-exports
      orchestrator.rs               # Main agent loop (load → extract → validate → act → log)
      intent.rs                     # Intent extraction (NL → structured QuoteIntent)
      slots.rs                      # Slot filling logic (what's missing, what to ask for)
      actions.rs                    # Action registry (what the agent can do)
      guardrails.rs                 # Permission checks, safety limits
      context.rs                    # Conversation context management

    # LLM integration
    llm/
      mod.rs                        # LlmProvider trait definition
      openai.rs                     # OpenAI API implementation
      anthropic.rs                  # Anthropic API implementation
      ollama.rs                     # Ollama (local) implementation
      prompts.rs                    # Prompt templates for each LLM task
      models.rs                     # LLM request/response types

    # Document generation
    docs/
      mod.rs                        # Re-exports
      render.rs                     # HTML → PDF pipeline
      templates.rs                  # Template loading and management

    # Catalog bootstrap (agent-first differentiator)
    catalog/
      mod.rs                        # Re-exports
      ingest.rs                     # Unified ingestion pipeline
      csv_parser.rs                 # CSV → product data extraction
      spreadsheet_parser.rs         # Spreadsheet → price matrix extraction
      pdf_parser.rs                 # PDF → product data extraction (LLM-assisted)
      normalizer.rs                 # Data normalization and deduplication
      reviewer.rs                   # Human review workflow

    # Quote intelligence (agent-first differentiator)
    intelligence/
      mod.rs                        # Re-exports
      rfp_parser.rs                 # RFP document → requirements extraction
      email_parser.rs               # Email thread → requirements extraction
      slack_parser.rs               # Slack thread → requirements extraction
      matcher.rs                    # Requirements → product matching
      draft_builder.rs              # Auto-generate draft quote from matches

    # Approval workflow
    approvals/
      mod.rs                        # Re-exports
      engine.rs                     # Approval routing logic
      matrix.rs                     # Multi-dimensional approval matrix
      chain.rs                      # Sequential and parallel approval chains
      notifications.rs              # Slack notifications to approvers
      escalation.rs                 # Auto-escalation logic

    # Audit and observability
    audit/
      mod.rs                        # Re-exports
      logger.rs                     # Audit event recording
      query.rs                      # Audit trail querying (for CLI and reports)

    # Composio integration
    composio/
      mod.rs                        # Re-exports
      client.rs                     # REST API client (reqwest-based)
      auth.rs                       # Authentication management
      actions.rs                    # Action execution helpers
      models.rs                     # Composio-specific data models

    # CLI interface
    cli/
      mod.rs                        # CLI command routing
      quote.rs                      # Quote management commands
      catalog.rs                    # Catalog management commands
      pricing.rs                    # Pricing rule management commands
      approval.rs                   # Approval management commands
      audit.rs                      # Audit trail commands
      crm.rs                        # CRM sync commands
      import.rs                     # Data import commands
```

---

## End-to-End Demo Scenarios

### Scenario 1: Net-New Quote

**Setup:** Acme Corp (Enterprise segment, US region) wants to buy the Pro Plan for
150 seats with SSO add-on and premium support for 12 months.

**Rep in Slack types:**
```
/quote new for Acme Corp, Pro Plan, 150 seats, 12 months, add SSO and premium support
```

**Agent responds in thread:**
```
📋 New Quote: Q-2026-0042 v1

Customer: Acme Corp (Enterprise, US)
Deal type: Net-new

Line Items:
  1. Pro Plan — 150 seats × $6.00/seat/mo = $900.00/mo
     (Enterprise 100+ tier pricing applied)
  2. SSO Add-on — 150 seats × $2.00/seat/mo = $300.00/mo
  3. Premium Support — 1 × $500.00/mo = $500.00/mo

Annual Total: $20,400.00

⚠️ Missing information:
  • Start date
  • Billing country
  • Payment terms

[ Confirm ] [ Edit ] [ Add Line ] [ Set Missing Fields ]
```

**Rep responds:**
```
Start March 1, US billing, net 30
```

**Agent extracts slots, validates, prices, and responds:**
```
✅ Quote Q-2026-0042 v1 — Fully Priced

Customer: Acme Corp (Enterprise, US)
Term: 12 months (Mar 1, 2026 — Feb 28, 2027)
Payment: Net 30

Line Items:
  1. Pro Plan — 150 seats × $6.00/seat/mo     $10,800.00
  2. SSO Add-on — 150 seats × $2.00/seat/mo    $3,600.00
  3. Premium Support — 1 × $500.00/mo           $6,000.00

                              Subtotal:  $20,400.00
                              Discount:       $0.00
                              Tax:            $0.00
                              ─────────────────────
                              Total:     $20,400.00

All policies passed ✓
No approval required ✓

[ Generate PDF ] [ Request Discount ] [ Edit ] [ Add Line ]
```

**Rep clicks "Generate PDF":**
```
📄 Quote PDF generated and uploaded.

File: Acme_Corp_Q-2026-0042_v1.pdf
Checksum: sha256:abc123...

Audit trail: 8 events logged
Pricing trace: complete (view with /quote audit Q-2026-0042)

[ Send to CRM ] [ New Version ] [ Done ]
```

### Scenario 2: Renewal Expansion with Discount

**Setup:** Acme Corp's contract is up for renewal. They want to add 50 seats and
request a 15% loyalty discount.

**Rep in Slack types:**
```
/quote new Acme renewal, add 50 seats to Pro, keep SSO and support, 15% loyalty discount
```

**Agent loads existing contract context and responds:**
```
📋 Renewal Quote: Q-2026-0043 v1

Customer: Acme Corp (Enterprise, US)
Existing contract: C-2025-0018 (expiring Feb 28, 2026)
Deal type: Renewal expansion

Current → Proposed:
  1. Pro Plan — 150 → 200 seats × $6.00/seat/mo    $14,400.00
  2. SSO Add-on — 150 → 200 seats × $2.00/seat/mo   $4,800.00
  3. Premium Support — unchanged                       $6,000.00

                              Subtotal:  $25,200.00
                              Discount (15%): -$3,780.00
                              Tax:            $0.00
                              ─────────────────────
                              Total:     $21,420.00

⚠️ Policy Alert:
  15% discount exceeds 10% auto-approval cap for Enterprise segment.
  Required approval: Sales Manager

Recommended: Request approval with renewal justification.

[ Request Approval ] [ Reduce Discount ] [ Edit ] [ Override ]
```

**Rep clicks "Request Approval":**
```
🔔 Approval Request Sent → #deal-desk

Request: APR-2026-0089
Quote: Q-2026-0043 (Acme Corp renewal)
Approver: Sales Manager

Justification (auto-drafted, editable):
"Acme Corp renewal expansion: adding 50 seats (+33%) with 15% loyalty
discount. Customer has been with us for 14 months with 98% utilization.
Discount exceeds standard 10% cap by 5%. Annual value increases from
$20,400 to $21,420 (+5%). Retention risk: competitor (Vendor X) has
offered similar tier at 20% discount per customer email 2/15."

Waiting for approval...

[ Edit Justification ] [ Cancel Request ]
```

**In #deal-desk channel:**
```
📋 Approval Required: Acme Corp Renewal

Quote: Q-2026-0043 — $21,420/year
Requested by: @rep_jane
Discount: 15% ($3,780)
Policy: Exceeds 10% Enterprise cap

Context:
• Customer tenure: 14 months
• Seat utilization: 98%
• Expansion: +50 seats (+33%)
• Competitive threat: Vendor X at 20% discount
• Net revenue impact: +$1,020/year vs. current contract

[ ✅ Approve ] [ ❌ Reject ] [ 💬 Request Changes ]
```

**Approver clicks "Approve":**

Back in the rep's thread:
```
✅ Approved by @manager_john at 2:45 PM
Comment: "Good retention play. Approved."

Quote Q-2026-0043 v1 is finalized.

[ Generate PDF ] [ Send to CRM ] [ Done ]
```

### Scenario 3: Discount Exception

**Setup:** Rep has an existing priced quote but needs a deeper discount for a
competitive situation.

**Rep in Slack types:**
```
/quote discount Q-2026-0040 change to 35% — losing to Vendor X on price
```

**Agent evaluates policies:**
```
⚠️ Discount Exception: Q-2026-0040

Current discount: 10% ($2,400)
Requested discount: 35% ($8,400)

Policy evaluation:
  ✗ FAIL: 35% exceeds 20% Enterprise cap → VP Sales approval required
  ✗ FAIL: 35% exceeds 30% maximum → CFO approval required
  ⚠️ WARNING: Margin drops to 48% (below 60% floor for SaaS)

Approval chain required (sequential):
  1. VP Sales (discount > 20%)
  2. CFO (discount > 30% + margin exception)

Margin analysis:
  Original:  $24,000 revenue, $9,600 cost → 60% margin ✓
  Discounted: $15,600 revenue, $9,600 cost → 38.5% margin ✗

[ Request Approval Chain ] [ Reduce to 20% ] [ Cancel ]
```

**Rep clicks "Request Approval Chain":**
```
🔔 Approval Chain Started: APR-2026-0092

Step 1 of 2: VP Sales → @vp_sarah (pending)
Step 2 of 2: CFO → @cfo_mike (waiting)

Auto-escalation: If no response in 4 hours, escalates to next level.

Justification:
"Competitive exception for TechCorp deal. Vendor X has undercut on price
by ~25%. Customer has expressed strong preference for our platform
(technical evaluation scored 4.2/5) but procurement is driving price
negotiation. 35% discount reduces margin to 38.5% but retains $15,600
ARR vs. $0 if lost. This customer has 3 departments considering expansion
with estimated $180K TCV over 24 months."

[ Edit Justification ] [ Cancel Chain ]
```

---

## The Pluggable LLM Trait

The LLM integration is designed as a Rust trait that any provider can implement.
This is critical for flexibility — users should be able to start with a cloud API
and switch to local inference (or vice versa) without changing any business logic.

```rust
// Core trait — every LLM provider implements this
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Extract structured intent from natural language
    async fn extract_intent(
        &self,
        message: &str,
        context: &ConversationContext,
    ) -> Result<QuoteIntent>;

    /// Match fuzzy product names to catalog entries
    async fn match_products(
        &self,
        query: &str,
        candidates: &[Product],
    ) -> Result<Vec<ProductMatch>>;

    /// Generate a human-friendly summary of a quote
    async fn summarize_quote(
        &self,
        quote: &Quote,
        lines: &[QuoteLine],
        pricing: &PricingSnapshot,
    ) -> Result<String>;

    /// Draft an approval justification
    async fn draft_justification(
        &self,
        quote: &Quote,
        policy_violations: &[PolicyViolation],
        deal_context: &DealContext,
    ) -> Result<String>;

    /// Parse an RFP or document into structured requirements
    async fn parse_document(
        &self,
        content: &str,
        document_type: DocumentType,
    ) -> Result<ExtractedRequirements>;

    /// Parse product data from unstructured text (for catalog bootstrap)
    async fn extract_product_data(
        &self,
        content: &str,
        source_type: SourceType,
    ) -> Result<Vec<ExtractedProduct>>;

    /// Get the provider name (for logging)
    fn provider_name(&self) -> &str;

    /// Get the model name (for logging)
    fn model_name(&self) -> &str;
}
```

**Implementations:**

| Provider | How it works | Best for |
|----------|-------------|----------|
| `OpenAiProvider` | Calls OpenAI API via reqwest | Best extraction quality (GPT-4o) |
| `AnthropicProvider` | Calls Anthropic API via reqwest | Best reasoning for complex documents |
| `OllamaProvider` | Calls local Ollama server (localhost:11434) | Fully local, maximum privacy |
| `MockProvider` | Returns pre-configured responses | Unit testing |

All LLM calls are logged to `llm_interaction_log` with input, output, token counts,
latency, and purpose. This enables:
- Debugging extraction failures
- Cost tracking across providers
- Quality comparison between providers
- Audit trail for any LLM-influenced decision

---

## Composio Integration Architecture

Since Composio doesn't have a Rust SDK, we build a thin REST client that calls
their API via `reqwest`.

**Authentication flow:**
1. On first run, user provides Composio API key (stored in config)
2. Quotey calls Composio to set up CRM connection (OAuth dance happens in browser)
3. Composio manages token refresh and credential storage
4. Quotey calls Composio actions as needed

**Integration surface:**

```
Composio REST API (https://api.composio.dev)
    │
    ├── POST /v1/actions/execute
    │     → crm.lookup_account
    │     → crm.get_deal
    │     → crm.create_deal
    │     → crm.update_deal
    │     → crm.write_quote
    │
    ├── GET /v1/connections
    │     → List active CRM connections
    │
    └── POST /v1/connections/initiate
          → Start new CRM connection (OAuth)
```

**Dual adapter pattern:**
- `StubCrmAdapter` — loads CSV fixtures into SQLite, simulates CRM operations locally
- `ComposioCrmAdapter` — calls Composio REST API for real CRM operations

Both implement the same `CrmAdapter` trait, so the rest of the system doesn't know
or care which one is active. Configuration determines which adapter is used:

```toml
[crm]
provider = "stub"  # or "composio"

[crm.stub]
fixtures_path = "config/demo_fixtures"

[crm.composio]
api_key = "${COMPOSIO_API_KEY}"
default_integration = "hubspot"  # or "salesforce"
```

---

## Error Handling and Recovery

Enterprise software must handle failures gracefully. Quotey addresses failure modes
at every layer:

**Slack connection failures:**
- Socket Mode automatically reconnects on disconnect
- In-flight operations are persisted in SQLite before Slack calls
- On reconnection, the agent checks for pending operations and retries

**LLM failures:**
- If intent extraction fails, fall back to regex-based pattern matching
- If fallback also fails, ask the user to rephrase or use structured input (Slack modal)
- All LLM calls have timeouts and retry with exponential backoff
- LLM errors are logged but never block the deterministic workflow

**Pricing engine failures:**
- If a price book entry is missing, the engine flags the specific product and stops
- Never falls back to a "default" price — missing data is always surfaced
- Constraint violations are reported with specific, actionable messages

**Approval workflow failures:**
- If an approver is unavailable, auto-escalation kicks in after configured timeout
- If Slack message delivery fails, retry with exponential backoff
- Approval state is always in SQLite — Slack is just the notification channel

**CRM sync failures:**
- Sync errors are logged but never block quoting operations
- The system works fully offline with local data
- CRM sync retries automatically on next interval

**Database failures:**
- SQLite is inherently resilient (WAL mode for concurrent access)
- Migrations run idempotently on startup
- Schema version is tracked to prevent partial migrations

---

## Testing Strategy

Testing a CPQ system is critical because incorrect prices or configurations have
real financial consequences.

### Unit Tests

**Pricing engine tests:**
- Every pricing rule type has a dedicated test
- Volume tier edge cases (boundary values, exact tier breaks)
- Formula evaluation with known inputs → known outputs
- Discount cap enforcement at every threshold
- Multi-price-book priority resolution

**Constraint engine tests:**
- Every constraint type has positive and negative tests
- Cross-product constraints with multiple line items
- Constraint violation message generation
- Suggested fix generation

**Policy engine tests:**
- Every policy type at every threshold
- Multi-policy interaction (when multiple policies apply)
- Edge cases at exact threshold values

### Integration Tests

**End-to-end flow tests:**
- Net-new quote from intent to PDF (with mock Slack, mock LLM)
- Renewal with discount requiring approval
- Discount exception with multi-level approval chain
- Quote versioning (create v1, revise to v2, compare)

**Database tests:**
- Migration up and down
- Concurrent access (multiple quotes being priced simultaneously)
- Audit trail completeness (every action produces an audit event)

### Snapshot Tests

**Pricing trace snapshots:**
- Known inputs → known pricing trace JSON
- Ensures pricing trace format doesn't regress
- Catches accidental changes to pricing calculations

**Slack message snapshots:**
- Known quote state → known Slack block output
- Ensures message formatting doesn't regress

### Property-Based Tests

**Pricing invariants:**
- Total always equals sum of line subtotals minus discounts plus tax
- Discount amount never exceeds subtotal
- Volume-tiered price is always <= base price
- Pricing trace always contains all line items

---

## Configuration File

```toml
# quotey.toml — main configuration

[general]
database_path = "quotey.db"
log_level = "info"                    # trace, debug, info, warn, error

[slack]
app_token = "${SLACK_APP_TOKEN}"      # xapp-...
bot_token = "${SLACK_BOT_TOKEN}"      # xoxb-...
# Socket Mode: no public URL needed

[slack.channels]
deal_desk = "#deal-desk"             # Where approval requests are posted
notifications = "#sales-ops"         # General notifications

[llm]
provider = "anthropic"               # "openai", "anthropic", "ollama", "mock"

[llm.openai]
api_key = "${OPENAI_API_KEY}"
model = "gpt-4o"

[llm.anthropic]
api_key = "${ANTHROPIC_API_KEY}"
model = "claude-sonnet-4-20250514"

[llm.ollama]
base_url = "http://localhost:11434"
model = "llama3.1"

[crm]
provider = "stub"                    # "stub" or "composio"
sync_interval_seconds = 300          # 5 minutes

[crm.stub]
fixtures_path = "config/demo_fixtures"

[crm.composio]
api_key = "${COMPOSIO_API_KEY}"
default_integration = "hubspot"

[quotes]
default_currency = "USD"
default_valid_days = 30              # Quote expiry
default_payment_terms = "net_30"
id_prefix = "Q"

[approvals]
auto_escalation_hours = 4            # Escalate if no response
max_approval_chain_length = 5        # Safety limit
reminder_interval_hours = 2          # Remind approvers

[pdf]
converter = "wkhtmltopdf"            # or "chromium"
converter_path = "/usr/local/bin/wkhtmltopdf"
default_template = "standard"
output_dir = "output/quotes"

[catalog_bootstrap]
require_human_review = true          # Always require review before activating imported products
max_batch_size = 100                 # Max products per import batch
```

---

## Deployment Model

Quotey is designed as a deployable alpha — someone else should be able to install and
run it against their Slack workspace.

**What the user gets:**
1. A single Rust binary (`quotey`)
2. A SQLite database file (created on first run)
3. A configuration file (`quotey.toml`)
4. HTML templates for PDF generation
5. Optionally: demo fixture CSVs

**Installation steps (target experience):**
1. Download the binary for your platform
2. Create a Slack app with Socket Mode enabled
3. Copy the app token and bot token into `quotey.toml`
4. Run `quotey migrate` to set up the database
5. Run `quotey seed-demo` to load demo data (optional)
6. Run `quotey start` to launch the bot
7. In Slack: `/quote new ...`

**CLI commands:**
```
quotey start                    # Start the bot (Slack Socket Mode)
quotey migrate                  # Run database migrations
quotey seed-demo                # Load demo products, price books, accounts
quotey catalog import <file>    # Import products from CSV/spreadsheet/PDF
quotey catalog list             # List all products
quotey catalog review           # Review pending imported products
quotey pricing list-books       # List price books
quotey pricing add-entry ...    # Add a price book entry
quotey pricing add-tier ...     # Add a volume tier
quotey rules list               # List all constraint rules
quotey rules add ...            # Add a constraint rule
quotey policies list            # List discount policies
quotey policies add ...         # Add a discount policy
quotey thresholds list          # List approval thresholds
quotey thresholds add ...       # Add an approval threshold
quotey quotes list              # List all quotes
quotey quotes show <id>         # Show quote details
quotey quotes audit <id>        # Show audit trail for a quote
quotey crm sync                 # Trigger CRM sync
quotey crm status               # Show CRM sync status
quotey composio connect         # Set up Composio CRM connection
quotey composio status          # Show Composio connection status
```

---

## Security Considerations

**Credentials:**
- All secrets (Slack tokens, API keys) loaded from environment variables, never stored in config file
- Config file uses `${ENV_VAR}` syntax for secret references
- `.env` file support for local development (excluded from git by .gitignore)

**Data privacy:**
- All quote data, customer data, and pricing data stays local (SQLite)
- LLM calls send only the minimum necessary context (not entire customer records)
- LLM interaction log allows auditing exactly what was sent to external APIs
- Ollama provider keeps everything fully local (no data leaves the machine)

**Input validation:**
- All user input from Slack is sanitized before database operations
- SQL queries use parameterized statements (sqlx enforces this at compile time)
- No raw SQL string concatenation anywhere

**Audit trail integrity:**
- Audit events are append-only (no UPDATE or DELETE on audit_event table)
- Pricing snapshots are immutable once created
- Every state change is recorded with actor, timestamp, and full context

---

## What "Done" Looks Like (Alpha Criteria)

The alpha is deployable when:

1. **Someone else can install it.** Download binary + config, add Slack tokens, run `quotey start`.
2. **All three flows work end-to-end.** Net-new, renewal, and discount exception from `/quote` to PDF.
3. **Pricing is deterministic and auditable.** Same inputs always produce same outputs, complete trace.
4. **Approvals route correctly.** Multi-level thresholds, Slack notifications, approve/reject via buttons.
5. **PDF quotes are professional.** Clean, branded, with all line items, pricing, and terms.
6. **Catalog bootstrap works.** Import a CSV of products, review, activate, and use in quotes.
7. **Quote intelligence works.** Paste an RFP excerpt, get a draft quote with matched products.
8. **CLI covers admin operations.** Manage products, pricing, policies, and audit trails.
9. **Audit trail is complete.** Every action, every decision, every price calculation is logged.
10. **Tests pass.** Pricing engine, constraint engine, and policy engine have comprehensive test coverage.

---

*Last updated: 2026-02-23 after initialization*
