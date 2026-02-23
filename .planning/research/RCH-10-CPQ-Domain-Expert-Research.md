# RCH-10: CPQ Domain Expert Research

**Bead:** `bd-256v.10`  
**Status:** Complete  
**Date:** 2026-02-23  
**Researcher:** LilacMountain (Codex)

## 1. Executive Summary

This artifact defines an initial CPQ domain perspective for Quotey with:

1. four practical user personas,
2. a ranked pain-point model,
3. a feature-priority matrix tied to business outcomes,
4. integration requirements by rollout tier,
5. a competitive snapshot and pricing expectations,
6. roadmap input mapped to current Quotey epics.

Top conclusion:

- Quotey should optimize for **speed-to-correct quote in Slack** and **deterministic trust**, not full-suite parity in v1.

## 2. Evidence Base and Confidence

## 2.1 Sources Used

1. Internal product context in `.planning/PROJECT.md`.
2. Current Quotey architecture and bead backlog.
3. Public CPQ product positioning pages (Salesforce, DealHub, Conga, PROS).
4. Practitioner discussion signals from Salesforce community channels.

## 2.2 Confidence Model

| Area | Confidence | Notes |
|---|---|---|
| persona shape and workflow needs | medium | strongly aligned with existing Quotey thesis and CPQ operating model |
| pain point ranking | medium | consistent with CPQ implementation patterns; still needs direct interviews |
| integration priority ordering | medium-high | stable demand pattern across CPQ deployments |
| pricing/package expectations | low-medium | requires direct buyer interviews and win/loss validation |

## 3. Persona Documents

## 3.1 Persona A: Sales Rep (Primary User)

**Role:** quota-carrying AE/SE hybrid in mid-market or enterprise teams.  
**Job-to-be-done:** produce accurate, approvable quotes quickly without leaving deal context.

Top needs:

1. fast quote drafting from natural language,
2. clear missing-field prompts,
3. immediate visibility into discount/approval status,
4. low-friction quote revision and resend.

Failure mode:

- rep abandons CPQ flow and reverts to spreadsheet/email when tool latency or complexity is high.

Success metric:

- median draft-to-send cycle time per quote.

## 3.2 Persona B: Deal Desk / Revenue Operations Analyst

**Role:** governs pricing policy, approvals, and quote quality at scale.  
**Job-to-be-done:** enforce policy without becoming a manual bottleneck.

Top needs:

1. deterministic rule enforcement and explainability,
2. high-signal exception queue,
3. clear audit trail for who approved what and why,
4. replay/debug tooling for disputed outcomes.

Failure mode:

- policy exceptions are resolved through ad-hoc chat, creating audit gaps.

Success metric:

- approval SLA and exception rework rate.

## 3.3 Persona C: Sales Manager / Approver

**Role:** approves non-standard discounts or special terms under time pressure.  
**Job-to-be-done:** make fast, defensible approval decisions with minimal back-and-forth.

Top needs:

1. concise approval packet (context, margin impact, precedent),
2. one-click approve/reject/delegate actions,
3. confidence that policy thresholds are pre-evaluated correctly.

Failure mode:

- delayed approvals due to poor context and manual data gathering.

Success metric:

- approval turnaround time.

## 3.4 Persona D: Systems Admin / CPQ Owner

**Role:** maintains pricing/catalog/rules and operational reliability.  
**Job-to-be-done:** keep CPQ system correct, observable, and maintainable with limited ops overhead.

Top needs:

1. stable release/update process,
2. safe migration and rollback mechanisms,
3. diagnostics and health tooling,
4. low-friction rule and catalog maintenance workflows.

Failure mode:

- deployment friction and brittle change management reduce trust in the system.

Success metric:

- change-failure rate and mean time to restore.

## 4. Pain Point Analysis

## 4.1 Ranked Pain Catalog

| Pain Point | Primary Persona | Severity | Frequency | Time Impact |
|---|---|---|---|---|
| quote construction requires too many manual steps | sales rep | high | high | high |
| approval context assembled manually across systems | approver / deal desk | high | high | high |
| policy/rule behavior difficult to explain after the fact | deal desk | high | medium | high |
| product/pricing data quality gaps at rollout time | admin / revops | high | medium | high |
| integration failures create silent CRM drift | revops / admin | high | medium | medium-high |
| user training burden for complex CPQ UI | sales rep | medium-high | high | medium-high |
| release/update process is operationally risky | admin | medium-high | medium | medium |

## 4.2 Root-Cause Themes

1. **Context fragmentation:** deal context is split across email, CRM, Slack, and docs.
2. **Human bottlenecks:** approval and exception handling are knowledge-worker constrained.
3. **Trust gaps:** when outputs are not explainable, organizations add manual control loops.
4. **Implementation drag:** catalog/rule readiness and integration complexity dominate timelines.

## 5. Feature Priority Matrix

## 5.1 Prioritization Method

Scored against:

1. user pain relief,
2. adoption leverage,
3. deterministic fit,
4. implementation tractability in current Quotey architecture.

## 5.2 Priority Table

