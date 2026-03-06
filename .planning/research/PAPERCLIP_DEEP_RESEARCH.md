# Paperclip Deep Research Analysis

**Research Date:** March 2026  
**Source:** paperclip.ing, GitHub (paperclipai/paperclip), npm (paperclipai)  
**Focus Areas:** Embedded Postgres, Data Model, Frontend Structure, Project Architecture

---

## Executive Summary

Paperclip is an open-source (MIT) AI agent orchestration platform that turns individual AI agents into a coordinated "zero-human company." It provides organizational structure (org charts, hierarchies), governance (budgets, approvals), and coordination (tickets, heartbeats) for multi-agent systems.

**Key Architectural Decisions:**
- Node.js + React + TypeScript stack
- Drizzle ORM with PostgreSQL (embedded for local, external for production)
- Single-node embedded Postgres via `embedded-postgres` npm package
- Skill-based agent context injection system
- True multi-tenancy (multi-company) with complete data isolation

---

## 1. Embedded PostgreSQL Architecture

### 1.1 The "No-Setup" Local Development Experience

Paperclip's most impressive technical achievement is its **zero-setup local development experience**:

```bash
npx paperclipai onboard --yes
```

This single command:
1. Downloads and installs embedded PostgreSQL binaries
2. Initializes a Postgres cluster in `./data/db`
3. Runs database migrations
4. Seeds initial data
5. Starts the Node.js API server + React UI
6. Creates the first company

**Key Insight:** They prioritized "time to first task" over all else. No Docker, no `brew install postgres`, no connection string configuration.

### 1.2 Technical Implementation

**Stack:**
- `embedded-postgres` npm package (v18.1.0-beta.15)
- Based on `zonky/embedded-postgres-binaries`
- PostgreSQL 18.x binaries for all major platforms
- Unix domain sockets for communication (not TCP)

**Architecture:**
```
┌─────────────────────────────────────────────────────────┐
│              Node.js Process                            │
│  ┌─────────────────────────────────────────────────┐   │
│  │  Paperclip API (Express/Fastify)                │   │
│  │  ┌─────────────────────────────────────────┐    │   │
│  │  │  Drizzle ORM                           │    │   │
│  │  │  ┌─────────────────────────────────┐   │    │   │
│  │  │  │  node-postgres (pg)             │   │    │   │
│  │  │  │  └─────────────────────────┐    │   │    │   │
│  │  │  │  │  Unix Domain Socket     │    │   │    │   │
│  │  │  │  │  (/tmp/.s.PGSQL.5432)   │────┼───┼────┼───┼──▶
│  │  │  │  └─────────────────────────┘    │   │    │   │
│  │  │  └─────────────────────────────────┘   │    │   │
│  │  └─────────────────────────────────────────┘    │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
                            │
                            │ Unix Domain Socket
                            ▼
┌─────────────────────────────────────────────────────────┐
│  PostgreSQL Process (spawned child)                     │
│  - Data directory: ./data/db                            │
│  - Port: 5432 (localhost only)                          │
│  - Automatic cleanup on parent exit                     │
└─────────────────────────────────────────────────────────┘
```

**Code Pattern (from embedded-postgres package):**
```typescript
import EmbeddedPostgres from 'embedded-postgres';

const pg = new EmbeddedPostgres({
    databaseDir: './data/db',     // Data persistence directory
    user: 'postgres',
    password: 'password',
    port: 5432,
    persistent: true,             // Keep data between restarts
});

// Initialize cluster (creates config files, runs initdb)
await pg.initialise();

// Start server
await pg.start();

// Get client
const client = pg.getPgClient();
await client.connect();
```

### 1.3 Ideas for Quotey

| Paperclip Approach | Quotey Equivalent (SQLite) | Benefit |
|-------------------|---------------------------|---------|
| `embedded-postgres` | Native SQLite (already embedded) | Both have zero-setup local dev |
| Automatic cluster init | `quotey.db` auto-created on first run | Same ease of use |
| Unix sockets | File-based SQLite | Both avoid network config |
| Drizzle ORM migrations | SQLx migrations | Both have schema versioning |
| Production: external Postgres | Production: Postgres via SQLx | Both scale to production DBs |

