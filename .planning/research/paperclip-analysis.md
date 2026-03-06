# Paperclip Deep Analysis — Ideas for Quotey

**Source**: https://paperclip.ing | https://github.com/paperclipai/paperclip
**Date**: 2026-03-06
**Purpose**: Extract data model patterns, governance structures, and operational ideas from Paperclip's AI agent orchestration platform to enhance quotey's CPQ agent system.

---

## What Paperclip Is

An open-source (MIT) orchestration platform for running "zero-human companies" with autonomous AI agents. TypeScript/Node.js, PostgreSQL (Drizzle ORM), React/Vite UI. Self-hosted, single-tenant, multi-company.

**Key insight for quotey**: Paperclip solves the *governance of autonomous agents* problem — the exact problem quotey faces when AI agents handle pricing decisions, quote creation, and approval workflows.

---

## 1. ENTITY & DATA MODEL IDEAS

### 1.1 Company-Scoped Multi-Tenancy
**Paperclip**: Every entity has `company_id`. Complete data isolation per company.
**Quotey idea**: Add `tenant_id` or `org_id` to all tables. Support multiple sales orgs sharing one quotey instance. Each org gets its own pricing policies, approval thresholds, product catalog, and audit trail. This enables MSP (managed service provider) or multi-division deployments.

### 1.2 Hierarchical Goal → Project → Task Traceability
**Paperclip**: `goals` (company → team → agent → task levels) → `projects` → `issues`. Every task traces to a company goal.
**Quotey idea**: **Quote Purpose Tracing**. Add a `deal_goal` or `opportunity_objective` entity that links quotes to business objectives:
- Sales target: "Close $500K in Q2 enterprise deals"
- Quote → Deal → Objective chain
- Dashboard: "Which quotes serve which business goals?"
- Prevents quote sprawl — if a quote can't trace to a goal, flag it

### 1.3 Agent Identity & Configuration as First-Class Entity
**Paperclip**: `agents` table with `adapter_type`, `adapter_config` (JSONB), `runtime_config`, `permissions`, `capabilities`, `role`, `title`, `reports_to` hierarchy.
**Quotey idea**: **Rep/Agent Profile Entity**. Instead of bare `actor_id` strings:
- `sales_rep` table: id, name, role (AE, SE, manager), title, team_id, reports_to
- `rep_config` JSONB: default discount authority, product specializations, territory
- `rep_permissions`: max discount %, can_override_pricing, auto_approve_threshold
- `rep_capabilities`: text description for AI context injection
- Enables: "This AE can discount up to 15% without approval; their manager can go to 25%"

### 1.4 Atomic Checkout / Single-Assignee Model
**Paperclip**: Issues have atomic checkout — single SQL `UPDATE WHERE status IN (?) AND assignee IS NULL`. Returns 409 on conflict.
**Quotey idea**: **Quote Locking / Atomic Claim**. Prevent two reps from editing the same quote:
- `quote_lock` with `locked_by`, `locked_at`, `lock_run_id`
- Atomic claim: `UPDATE quotes SET locked_by = ? WHERE id = ? AND locked_by IS NULL`
- 409 conflict response with current owner info
- Auto-release after timeout (stale lock detection)
- Prevents: duplicate quote sends, conflicting discount changes, approval race conditions

### 1.5 Issue Comments as Communication Channel
**Paperclip**: No separate messaging — all communication via task comments. `issue_comments` with `author_agent_id` or `author_user_id`.
**Quotey idea**: **Quote Thread / Activity Feed**. Replace scattered Slack messages with structured quote comments:
- `quote_comment` table: quote_id, author_type (rep/manager/system/ai), author_id, body, created_at
- AI agent posts pricing rationale as a comment
- Manager posts approval note as a comment
- System posts "price changed from X to Y" as a comment
- Single chronological thread per quote — complete context for anyone picking it up
- Slack integration: comments sync bidirectionally with Slack thread

