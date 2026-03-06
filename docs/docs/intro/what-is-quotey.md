# What is Quotey?

Quotey is a **Rust-based, local-first CPQ (Configure, Price, Quote) agent** that lives in Slack. It replaces rigid CPQ screens with natural language interaction — sales reps express intent, and the agent gathers context, configures products, runs deterministic pricing, manages approvals, and generates PDF quotes. Everything persists in local SQLite with a complete audit trail.

## The Problem Quotey Solves

Traditional CPQ implementations fail for five consistent reasons:

1. **Data readiness is the silent killer** — 43% of manufacturers still rely on spreadsheets despite product complexity. Product data is scattered across systems, and creating a single source of truth is often harder than the CPQ implementation itself.

2. **Scope creep from pricing complexity** — What starts as "we have 3 pricing tiers" turns into thousands of customer-specific price books, negotiated rates, and regional variations.

3. **Integration stalls** — CPQ must read/write to CRM, ERP, billing, and engineering systems. These integrations are rarely clean.

4. **User adoption failure** — Sales reps revert to spreadsheets when the CPQ system is slower than their existing process. Salesforce CPQ hits performance ceilings at 500+ line quotes.

5. **Organizational misalignment** — Without executive sponsorship spanning sales, ops, finance, and product, each team optimizes for their own workflow.

## The Quotey Solution

Quotey addresses these failures through an **agent-first approach** that traditional CPQ architecturally cannot match:

### Natural Language Interaction

Sales reps express what they need in plain language, right in Slack:

```
/quote new for Acme Corp, Pro Plan, 150 seats, 12 months, 
add SSO and premium support
```

The agent handles the tedious work (data gathering, slot filling, calculation, formatting) while humans make the judgment calls (deal strategy, exception approval, customer relationships).

### Catalog Bootstrap from Unstructured Sources

An agent that ingests disparate data sources (spreadsheets, PDFs, ERP exports, even tribal knowledge in emails) and constructs a normalized product catalog. This 80% automated catalog construction saves months of implementation time.

### Quote Intelligence from Unstructured Context

The agent ingests RFP documents, customer emails, and Slack threads, extracts requirements, maps them to product configurations, and pre-populates quotes. Reps review and adjust rather than building from scratch.

### Deterministic Core with Audit Trail

Every price calculation, constraint validation, and policy decision is deterministic and fully auditable. The system produces a complete **pricing trace** showing exactly how every number was calculated.

## Core Value Proposition

Sales reps can create accurate, policy-compliant, fully-audited quotes through natural conversation in Slack — without touching a CPQ UI, without waiting days for approvals, and without the 6-18 month implementation cycle of traditional CPQ.

> **If everything else fails, this must work:** A rep types a quote request in Slack, the system prices it deterministically and correctly, and a PDF comes back with a complete audit trail proving exactly how every number was calculated.

## Key Differentiators

| Traditional CPQ | Quotey |
|----------------|--------|
| Rigid UI workflows | Natural language in Slack |
| 6-18 month implementation | Operational in days |
| Requires clean data upfront | Bootstraps from messy data |
| Cloud-dependent, vendor lock-in | Local-first, zero external dependencies |
| Sales reps need training | Reps already know Slack |
| Pricing decisions hidden in code | Deterministic, explainable, auditable |

## Who is Quotey For?

The target user is a **sales rep or deal desk analyst** in a mid-to-enterprise organization who currently quotes using:

- Spreadsheets and manual processes
- A legacy CPQ tool that's too slow or rigid
- Manual CRM workflows with approval chains via email

Quotey meets them where they already work — **Slack** — and eliminates the CPQ training burden entirely.

## Market Timing

Salesforce CPQ entered End-of-Sale (March 2025) with projected EOL 2029-2030. Thousands of enterprises need to migrate, and the migration path to Salesforce Revenue Cloud takes 18-24 months. This creates a 3-year market window for alternatives that:

1. Don't require big-bang migrations
2. Can be adopted incrementally alongside existing CRM/ERP
3. Provide value from Day 1 on simple use cases while growing into complex ones
4. Don't lock organizations into another cloud platform

## What's Real vs. Hype

In 2025-2026, "agentic" is used loosely. Here's what's real in Quotey:

**Real and valuable:**
- ✅ Natural language configuration
- ✅ Predictive deal scoring
- ✅ Document generation from quote data
- ✅ Quote anomaly detection
- ✅ Catalog bootstrap from unstructured data

**Intentionally not built (safety-first):**
- ❌ Fully autonomous quoting without human involvement
- ❌ AI deciding prices (dangerous, not innovative)
- ❌ AI replacing Deal Desk judgment

Quotey's position: **Agent-assisted workflows with deterministic guardrails.**

## Next Steps

- [Getting Started](./getting-started) — Install and run your first quote
- [Architecture Overview](../architecture/overview) — Understand how it all fits together
- [Key Concepts](./key-concepts) — Learn the fundamental ideas behind Quotey