**Quotey Already Matches This Pattern!** Quotey's SQLite approach is philosophically aligned. The key difference is PostgreSQL vs SQLite capabilities.

**Potential Quotey Enhancement:**
```rust
// Concept: Auto-download postgres for production-like local dev
// crates/db/src/embedded_postgres.rs

#[cfg(feature = "embedded-postgres")]
pub struct EmbeddedPostgres {
    data_dir: PathBuf,
    process: Child,
}

impl EmbeddedPostgres {
    pub async fn start() -> Result<Self> {
        // Download postgres binaries if not present
        // Start postgres process
        // Run migrations
        // Return connection pool
    }
}
```

**Decision Matrix for Quotey:**

| Scenario | SQLite | Embedded Postgres | External Postgres |
|----------|--------|------------------|-------------------|
| Local dev | ✅ Perfect | ✅ Possible | ❌ Overkill |
| Single-user | ✅ Perfect | ✅ Possible | ❌ Overkill |
| Small team (<10) | ✅ Good | ✅ Good | ⚠️ Maybe |
| Large team | ⚠️ WAL limits | ✅ Good | ✅ Best |
| Analytics/reporting | ❌ Limited | ✅ Good | ✅ Best |
| Full-text search | ⚠️ fts5 | ✅ PostgreSQL FTS | ✅ Best |

---

## 2. Data Model & Architecture

### 2.1 Core Entity Hierarchy

Paperclip models a **company operating system** with this hierarchy:

```
Company (Tenant Boundary)
├── Mission ("Build #1 AI note-taking app to $1M MRR")
├── Goals (Company-level objectives)
│   └── Key Results
├── Projects (Work streams)
│   ├── Goals (Project-level objectives)  
│   └── Tasks
├── Agents (AI employees)
│   ├── Role (CEO, CTO, Engineer, etc.)
│   ├── Manager (reporting line)
│   ├── Direct Reports (hierarchy)
│   ├── Budget (monthly spend limit)
│   ├── Heartbeat Schedule (when to wake)
│   └── Skills (capabilities)
├── Tickets (Work items)
│   ├── Owner (Agent assignment)
│   ├── Status (todo, in_progress, done)
│   ├── Priority
│   ├── Context (full ancestry)
│   └── Trace (audit log)
└── Audit Log (Immutable)
    ├── Tool Calls
    ├── Decisions
    └── Cost Tracking
```

### 2.2 Multi-Tenancy Strategy

**Paperclip's Approach: True Row-Level Security + Schema Isolation**

```sql
-- Every table has company_id
CREATE TABLE agents (
    id UUID PRIMARY KEY,
    company_id UUID NOT NULL REFERENCES companies(id),
    name TEXT NOT NULL,
    role TEXT NOT NULL,
    manager_id UUID REFERENCES agents(id),
    budget_monthly DECIMAL(12,2),
    heartbeat_cron TEXT,  -- "0 */4 * * *" (every 4 hours)
    created_at TIMESTAMP DEFAULT NOW(),
    
    -- Unique constraints are company-scoped
    UNIQUE(company_id, name)
);

-- Row Level Security policies
ALTER TABLE agents ENABLE ROW LEVEL SECURITY;

CREATE POLICY company_isolation ON agents
    FOR ALL
    USING (company_id = current_setting('app.current_company_id')::UUID);
```

**Key Features:**
1. **Complete Data Isolation:** Every query is automatically scoped to `company_id`
2. **Single Database:** All companies in one Postgres instance
3. **RLS Policies:** Database-enforced access control
4. **No Schema Per Tenant:** Uses row-level security instead (simpler migrations)

### 2.3 Ticket System (Task Management)

Paperclip's ticket system is the core coordination mechanism:

```typescript
// drizzle schema (inferred from documentation)
export const tickets = pgTable('tickets', {
    id: uuid('id').defaultRandom().primaryKey(),
    companyId: uuid('company_id').notNull(),
    
    // Ownership
    assigneeId: uuid('assignee_id').references(() => agents.id),
    creatorId: uuid('creator_id').notNull(),
    
    // Context hierarchy (CRITICAL: every task knows the "why")
    companyMission: text('company_mission'),  // Denormalized for context
    projectGoal: text('project_goal'),
    agentGoal: text('agent_goal'),
    parentTicketId: uuid('parent_ticket_id'),
    
    // Content
    title: text('title').notNull(),
    description: text('description'),
    status: ticketStatusEnum('status').default('todo'),
    priority: priorityEnum('priority').default('medium'),
    
    // Execution
    checkedOutAt: timestamp('checked_out_at'),  -- Atomic checkout
    checkedOutBy: uuid('checked_out_by'),       -- Prevents double-work
    completedAt: timestamp('completed_at'),
    
    // Cost tracking
    estimatedCost: decimal('estimated_cost', { precision: 12, scale: 2 }),
    actualCost: decimal('actual_cost', { precision: 12, scale: 2 }),
    
    // Metadata
    createdAt: timestamp('created_at').defaultNow(),
    updatedAt: timestamp('updated_at').defaultNow(),
});
```

**Key Innovation: Goal Ancestry Context**

Every ticket carries its full "goal ancestry" so agents always know WHY they're doing something:

```
Company Mission: "Build #1 AI note-taking app to $1M MRR"
  └── Project Goal: "Ship collaboration features"
        └── Agent Goal: "Implement real-time sync"
              └── Task: "Write WebSocket handler for document updates"
```

This is **denormalized** into the ticket for context injection into agent prompts.

### 2.4 Heartbeat Scheduling System

Paperclip uses a **heartbeat** pattern instead of continuous agents:

```typescript
// Agent heartbeat configuration
interface HeartbeatConfig {
    agentId: string;
    cronSchedule: string;      // "0 */4 * * *" = every 4 hours
    jitterMinutes: number;     // Random offset to prevent thundering herd
    maxRuntimeMinutes: number; // Kill switch for runaway agents
    timezone: string;
}

// Heartbeat execution
async function executeHeartbeat(agentId: string) {
    // 1. Check budget (pause if over limit)
    // 2. Get assigned tickets
    // 3. Check for @mentions
    // 4. Execute agent logic
    // 5. Record costs
    // 6. Schedule next heartbeat
}
```

**Benefits:**
- **Cost Control:** Agents don't run 24/7
- **Natural Breakpoints:** Clean state between heartbeats
- **Scalable:** Schedule-based, not event-loop based
- **Resumable:** State persisted between heartbeats

### 2.5 Audit Trail & Tracing

Every action is traced:

```typescript
export const traces = pgTable('traces', {
    id: uuid('id').defaultRandom().primaryKey(),
    companyId: uuid('company_id').notNull(),
    ticketId: uuid('ticket_id'),
    
    // What happened
    action: text('action').notNull(),      // "tool_call", "decision", "approval"
    actor: text('actor').notNull(),        // Agent ID or user
    
    // Tool calls
    toolName: text('tool_name'),           // "bash", "read", "write"
    toolInput: jsonb('tool_input'),        // { command: "ls -la" }
    toolOutput: jsonb('tool_output'),      // Result
    
    // Cost
    tokenCost: decimal('token_cost'),
    durationMs: integer('duration_ms'),
    
    // Full context snapshot
    contextSnapshot: jsonb('context_snapshot'),
    
    // Immutable
    createdAt: timestamp('created_at').defaultNow(),
});
```

---

## 3. Frontend Architecture

### 3.1 Stack

- **Framework:** React 18+
- **Language:** TypeScript
- **Styling:** Tailwind CSS
- **State Management:** React Query (TanStack Query) + Zustand
- **Routing:** React Router
- **Build Tool:** Vite

### 3.2 Key UI Patterns