### 1.6 Configurable Adapter System
**Paperclip**: Agents have pluggable adapters (process, HTTP, Claude, Codex, Cursor). `adapter_type` + `adapter_config` JSONB.
**Quotey idea**: **Integration Adapter Pattern**. Make quotey's external integrations pluggable:
- CRM adapter: `adapter_type = "salesforce" | "hubspot" | "pipedrive" | "none"`
- Notification adapter: `adapter_type = "slack" | "teams" | "email" | "webhook"`
- PDF adapter: `adapter_type = "built_in" | "docusign" | "pandadoc"`
- ERP adapter: `adapter_type = "netsuite" | "quickbooks" | "none"`
- Each adapter has its own config JSONB blob
- Swap integrations without code changes

---

## 2. GOVERNANCE & APPROVAL IDEAS

### 2.1 Typed Approval System with Payload
**Paperclip**: `approvals` with `type` (hire_agent, approve_ceo_strategy), `status` (pending → approved/rejected/cancelled), `payload` JSONB, `decision_note`, `decided_by_user_id`.
**Quotey idea**: **Rich Approval Requests**. Enhance current approval_request:
- Add `type` field: `discount_override`, `price_exception`, `non_standard_terms`, `custom_bundle`, `competitor_match`, `executive_escalation`
- Add `payload` JSONB: full context (original price, requested price, justification, competitor data, deal urgency)
- Add `decision_note`: approver's rationale (auditable)
- Add `revision_requested` status: "go back and adjust the discount to 12% instead of 20%"
- Add `resubmit` flow: rep adjusts and resubmits without creating new request
- Enables approval analytics: "What % of discount_override requests get approved?"

### 2.2 Approval Comments / Discussion Thread
**Paperclip**: `approval_comments` table — discussion on approvals before decision.
**Quotey idea**: **Approval Discussion Thread**. Before approving/rejecting:
- Manager comments: "What's the competitive situation?"
- Rep replies: "They have a Competitor X quote at $Y"
- AI posts: "Historical win rate at this discount level: 73%"
- Finance comments: "Margin impact: -$X on this deal"
- All captured in `approval_comment` table, linked to approval_request
- Decision happens AFTER informed discussion, not blind approve/reject

### 2.3 Board Override / Emergency Controls
**Paperclip**: Board can pause/resume/terminate any agent, reassign any task, edit budgets, override approvals at any time.
**Quotey idea**: **Sales Ops Emergency Controls**:
- `quote_freeze`: Ops can freeze all active quotes (pricing change rollout)
- `rep_pause`: Temporarily disable a rep's quoting ability (compliance investigation)
- `price_override`: Ops can force-adjust any quote's pricing (correction)
- `approval_bypass`: VP+ can bypass normal approval chain for urgent deals
- All emergency actions logged to audit trail with justification required

### 2.4 requireBoardApprovalForNewAgents (Company-Level Policy Toggle)
**Paperclip**: `companies.require_board_approval_for_new_agents` boolean.
**Quotey idea**: **Org-Level Policy Toggles**:
- `org_settings.require_manager_approval_above_discount_pct`: threshold (e.g., 10%)
- `org_settings.require_finance_approval_above_deal_value`: threshold (e.g., $100K)
- `org_settings.auto_approve_standard_pricing`: boolean
- `org_settings.allow_custom_line_items`: boolean
- `org_settings.require_legal_review_for_custom_terms`: boolean
- Runtime-configurable without code deploys

---

## 3. COST TRACKING & BUDGET IDEAS

### 3.1 Granular Cost Events
**Paperclip**: `cost_events` with provider, model, input_tokens, output_tokens, cost_cents, billing_code. Immutable append-only.
**Quotey idea**: **AI Usage Cost Tracking Per Quote**:
- Track every AI call made during quote creation (intent extraction, pricing lookup, PDF generation)
- `ai_cost_event`: quote_id, model, input_tokens, output_tokens, cost_cents, operation (e.g., "intent_extraction", "price_recommendation", "pdf_generation")
- Dashboard: "AI cost per quote", "AI cost per deal closed", "AI ROI"
- Budget alerts: "AI spend this month exceeds $X"

### 3.2 Monthly Budget with Soft/Hard Limits
**Paperclip**: 80% soft warning, 100% hard auto-pause. Company + agent level budgets.
**Quotey idea**: **Discount Budget System**:
- Each rep gets a monthly discount budget (total $ of discounts they can give)
- Soft alert at 80%: "You've used 80% of your discount authority this month"
- Hard limit at 100%: requires manager approval for any further discounts
- Manager has team discount budget; VP has org discount budget
- Prevents: end-of-quarter discount panic, margin erosion
- Dashboard: "Discount budget utilization by rep/team/org"

