# UX Baseline and Friction Ledger

## Overview
This document captures the current state of Quotey UX before any UI-facing changes. It identifies where users hesitate, where flows break, and where context is lost.

---

## User Journeys

### Journey 1: Net-New Quote Creation
```
1. Rep types /quote new in Slack
2. Agent asks for missing info (customer, products, term)
3. Rep provides details via natural language
4. Agent validates constraints
5. Agent prices quote
6. Policy check runs
7. If approval needed → approval flow
8. Quote finalized → PDF generated
9. Share via portal or send directly
```

### Journey 2: Renewal Quote
```
1. Rep types /quote new for [customer] renewal
2. Agent loads existing contract context
3. Rep specifies changes (add seats, new products)
4. Agent applies renewal-specific pricing
5. Policy check (renewal discounts)
6. Approval flow if needed
7. Quote finalized
```

### Journey 3: Discount Exception
```
1. Rep has existing priced quote
2. Rep requests discount change
3. Agent evaluates against policy matrix
4. If exceeds threshold → approval chain
5. After approval → reprice → regenerate PDF
```

### Journey 4: Customer Portal Interaction
```
1. Rep shares portal link with customer
2. Customer views quote in browser
3. Customer can approve/reject/comment
4. Actions sync back to Slack
```

---

## Friction Points Identified

### F-001: Portal Subtotal Accumulation Bug
**Severity**: Critical (P0)
**Location**: Portal rendering (`crates/server/src/portal.rs`)
**Description**: Current flow references an invalid accumulator and can output wrong totals.
**Impact**: Customers see incorrect pricing, losing trust.
**Status**: ✅ **RESOLVED** in commit 4a7c8a2 - Portal now uses authoritative pricing snapshot for totals.
**Evidence**: Task `quotey-ux-001-10` addresses this.

### F-002: Token Lookup Permissive Fallback
**Severity**: Critical (P0)
**Location**: Portal token validation
**Description**: Allows fallback quote resolution by raw ID when token lookup fails.
**Impact**: Security risk - improper access control.
**Status**: ✅ **RESOLVED** in commit 4a7c8a2 - Token lookup now fails closed with explicit error messages.
**Evidence**: Task `quotey-ux-001-11` addresses this.

### F-003: Implicit Tax/Payment Assumptions
**Severity**: High (P1)
**Location**: Pricing engine output, Slack messages, Portal display
**Description**: Tax and payment terms show as defaults without explicit assumption status.
**Impact**: Users don't know what was assumed vs confirmed.
**Evidence**: Task `quotey-ux-001-12` addresses this.

### F-004: Ambiguous Loading/Error States
**Severity**: High (P1)
**Location**: Slack handlers, Portal async operations
**Description**: No standardized loading, empty, or error states with recovery actions.
**Impact**: Users see spinners or generic errors with no clear path forward.
**Evidence**: Task `quotey-ux-001-13` addresses this.

### F-005: No Explicit Assumption Cards
**Severity**: High (P1)
**Location**: Quote configuration, pricing inputs
**Description**: Users forced to infer hidden logic during pricing decisions.
**Impact**: Lack of transparency, potential for misunderstandings.
**Evidence**: Task `quotey-ux-001-6` addresses this.

### F-006: Non-Deterministic Status Language
**Severity**: Medium (P1)
**Location**: All Slack messages, Portal UI
**Description**: Status, warning, and action language inconsistent across surfaces.
**Impact**: Users must translate between Slack and portal meanings.
**Evidence**: Task `quotey-ux-001-3` addresses this.

### F-007: Unmapped Quote State Machine
**Severity**: Medium (P1)
**Location**: Slack handlers
**Description**: Quote journey states and transitions not formally defined.
**Impact**: Users don't always know what happened, what changed, what to do next.
**Evidence**: Task `quotey-ux-001-4` addresses this.

### F-008: No Session Persistence
**Severity**: Medium (P1)
**Location**: Slack conversation handling
**Description**: Interrupted flows lose context (thread breaks, app restarts, network churn).
**Impact**: Users must start over when interrupted.
**Evidence**: Task `quotey-ux-001-5` addresses this.