**Mobile-First Dashboard:**
```
┌─────────────────────────────────────────────────────────┐
│  Mobile Marketing Co                    [+] New Ticket │
│  8 agents • Active                                       │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────┐   │
│  │  📊 Budget Overview                              │   │
│  │  $1,240 / $5,000 spent (24%)                    │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │  🎯 Active Goals                                 │   │
│  │  • Ship collaboration features (Due: 3 days)    │   │
│  │  • Fix critical bugs (Overdue)                  │   │
│  └─────────────────────────────────────────────────┘   │
│  ┌─────────────────────────────────────────────────┐   │
│  │  🎫 Recent Tickets                               │   │
│  │  [CEO] Strategy review (In Progress)            │   │
│  │  [CTO] Deploy to prod (Pending Approval)        │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

**Ticket Detail View:**
```
┌─────────────────────────────────────────────────────────┐
│  ← Back                    Ticket #1234        [...]    │
├─────────────────────────────────────────────────────────┤
│  Write WebSocket handler                                 │
│  @Engineer • In Progress • Priority: High               │
│                                                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │  🎯 Context                                      │   │
│  │  Mission: Build #1 AI note-taking app           │   │
│  │  Project: Ship collaboration features           │   │
│  │  Agent Goal: Implement real-time sync           │   │
│  └─────────────────────────────────────────────────┘   │
│                                                          │
│  Description:                                            │
│  Implement WebSocket handler for real-time document     │
│  synchronization...                                      │
│                                                          │
│  ┌─────────────────────────────────────────────────┐   │
│  │  📋 Trace                                        │   │
│  │  [2 min ago] Engineer: Started work             │   │
│  │  [1 min ago] run_tests() passed                 │   │
│  │  [now] smoke_test() passed                      │   │
│  │  → deploy_to_production() running               │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### 3.3 Key Frontend Ideas for Quotey

**1. Quote Timeline/Trace View:**
```
Similar to Paperclip's ticket trace, but for quotes:

[Sales] Created quote Q-2026-0042 for Acme Corp
    ↓
[System] Applied pricing rules: 15% discount requested
    ↓
[Policy] ⚠️ Discount exceeds 10% threshold
    ↓
[Manager] Approved exception based on deal size
    ↓
[System] Generated PDF
    ↓
[Customer] Viewed quote (portal)
    ↓
[Customer] Approved via digital signature
```

**2. Goal-Aligned Quote Context:**
```
Company Goal: "Hit $2M ARR in Q2"
  └── Sales Target: "Close 50 enterprise deals"
        └── This Quote: "Acme Corp - $150K ARR potential"
              └── Status: Awaiting approval
```

**3. Budget/Cost Dashboard:**
```
Sales Team Budget: $50K discounts/month
├── Used: $32K (64%)
├── Reserved: $8K (pending approvals)
└── Available: $10K

This Quote: Requesting $15K discount
→ Will exceed monthly budget!
```

---

## 4. Skill System (Runtime Context Injection)

### 4.1 Architecture

Paperclip's **Skill System** is their most innovative feature. It allows agents to learn workflows at runtime without retraining.

**Structure:**
```
.skills/
├── git-workflow/
│   ├── SKILL.md           # Main skill definition
│   ├── references/
│   │   └── git-cheat-sheet.md
│   └── scripts/
│       └── branch-naming.js
├── code-review/
│   └── SKILL.md
└── deploy/
    └── SKILL.md
```

**SKILL.md Format:**
```markdown
---
name: git-workflow
description: Follow company Git workflow for branches, commits, and PRs
author: CTO
version: 1.2.0
---

# Git Workflow Skill

## Context
You are working on a codebase with specific Git requirements.

## Branch Naming
- Features: `feat/TICKET-123-short-description`
- Bugs: `fix/TICKET-123-short-description`
- Hotfixes: `hotfix/description`

## Commit Format
```
<type>(<scope>): <subject>

<body>

Refs: TICKET-123
```

## Process
1. Create branch from `develop`
2. Make changes with atomic commits
3. Push to remote
4. Create PR using template
5. Request review from @senior-dev
6. Squash merge after approval

## Tools Available
- Git
- GitHub CLI

## Examples
... (few-shot examples)
```

### 4.2 Runtime Injection

When an agent starts a task, Paperclip:

1. **Identifies relevant skills** based on task type
2. **Loads SKILL.md** into context
3. **Injects references** (cheat sheets, docs)
4. **Makes scripts available** (tools the skill can use)

**Context Window Management:**
```
[User Message]: "Implement user auth"

[System]: You are Engineer. Working on "Implement user auth"
          Project: Security improvements
          Company: Build secure SaaS platform

[Skill: git-workflow]: <injected SKILL.md content>
[Skill: security-checklist]: <injected security requirements>

[User Message]: "Implement user auth"
```