### 3.3 Billing Code Attribution
**Paperclip**: `billing_code` on issues and cost_events for cross-team cost attribution.
**Quotey idea**: **Deal Cost Attribution**:
- Tag quotes with `cost_center`, `campaign_id`, `channel` (inbound/outbound/partner)
- Track: discount costs, AI costs, processing costs per attribution dimension
- Enables: "Inbound deals average 5% discount; outbound average 12%"
- Finance reporting: margin analysis by channel/campaign/team

---

## 4. OBSERVABILITY & AUDIT IDEAS

### 4.1 Immutable Activity Log with Actor Model
**Paperclip**: `activity_log` with `actor_type` (agent/user/system), `actor_id`, `action`, `entity_type`, `entity_id`, `details` JSONB. Every mutation logged.
**Quotey idea**: **Enhanced Audit Trail**. Quotey already has `audit_event`, but could be enriched:
- Add `actor_type`: rep/manager/system/ai/integration
- Add structured `details` JSONB: before/after values for every field change
- Add `run_id` or `session_id`: link audit events to the AI session that triggered them
- Add `entity_type` + `entity_id`: polymorphic reference to any entity
- Enables: "Show me everything that happened to quote Q-2026-0042"
- Enables: "Show me every action the AI agent took today"

### 4.2 Real-Time Live Events
**Paperclip**: `publishLiveEvent` after every mutation — WebSocket broadcast with entity type and company routing.
**Quotey idea**: **Live Quote Updates**:
- WebSocket/SSE from server to portal/dashboard
- Events: quote_updated, approval_requested, approval_decided, comment_added, price_changed
- Slack integration: live events trigger Slack notifications
- Dashboard: real-time deal pipeline movement
- Portal: customer sees "Your quote is being reviewed" live status

### 4.3 Dashboard Summary Metrics
**Paperclip**: Dashboard endpoint returns agent counts by status, issue counts by status, MTD spend, budget utilization, pending approvals, stale task count.
**Quotey idea**: **Ops Dashboard API**:
- Active quotes by status (draft/sent/reviewed/accepted/expired)
- Pending approvals count + average wait time
- MTD: quotes created, quotes won, win rate, average discount
- Pipeline: total pipeline value, weighted pipeline
- Stale: quotes untouched for >X days
- AI: queries handled, cost, most common operations
- Health: average quote creation time, approval turnaround time

---

## 5. EXECUTION & SCHEDULING IDEAS

### 5.1 Heartbeat Protocol
**Paperclip**: Agents wake on intervals (heartbeat), check for work, execute, report back. Configurable interval (min 30s), skip if paused/budget-exceeded/already-running.
**Quotey idea**: **Scheduled Quote Actions**:
- Auto-follow-up: "Ping customer if quote not viewed in 3 days"
- Auto-expire: "Close quote if not accepted in 30 days"
- Auto-escalate: "If approval pending > 4 hours, escalate to VP"
- Price refresh: "Re-price quotes with updated catalog every night"
- Reminder: "Notify rep of quotes expiring this week"
- Implementation: `scheduled_action` table with `action_type`, `entity_id`, `fire_at`, `status`

### 5.2 Orphan Recovery / Stale Detection
**Paperclip**: `reapOrphanedRuns()` marks stale runs as failed. Dashboard surfaces stale tasks.
**Quotey idea**: **Stale Quote Detection**:
- Quotes in "draft" for > 7 days → flag as stale
- Quotes "sent" but not viewed in > 5 days → trigger follow-up
- Approvals pending > SLA threshold → auto-escalate
- Negotiation sessions idle > 48 hours → auto-close with reason
- Weekly "health check" report to sales ops

### 5.3 Agent Runtime State Persistence
**Paperclip**: `agent_runtime_state` with `session_id`, `state_json`, token counters, last run status. Persists across heartbeats.
**Quotey idea**: **AI Context Persistence Across Sessions**:
- `ai_session_state`: rep_id, quote_id, context_json, last_interaction_at
- When rep returns to a quote, AI remembers: previous pricing discussions, customer preferences, discount rationale
- Avoids: "I already told the bot this 3 times"
- Enables: multi-turn negotiation memory across Slack sessions