### F-009: No Pricing Rationale Panel
**Severity**: Medium (P1)
**Location**: Portal quote view, Slack pricing display
**Description**: Users see totals but not how they were derived.
**Impact**: Hard to validate or explain pricing to customers.
**Evidence**: Task `quotey-ux-001-7` addresses this.

### F-010: Poor Portal Information Hierarchy
**Severity**: Medium (P1)
**Location**: Portal quote viewer
**Description**: Non-critical details obscure decision-critical information.
**Impact**: Users miss totals, assumptions, next actions.
**Evidence**: Task `quotey-ux-001-8` addresses this.

### F-011: Ambiguous Approval Actions
**Severity**: Medium (P1)
**Location**: Portal approval UI
**Description**: Approve/reject/comment behavior not explicit.
**Impact**: Users may accidentally approve without understanding implications.
**Evidence**: Task `quotey-ux-001-9` addresses this.

### F-012: No Slack-to-Portal Continuity
**Severity**: Low (P2)
**Location**: Portal link handling
**Description**: No state continuity when navigating from Slack to portal.
**Impact**: Context summary not preserved between surfaces.
**Evidence**: Task `quotey-ux-001-16` addresses this.

### F-013: Accessibility Gaps
**Severity**: Low (P2)
**Location**: All UI surfaces
**Description**: Primary quote and approval paths not fully keyboard accessible.
**Impact**: Users with disabilities have degraded experience.
**Evidence**: Task `quotey-ux-001-15` addresses this.

---

## Metrics to Track

| Metric | Current Baseline | Target |
|--------|-----------------|--------|
| Quote completion latency | TBD | < 2 min |
| Assumption frequency | TBD | Explicit for 100% of assumptions |
| Error rework count | TBD | < 5% of quotes |
| Drop-off per step | TBD | < 10% per step |
| Approval confusion escalations | TBD | < 2% of approvals |

---

## Personas

### Persona 1: Sales Rep (Primary)
- Creates quotes daily
- Needs speed and clarity
- Values confidence in pricing accuracy
- Frustrated by: slow responses, unclear next steps, hidden assumptions

### Persona 2: Deal Desk Analyst
- Reviews approval requests
- Needs context for decisions
- Values transparency in discount justification
- Frustrated by: incomplete context, unclear policy violations

### Persona 3: Customer (Portal User)
- Views and approves quotes
- Needs clarity on pricing
- Values professional presentation
- Frustrated by: confusing totals, unclear next steps, no way to ask questions

---

## Acceptance Criteria (from bead quotey-ux-001-1)

- [x] Baseline report includes 3 personas (complete)
- [x] At least 10 friction points documented (13 identified)
- [x] Metrics include completion latency, assumption frequency, error rework count (defined - needs runtime measurement)
- [x] Baseline files are versioned (this document)

## Current Status

| Friction Point | Status | Commit |
|----------------|--------|--------|
| F-001: Portal Subtotal Accumulation Bug | **FIXED** | 4a7c8a2 |
| F-002: Token Lookup Permissive Fallback | **VERIFIED** (already fails closed) | 4a7c8a2 |
| F-003: Implicit Tax/Payment Assumptions | **PENDING** (quotey-ux-001-12) | - |
| F-004: Ambiguous Loading/Error States | **PENDING** (quotey-ux-001-13) | - |
| F-005: No Explicit Assumption Cards | **PENDING** (quotey-ux-001-6) | - |
| F-006: Non-Deterministic Status Language | **PENDING** (quotey-ux-001-3) | - |
| F-007: Unmapped Quote State Machine | **PENDING** (quotey-ux-001-4) | - |
| F-008: No Session Persistence | **PENDING** (quotey-ux-001-5) | - |
| F-009: No Pricing Rationale Panel | **PENDING** (quotey-ux-001-7) | - |
| F-010: Poor Portal Information Hierarchy | **PENDING** (quotey-ux-001-8) | - |
| F-011: Ambiguous Approval Actions | **PENDING** (quotey-ux-001-9) | - |
| F-012: No Slack-to-Portal Continuity | **PENDING** (quotey-ux-001-16) | - |
| F-013: Accessibility Gaps | **PENDING** (quotey-ux-001-15) | - |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-02-26 | Initial baseline with 13 friction points |