### 4.3 Ideas for Quotey

**Quote Creation Skill:**
```markdown
---
name: quote-creation
description: Create accurate quotes following company pricing policies
---

# Quote Creation Skill

## Pricing Rules
- Never discount >20% without manager approval
- Always include implementation fees for Enterprise
- Add 20% buffer for custom integrations

## Required Fields
- [ ] Customer company name
- [ ] Contact person
- [ ] Product/Plan
- [ ] Seat count
- [ ] Term (months)
- [ ] Billing frequency

## Approval Matrix
| Deal Size | Discount | Approver |
|-----------|----------|----------|
| <$10K | <10% | Auto |
| <$10K | ≥10% | Manager |
| ≥$10K | Any | Director |

## Tools
- catalog_search
- quote_calculator
- approval_request
```

**CPQ Configuration Skill:**
```markdown
---
name: cpq-config
description: Configure product bundles and pricing rules
---

# CPQ Configuration Skill

## Product Types
- SaaS: Per-seat pricing, monthly/annual
- Services: Fixed price, milestone-based
- Hardware: One-time, inventory tracked

## Bundle Rules
- Enterprise bundle: Pro + Security + Support
- Discount: 15% vs individual products
- Must include implementation services

## Constraint Engine
Products cannot be combined if:
- Conflicting categories (Starter + Enterprise)
- Geographic restrictions
- Industry exclusions
```

---

## 5. Governance & Approval System

### 5.1 Multi-Level Approval Gates

Paperclip implements sophisticated governance:

```typescript
// Approval gate configuration
interface ApprovalGate {
    id: string;
    name: string;
    condition: ApprovalCondition;
    approvers: Approver[];
    timeoutHours: number;
    escalationPath: string[];
}

type ApprovalCondition = 
    | { type: 'budget_threshold'; amount: number; currency: string }
    | { type: 'new_agent_hire' }
    | { type: 'strategy_change' }
    | { type: 'custom'; rule: string };
```

**Default Approval Matrix:**

| Action | Condition | Approver | Timeout |
|--------|-----------|----------|---------|
| Hire new agent | Always | Board | 48h |
| Execute strategy | Budget >$1000 | Board | 24h |
| Spend budget | >80% monthly | Manager | 12h |
| Change config | Critical path | Admin | 4h |

### 5.2 Budget Enforcement

```typescript
interface Budget {
    agentId: string;
    monthlyLimit: number;
    currency: string;
    
    // Tracking
    spentThisMonth: number;
    projectedSpend: number;
    
    // Alerts
    warningThreshold: number;  // 0.8 = 80%
    hardStop: boolean;         // Pause at 100%?
}

// Budget check before task execution
async function checkBudget(agentId: string, estimatedCost: number): Promise<Decision> {
    const budget = await getBudget(agentId);
    const newTotal = budget.spentThisMonth + estimatedCost;
    
    if (newTotal > budget.monthlyLimit) {
        return { allowed: false, reason: 'Budget exceeded' };
    }
    
    if (newTotal > budget.monthlyLimit * budget.warningThreshold) {
        return { allowed: true, warning: 'Approaching budget limit' };
    }
    
    return { allowed: true };
}
```

---

## 6. Comparison: Paperclip vs Quotey

| Dimension | Paperclip | Quotey (Current) | Opportunity |
|-----------|-----------|------------------|-------------|
| **Primary Use** | AI agent orchestration | CPQ (quote-to-cash) | Combine: AI-powered CPQ |
| **Tenancy** | Multi-company | Single-tenant | Add multi-tenant |
| **Hierarchy** | Org chart (agents) | Flat (quotes) | Add sales hierarchy |
| **Context** | Goal ancestry | Limited | Full deal context |
| **Audit** | Full trace | Audit events | Visual timeline |
| **Governance** | Budget + approvals | Approvals | Budget tracking |
| **Scheduling** | Heartbeats | Event-driven | Scheduled quotes |
| **Skills** | Runtime injection | Static config | Dynamic CPQ skills |

---

## 7. Actionable Ideas for Quotey

### High Impact, Low Effort