| Capability | Priority | Primary Persona | Why |
|---|---|---|---|
| NL quote draft + slot-filling workflow | P0 | sales rep | direct time-to-quote reduction and adoption driver |
| deterministic pricing trace + explain any number | P0 | rep / deal desk | trust and audit readiness |
| threshold-based approval routing with Slack actions | P0 | approver / deal desk | removes approval bottleneck |
| exception handling + resilient retries for adapters | P0 | admin / revops | reliability requirement for production trust |
| catalog bootstrap from messy source files | P1 | admin / revops | reduces initial deployment friction |
| precedent-based recommendation support | P1 | deal desk / approver | improves consistency and decision speed |
| renewal delta intelligence | P1 | sales rep / manager | expansion use case leverage |
| full optimization sandbox/replay tooling | P2 | deal desk | high value, but can follow core adoption |

## 6. Integration Requirements

## 6.1 System Priority Tiers

| Tier | Integrations | Requirement Level | Notes |
|---|---|---|---|
| P0 | Salesforce, HubSpot | required | core CRM read/write loop for alpha credibility |
| P1 | DocuSign / Adobe Sign, SharePoint / Google Drive | recommended | post-quote workflow and storage continuity |
| P1 | NetSuite / Dynamics connectors (or export contracts) | recommended | finance handoff where direct ERP is needed |
| P2 | deep ERP/BOM real-time orchestration | deferred | high complexity, lower immediate adoption leverage |

## 6.2 Interface Expectations

1. deterministic idempotent writes (no duplicate quote artifacts),
2. explicit conflict handling and reconciliation queue,
3. retry/backoff strategy with user-visible degraded states,
4. audit fields that tie external writes to internal quote version.

## 7. Competitive Analysis Snapshot

## 7.1 Positioning Comparison

| Vendor | Relative Strength | Common Limitation vs Quotey Thesis |
|---|---|---|
| Salesforce Revenue Cloud | ecosystem depth and CRM adjacency | migration complexity and workflow rigidity risk |
| DealHub | faster quote-to-revenue story | less emphasis on local-first deterministic architecture |
| Conga | broad quote/document lifecycle footprint | heavier implementation overhead |
| PROS Smart CPQ | advanced pricing and optimization | higher operational/data maturity requirements |

## 7.2 Quotey Differentiation to Preserve

1. local-first deployment and ownership,
2. Slack-native interaction model,
3. strict deterministic authority for prices/policies/approvals,
4. auditable trail by default.

## 8. Pricing and Packaging Research (Initial)

## 8.1 Buyer Expectation Patterns

Likely expectations in target segment:

1. low-friction pilot entry (time-to-first-value over broad feature count),
2. predictable per-user or per-workspace pricing,
3. transparent packaging around core CPQ workflow and governance features,
4. optional premium tiers for advanced intelligence/automation.

## 8.2 Packaging Guidance for Quotey

1. **Base tier:** quote creation, deterministic pricing, approval routing, audit trail.
2. **Growth tier:** enhanced integrations + analytics + operational controls.
3. **Advanced tier:** intelligence add-ons (precedent graph, adaptive extraction memory, advanced replay tooling).

Validation requirement:

- run at least 8-12 structured buyer interviews before locking price points.

## 9. Product Roadmap Input

## 9.1 Near-Term Inputs (P0/P1)

1. keep foundation work focused on reliability, deterministic flow, and observability,
2. prioritize CPQ core + approval + integration resilience before broad feature expansion,
3. enforce release/update safety work as first-class (already aligned with `RCH-09`).

## 9.2 Backlog Steering Rules

1. features that reduce quote-cycle latency and approval delay move up,
2. features that add UI complexity without measurable cycle-time gain move down,
3. any capability that weakens deterministic explainability is rejected.

## 10. Validation Plan (Follow-On Work)

To move from medium-confidence synthesis to high-confidence market fit:

1. interview 10 sales reps across 3 industries,
2. interview 6 deal-desk/revops stakeholders,
3. interview 4 approvers/managers on decision latency,
4. run 5 admin/operator interviews on deployment/update friction,
5. convert findings into quantified persona-weighted priority scores.

## 11. Deliverable Mapping for `bd-256v.10`

Requested outputs covered:

1. **User persona documents:** Section 3.
2. **Pain point analysis:** Section 4.
3. **Feature priority matrix:** Section 5.
4. **Integration requirements doc:** Section 6.
5. **Competitive analysis:** Section 7.
6. **Pricing research:** Section 8.
7. **Product roadmap input:** Section 9.

## 12. References

1. Internal product architecture context: `.planning/PROJECT.md`
2. Salesforce CPQ / Revenue Cloud product context: https://www.salesforce.com/ap/cpq/
3. DealHub CPQ product overview: https://dealhub.io/product/cpq/
4. Conga CPQ product overview: https://conga.com/products/cpq
5. PROS Smart CPQ overview: https://pros.com/solutions/smart-cpq/
6. Salesforce practitioner community signal example: https://salesforce.stackexchange.com/questions/tagged/cpq