---

## 6. ORGANIZATIONAL STRUCTURE IDEAS

### 6.1 Reporting Hierarchy (reports_to)
**Paperclip**: `agents.reports_to` self-referential FK. Org chart visualization.
**Quotey idea**: **Sales Org Hierarchy**:
- `sales_rep.reports_to` → manager → VP → CRO
- Approval routing follows hierarchy: rep → manager → VP (automatic)
- Discount authority cascades: rep 10% → manager 20% → VP 30% → CRO unlimited
- Org chart view in dashboard showing team structure and performance

### 6.2 Skills / Capabilities Description
**Paperclip**: Agents have `capabilities` text + SKILLS.md injection for context.
**Quotey idea**: **Rep Specialization / Competency Profile**:
- `rep_capabilities`: "Enterprise security products", "SMB bundles", "Partner channel"
- AI uses capabilities to route complex quotes to specialists
- "This quote involves custom security configuration — routing to Sarah (security specialist)"
- Lead assignment: match inbound request to best-fit rep

### 6.3 Company Portability / Templates
**Paperclip**: Export company config as template (manifest.json + markdown). Import to create new company.
**Quotey idea**: **Pricing Configuration Templates**:
- Export: "Enterprise pricing config" → template with products, policies, approval rules, discount tiers
- Import: new org spins up with battle-tested pricing config
- Versioning: "Q1 2026 pricing" vs "Q2 2026 pricing" as named configs
- Rollback: revert to previous pricing configuration instantly
- A/B test: run two pricing configs for different segments

---

## 7. TECHNICAL ARCHITECTURE IDEAS

### 7.1 Drizzle-Style Schema-as-Code
**Paperclip**: Each entity gets its own schema file (`packages/db/src/schema/agents.ts`), exported via barrel index.
**Quotey idea**: Already doing this with sqlx migrations, but could adopt the per-entity schema documentation pattern. Each migration could have a companion `.md` file explaining the entity's purpose and relationships.

### 7.2 Service Layer Pattern
**Paperclip**: `server/src/services/*.ts` — one service per entity with clear business rules, validation, side effects.
**Quotey idea**: Strengthen the repository pattern with explicit service-layer functions that encode business rules:
- `QuoteService::create_with_validation()` — validates product exists, rep has authority, pricing is current
- `ApprovalService::decide()` — validates approver role, logs decision, triggers side effects
- `NegotiationService::evaluate_with_guardrails()` — enforces concession policy, budget limits

### 7.3 JSONB for Flexible Config
**Paperclip**: Heavy use of JSONB for `adapter_config`, `runtime_config`, `permissions`, `payload`, `details`.
**Quotey idea**: Use JSONB more for extensible metadata:
- `quote.metadata` JSONB: custom fields per org (industry, segment, competitors, urgency)
- `product.attributes` JSONB: arbitrary product attributes beyond fixed columns
- `approval_request.context` JSONB: rich context for approver (competitive intel, customer history, AI recommendations)

### 7.4 Issue Prefix + Counter (Human-Readable IDs)
**Paperclip**: `companies.issue_prefix` + `issue_counter` → generates "PAP-42" style identifiers.
**Quotey idea**: Already have `Q-2026-XXXX`. Could extend:
- Per-org prefix: `ACME-Q-0042` vs `BETA-Q-0017`
- Approval IDs: `ACME-APR-0001`
- Negotiation IDs: `ACME-NEG-0001`
- Makes multi-tenant IDs unambiguous

---

## 8. UI / UX IDEAS

### 8.1 Kanban Board for Quotes
**Paperclip**: `KanbanBoard.tsx` for issues with drag-drop status transitions.
**Quotey idea**: **Quote Pipeline Kanban**:
- Columns: Draft → Priced → Sent → Under Review → Approved → Accepted → Won/Lost
- Drag-drop to advance quotes through pipeline
- Color-coded by age (green → yellow → red as quote ages)
- Quick actions: send, approve, clone, archive

### 8.2 Org Chart Visualization
**Paperclip**: `OrgChart.tsx` with live status indicators.
**Quotey idea**: **Sales Team Org Chart**:
- Tree view of sales org
- Per-node metrics: active quotes, pipeline value, discount budget remaining
- Status: green (active), yellow (near budget limit), red (over limit)
- Click to drill into rep's quotes