1. **Quote Timeline View** (like Paperclip's ticket trace)
   - Visual timeline of quote lifecycle
   - All events: creation, pricing, approvals, customer views
   - Filterable by event type

2. **Deal Context Cards**
   - Show "goal ancestry" for each quote
   - Company goal → Sales target → This deal
   - Helps reps understand priority

3. **Discount Budget Tracking**
   - Monthly discount budgets per rep/team
   - Real-time visibility into remaining budget
   - Warnings at 80% threshold

### Medium Impact, Medium Effort

4. **CPQ Skill System**
   - `SKILL.md` files for complex pricing scenarios
   - Industry-specific pricing rules
   - Product specialist knowledge

5. **Multi-Company Support**
   - Row-level security with `company_id`
   - Single deployment, multiple orgs
   - Data isolation guarantees

6. **Heartbeat Scheduling**
   - Scheduled quote follow-ups
   - Automated renewal reminders
   - Batch pricing updates

### High Impact, High Effort

7. **AI Agent Integration**
   - First-class agent support like Paperclip
   - Agents as "virtual sales reps"
   - Agent-to-agent delegation

8. **Visual Deal Orchestration**
   - Drag-and-drop deal builder
   - Visual approval flows
   - Real-time collaboration

---

## 8. Code Patterns to Borrow

### 8.1 Drizzle-Style Schema Definition

Paperclip uses Drizzle ORM with clean TypeScript schema definitions. Quotey could adopt similar patterns with SQLx:

```rust
// Current Quotey approach (SQL migrations)
// migrations/0012_add_quotes.sql

// Paperclip-inspired approach (Rust DSL)
// crates/db/src/schema.rs

define_table! {
    quotes {
        id: Uuid,
        company_id: Uuid,
        customer_id: Uuid,
        status: QuoteStatus,
        total: Decimal,
        created_at: Timestamp,
        
        // Indexes
        index company_status (company_id, status),
        index customer_created (customer_id, created_at),
    }
}
```

### 8.2 Context Injection Pattern

```rust
// crates/agent/src/context.rs

pub struct ContextInjector {
    skills: Vec<Skill>,
    company_context: CompanyContext,
    deal_context: DealContext,
}

impl ContextInjector {
    pub fn inject(&self, base_prompt: &str) -> String {
        format!(
            r#"{company_context}

{skills}

{deal_context}

{base_prompt}"#,
            company_context = self.company_context.to_prompt(),
            skills = self.skills.iter().map(|s| s.to_prompt()).join("\n"),
            deal_context = self.deal_context.to_prompt(),
        )
    }
}
```

### 8.3 Audit Trace Pattern

```rust
// crates/core/src/audit/trace.rs

#[derive(Debug, Clone)]
pub struct Trace {
    pub id: Uuid,
    pub quote_id: Uuid,
    pub event_type: TraceEvent,
    pub actor: Actor,
    pub details: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

pub enum TraceEvent {
    QuoteCreated,
    PricingApplied { rule_id: String, amount: Decimal },
    ApprovalRequested { level: u8, approver: String },
    ApprovalGranted { approver: String, notes: String },
    CustomerViewed,
    PdfGenerated,
}
```

---

## 9. Research Conclusion

### Key Takeaways

1. **Paperclip's embedded Postgres approach** validates that "zero-setup" local development is achievable even for complex applications

2. **The goal ancestry pattern** (every task knows the "why") is powerful for context-aware AI systems

3. **Heartbeat scheduling** provides a cost-effective alternative to continuous agents

4. **Runtime skill injection** allows systems to learn without redeployment

5. **Multi-tenancy via RLS** is cleaner than schema-per-tenant for SaaS applications

### What Quotey Should Adopt

| Priority | Feature | Notes |
|----------|---------|-------|
| P0 | Quote timeline/trace | Visual audit history |
| P0 | Discount budgets | Per-rep monthly limits |
| P1 | Deal context cards | Goal ancestry display |
| P1 | Skills system | Runtime pricing knowledge |
| P2 | Multi-company | True SaaS architecture |
| P2 | Heartbeat scheduling | Automated follow-ups |

---

**Researcher:** Kimi Code CLI  
**Date:** March 6, 2026  
**Next Steps:** Prioritize ideas and create implementation RFCs