### 8.3 Command Palette
**Paperclip**: `CommandPalette.tsx` — keyboard-driven navigation.
**Quotey idea**: **Quick Actions Palette** (Cmd+K):
- "Create quote for [customer]"
- "Find quote Q-2026-..."
- "Approve pending requests"
- "Check pricing for [product]"
- Fast keyboard-driven workflow for power users

### 8.4 Filter Bar + Inbox
**Paperclip**: `FilterBar.tsx`, `Inbox.tsx`, `MyIssues.tsx`
**Quotey idea**: **Rep Inbox / My Quotes**:
- Personalized view: my active quotes, my pending approvals, my follow-ups
- Filters: by status, by customer, by age, by value
- Priority inbox: expired quotes, stale approvals, customer-viewed-but-not-responded

---

## 9. SECURITY IDEAS

### 9.1 Secret Scrubbing
**Paperclip**: Separate `company_secrets` + `company_secret_versions` tables. Config APIs redact plaintext. Activity/approval payloads never persist raw secrets.
**Quotey idea**: **Credential Isolation**:
- API keys for CRM/ERP integrations stored in separate `integration_secrets` table
- Never logged in audit trail
- Encrypted at rest with master key
- Rotatable without downtime

### 9.2 Redaction in Logs
**Paperclip**: `sanitizeRecord()` before database insertion of activity details.
**Quotey idea**: Already sanitize, but formalize:
- Define sensitive field patterns: `*_token`, `*_key`, `*_secret`, `password`, `ssn`
- Auto-redact in audit_event details JSONB
- Log scrubber for structured JSON logs

---

## PRIORITY RANKING (Impact × Effort)

### Quick Wins (High Impact, Low Effort)
1. **Quote Comments/Activity Feed** (1.5) — simple table, huge UX improvement
2. **Rich Approval Types + Payload** (2.1) — extends existing approval_request
3. **Org-Level Policy Toggles** (2.4) — settings table, runtime configurable
4. **Stale Quote Detection** (5.2) — scheduled check, Slack notification
5. **Dashboard Summary API** (4.3) — aggregation queries over existing data

### Medium Effort, High Value
6. **Rep Profile Entity** (1.3) — new table + migration, big governance improvement
7. **Quote Locking** (1.4) — atomic checkout pattern, prevents conflicts
8. **Approval Discussion Thread** (2.2) — new table, richer approval workflow
9. **Discount Budget System** (3.2) — monthly budget tracking per rep
10. **AI Cost Tracking Per Quote** (3.1) — instrument AI calls

### Bigger Investments
11. **Sales Org Hierarchy** (6.1) — reports_to chain, approval routing
12. **Scheduled Quote Actions** (5.1) — heartbeat-style scheduler
13. **AI Context Persistence** (5.3) — multi-session memory
14. **Integration Adapter Pattern** (1.6) — pluggable CRM/notification/PDF
15. **Multi-Tenant Org Scoping** (1.1) — company_id on all tables
16. **Quote Pipeline Kanban** (8.1) — full UI component
17. **Pricing Configuration Templates** (6.3) — export/import config

---

## KEY ARCHITECTURAL TAKEAWAYS

1. **Everything traces to a goal**: Paperclip won't let you create orphan work. Quotey should enforce that every quote traces to a deal/opportunity/objective.

2. **Immutable audit trail is non-negotiable**: Every mutation, every actor, every timestamp. Quotey has this partially — make it complete.

3. **Atomic ownership prevents chaos**: Single assignee + atomic checkout. Apply to quotes, approvals, negotiations.

4. **JSONB for extensibility, fixed columns for queries**: Use JSONB for org-specific metadata; use typed columns for anything you filter/sort/aggregate on.

5. **Governance is a feature, not overhead**: Budget limits, approval gates, emergency controls. Sales ops needs the same control surface that Paperclip gives to company boards.

6. **Comments replace side-channels**: If discussion happens in Slack DMs, it's lost. If it happens on the quote itself, it's context for anyone who touches that quote.

7. **Cost tracking enables ROI**: Track what the AI costs per operation. This justifies the system's existence.
