# Architecture Decision Research Dossier

**Project:** Quotey (Rust local-first CPQ agent)  
**Date:** 2026-02-23  
**Scope:** Foundation and near-foundation architectural decisions  
**Primary bead:** `bd-3d8.11.1`  
**Dependent beads impacted:** `bd-3d8.11.2` through `bd-3d8.11.10`, and foundation gate `bd-3d8.10`

---

## 1. Why This Document Exists

Quotey is intentionally ambitious: natural-language UX in Slack, deterministic CPQ core, strict auditability, local-first deployment, and a multi-agent build process.

That combination creates a high risk of architectural drift if we let implementation happen before decision framing.

This dossier exists to prevent that.

It defines:
- The architecture decision protocol all agents should follow.
- The concrete decisions we need before broad implementation.
- Tradeoff analysis and rejection rationale for each major decision.
- A phased dependency sequence that keeps the system coherent.
- Verification methods that prove decisions work in practice.

This is not intended to freeze innovation.
It is intended to freeze interfaces, invariants, and contracts where change cost is highest.

---

## 2. Non-Negotiables (Restated from Project Instructions)

1. LLMs are translators, not deterministic decision-makers.
2. Pricing, constraints, policy, and approvals remain deterministic.
3. SQLite is the source of truth for operational state and rules.
4. Slack is the primary interaction surface for v1.
5. Architecture quality and maintainability outrank speed-only shortcuts.
6. Foundation scaffold must complete before broad feature streams.

Interpretation for architecture:
- Any design that can accidentally put price authority in the LLM is invalid.
- Any design that makes deterministic trace reconstruction difficult is invalid.
- Any design that requires web dashboard dependency for core flow is out-of-scope for foundation.

---

## 3. Research Method and Quality Bar

### 3.1 Source Hierarchy

For technical architecture decisions, source quality is ranked as:

1. Official language/runtime/framework/database docs.
2. Official crate docs for crates we will use in production.
3. Vendor product docs (Slack, Composio, etc.).
4. Maintainer statements/changelogs.
5. Community discussion only for context, never as primary evidence.

### 3.2 Evidence Rules

For each ADR candidate:
- Include at least 2 primary sources for core claims.
- Separate hard facts from assumptions.
- Explicitly list unresolved uncertainties.
- Define operational verification criteria before implementation.

### 3.3 Confidence Scale

- `High`: Decision grounded in mature docs and known implementation patterns; low novelty.
- `Medium`: Strong sources exist, but constraints are project-specific and need spike validation.
- `Low`: Material unknowns remain; decision is a temporary default with explicit revisit trigger.

### 3.4 Decision States

- `Proposed`: Analysis complete enough for review.
- `Accepted`: Team agrees and implementation can proceed.
- `Trial`: Temporary decision pending time-boxed spike.
- `Deferred`: Explicitly postponed; not allowed to block foundation.
- `Superseded`: Replaced by a newer accepted decision.

---

## 4. ADR Template (Standard for All Agents)

Use this template for all future ADRs.

```md
# ADR-XXXX: <Short Decision Name>

## Status
Proposed | Accepted | Trial | Deferred | Superseded

## Date
YYYY-MM-DD

## Context
What problem are we solving and why now?

## Decision Drivers
- Driver 1
- Driver 2
- Driver 3

## Options Considered
### Option A
Description
Pros
Cons

### Option B
Description
Pros
Cons

### Option C
Description
Pros
Cons

## Decision
What we choose and where it applies.

## Rationale
Why this option wins now.

## Consequences
### Positive
- ...
### Negative
- ...
### Neutral / Tradeoff
- ...

## Guardrails
Implementation rules that must not be violated.

## Verification Plan
How we prove this decision works.

## Revisit Triggers
Specific events/metrics that force reevaluation.

## Bead Mapping
Primary bead and dependent beads.

## References
Official source links.
```

---

## 5. Decision Dependency Graph (Narrative)

The critical chain:

1. Runtime and crate boundaries.
2. Config/secrets and process bootstrap.
3. Database contract and migration model.
4. Deterministic domain model and flow state machine.
5. Slack event ingress/ack/idempotency boundary.
6. Pricing and policy execution contract.
7. Audit/event model and observability.
8. Reliability and failure-recovery semantics.
9. Security baseline.
10. Test/verification architecture and release gates.

If this sequence is violated, teams will implement components that later require interface-breaking rewrites.

---

## 6. Decision Index and Bead Traceability

| ADR ID | Topic | Primary Bead | Depends On | Confidence | Target State |
|---|---|---|---|---|---|
| ADR-0001 | Workspace and crate topology | `bd-3d8.1` | `bd-3d8.12` | High | Accepted |
| ADR-0002 | Async runtime + process model | `bd-3d8.1` | ADR-0001 | High | Accepted |
| ADR-0003 | Config layering + secrets | `bd-3d8.2` | ADR-0001 | High | Accepted |
| ADR-0004 | SQLite connection and pragma policy | `bd-3d8.3` | ADR-0003 | High | Accepted |
| ADR-0005 | Migration strategy and compatibility | `bd-3d8.3` | ADR-0004 | High | Accepted |
| ADR-0006 | Canonical domain model contracts | `bd-3d8.4`, `bd-3d8.11.2` | ADR-0001, ADR-0004 | Medium | Proposed |
| ADR-0007 | Deterministic flow engine semantics | `bd-3d8.4`, `bd-3d8.11.2` | ADR-0006 | Medium | Proposed |
| ADR-0008 | Pricing precision and representation | `bd-3d8.4`, `bd-3d8.11.3` | ADR-0006 | High | Accepted |
| ADR-0009 | Rules/policy eval ordering | `bd-3d8.11.3` | ADR-0008 | Medium | Proposed |
| ADR-0010 | Slack Socket Mode ingress boundary | `bd-3d8.5`, `bd-3d8.11.4` | ADR-0002, ADR-0003 | High | Accepted |
| ADR-0011 | Slack command grammar and thread lifecycle | `bd-3d8.11.4` | ADR-0010 | Medium | Proposed |
| ADR-0012 | Idempotency and retry strategy | `bd-3d8.11.5` | ADR-0010, ADR-0004 | Medium | Proposed |
| ADR-0013 | Approval governance model | `bd-3d8.11.6` | ADR-0009, ADR-0011 | Medium | Proposed |
| ADR-0014 | CRM adapter boundary (Composio) | `bd-3d8.11.7` | ADR-0003, ADR-0012 | Medium | Trial |
| ADR-0015 | Observability + trace model | `bd-3d8.8`, `bd-3d8.11.8` | ADR-0002, ADR-0012 | High | Accepted |
| ADR-0016 | Security baseline and secret handling | `bd-3d8.11.9` | ADR-0003, ADR-0015 | High | Accepted |
| ADR-0017 | CLI and operator contract | `bd-3d8.9` | ADR-0003, ADR-0005 | High | Accepted |
| ADR-0018 | Health checks and readiness semantics | `bd-3d8.10.2` | ADR-0004, ADR-0015 | High | Accepted |
| ADR-0019 | Verification architecture and quality gates | `bd-3d8.10.3` | ADR-0015, ADR-0017 | High | Accepted |
| ADR-0020 | Decision freeze and change-control protocol | `bd-3d8.11.10` | ADR-0001..0019 | Medium | Proposed |

---

## 7. ADR-0001: Workspace and Crate Topology

### Status
Proposed (high confidence)

### Context
Foundation work needs stable module boundaries before implementation.
A monolithic crate increases accidental coupling and makes deterministic contracts harder to enforce.

### Decision Drivers
- Fast incremental compile times.
- Explicit ownership boundaries.
- Testability of pure business logic independent of I/O.
- Compatibility with multi-agent parallel development.

### Options Considered

#### Option A: Single crate with modules only
Pros:
- Simpler startup.
- Fewer Cargo manifests.

Cons:
- Weak boundary enforcement.
- Easier for infra concerns to leak into domain logic.
- Higher merge conflict probability across agents.

#### Option B: Cargo workspace with domain-specific crates
Pros:
- Formal boundaries via crate dependencies.
- Better compile unit isolation.
- Cleaner mocking and contract tests.

Cons:
- Slightly more setup overhead.
- Requires deliberate dependency governance.

#### Option C: Workspace with many micro-crates
Pros:
- Maximum isolation.

Cons:
- Over-fragmentation for early stage.
- Dependency graph complexity overhead.

### Decision
Choose Option B.
Use a workspace with bounded crate count aligned to architecture domains:
- `core`
- `db`
- `slack`
- `agent`
- `cli`
- `server`

### Rationale
Cargo workspaces are first-class for shared lockfile, shared output, and coordinated metadata while maintaining crate-level boundaries.
Resolver v2 behavior is now normal for modern edition and should be explicitly set in root manifest.

### Guardrails
- `core` must not depend on runtime/network/database crates.
- Domain types live in `core`; adapters implement traits outside `core`.
- All cross-crate dependencies require explicit reason in code review.

### Verification
- `cargo build --workspace` succeeds.
- `cargo tree -i` confirms no `tokio`/`sqlx` in `core` dependency graph.
- Workspace lints apply uniformly.

### Revisit Triggers
- Compile times become bottleneck despite boundaries.
- New bounded context justifies extraction.

### References
- [Cargo workspaces reference](https://doc.rust-lang.org/cargo/reference/workspaces.html)
- [Rust Book: Cargo workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Cargo resolver reference](https://doc.rust-lang.org/cargo/reference/resolver.html)

---

## 8. ADR-0002: Async Runtime and Process Model

### Status
Proposed (high confidence)

### Context
Quotey needs Slack socket loop, database access, and background tasks.
Runtime choice influences cancellation, shutdown safety, observability, and third-party crate compatibility.

### Options Considered

#### Option A: Tokio runtime
Pros:
- Ecosystem default for async Rust.
- Broad compatibility (`axum`, `sqlx`, `reqwest`, Slack ecosystem).
- Mature shutdown/cancellation patterns.

Cons:
- Requires discipline to avoid unbounded task spawning.

#### Option B: async-std/smol runtime
Pros:
- Smaller ecosystem in some cases.

Cons:
- Lower compatibility with targeted crates.
- More adapters required.

### Decision
Use Tokio as the single runtime.
Adopt structured shutdown using cancellation token + signal handling.

### Rationale
Given crate choices already aligned to Tokio, alternative runtime adds complexity without upside.
Tokio’s documented graceful shutdown patterns are sufficient for foundation reliability.

### Guardrails
- No detached background loops without cancellation path.
- All long-running tasks must have join/abort semantics.
- Shutdown deadline enforced via config.

### Verification
- Integration test triggers shutdown and verifies no hung task after deadline.
- Long-running components receive cancellation and flush audit queue.

### References
- [Tokio graceful shutdown topic](https://tokio.rs/tokio/topics/shutdown)
- [Tokio ctrl_c signal](https://docs.rs/tokio/latest/tokio/signal/fn.ctrl_c.html)
- [tokio-util CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)

---

## 9. ADR-0003: Configuration Layering and Secret Handling

### Status
Proposed (high confidence)

### Context
We need deterministic runtime config behavior across local development, demos, CI, and future deployments.
Misconfiguration must fail fast with actionable errors.

### Decision
Adopt layered config precedence:
1. Built-in defaults.
2. Config file.
3. Environment variable overrides.
4. CLI argument overrides.

Use typed config structs deserialized via `serde`.
Use `secrecy::SecretString` for tokens and API keys.

### Options Considered

#### Option A: Environment-only config
Pros:
- Simple twelve-factor style.

Cons:
- Harder local onboarding and reproducibility.
- Poor discoverability for non-secret defaults.

#### Option B: Layered config (selected)
Pros:
- Reproducible local defaults.
- Ops-friendly overrides.
- Clear precedence model.

Cons:
- Requires explicit merge/validation logic.

### Rationale
Layering prevents drift between local and CI while keeping secret injection safe.
`secrecy` reduces accidental leakage via debug formatting.

### Guardrails
- Config load and validation happen before starting network loops.
- Required secrets for enabled features must be validated at startup.
- Effective config print command must redact secrets by default.

### Verification
- Unit tests for precedence and validation.
- CLI command `quotey config` validates redaction policy.

### References
- [config crate docs](https://docs.rs/config/latest/config/)
- [envy crate docs](https://docs.rs/envy/latest/envy/)
- [secrecy crate docs](https://docs.rs/secrecy/latest/secrecy/)

---

## 10. ADR-0004: SQLite Connection, Pooling, and PRAGMA Policy

### Status
Proposed (high confidence)

### Context
SQLite is local-first source of truth.
CPQ workloads include frequent reads and periodic writes with strict integrity and traceability.

### Decision
Use SQLx SQLite pool with explicit connect options:
- Enable foreign keys.
- Enable WAL journaling for concurrency profile.
- Set busy timeout.
- Keep pool size conservative due single-writer semantics.

### Options Considered

#### Option A: Default SQLite settings
Pros:
- Zero setup.

Cons:
- Foreign keys may not be enforced consistently unless enabled.
- Concurrency behavior less predictable.

#### Option B: Explicit PRAGMA/connect policy (selected)
Pros:
- Deterministic behavior.
- Better read/write concurrency in WAL.
- Controlled lock handling.

Cons:
- Must document policy and startup checks.

### Rationale
Official SQLite docs emphasize WAL advantages for reader/writer concurrency and busy timeout behavior.
Foreign key enforcement must be explicit.

### Guardrails
- App start logs current pragma state (without secrets).
- Health check includes DB capability sanity check.
- No implicit fallback to relaxed integrity mode.

### Verification
- Integration tests verify foreign key enforcement.
- Load test with concurrent readers during writes.
- Migration checks ensure pragma assumptions documented.

### References
- [SQLite WAL](https://www.sqlite.org/wal.html)
- [SQLite foreign keys](https://www.sqlite.org/foreignkeys.html)
- [SQLite PRAGMA reference](https://www.sqlite.org/pragma.html)
- [SQLx SqliteConnectOptions](https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html)

---

## 11. ADR-0005: Migration Strategy and Schema Compatibility

### Status
Proposed (high confidence)

### Context
Schema evolution must be reproducible across developer machines and CI.
Foundation requires dependable baseline schema and reversible migration strategy.

### Decision
Use SQLx migration system with migrations committed to repo and executed by CLI command.
Embed/locate migrations deterministically from runtime binary.

### Options Considered

#### Option A: Hand-rolled migration runner
Pros:
- Full control.

Cons:
- Reinvents a solved problem.
- Higher bug risk.

#### Option B: SQLx migration support (selected)
Pros:
- Standardized and widely used.
- Works with SQLx ecosystem and CLI.
- Simpler operational docs.

Cons:
- Need discipline for migration review.

### Rationale
Use proven mechanism already aligned with chosen DB stack.
Compile-time query checks and migration tooling belong to same operational workflow.

### Guardrails
- Every migration needs forward intent and rollback strategy notes.
- Breaking schema changes require data migration plan.
- Migration command is idempotent.

### Verification
- `quotey migrate` on clean DB.
- `quotey migrate` on already migrated DB.
- Integration test for rollback path in non-prod fixture.

### References
- [SQLx migrate macro](https://docs.rs/sqlx/latest/sqlx/macro.migrate.html)
- [sqlx-cli docs](https://docs.rs/crate/sqlx-cli/latest)

---

## 12. ADR-0006: Canonical Domain Model and Invariants

### Status
Proposed (medium confidence)

### Context
CPQ logic fails when core entities are under-specified.
Natural language front-end increases pressure on clear canonical model.

### Decision
Define strict canonical domain entities in `core` with explicit invariants and typed identifiers.

Core aggregates for foundation:
- Quote
- QuoteLine
- Product
- Rule
- Policy
- ApprovalRequest
- AuditEvent

### Candidate Invariants (initial)
- A quote has one owning account identifier.
- A quote line references a valid product revision.
- Monetary values are non-floating deterministic decimals.
- Approval-required state cannot transition to final issue state without approval artifact.
- Every price mutation emits a trace step.

### Options Considered

#### Option A: Lightweight structs, validate only in services
Pros:
- Faster initial coding.

Cons:
- Invariants become scattered.
- Invalid states easier to construct.

#### Option B: Invariant-centric domain model (selected)
Pros:
- Invalid states prevented early.
- Cleaner deterministic reasoning.

Cons:
- Slightly more design upfront.

### Guardrails
- No public constructors that bypass invariant checks.
- Domain errors are explicit and user-safe.

### Verification
- Property and table-driven tests for invariants.
- Mutation tests for illegal transition attempts.

### References
- [Rust error handling patterns with thiserror](https://docs.rs/thiserror/latest/thiserror/)
- [serde derive docs](https://docs.rs/serde/latest/serde/)

---

## 13. ADR-0007: Deterministic Flow Engine Semantics

### Status
Proposed (medium confidence)

### Context
Conversation-driven interaction can create ambiguous progression.
Need deterministic flow state machine controlling required fields and legal transitions.

### Decision
Implement explicit state machine in `core::flow` where each transition:
- validates required context,
- records causal event,
- returns next legal state or typed failure.

LLM output only proposes intents/slots.
Flow engine confirms and transitions deterministically.

### Options Considered

#### Option A: Implicit flow via agent prompts and tool calls
Pros:
- Fast prototyping.

Cons:
- Hard to audit.
- Easy to drift across branches.

#### Option B: Explicit deterministic state machine (selected)
Pros:
- Auditable and testable.
- Clear legal transitions.

Cons:
- More explicit modeling work.

### Guardrails
- No side effects in transition function.
- Transition outputs include required follow-up actions.
- Every transition writes audit metadata.

### Verification
- State transition table tests.
- Replay test: same input event sequence yields identical final state.

### References
- [Tokio runtime docs for deterministic task orchestration primitives](https://docs.rs/tokio/latest/tokio/)
- [Rust serde model serialization for event replay](https://docs.rs/serde/latest/serde/)

---

## 14. ADR-0008: Pricing Precision and Representation

### Status
Proposed (high confidence)

### Context
CPQ requires contract-grade monetary determinism.
Binary floating point is unsafe for financial totals.

### Decision
Use `rust_decimal::Decimal` for price arithmetic.
Persist monetary values in SQLite in a deterministic representation agreed by schema contract.
Prefer integer minor units or normalized text decimal depending on query needs.

### Options Considered

#### Option A: f64/f32 floats
Pros:
- Fast and familiar.

Cons:
- Rounding anomalies.
- Non-deterministic display/math mismatch risks.

#### Option B: Decimal (selected)
Pros:
- Financial correctness.
- Better deterministic traces.

Cons:
- Slightly higher computational overhead.

### Rationale
`rust_decimal` avoids binary floating-point artifacts.
SQLite docs explicitly caution about floating-point precision behavior.

### Guardrails
- No float types in core pricing domain.
- Rounding rules explicit per currency/unit policy.
- Price trace captures pre/post rounding steps.

### Verification
- Golden test vectors for discounts, tiers, bundles.
- Cross-check totals against reference calculations.

### References
- [rust_decimal crate docs](https://docs.rs/rust_decimal/latest/rust_decimal/)
- [SQLite floating point notes](https://sqlite.org/floatingpoint.html)

---

## 15. ADR-0009: Rules and Policy Evaluation Ordering

### Status
Proposed (medium confidence)

### Context
Pricing/policy outcomes differ if rule ordering is implicit.
Need stable, explainable evaluation sequence.

### Decision
Adopt explicit multi-stage policy pipeline:
1. Eligibility and hard constraints.
2. Base price selection.
3. Contextual adjustments (volume, term, segment).
4. Discount policy enforcement.
5. Approval threshold checks.
6. Final trace compilation.

### Options Considered

#### Option A: Priority integer ordering only
Pros:
- Simple to store.

Cons:
- Easy to misconfigure semantic stage interactions.

#### Option B: Stage + priority ordering (selected)
Pros:
- More legible semantics.
- Better debugging.

Cons:
- Slightly richer schema.

### Guardrails
- Stage skipping is explicit and logged.
- Conflicting rule match behavior is deterministic (first win, max win, additive per rule family).

### Verification
- Rule engine tests with intentionally conflicting inputs.
- Trace output inspected for stage-level explainability.

### References
- [SQLx query macros for compile-time checked retrieval](https://docs.rs/sqlx/latest/sqlx/)
- [SQLite query semantics reference](https://www.sqlite.org/lang_select.html)

---

## 16. ADR-0010: Slack Socket Mode Ingress Boundary

### Status
Proposed (high confidence)

### Context
Slack is primary user interface.
Socket Mode event ingress reliability and ack strategy define user-perceived responsiveness.

### Decision
Use Socket Mode listener with minimal ingress handler responsibilities:
- Parse and validate event payload.
- Ack quickly.
- Enqueue deterministic command handling path.

### Options Considered

#### Option A: Handle full workflow inline in event callback
Pros:
- Fewer moving parts initially.

Cons:
- Higher timeout/retry risk.
- Harder to isolate failures.

#### Option B: Ack-fast + enqueue processing (selected)
Pros:
- Better resilience.
- Cleaner retry/idempotency boundary.

Cons:
- Requires command queue abstraction.

### Rationale
Slack docs emphasize timely acknowledgment patterns and request handling constraints.
Socket Mode is officially supported for apps without public request URL management.

### Guardrails
- Ingress handler must be non-blocking and short.
- Idempotency key extracted from Slack envelope/event metadata.

### Verification
- Simulate delayed downstream processing and confirm ack latency remains bounded.
- Replay duplicate envelope IDs to validate dedupe behavior.

### References
- [Slack Socket Mode](https://api.slack.com/apis/connections/socket)
- [Slack events API overview](https://docs.slack.dev/apis/events-api/)
- [slack-morphism crate](https://docs.rs/slack-morphism/latest/slack_morphism/)

---

## 17. ADR-0011: Slack Command Grammar and Thread Lifecycle

### Status
Proposed (medium confidence)

### Context
Natural language is flexible; operations need predictable command grammar and thread lifecycle policy.

### Decision
Define hybrid interaction model:
- Slash commands initiate deterministic flows.
- Thread replies provide conversational progression.
- Structured Slack components (buttons/select/modals) resolve ambiguity.

Lifecycle states per thread:
- Initialized
- GatheringContext
- DraftConfigured
- Priced
- ApprovalPending
- Approved
- Rejected
- Finalized
- Expired

### Options Considered

#### Option A: Pure natural language in thread
Pros:
- Minimal UX friction.

Cons:
- Ambiguous state transitions.
- Harder to validate critical fields.

#### Option B: Hybrid grammar + components (selected)
Pros:
- Better determinism.
- Better UX under uncertainty.

Cons:
- More UX design artifacts.

### Guardrails
- Critical transitions require explicit confirmation action.
- Free-form text never directly mutates final financial outputs without deterministic revalidation.

### Verification
- Thread simulation tests across lifecycle branches.
- User acceptance tests for ambiguity resolution prompts.

### References
- [Slack slash commands](https://docs.slack.dev/interactivity/implementing-slash-commands)
- [Slack Block Kit overview](https://api.slack.com/block-kit)

---

## 18. ADR-0012: Idempotency and Retry Strategy

### Status
Proposed (medium confidence)

### Context
Event delivery retries and transient failures can produce duplicate processing or inconsistent state.

### Decision
Implement idempotency contract at command execution boundary:
- Generate deterministic operation key from event metadata + command semantic key.
- Persist operation key and completion status.
- Treat retries as safe replays.

### Options Considered

#### Option A: Best-effort duplicate suppression in memory
Pros:
- Quick implementation.

Cons:
- Process restarts lose memory.
- Multi-worker future incompatible.

#### Option B: Durable idempotency ledger in SQLite (selected)
Pros:
- Survives restarts.
- Auditable and deterministic.

Cons:
- Additional schema + cleanup strategy.

### Guardrails
- Command handlers must be idempotent by design.
- Side effects happen after state reservation when possible.

### Verification
- Test duplicate event delivery with injected crash between steps.
- Ensure eventual single committed business outcome.

### References
- [Slack retries and rate handling context](https://api.slack.com/apis/rate-limits)
- [SQLite transactional behavior](https://www.sqlite.org/lang_transaction.html)

---

## 19. ADR-0013: Approval Governance and Delegation Model

### Status
Proposed (medium confidence)

### Context
Discount exceptions and high-risk deals require clear approval routing with auditability.

### Decision
Policy-driven approval matrix persisted in SQLite:
- Threshold bands by discount, margin, or risk category.
- Resolver chain with fallback delegates.
- Explicit timeout/escalation policy.

### Options Considered

#### Option A: Hardcoded approval logic
Pros:
- Fast initial implementation.

Cons:
- Operationally brittle.
- Requires deploy for rule changes.

#### Option B: DB-managed approval matrix (selected)
Pros:
- Runtime configurable.
- Better audit history.

Cons:
- Requires admin tooling/validation.

### Guardrails
- Every approval decision captures rationale and actor identity.
- Auto-approval only on explicitly configured low-risk bands.

### Verification
- Matrix simulation tests across threshold boundaries.
- Escalation path tests for unavailable approvers.

### References
- [SQLite data model references](https://www.sqlite.org/lang_createtable.html)
- [Project planning requirement: deterministic policy engine](./PROJECT.md)

---

## 20. ADR-0014: CRM Adapter Boundary via Composio REST

### Status
Trial (medium confidence)

### Context
Need practical path for CRM integration while preserving local-first core and deterministic boundaries.

### Decision
Define adapter trait in core boundary; implement initial REST adapter for Composio, plus fixture-based offline adapter for demos and tests.

### Options Considered

#### Option A: Direct vendor-specific CRM integration first
Pros:
- Potentially optimized for one CRM.

Cons:
- High coupling and early lock-in.

#### Option B: Adapter trait + Composio REST + fixture adapter (selected)
Pros:
- Swappable integration boundary.
- Supports offline development.

Cons:
- Adds abstraction layer and mapping complexity.

### Guardrails
- Core domain does not depend on Composio types.
- Mapping failures are explicit and observable.
- Adapter timeouts/retries are bounded.

### Verification
- Contract tests against fixture adapter.
- Integration smoke tests for Composio API path behind feature flag.

### Revisit Trigger
- If Composio API coverage is insufficient for required CRM actions.

### References
- [Composio API reference](https://docs.composio.dev/api-reference/v-3)
- [Composio integrations overview](https://docs.composio.dev/home/integrations)

---

## 21. ADR-0015: Observability, Trace Correlation, and Logging Shape

### Status
Proposed (high confidence)

### Context
Without coherent observability, deterministic claims cannot be operationally proven.

### Decision
Use `tracing` with structured fields and correlation IDs spanning:
- Slack ingress event.
- Flow transition.
- Pricing evaluation.
- Policy checks.
- Approval action.
- Outbound Slack response.

### Options Considered

#### Option A: ad-hoc log strings
Pros:
- Low startup effort.

Cons:
- Poor machine queryability.
- Weak audit/correlation support.

#### Option B: structured tracing spans (selected)
Pros:
- Better debuggability and metrics extraction.
- Consistent context propagation.

Cons:
- Requires log schema discipline.

### Guardrails
- PII and secrets must never be logged.
- Every business operation has stable `trace_id` and `operation_id`.

### Verification
- Integration tests assert presence of required fields in logs.
- Troubleshooting drill reconstructs one quote end-to-end from logs + audit rows.

### References
- [tracing crate docs](https://docs.rs/tracing/latest/tracing/)
- [tracing-subscriber EnvFilter](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)

---

## 22. ADR-0016: Security Baseline and Secret Management

### Status
Proposed (high confidence)

### Context
Slack tokens, LLM API keys, and CRM tokens are sensitive and high-impact.

### Decision
Adopt minimal security baseline for foundation:
- Secret values wrapped in `SecretString`.
- No secret values in logs or panic paths.
- Config validation rejects placeholder production tokens.
- Local SQLite file permissions documented and checked by `doctor` command.

### Options Considered

#### Option A: Basic env vars, no typed secret wrappers
Pros:
- Simpler code.

Cons:
- Higher accidental leak risk.

#### Option B: Typed secret handling + operational checks (selected)
Pros:
- Stronger default safety.
- Better operational posture.

Cons:
- Slight implementation overhead.

### Guardrails
- Redaction-by-default for diagnostics.
- Any serialization of config structures excludes secret fields.

### Verification
- Unit tests for redaction behavior.
- `doctor` command checks for insecure DB file permissions.

### References
- [secrecy crate docs](https://docs.rs/secrecy/latest/secrecy/)
- [SQLite security considerations](https://www.sqlite.org/security.html)

---

## 23. ADR-0017: CLI and Operator Contract

### Status
Proposed (high confidence)

### Context
Operational control in v1 is CLI-first.
Need stable command contract for migrations, health checks, config introspection, and demo setup.

### Decision
Define core CLI commands:
- `quotey start`
- `quotey migrate`
- `quotey seed`
- `quotey doctor`
- `quotey config`

Use `clap` derive for typed parsing and generated help.

### Options Considered

#### Option A: Manual arg parsing
Pros:
- Fewer dependencies.

Cons:
- Higher bug surface.
- Worse UX/docs.

#### Option B: clap derive (selected)
Pros:
- Mature parser.
- Better maintainability.

Cons:
- Slight compile-time overhead.

### Guardrails
- Command outputs must be machine-readable when needed (`--json` where relevant).
- Errors must be actionable and concise.

### Verification
- Snapshot tests for help output and command validation.
- CLI smoke tests in CI.

### References
- [clap derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html)
- [clap command docs](https://docs.rs/clap/latest/clap/)

---

## 24. ADR-0018: Health, Readiness, and Diagnostics Semantics

### Status
Proposed (high confidence)

### Context
Need clear distinction between process liveness and operational readiness.

### Decision
Expose minimal health endpoint and richer CLI diagnostics:
- `/health` for process liveness.
- `quotey doctor` for dependency and config checks.

### Options Considered

#### Option A: Health endpoint only
Pros:
- Simpler.

Cons:
- Weak diagnostics depth.

#### Option B: endpoint + doctor command (selected)
Pros:
- Better operator experience.
- Keeps runtime endpoint simple.

Cons:
- Two surfaces to maintain.

### Guardrails
- `/health` must stay fast and side-effect free.
- `doctor` should include DB migration status and config sanity checks.

### Verification
- Integration test for health endpoint.
- Doctor command test matrix (valid config, missing token, migration mismatch).

### References
- [axum docs](https://docs.rs/axum/latest/axum/)
- [Tokio runtime docs](https://docs.rs/tokio/latest/tokio/)

---

## 25. ADR-0019: Verification Architecture and Quality Gates

### Status
Proposed (high confidence)

### Context
Project requires deterministic confidence and multi-agent consistency.
Need clear minimal gate stack for every change set.

### Decision
Quality gates by scope:

For planning-only changes:
- `br lint` / graph sanity.
- Documentation consistency checks.

For code changes:
- `cargo fmt -- --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`
- targeted integration tests
- optional `cargo nextest run` for speed in larger suites
- `cargo deny check` in CI lane

### Options Considered

#### Option A: minimal local tests only
Pros:
- Fast.

Cons:
- Regression risk.

#### Option B: layered quality gates (selected)
Pros:
- Better stability.
- Reproducible baseline.

Cons:
- Longer CI runtime if unmanaged.

### Guardrails
- No warning debt during foundation.
- New modules require at least smoke tests.

### Verification
- CI pipeline green on required gate set.
- Gate exceptions require bead note + explicit time-box.

### References
- [cargo-nextest docs](https://nexte.st/docs/)
- [cargo-deny docs](https://embarkstudios.github.io/cargo-deny/)

---

## 26. ADR-0020: Decision Freeze and Change Control Protocol

### Status
Proposed (medium confidence)

### Context
Multiple agents can unintentionally fork architecture if change control is implicit.

### Decision
Adopt lightweight decision freeze protocol:
- Foundation ADR set (`0001-0020`) moves to `Accepted` before feature-scale implementation.
- Post-freeze changes require:
  - explicit superseding ADR,
  - bead link,
  - migration/compatibility note,
  - rollback statement.

### Options Considered

#### Option A: informal discussion-driven updates
Pros:
- Flexible.

Cons:
- Hard to audit.
- High coordination burden.

#### Option B: explicit ADR supersession protocol (selected)
Pros:
- Strong traceability.
- Lower coordination confusion.

Cons:
- Slight process overhead.

### Guardrails
- No silent architectural pivots in implementation PRs.
- ADR status updates must happen in same commit stream as major design changes.

### Verification
- Periodic audit of accepted decisions vs code reality.
- Bead graph references include ADR IDs.

### References
- [Project planning workflow requirements](./PROJECT.md)
- [beads/br workflow conventions in AGENTS](../AGENTS.md)

---

## 27. Cross-Decision Risk Register

| Risk ID | Description | Probability | Impact | Mitigation | Owner Bead |
|---|---|---|---|---|---|
| R-01 | Over-coupling between core and adapters | Medium | High | Enforce crate boundaries + dependency audits | `bd-3d8.1` |
| R-02 | Slack retries causing duplicate state transitions | Medium | High | Durable idempotency ledger | `bd-3d8.11.5` |
| R-03 | Pricing nondeterminism from numeric representation drift | Low | High | Decimal-only core + trace tests | `bd-3d8.4` |
| R-04 | Migration incompatibility across environments | Medium | High | strict migration process + smoke tests | `bd-3d8.3` |
| R-05 | Secret leakage in logs | Medium | High | redaction tests + secrecy wrappers | `bd-3d8.11.9` |
| R-06 | Approval policy ambiguity causing routing errors | Medium | Medium | stage-ordered policy model | `bd-3d8.11.6` |
| R-07 | Slow or flaky startup due config validation gaps | Medium | Medium | fail-fast validation + doctor command | `bd-3d8.2`, `bd-3d8.9` |
| R-08 | Agent-driven architectural drift | High | High | decision freeze + bead traceability | `bd-3d8.11.10` |

---

## 28. Rejected Patterns and Why

### 28.1 “Let the LLM decide final prices”
Rejected because it violates deterministic audit principle and introduces contractual risk.

### 28.2 “Keep everything in one crate until later”
Rejected due early coupling debt and multi-agent merge conflict risk.

### 28.3 “Use floating point for money now, fix later”
Rejected due high cost of retrofitting financial determinism.

### 28.4 “Do full event processing inside Slack ingress callback”
Rejected due ack latency and retry amplification risk.

### 28.5 “Hardcode policy rules in Rust for speed”
Rejected because CPQ policy changes require operational agility without redeploy for every update.

---

## 29. Foundation Execution Sequence (Concrete)

### Phase A: Operating model and boundary freeze
- Accept ADR-0001, ADR-0002, ADR-0003, ADR-0015, ADR-0017.
- Output: stable crate skeleton + startup contract + trace policy.

### Phase B: Data and deterministic core
- Accept ADR-0004, ADR-0005, ADR-0006, ADR-0008, ADR-0009.
- Output: schema + invariants + deterministic pricing/rule pipeline.

### Phase C: Interaction reliability
- Accept ADR-0010, ADR-0011, ADR-0012, ADR-0018.
- Output: robust Slack ingress, lifecycle handling, idempotent command path.

### Phase D: Governance and external integration
- Accept ADR-0013, ADR-0014, ADR-0016, ADR-0019, ADR-0020.
- Output: approval governance, adapter boundary, security baseline, gate discipline.

---

## 30. Verification Matrix by ADR

| ADR | Unit Tests | Integration Tests | Operational Checks | Artifacts |
|---|---|---|---|---|
| 0001 | dependency boundary assertions | workspace build smoke | compile-time graph check | crate graph doc |
| 0002 | cancellation behavior | graceful shutdown test | shutdown latency metric | runtime policy note |
| 0003 | config precedence tests | startup fail-fast tests | redacted config dump | config contract doc |
| 0004 | DB option parsing tests | pragma behavior tests | connection diagnostics | DB policy checklist |
| 0005 | migration ordering tests | fresh+existing DB migration | migration status command | migration runbook |
| 0006 | invariant constructor tests | repository roundtrip tests | invalid-state rejection counts | domain contract notes |
| 0007 | transition legality tests | replay determinism tests | transition audit completeness | state machine map |
| 0008 | rounding vector tests | cross-stage total consistency | decimal/precision check | pricing trace examples |
| 0009 | stage ordering tests | conflicting-rule scenario tests | rule hit distribution | rule pipeline schema |
| 0010 | parser/ack unit tests | ingress load simulation | ack latency SLI | ingress contract doc |
| 0011 | grammar parse tests | thread lifecycle scenario tests | unresolved-slot rate | UX grammar guide |
| 0012 | idempotency key tests | duplicate delivery simulation | dedupe hit ratio | retry contract |
| 0013 | threshold matrix tests | escalation workflow tests | approval SLA metrics | approval policy schema |
| 0014 | mapping contract tests | adapter smoke tests | API failure rate | adapter compatibility sheet |
| 0015 | span field presence tests | end-to-end trace reconstruction | trace completeness KPI | observability guide |
| 0016 | redaction tests | secret-loading fault tests | security lint checks | security baseline doc |
| 0017 | command parse tests | CLI smoke tests | operator error-rate trend | CLI contract doc |
| 0018 | endpoint handler tests | health+doctor integration | startup readiness timing | ops runbook |
| 0019 | gate script tests | CI gate pipeline | gate pass rate | quality checklist |
| 0020 | decision workflow tests | ADR supersession drill | architecture drift audit cadence | change-control protocol |

---

## 31. Architecture KPIs (Initial)

### Determinism KPIs
- Replayed event sequence yields identical quote total and state (target: 100%).
- Pricing trace completeness for finalized quotes (target: 100%).

### Reliability KPIs
- Slack ingress ack p95 latency (target: under configured threshold).
- Duplicate delivery causing duplicate business side effects (target: 0).

### Quality KPIs
- Clippy warning count on protected branch (target: 0).
- Migration failures in CI/provisioning flows (target: 0).

### Security KPIs
- Secret leakage incidents in logs (target: 0).
- Doctor command secret/config misconfiguration findings trend (target: downward).

### Operability KPIs
- Mean time to diagnose quote failure from logs + audit (target: under defined SLA).
- Onboarding time to run local scaffold successfully (target: shrinking trend).

---

## 32. Open Questions (Intentionally Unresolved)

1. Should money persist as integer minor units everywhere, or decimal text with strict parser?
2. Should rule evaluation support explainable conflict strategies per rule family from day one?
3. How aggressively should we normalize Slack conversational grammar vs preserving free-form flexibility?
4. Do we need a lightweight durable queue abstraction immediately, or can SQLite transaction-bound dispatch suffice in foundation?
5. What is the minimal acceptable CRM sync contract for first real user validation?
6. Should audit events be append-only immutable rows with correction events vs mutable flags?
7. What exact SLOs should gate production-like readiness for the alpha milestone?

Each open question should become explicit bead tasks if not resolved during current research wave.

---

## 33. Multi-Agent Coordination Rules for Architecture Work

1. No implementation-level interface changes in core crates without ADR reference.
2. Each architectural change PR/commit references one ADR ID and one bead ID.
3. Conflicting proposals are resolved by superseding ADR, not ad-hoc code divergence.
4. All final ADR states are mirrored in bead comments for discoverability.

Suggested commit-note pattern:
`adr: accept ADR-0004 sqlite pragma policy (bd-3d8.3)`

---

## 34. Suggested File Layout for ADR Artifacts

To keep planning coherent without excessive sprawl:

- `.planning/ARCHITECTURE_DECISION_RESEARCH.md` (this dossier)
- `.planning/adr/ADR-0001-...md` (optional extraction once accepted)
- `.planning/adr/INDEX.md` (status + supersession map)

Extraction rule:
- Keep decision drafts in this dossier.
- Split into separate ADR files only after decision reaches `Accepted`.

---

## 35. Immediate Next Actions (Execution-Oriented)

1. Finalize ADR-0001 through ADR-0005 and mark accepted.
2. Create canonical domain invariant checklist for ADR-0006.
3. Prototype idempotency ledger schema for ADR-0012.
4. Draft Slack grammar and lifecycle examples for ADR-0011.
5. Define KPI instrumentation fields required by ADR-0015.
6. Run decision freeze prep for ADR-0020 with explicit unresolved list.

---

## 36. Deliverables Checkpoint (What This Research Wave Produces)

- A complete ADR protocol and template.
- A foundation decision set (20 ADRs) with rationale and guardrails.
- A cross-decision dependency narrative and verification matrix.
- A risk register and explicit unresolved question backlog.
- A direct mapping from architecture decisions to current bead graph.

This is sufficient to support disciplined implementation without waiting for full production hardening research.

---

## 37. Detailed Research Notes by Topic

### 37.1 Cargo Workspace and Resolver Behavior

Facts captured:
- Workspaces share lockfile and target dir.
- Root package can centralize metadata and dependency strategy.
- Resolver behavior is explicitly configurable and should be pinned in root.

Implications for Quotey:
- Keep workspace root intentionally thin and explicit.
- Standardize profile/lint settings centrally.

Potential pitfalls:
- Implicit dependency bleed if crate boundaries not enforced by review.

### 37.2 Tokio Shutdown and Cancellation

Facts captured:
- Tokio provides signal and cancellation primitives.
- Graceful shutdown is a documented multi-step pattern.

Implications:
- Shutdown model should be designed at start, not retrofitted.
- Long-running loops must cooperate with cancellation.

Potential pitfalls:
- Spawned tasks without join tracking can stall shutdown.

### 37.3 SQLx + SQLite Operational Realities

Facts captured:
- SQLx provides typed query and migration support.
- SQLite WAL improves reader/writer concurrency profile.
- Foreign key behavior needs explicit enabling discipline.

Implications:
- Database contract belongs in architecture layer, not only implementation notes.
- App startup should validate expected DB behavior.

Potential pitfalls:
- Mismatch between local dev sqlite settings and CI/prod-like settings.

### 37.4 Monetary Determinism

Facts captured:
- Decimal crates avoid binary floating-point artifacts.
- SQLite floating point caveats reinforce avoiding float for money.

Implications:
- Money representation policy must be explicit in schema and domain.

Potential pitfalls:
- Mixed representation between DB and domain causing rounding drift.

### 37.5 Slack Interaction Model

Facts captured:
- Socket Mode is official pattern for apps without public HTTP endpoint requirement.
- Block Kit and slash commands provide structured interaction scaffolding.
- Rate and retry constraints require resilient handler design.

Implications:
- Fast-ack boundary and idempotent backend handling are foundational.

Potential pitfalls:
- Too much synchronous work in callback path causing retries.

### 37.6 Observability and Logging

Facts captured:
- `tracing` supports span-based structured context.
- Subscriber filtering supports runtime control.

Implications:
- Correlation IDs should be first-class fields from ingress to persistence.

Potential pitfalls:
- Inconsistent field naming undermines queryability.

### 37.7 CLI and Operator Experience

Facts captured:
- `clap` derive supports robust command modeling.

Implications:
- Operator contract should be typed and tested like production API.

Potential pitfalls:
- Ad-hoc command semantics drift without snapshot tests.

### 37.8 Security Defaults

Facts captured:
- Secret wrappers reduce accidental exposure through debug formatting.
- SQLite file-level posture matters in local-first deployments.

Implications:
- Include security checks in `doctor` and startup validation.

Potential pitfalls:
- Assuming local deployment implies low risk.

### 37.9 External Integration Boundary

Facts captured:
- Composio publishes API docs and broad integration model.

Implications:
- Adapter abstraction is critical to keep core deterministic and portable.

Potential pitfalls:
- Leaking vendor-specific semantics into domain model.

### 37.10 Quality Gates and Supply Chain

Facts captured:
- `nextest` improves Rust test runtime ergonomics.
- `cargo-deny` can enforce dependency policy checks.

Implications:
- Define gate profiles by change scope to avoid over/under testing.

Potential pitfalls:
- Turning quality gates into optional suggestions.

---

## 38. Architectural Decision Review Checklist

Use this checklist during decision review meetings:

1. Is the problem statement concrete and current?
2. Are options materially distinct?
3. Is there a clearly chosen option?
4. Are rejection reasons documented for non-selected options?
5. Are deterministic and safety principles preserved?
6. Is implementation ownership mapped to active beads?
7. Are verification criteria explicit and testable?
8. Are observability implications defined?
9. Are security implications defined?
10. Are revisit triggers measurable and objective?

Only move `Proposed -> Accepted` when all checklist items pass.

---

## 39. Suggested Supersession Format

When replacing an accepted decision, append this block to both old and new records:

```md
Superseded by: ADR-XXXX on YYYY-MM-DD
Reason: <short reason>
Migration impact: <none | low | medium | high>
Rollback path: <describe>
```

This keeps architectural history auditable.

---

## 40. Source Index (Primary References)

### Rust and Cargo
- https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html
- https://doc.rust-lang.org/cargo/reference/workspaces.html
- https://doc.rust-lang.org/cargo/reference/resolver.html

### Tokio Runtime and Shutdown
- https://tokio.rs/tokio/topics/shutdown
- https://docs.rs/tokio/latest/tokio/signal/fn.ctrl_c.html
- https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html

### SQLx and SQLite
- https://docs.rs/sqlx/latest/sqlx/
- https://docs.rs/sqlx/latest/sqlx/macro.migrate.html
- https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html
- https://www.sqlite.org/wal.html
- https://www.sqlite.org/foreignkeys.html
- https://www.sqlite.org/pragma.html
- https://sqlite.org/floatingpoint.html

### Slack Platform
- https://api.slack.com/apis/connections/socket
- https://docs.slack.dev/apis/events-api/
- https://docs.slack.dev/interactivity/implementing-slash-commands
- https://api.slack.com/block-kit
- https://api.slack.com/apis/rate-limits
- https://docs.rs/slack-morphism/latest/slack_morphism/

### Config, Secrets, CLI, Observability
- https://docs.rs/config/latest/config/
- https://docs.rs/envy/latest/envy/
- https://docs.rs/secrecy/latest/secrecy/
- https://docs.rs/clap/latest/clap/
- https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html
- https://docs.rs/tracing/latest/tracing/
- https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html
- https://docs.rs/thiserror/latest/thiserror/
- https://docs.rs/rust_decimal/latest/rust_decimal/

### Integration and QA Tooling
- https://docs.composio.dev/api-reference/v-3
- https://docs.composio.dev/home/integrations
- https://nexte.st/docs/
- https://embarkstudios.github.io/cargo-deny/

---

## 41. Draft ADR Status Recommendations for Immediate Adoption

Recommended to accept now:
- ADR-0001
- ADR-0002
- ADR-0003
- ADR-0004
- ADR-0005
- ADR-0008
- ADR-0010
- ADR-0015
- ADR-0016
- ADR-0017
- ADR-0018
- ADR-0019

Keep proposed/trial until dedicated task passes:
- ADR-0006
- ADR-0007
- ADR-0009
- ADR-0011
- ADR-0012
- ADR-0013
- ADR-0014
- ADR-0020

---

## 42. “Future Self” Notes

1. Preserve deterministic authority boundaries at all costs.
2. Integration convenience is never allowed to leak into core invariants.
3. Logging without correlation IDs is noise, not observability.
4. If a decision is contentious, force explicit option analysis, not hallway consensus.
5. If architecture and bead graph diverge, treat it as a critical coordination bug.
6. Keep documentation and implementation in the same workstream to avoid stale plans.

---

## 43. Suggested Follow-on Documentation Tasks

1. Split accepted ADRs into dedicated files under `.planning/adr/`.
2. Add a machine-readable ADR index (`.json`) for tooling checks.
3. Add architecture drift checks in CI (validate ADR IDs in commits touching core boundaries).
4. Add a "decision impact" section to PR template (when PR templates are introduced).

---

## 44. Completion Statement

This dossier establishes an execution-grade architectural decision framework for Quotey foundation.
It is intentionally explicit so multiple agents can implement in parallel without breaking deterministic behavior, auditability, or operational reliability.

Next step is not more abstract planning.
Next step is controlled acceptance of ADRs and mapping each accepted decision into implementation tasks already present in the bead graph.


---

## 45. ADR Execution Worksheets (Implementation-Ready)

The following worksheets convert each ADR into an executable design contract.

Each worksheet includes:
- Inputs
- Outputs
- Preconditions
- Postconditions
- Invariants
- Failure modes
- Telemetry expectations
- Ownership and dependency hints

### 45.1 Worksheet for ADR-0001 (Workspace and Crate Topology)

**Inputs:**
- Existing repository structure.
- `.planning/PROJECT.md` module target layout.

**Outputs:**
- Workspace `Cargo.toml`.
- Crate manifests and skeleton module trees.

**Preconditions:**
- Toolchain baseline from `bd-3d8.12` complete.

**Postconditions:**
- `cargo build --workspace` succeeds.
- No prohibited dependency edges.

**Invariants:**
- `core` crate remains infrastructure-agnostic.
- Adapter crates only depend inward to `core`.

**Failure Modes:**
- Hidden cyclical crate dependencies.
- Runtime crates leaking into domain crate.

**Telemetry:**
- Build duration trend by crate.
- Dependency graph complexity trend.

**Owner Beads:**
- `bd-3d8.1`, `bd-3d8.1.1` to `.1.7`.

### 45.2 Worksheet for ADR-0002 (Runtime and Process Model)

**Inputs:**
- Runtime crate selection.
- Service loop responsibilities.

**Outputs:**
- Process bootstrap with controlled task lifecycle.
- Shutdown path contract.

**Preconditions:**
- Crate boundaries established.

**Postconditions:**
- Process exits cleanly under signal.

**Invariants:**
- No orphan long-running tasks.
- Cancellation path always present.

**Failure Modes:**
- Hung shutdown due non-cooperative tasks.
- Dropped in-flight audit writes.

**Telemetry:**
- Shutdown duration p50/p95.
- Task cancellation completion counts.

**Owner Beads:**
- `bd-3d8.1`, `bd-3d8.10.1`.

### 45.3 Worksheet for ADR-0003 (Config + Secrets)

**Inputs:**
- Default config file.
- Environment variables.
- CLI overrides.

**Outputs:**
- Effective typed config object.
- Redacted diagnostics representation.

**Preconditions:**
- Config schema agreed by core and adapters.

**Postconditions:**
- Fail-fast on invalid/insufficient config.

**Invariants:**
- Secret values never appear in logs.
- Precedence ordering deterministic.

**Failure Modes:**
- Inconsistent precedence behavior across commands.
- Secret leakage through debug output.

**Telemetry:**
- Startup config validation failures by category.

**Owner Beads:**
- `bd-3d8.2`, `bd-3d8.2.1` to `.2.4`.

### 45.4 Worksheet for ADR-0004 (SQLite Policy)

**Inputs:**
- DB URL.
- PRAGMA policy values.

**Outputs:**
- Stable DB connection initialization.
- Runtime checks for pragma posture.

**Preconditions:**
- Config validation complete.

**Postconditions:**
- Connection pool healthy.
- Foreign key behavior confirmed.

**Invariants:**
- Foreign keys enabled.
- Busy timeout set.
- WAL mode target applied.

**Failure Modes:**
- Unexpected lock contention.
- Integrity violations from disabled constraints.

**Telemetry:**
- Busy timeout hit count.
- DB lock wait time distribution.

**Owner Beads:**
- `bd-3d8.3`, `bd-3d8.3.1` to `.3.3`.

### 45.5 Worksheet for ADR-0005 (Migrations)

**Inputs:**
- Migration files.
- Current schema version state.

**Outputs:**
- Migrated DB to expected schema version.

**Preconditions:**
- DB connectivity confirmed.

**Postconditions:**
- Migration command idempotent.
- Schema drift detectable.

**Invariants:**
- No out-of-band schema mutation.

**Failure Modes:**
- Partially applied migrations.
- Migration order mismatch.

**Telemetry:**
- Migration duration.
- Migration failure reason taxonomy.

**Owner Beads:**
- `bd-3d8.3`, `bd-3d8.10.4`.

### 45.6 Worksheet for ADR-0006 (Domain Invariants)

**Inputs:**
- Domain object constructors.
- Rule and policy model contracts.

**Outputs:**
- Invariant-safe entity constructors.
- Typed domain errors.

**Preconditions:**
- Canonical entity list aligned.

**Postconditions:**
- Illegal states become unrepresentable or explicitly rejected.

**Invariants:**
- IDs strongly typed.
- Money strongly typed.
- Required references enforced.

**Failure Modes:**
- Direct struct mutation bypassing checks.
- Optional fields masking required lifecycle data.

**Telemetry:**
- Rejected domain mutation attempts.

**Owner Beads:**
- `bd-3d8.4`, `bd-3d8.4.1` to `.4.3`, `bd-3d8.11.2`.

### 45.7 Worksheet for ADR-0007 (Flow Engine)

**Inputs:**
- Current state.
- Validated event intent.

**Outputs:**
- Next state + effect plan.

**Preconditions:**
- Event validated and deduped.

**Postconditions:**
- Deterministic transition result.

**Invariants:**
- No hidden side effects in transition function.
- Transition legality matrix complete.

**Failure Modes:**
- Ambiguous transitions.
- Missing guardrails on risky transitions.

**Telemetry:**
- Transition rejection rates by reason.

**Owner Beads:**
- `bd-3d8.4`, `bd-3d8.11.2`.

### 45.8 Worksheet for ADR-0008 (Pricing Precision)

**Inputs:**
- Base price candidate.
- Rule outputs.
- Discount/policy data.

**Outputs:**
- Final price with trace steps.

**Preconditions:**
- Product and pricing data complete.

**Postconditions:**
- Total deterministic across replays.

**Invariants:**
- Decimal-only arithmetic in pricing path.

**Failure Modes:**
- Mixed float/decimal boundaries.
- Inconsistent rounding policy.

**Telemetry:**
- Price discrepancy detections in replay tests.

**Owner Beads:**
- `bd-3d8.4`, `bd-3d8.11.3`.

### 45.9 Worksheet for ADR-0009 (Rules Ordering)

**Inputs:**
- Rule set.
- Evaluation context.

**Outputs:**
- Ordered rule decisions.

**Preconditions:**
- Rule stages configured.

**Postconditions:**
- Reproducible rule outcomes.

**Invariants:**
- Stage ordering explicit.
- Conflict strategy explicit per stage.

**Failure Modes:**
- Hidden priority collisions.
- Non-deterministic match set ordering.

**Telemetry:**
- Rule stage execution counts.

**Owner Beads:**
- `bd-3d8.11.3`.

### 45.10 Worksheet for ADR-0010 (Slack Ingress)

**Inputs:**
- Socket Mode envelope.

**Outputs:**
- Ack + normalized command event.

**Preconditions:**
- Slack auth/context valid.

**Postconditions:**
- Ack under target latency threshold.

**Invariants:**
- No business side effects before idempotency reserve.

**Failure Modes:**
- Slow ingress callback.
- Duplicate event dispatch.

**Telemetry:**
- Ack latency distribution.
- Duplicate event rate.

**Owner Beads:**
- `bd-3d8.5`, `bd-3d8.11.4`.

### 45.11 Worksheet for ADR-0011 (Command Grammar)

**Inputs:**
- Slash command payload.
- Thread message content.

**Outputs:**
- Structured intent + slot map + confidence.

**Preconditions:**
- Command recognized and authenticated.

**Postconditions:**
- Missing required slots explicitly requested.

**Invariants:**
- Critical fields confirmed through deterministic validation.

**Failure Modes:**
- Grammar ambiguity leading to wrong quote context.
- Unbounded conversational loops.

**Telemetry:**
- Clarification prompt frequency.
- Slot extraction confidence distribution.

**Owner Beads:**
- `bd-3d8.11.4`.

### 45.12 Worksheet for ADR-0012 (Idempotency)

**Inputs:**
- Operation key.
- Command payload hash.

**Outputs:**
- New operation execution or replay response.

**Preconditions:**
- Operation key derivation deterministic.

**Postconditions:**
- Exactly-once business effect semantics.

**Invariants:**
- Same key + same semantic payload yields same business outcome.

**Failure Modes:**
- Key collisions.
- Side effects before operation lock.

**Telemetry:**
- Idempotency hit/miss ratio.

**Owner Beads:**
- `bd-3d8.11.5`.

### 45.13 Worksheet for ADR-0013 (Approval Governance)

**Inputs:**
- Deal summary.
- Discount/margin metrics.
- Approval matrix rules.

**Outputs:**
- Route decision + approver set.

**Preconditions:**
- Policy matrix active and valid.

**Postconditions:**
- Approval request traceable and reconstructable.

**Invariants:**
- Route decisions deterministic given same matrix and inputs.

**Failure Modes:**
- Missing fallback approver.
- Stale policy snapshot usage.

**Telemetry:**
- Approval routing latency.
- Escalation frequency.

**Owner Beads:**
- `bd-3d8.11.6`.

### 45.14 Worksheet for ADR-0014 (CRM Adapter)

**Inputs:**
- Domain-side sync requests.

**Outputs:**
- Adapter call outcomes mapped into domain DTOs.

**Preconditions:**
- Adapter capability declared.

**Postconditions:**
- Domain remains vendor-agnostic.

**Invariants:**
- No Composio type leakage into `core`.

**Failure Modes:**
- Partial sync without compensating record.
- Vendor schema changes causing breakages.

**Telemetry:**
- Adapter call success/failure by endpoint.

**Owner Beads:**
- `bd-3d8.11.7`.

### 45.15 Worksheet for ADR-0015 (Observability)

**Inputs:**
- Trace context from ingress.
- Domain and adapter events.

**Outputs:**
- Structured spans/logs with correlation IDs.

**Preconditions:**
- Logging policy loaded.

**Postconditions:**
- Single quote journey traceable across subsystems.

**Invariants:**
- Required fields present in each critical span.

**Failure Modes:**
- Missing correlation propagation.
- Sensitive data included in logs.

**Telemetry:**
- Trace completeness score.

**Owner Beads:**
- `bd-3d8.8`, `bd-3d8.11.8`.

### 45.16 Worksheet for ADR-0016 (Security Baseline)

**Inputs:**
- Secret-bearing config.
- Runtime environment posture.

**Outputs:**
- Security checks and redaction-safe runtime behavior.

**Preconditions:**
- Config load completed.

**Postconditions:**
- Insecure posture flags emitted before processing starts.

**Invariants:**
- No secret output in logs/errors.

**Failure Modes:**
- Token leakage by accidental formatting.
- Insecure file permissions ignored.

**Telemetry:**
- Security check pass/fail count.

**Owner Beads:**
- `bd-3d8.11.9`.

### 45.17 Worksheet for ADR-0017 (CLI Contract)

**Inputs:**
- CLI args and config context.

**Outputs:**
- Deterministic command behavior and machine-readable status.

**Preconditions:**
- Subcommand selection valid.

**Postconditions:**
- Errors actionable and non-ambiguous.

**Invariants:**
- Command semantics stable across patch releases.

**Failure Modes:**
- Silent behavior changes in command flags.

**Telemetry:**
- Command failure rates by subcommand.

**Owner Beads:**
- `bd-3d8.9`.

### 45.18 Worksheet for ADR-0018 (Health and Readiness)

**Inputs:**
- Runtime subsystem statuses.

**Outputs:**
- Liveness and readiness decisions.

**Preconditions:**
- Subsystems initialized.

**Postconditions:**
- Health endpoint and doctor command aligned.

**Invariants:**
- Health endpoint remains lightweight.

**Failure Modes:**
- False healthy while critical dependency unavailable.

**Telemetry:**
- Health check failure cause taxonomy.

**Owner Beads:**
- `bd-3d8.10.2`, `bd-3d8.9`.

### 45.19 Worksheet for ADR-0019 (Quality Gates)

**Inputs:**
- Change scope classification.

**Outputs:**
- Required gate command set.

**Preconditions:**
- Build/test tooling available.

**Postconditions:**
- Green required gates before merge/push.

**Invariants:**
- No bypass without explicit documented exception.

**Failure Modes:**
- Flaky tests reducing trust.
- Drift between documented and actual gate set.

**Telemetry:**
- Gate failure rates and flake metrics.

**Owner Beads:**
- `bd-3d8.10.3`.

### 45.20 Worksheet for ADR-0020 (Decision Freeze)

**Inputs:**
- ADR state index.
- Implementation status.

**Outputs:**
- Decision freeze report and accepted baseline list.

**Preconditions:**
- Critical proposed ADRs reviewed.

**Postconditions:**
- Architectural baseline explicit for all agents.

**Invariants:**
- Any post-freeze change requires superseding ADR.

**Failure Modes:**
- Silent architecture drift.

**Telemetry:**
- Drift findings per review cycle.

**Owner Beads:**
- `bd-3d8.11.10`, `bd-3d8.11.11`.


---

## 46. Failure Mode and Effects Analysis (FMEA) for Foundation

Scale definitions:
- Severity: 1 (low) to 10 (critical).
- Likelihood: 1 (rare) to 10 (frequent).
- Detectability: 1 (easy detect) to 10 (hard detect).
- RPN: Severity × Likelihood × Detectability.

Prioritize mitigation for RPN >= 180.

### 46.1 FMEA Entries

| ID | Failure Mode | Cause | Effect | Sev | Lik | Det | RPN | Mitigation | ADR Link |
|---|---|---|---|---:|---:|---:|---:|---|---|
| FM-001 | Core crate depends on DB crate | boundary drift | architecture coupling | 8 | 4 | 6 | 192 | dependency lint + review gate | ADR-0001 |
| FM-002 | Core crate depends on tokio | convenience import | runtime leak into domain | 7 | 5 | 6 | 210 | dependency graph check in CI | ADR-0001 |
| FM-003 | Missing workspace resolver config | implicit default assumptions | dependency resolution drift | 5 | 4 | 5 | 100 | set resolver in root manifest | ADR-0001 |
| FM-004 | Detached task without cancellation | rushed async code | shutdown hang | 8 | 5 | 6 | 240 | cancellation token enforcement | ADR-0002 |
| FM-005 | Signal handler not wired | bootstrap omission | ungraceful stop | 6 | 3 | 4 | 72 | startup self-check + integration test | ADR-0002 |
| FM-006 | Config precedence mismatch | loader bug | inconsistent env behavior | 6 | 4 | 5 | 120 | precedence unit tests | ADR-0003 |
| FM-007 | Secret logged via Debug | accidental formatting | credential leak | 10 | 4 | 7 | 280 | SecretString + linted redaction | ADR-0003/0016 |
| FM-008 | Required token missing but process starts | weak validation | runtime failures in user flow | 8 | 5 | 4 | 160 | fail-fast startup validation | ADR-0003 |
| FM-009 | Foreign keys disabled | pragma not applied | orphaned relational data | 8 | 4 | 5 | 160 | pragma assertion on connect | ADR-0004 |
| FM-010 | Busy timeout too low | default values | lock error spikes | 6 | 6 | 5 | 180 | tuned timeout + metrics | ADR-0004 |
| FM-011 | WAL not active when expected | env drift | reader/writer contention | 6 | 5 | 6 | 180 | startup WAL status check | ADR-0004 |
| FM-012 | Migration order conflict | merge collision | schema inconsistency | 8 | 4 | 6 | 192 | migration naming policy + CI check | ADR-0005 |
| FM-013 | Non-idempotent migration script | unsafe SQL | failed redeploy | 7 | 3 | 6 | 126 | migration dry-run harness | ADR-0005 |
| FM-014 | Missing rollback plan | rushed migration | long recovery time | 7 | 4 | 7 | 196 | mandatory rollback note | ADR-0005 |
| FM-015 | Domain object created in invalid state | public fields/constructors | hidden downstream failures | 8 | 5 | 6 | 240 | invariant constructors only | ADR-0006 |
| FM-016 | Invariant checks bypassed in tests and leaked to prod | test helper misuse | inconsistent behavior | 7 | 3 | 6 | 126 | test-only constructors gated | ADR-0006 |
| FM-017 | Flow transition has side effects | poor layering | replay non-determinism | 9 | 4 | 6 | 216 | pure transition contract | ADR-0007 |
| FM-018 | Illegal transition allowed | incomplete guard matrix | invalid quote lifecycle | 8 | 4 | 6 | 192 | transition legality tests | ADR-0007 |
| FM-019 | Price uses float path | mixed numeric types | rounding defects | 9 | 5 | 6 | 270 | decimal-only type policy | ADR-0008 |
| FM-020 | Rounding policy inconsistent by module | local helper divergence | total mismatch | 8 | 4 | 6 | 192 | centralized rounding helper | ADR-0008 |
| FM-021 | Rule stage ordering undefined | schema gap | unpredictable outcomes | 8 | 5 | 7 | 280 | explicit stage column + engine checks | ADR-0009 |
| FM-022 | Conflicting rules resolve nondeterministically | no tie-break strategy | unstable quote results | 8 | 4 | 7 | 224 | stage strategy policy | ADR-0009 |
| FM-023 | Slack ingress does heavy work pre-ack | naive implementation | retries and duplicates | 8 | 6 | 5 | 240 | ack-fast boundary | ADR-0010 |
| FM-024 | Event dedupe key missing | parser omission | duplicate execution | 8 | 5 | 6 | 240 | deterministic key derivation | ADR-0012 |
| FM-025 | Thread context not bound to quote ID | mapping bug | cross-quote contamination | 9 | 3 | 7 | 189 | strict thread lifecycle binding | ADR-0011 |
| FM-026 | Slash command grammar ambiguous | loose parser | wrong action chosen | 7 | 5 | 6 | 210 | explicit grammar and confirmations | ADR-0011 |
| FM-027 | Idempotency table write after side effects | ordering bug | duplicate external effects | 9 | 4 | 7 | 252 | reserve-before-side-effect pattern | ADR-0012 |
| FM-028 | Idempotency key collision | weak key strategy | suppressed valid operations | 7 | 2 | 8 | 112 | include semantic dimensions in key | ADR-0012 |
| FM-029 | Approval threshold matrix gap | policy config omission | unrouteable approvals | 8 | 4 | 6 | 192 | completeness validator | ADR-0013 |
| FM-030 | Approver fallback missing | org changes | stuck approvals | 7 | 5 | 5 | 175 | fallback chain required | ADR-0013 |
| FM-031 | Vendor DTO leaks into domain | adapter shortcut | lock-in and coupling | 7 | 4 | 7 | 196 | adapter anti-corruption layer | ADR-0014 |
| FM-032 | Composio API timeout not bounded | missing timeout config | worker starvation | 7 | 5 | 5 | 175 | bounded retry/backoff | ADR-0014 |
| FM-033 | Trace IDs not propagated | middleware gap | impossible root-cause reconstruction | 8 | 5 | 7 | 280 | required span fields contract | ADR-0015 |
| FM-034 | Structured fields drift across modules | naming inconsistency | fragmented observability | 6 | 6 | 6 | 216 | schema naming standard | ADR-0015 |
| FM-035 | Secret accidentally included in error chain | poor error wrapping | sensitive disclosure | 10 | 3 | 7 | 210 | secure error sanitization | ADR-0016 |
| FM-036 | SQLite file permissions too broad | default umask | local data exposure | 8 | 4 | 6 | 192 | doctor permission check | ADR-0016 |
| FM-037 | CLI command behavior changes silently | flag semantics drift | operator confusion | 6 | 5 | 6 | 180 | command snapshots + changelog | ADR-0017 |
| FM-038 | `doctor` misses critical dependency | incomplete checks | false confidence | 7 | 4 | 6 | 168 | checklist-driven doctor tests | ADR-0018 |
| FM-039 | `/health` includes expensive checks | poor endpoint scope | liveness instability | 5 | 5 | 5 | 125 | strict lightweight handler | ADR-0018 |
| FM-040 | Gate commands differ per contributor | undocumented variance | inconsistent quality | 7 | 6 | 6 | 252 | unified gate script | ADR-0019 |
| FM-041 | Flaky tests ignored repeatedly | weak discipline | degraded trust in CI | 7 | 5 | 5 | 175 | flake budget and quarantine policy | ADR-0019 |
| FM-042 | Architecture changes bypass ADR updates | process drift | decision debt | 8 | 5 | 7 | 280 | ADR supersession requirement | ADR-0020 |
| FM-043 | Bead graph and ADR dependencies diverge | coordination lag | scheduling confusion | 7 | 6 | 6 | 252 | periodic sync audit | ADR-0020 |
| FM-044 | Audit event missing on manual override | tool path bypass | incomplete trace | 9 | 3 | 7 | 189 | mandatory audit hooks | ADR-0007/0015 |
| FM-045 | Quote finalized before policy re-check | path shortcut | compliance violations | 9 | 4 | 6 | 216 | pre-finalization hard gate | ADR-0009/0013 |
| FM-046 | Renewal flow uses stale product revision | cache bug | invalid pricing | 8 | 4 | 6 | 192 | revision pinning policy | ADR-0006 |
| FM-047 | Concurrency bug in quote versioning | race condition | conflicting revisions | 8 | 5 | 7 | 280 | optimistic locking/version checks | ADR-0004/0006 |
| FM-048 | Approval UI action replayed | Slack retries | duplicate approval records | 7 | 5 | 6 | 210 | action idempotency keys | ADR-0012/0013 |
| FM-049 | External adapter partial success untracked | missing saga note | state divergence | 8 | 4 | 7 | 224 | compensating event logs | ADR-0014/0015 |
| FM-050 | Pricing trace omits one rule stage | serialization bug | poor explainability | 7 | 4 | 7 | 196 | trace completeness assertion | ADR-0008/0009 |
| FM-051 | Manual DB edits bypass migrations | operator habit | schema drift | 8 | 3 | 7 | 168 | migration-only policy + doctor warning | ADR-0005 |
| FM-052 | Invalid currency scale accepted | weak validation | rounding anomalies | 7 | 5 | 6 | 210 | currency scale validator | ADR-0008 |
| FM-053 | Slot filling confidence ignored | UX shortcut | bad defaults propagated | 6 | 6 | 5 | 180 | confidence threshold gating | ADR-0011 |
| FM-054 | Thread lifecycle state not persisted atomically | transaction split | state/audit mismatch | 8 | 4 | 7 | 224 | transactional state+event write | ADR-0007/0012 |
| FM-055 | Old policy snapshot cached too long | cache TTL issue | outdated compliance checks | 8 | 4 | 7 | 224 | snapshot version checks | ADR-0009/0013 |
| FM-056 | Missing SLOs for ingestion | no target | slow regressions unnoticed | 6 | 6 | 7 | 252 | define SLO and alerting | ADR-0015/0019 |
| FM-057 | CLI outputs non-deterministic ordering | map iteration | automation fragility | 5 | 6 | 6 | 180 | sorted output contract | ADR-0017 |
| FM-058 | Quote export uses stale view model | sync lag | inconsistent customer docs | 7 | 4 | 6 | 168 | export snapshot binding | ADR-0006/0007 |
| FM-059 | Correlation ID regenerated mid-flow | context reset | broken trace continuity | 7 | 4 | 7 | 196 | immutable correlation propagation | ADR-0015 |
| FM-060 | Decision freeze skipped due delivery pressure | process bypass | architecture fragmentation | 8 | 5 | 8 | 320 | freeze gate tied to bead completion | ADR-0020 |

### 46.2 Highest Priority Mitigation Queue (RPN >= 240)

- FM-002: runtime dependency leak into domain crate.
- FM-004: uncooperative tasks hanging shutdown.
- FM-015: invalid domain state construction.
- FM-017: flow transitions with side effects.
- FM-019: float usage in money path.
- FM-023: slow pre-ack processing.
- FM-024: missing dedupe keys.
- FM-027: side effects before idempotency reserve.
- FM-033: broken trace propagation.
- FM-040: inconsistent quality gate usage.
- FM-042: architectural drift without ADR updates.
- FM-047: quote version race conditions.
- FM-060: skipped decision freeze.

### 46.3 Mitigation Sprint Ordering

1. Boundary enforcement (`FM-002`, `FM-015`, `FM-017`).
2. Ingress reliability and idempotency (`FM-023`, `FM-024`, `FM-027`).
3. Determinism and financial integrity (`FM-019`, `FM-047`).
4. Observability and process integrity (`FM-033`, `FM-040`, `FM-042`, `FM-060`).


---

## 47. Scenario Catalog (Deterministic Test and Validation Cases)

This section provides concrete scenario definitions that can be turned into integration tests and simulation fixtures.

Scenario format:
- `Scenario ID`
- `Intent`
- `Setup`
- `Action`
- `Expected deterministic result`
- `Expected audit/trace assertions`
- `Primary ADR linkage`

### 47.1 Startup and Configuration Scenarios

#### SCN-001
Intent: verify startup with only default config file.
Setup: valid local config file, no env overrides.
Action: run `quotey start`.
Expected deterministic result: process starts; config resolved from file/defaults.
Expected audit/trace assertions: startup span includes config source summary, redacted secrets.
Primary ADR linkage: ADR-0003, ADR-0017.

#### SCN-002
Intent: verify env variable override precedence.
Setup: config file sets DB path A, env sets DB path B.
Action: run `quotey config`.
Expected deterministic result: effective DB path = B.
Expected audit/trace assertions: precedence decision logged without secret values.
Primary ADR linkage: ADR-0003.

#### SCN-003
Intent: verify CLI override precedence over env.
Setup: env sets log level info; CLI sets debug.
Action: run `quotey start --log-level debug`.
Expected deterministic result: effective level debug.
Expected audit/trace assertions: startup config source indicates CLI override.
Primary ADR linkage: ADR-0003, ADR-0017.

#### SCN-004
Intent: fail-fast on missing required Slack token.
Setup: Slack enabled, bot token absent.
Action: run `quotey start`.
Expected deterministic result: startup fails before network loops.
Expected audit/trace assertions: structured error category `config.missing_secret`.
Primary ADR linkage: ADR-0003, ADR-0016.

#### SCN-005
Intent: verify secret redaction in diagnostics.
Setup: config with known fake token value.
Action: run `quotey config --show-effective`.
Expected deterministic result: token hidden/redacted.
Expected audit/trace assertions: output never includes token literal.
Primary ADR linkage: ADR-0003, ADR-0016.

#### SCN-006
Intent: ensure migration command is idempotent.
Setup: database already migrated.
Action: run `quotey migrate` twice.
Expected deterministic result: second run reports no-op success.
Expected audit/trace assertions: migration summary unchanged on second run.
Primary ADR linkage: ADR-0005.

#### SCN-007
Intent: validate WAL activation check.
Setup: DB starts in non-WAL mode.
Action: app initializes DB policy.
Expected deterministic result: WAL policy applied or explicit fail with reason.
Expected audit/trace assertions: pragma verification event emitted.
Primary ADR linkage: ADR-0004.

#### SCN-008
Intent: verify foreign key enforcement.
Setup: attempt insert child row with nonexistent parent.
Action: run repository write.
Expected deterministic result: write rejected.
Expected audit/trace assertions: DB integrity violation recorded.
Primary ADR linkage: ADR-0004.

#### SCN-009
Intent: detect insecure DB file permissions.
Setup: DB file world-readable.
Action: run `quotey doctor`.
Expected deterministic result: doctor warns/fails based on policy.
Expected audit/trace assertions: security finding emitted.
Primary ADR linkage: ADR-0016, ADR-0018.

#### SCN-010
Intent: validate clean shutdown under signal.
Setup: running process with active listeners.
Action: send SIGINT.
Expected deterministic result: orderly shutdown within timeout.
Expected audit/trace assertions: `shutdown.start` then `shutdown.complete` spans present.
Primary ADR linkage: ADR-0002.

### 47.2 Slack Ingress and Thread Lifecycle Scenarios

#### SCN-011
Intent: slash command starts new quote thread.
Setup: valid slash command payload.
Action: `/quote create for acme renewal`.
Expected deterministic result: quote session created and thread context bound.
Expected audit/trace assertions: ingress event, flow init event, thread binding event.
Primary ADR linkage: ADR-0010, ADR-0011.

#### SCN-012
Intent: duplicate slash command envelope deduped.
Setup: same envelope delivered twice.
Action: process duplicate events.
Expected deterministic result: one business operation, second acknowledged as replay.
Expected audit/trace assertions: idempotency hit logged.
Primary ADR linkage: ADR-0012.

#### SCN-013
Intent: high latency downstream does not delay ack.
Setup: injected delay in command processor.
Action: send command event.
Expected deterministic result: ack remains within target latency.
Expected audit/trace assertions: separate spans for ingress ack and async processing.
Primary ADR linkage: ADR-0010.

#### SCN-014
Intent: unknown command grammar prompts guidance.
Setup: malformed slash command.
Action: `/quote do-something-weird`.
Expected deterministic result: user receives help text and no state mutation.
Expected audit/trace assertions: grammar parse failure event.
Primary ADR linkage: ADR-0011.

#### SCN-015
Intent: thread reply maps to existing quote context.
Setup: active quote thread.
Action: user replies with additional requirements.
Expected deterministic result: same quote context updated through flow engine.
Expected audit/trace assertions: thread-to-quote binding verified.
Primary ADR linkage: ADR-0011.

#### SCN-016
Intent: cross-thread reply rejected for wrong quote.
Setup: user replies in unrelated thread with quote action.
Action: attempt quote mutate command.
Expected deterministic result: safe rejection + guidance.
Expected audit/trace assertions: context mismatch warning event.
Primary ADR linkage: ADR-0011, ADR-0007.

#### SCN-017
Intent: block action callback idempotency.
Setup: interactive button clicked, duplicate callback replayed.
Action: process two identical callbacks.
Expected deterministic result: single transition effect.
Expected audit/trace assertions: duplicate callback dedupe record.
Primary ADR linkage: ADR-0012, ADR-0013.

#### SCN-018
Intent: expired thread lifecycle rejects updates.
Setup: quote marked expired.
Action: user attempts additional line item add in thread.
Expected deterministic result: mutation denied with reopen/new quote option.
Expected audit/trace assertions: illegal transition logged.
Primary ADR linkage: ADR-0007, ADR-0011.

#### SCN-019
Intent: missing required slots triggers structured prompt.
Setup: partial intent extraction.
Action: user asks for quote without quantity.
Expected deterministic result: bot requests missing mandatory fields.
Expected audit/trace assertions: missing-slot event with required slot set.
Primary ADR linkage: ADR-0011.

#### SCN-020
Intent: low-confidence extraction requires confirmation.
Setup: fuzzy product mention with low confidence.
Action: user requests "the enterprise thing with add-ons".
Expected deterministic result: confirmation prompt before pricing.
Expected audit/trace assertions: confidence threshold decision logged.
Primary ADR linkage: ADR-0011.

#### SCN-021
Intent: concurrent user messages preserve deterministic ordering.
Setup: two rapid thread replies with edits.
Action: process both events.
Expected deterministic result: deterministic order strategy applied.
Expected audit/trace assertions: operation sequence numbers recorded.
Primary ADR linkage: ADR-0012.

#### SCN-022
Intent: Slack reconnect event recovery.
Setup: temporary socket disconnect.
Action: listener reconnect sequence.
Expected deterministic result: session resumes and pending operations continue safely.
Expected audit/trace assertions: reconnect span with downtime duration.
Primary ADR linkage: ADR-0010, ADR-0002.

#### SCN-023
Intent: rate limit response handling.
Setup: outbound Slack API receives rate limit signal.
Action: send burst of status updates.
Expected deterministic result: bounded retry/backoff, no data corruption.
Expected audit/trace assertions: rate-limit counter increments.
Primary ADR linkage: ADR-0010, ADR-0015.

#### SCN-024
Intent: invalid signature/token context rejected.
Setup: forged or invalid auth context.
Action: process inbound event.
Expected deterministic result: event rejected safely.
Expected audit/trace assertions: security rejection event.
Primary ADR linkage: ADR-0016, ADR-0010.

#### SCN-025
Intent: slash command in DM and channel contexts.
Setup: same command from different Slack contexts.
Action: initiate quote from DM and channel.
Expected deterministic result: context policy applied consistently.
Expected audit/trace assertions: origin context tagged in trace.
Primary ADR linkage: ADR-0011.

### 47.3 Domain and Flow Engine Scenarios

#### SCN-026
Intent: initialize quote with mandatory metadata.
Setup: valid account and opportunity identifiers.
Action: create quote entity.
Expected deterministic result: quote enters initialized state.
Expected audit/trace assertions: quote.created event includes invariant snapshot.
Primary ADR linkage: ADR-0006, ADR-0007.

#### SCN-027
Intent: reject quote line with zero quantity.
Setup: line creation request with quantity 0.
Action: apply mutation.
Expected deterministic result: validation error, no state mutation.
Expected audit/trace assertions: invariant violation reason captured.
Primary ADR linkage: ADR-0006.

#### SCN-028
Intent: reject transition to finalized when approval pending.
Setup: state approval_pending.
Action: attempt finalize command.
Expected deterministic result: transition denied.
Expected audit/trace assertions: transition rejection event.
Primary ADR linkage: ADR-0007, ADR-0013.

#### SCN-029
Intent: allow transition from priced to approval_pending.
Setup: valid priced quote with policy threshold exceeded.
Action: request approval.
Expected deterministic result: state transition succeeds.
Expected audit/trace assertions: state before/after and policy evidence recorded.
Primary ADR linkage: ADR-0007, ADR-0013.

#### SCN-030
Intent: replay event stream recreates same final state.
Setup: deterministic event sequence fixture.
Action: replay from initial state.
Expected deterministic result: same final state and key totals.
Expected audit/trace assertions: replay checksum match.
Primary ADR linkage: ADR-0007.

#### SCN-031
Intent: enforce version check on concurrent edits.
Setup: two updates based on same version.
Action: apply both updates.
Expected deterministic result: one succeeds, second conflicts.
Expected audit/trace assertions: optimistic lock conflict event.
Primary ADR linkage: ADR-0006, ADR-0004.

#### SCN-032
Intent: domain error typing is stable for UI mapping.
Setup: trigger invariant and policy errors.
Action: inspect error categories.
Expected deterministic result: typed errors map consistently to user responses.
Expected audit/trace assertions: error category tags present.
Primary ADR linkage: ADR-0006.

#### SCN-033
Intent: verify no side effects in pure transition step.
Setup: instrumentation around transition function.
Action: execute transitions.
Expected deterministic result: no DB/network call inside pure transition.
Expected audit/trace assertions: side-effect boundary spans separate.
Primary ADR linkage: ADR-0007.

#### SCN-034
Intent: transition guard on missing required fields.
Setup: required slot absent.
Action: attempt move to priced state.
Expected deterministic result: denied with specific missing field set.
Expected audit/trace assertions: guard failure payload includes missing set.
Primary ADR linkage: ADR-0007.

#### SCN-035
Intent: quote expiration transition timing.
Setup: quote passes expiry timestamp.
Action: scheduler/command checks lifecycle.
Expected deterministic result: state transitions to expired.
Expected audit/trace assertions: expiration event with timestamp evidence.
Primary ADR linkage: ADR-0007.

#### SCN-036
Intent: clone quote creates independent version lineage.
Setup: existing quote with lines and approvals.
Action: clone command.
Expected deterministic result: new quote ID with lineage reference.
Expected audit/trace assertions: clone provenance event.
Primary ADR linkage: ADR-0006, ADR-0007.

#### SCN-037
Intent: amendment preserves immutable prior version.
Setup: finalized quote.
Action: start amendment.
Expected deterministic result: new editable version; prior remains immutable.
Expected audit/trace assertions: version chain integrity events.
Primary ADR linkage: ADR-0006.

#### SCN-038
Intent: lifecycle summary query determinism.
Setup: quote with many transitions.
Action: compute lifecycle summary view.
Expected deterministic result: summary stable independent of query order.
Expected audit/trace assertions: lifecycle hash recorded.
Primary ADR linkage: ADR-0007, ADR-0004.

#### SCN-039
Intent: reject unknown state transition enum.
Setup: malformed or future enum value in replay input.
Action: parse and transition.
Expected deterministic result: explicit unsupported-state error.
Expected audit/trace assertions: parse rejection event.
Primary ADR linkage: ADR-0007.

#### SCN-040
Intent: ensure deterministic sorting of line items for pricing input.
Setup: unsorted line insertion order.
Action: run pricing.
Expected deterministic result: sorted canonical order used.
Expected audit/trace assertions: normalized order metadata.
Primary ADR linkage: ADR-0008, ADR-0007.

### 47.4 Pricing and Policy Scenarios

#### SCN-041
Intent: base price retrieval by price book.
Setup: valid product and account segment.
Action: run pricing stage 1.
Expected deterministic result: correct base price selected.
Expected audit/trace assertions: price book selection trace step.
Primary ADR linkage: ADR-0008, ADR-0009.

#### SCN-042
Intent: volume tier discount application.
Setup: quantity above tier threshold.
Action: run pricing stage.
Expected deterministic result: expected tier discount applied once.
Expected audit/trace assertions: tier rule ID captured.
Primary ADR linkage: ADR-0009.

#### SCN-043
Intent: conflicting discount rules resolved by stage strategy.
Setup: two matching discount rules.
Action: run policy engine.
Expected deterministic result: deterministic chosen outcome.
Expected audit/trace assertions: conflict resolution reason captured.
Primary ADR linkage: ADR-0009.

#### SCN-044
Intent: margin floor violation triggers approval requirement.
Setup: discount results in margin below policy floor.
Action: run policy checks.
Expected deterministic result: approval required flag set.
Expected audit/trace assertions: margin floor policy evidence.
Primary ADR linkage: ADR-0013, ADR-0009.

#### SCN-045
Intent: currency rounding consistency across line and total.
Setup: many fractional decimal operations.
Action: compute line totals and grand total.
Expected deterministic result: no cent-level mismatch.
Expected audit/trace assertions: rounding steps included in trace.
Primary ADR linkage: ADR-0008.

#### SCN-046
Intent: null/empty policy data handling.
Setup: missing optional rule attributes.
Action: execute rule evaluation.
Expected deterministic result: stable default handling with explicit warnings.
Expected audit/trace assertions: policy fallback event.
Primary ADR linkage: ADR-0009.

#### SCN-047
Intent: stale policy version detection.
Setup: policy updated mid-quote session.
Action: finalize pricing.
Expected deterministic result: either pin old version with disclosure or require reprice.
Expected audit/trace assertions: policy version mismatch event.
Primary ADR linkage: ADR-0009, ADR-0013.

#### SCN-048
Intent: minimum order quantity constraint.
Setup: line item below MOQ.
Action: validate configuration.
Expected deterministic result: constraint failure and user prompt.
Expected audit/trace assertions: constraint ID and rule text reference.
Primary ADR linkage: ADR-0006, ADR-0009.

#### SCN-049
Intent: mutually exclusive options conflict.
Setup: incompatible options selected.
Action: run constraint engine.
Expected deterministic result: conflict identified deterministically.
Expected audit/trace assertions: exclusion rule trace step.
Primary ADR linkage: ADR-0006, ADR-0009.

#### SCN-050
Intent: dependency requirement missing.
Setup: option B selected without required option A.
Action: run constraints.
Expected deterministic result: violation raised; remediation suggestion returned.
Expected audit/trace assertions: dependency rule trace step.
Primary ADR linkage: ADR-0006, ADR-0009.

#### SCN-051
Intent: large quote line count performance sanity.
Setup: quote with 500 lines in fixture.
Action: run deterministic pricing.
Expected deterministic result: completes under target threshold.
Expected audit/trace assertions: stage timing metrics emitted.
Primary ADR linkage: ADR-0008, ADR-0015.

#### SCN-052
Intent: negative discount input validation.
Setup: malformed policy with negative percentage bounds.
Action: policy load/validate.
Expected deterministic result: startup or policy import rejects config.
Expected audit/trace assertions: validation error category.
Primary ADR linkage: ADR-0003, ADR-0009.

#### SCN-053
Intent: policy stage skipping logged explicitly.
Setup: no applicable adjustment rules.
Action: run engine.
Expected deterministic result: stage skipped deterministically.
Expected audit/trace assertions: stage skip reason event.
Primary ADR linkage: ADR-0009, ADR-0015.

#### SCN-054
Intent: deterministic replay after rule set reorder with same semantics.
Setup: equivalent rules with different insertion order.
Action: run pricing.
Expected deterministic result: identical outputs.
Expected audit/trace assertions: canonical ordering metadata.
Primary ADR linkage: ADR-0009.

#### SCN-055
Intent: unsupported formula operator rejected safely.
Setup: imported formula with unknown operator.
Action: evaluate formula.
Expected deterministic result: explicit policy parse error.
Expected audit/trace assertions: formula rejection audit event.
Primary ADR linkage: ADR-0009.

### 47.5 Approval and External Adapter Scenarios

#### SCN-056
Intent: threshold route to manager approval.
Setup: discount in manager band.
Action: submit approval request.
Expected deterministic result: route to manager only.
Expected audit/trace assertions: route decision factors recorded.
Primary ADR linkage: ADR-0013.

#### SCN-057
Intent: threshold route to VP and finance.
Setup: extreme discount and low margin.
Action: submit approval request.
Expected deterministic result: multi-approver route selected.
Expected audit/trace assertions: multi-level route event.
Primary ADR linkage: ADR-0013.

#### SCN-058
Intent: approver timeout triggers escalation.
Setup: approver does not respond within SLA.
Action: run escalation scheduler.
Expected deterministic result: request escalated to fallback approver.
Expected audit/trace assertions: escalation event with reason.
Primary ADR linkage: ADR-0013.

#### SCN-059
Intent: duplicate approval action deduped.
Setup: same approver action replayed.
Action: process action twice.
Expected deterministic result: first accepted, second acknowledged as duplicate.
Expected audit/trace assertions: idempotency hit on approval action.
Primary ADR linkage: ADR-0012, ADR-0013.

#### SCN-060
Intent: approval denied transitions quote to denied state.
Setup: approval request active.
Action: approver clicks deny.
Expected deterministic result: quote state set to denied.
Expected audit/trace assertions: denial rationale captured.
Primary ADR linkage: ADR-0013, ADR-0007.

#### SCN-061
Intent: approval granted unblocks finalization.
Setup: all required approvers approved.
Action: process final approval.
Expected deterministic result: quote transitions to approved.
Expected audit/trace assertions: cumulative approval evidence logged.
Primary ADR linkage: ADR-0013.

#### SCN-062
Intent: Composio adapter unavailable fallback behavior.
Setup: network outage for adapter endpoint.
Action: sync request attempted.
Expected deterministic result: operation fails safely with retry policy.
Expected audit/trace assertions: adapter failure category and retry policy event.
Primary ADR linkage: ADR-0014.

#### SCN-063
Intent: Composio payload mapping mismatch.
Setup: unexpected field from API.
Action: parse payload.
Expected deterministic result: mapping error explicit, domain unchanged.
Expected audit/trace assertions: schema mismatch diagnostics.
Primary ADR linkage: ADR-0014.

#### SCN-064
Intent: fixture adapter parity with Composio contract.
Setup: same domain request to fixture and real adapter mock.
Action: compare mapped outputs.
Expected deterministic result: contract-consistent fields.
Expected audit/trace assertions: contract-test report artifact.
Primary ADR linkage: ADR-0014.

#### SCN-065
Intent: partial external sync compensation logging.
Setup: update succeeded remotely, local commit fails.
Action: process sync transaction.
Expected deterministic result: compensating event recorded.
Expected audit/trace assertions: compensation event with correlation ID.
Primary ADR linkage: ADR-0014, ADR-0015.

### 47.6 Observability, Security, and Operational Scenarios

#### SCN-066
Intent: full quote lifecycle trace reconstruction.
Setup: run end-to-end quote scenario.
Action: query logs and audit rows by correlation ID.
Expected deterministic result: complete timeline reconstructable.
Expected audit/trace assertions: no missing critical span segments.
Primary ADR linkage: ADR-0015.

#### SCN-067
Intent: confirm no secrets in structured logs.
Setup: run operations with real-looking secret placeholders.
Action: inspect captured logs.
Expected deterministic result: secrets absent/redacted.
Expected audit/trace assertions: redaction markers where needed.
Primary ADR linkage: ADR-0016, ADR-0015.

#### SCN-068
Intent: health endpoint remains lightweight under load.
Setup: concurrent health checks during active processing.
Action: benchmark endpoint latency.
Expected deterministic result: stable low-latency responses.
Expected audit/trace assertions: no expensive DB scans in health path.
Primary ADR linkage: ADR-0018.

#### SCN-069
Intent: doctor command detects migration drift.
Setup: DB missing latest migration.
Action: run `quotey doctor`.
Expected deterministic result: migration drift warning/failure.
Expected audit/trace assertions: drift details in diagnostics output.
Primary ADR linkage: ADR-0018, ADR-0005.

#### SCN-070
Intent: operator command output machine readability.
Setup: run `doctor` with machine mode.
Action: parse JSON output.
Expected deterministic result: stable schema for automation.
Expected audit/trace assertions: command metadata includes version.
Primary ADR linkage: ADR-0017.

#### SCN-071
Intent: clippy warning gate enforcement.
Setup: introduce deliberate warning in branch.
Action: run gate commands.
Expected deterministic result: gate fails.
Expected audit/trace assertions: failure captured in CI artifacts.
Primary ADR linkage: ADR-0019.

#### SCN-072
Intent: nextest profile execution consistency.
Setup: same test suite under nextest.
Action: run test gate repeatedly.
Expected deterministic result: stable pass/fail behavior.
Expected audit/trace assertions: flake rate tracked.
Primary ADR linkage: ADR-0019.

#### SCN-073
Intent: cargo-deny dependency policy enforcement.
Setup: add disallowed dependency license.
Action: run deny check.
Expected deterministic result: gate failure with policy reason.
Expected audit/trace assertions: dependency policy event.
Primary ADR linkage: ADR-0019, ADR-0016.

#### SCN-074
Intent: decision freeze audit detects drift.
Setup: code change in architecture boundary without ADR update.
Action: run architecture audit checklist.
Expected deterministic result: drift finding emitted.
Expected audit/trace assertions: ADR mismatch report generated.
Primary ADR linkage: ADR-0020.

#### SCN-075
Intent: superseding ADR process check.
Setup: propose architecture change post-freeze.
Action: create superseding ADR and link beads.
Expected deterministic result: change process accepted only with complete metadata.
Expected audit/trace assertions: supersession links verified.
Primary ADR linkage: ADR-0020.

#### SCN-076
Intent: backup/restore local DB integrity.
Setup: create backup, mutate DB, restore backup.
Action: run restore verification.
Expected deterministic result: recovered schema and critical data intact.
Expected audit/trace assertions: restore audit event.
Primary ADR linkage: ADR-0004, ADR-0005.

#### SCN-077
Intent: process crash mid-operation recovery.
Setup: force crash after idempotency reservation before final commit.
Action: restart and replay event.
Expected deterministic result: exactly-once business outcome preserved.
Expected audit/trace assertions: recovery replay event.
Primary ADR linkage: ADR-0012.

#### SCN-078
Intent: long-running approval flow across restarts.
Setup: pending approval, process restart.
Action: restart and process approval action.
Expected deterministic result: state continuity preserved.
Expected audit/trace assertions: continuity event chain intact.
Primary ADR linkage: ADR-0013, ADR-0012.

#### SCN-079
Intent: logging format switch via config.
Setup: pretty vs JSON format toggle.
Action: run both modes.
Expected deterministic result: same semantic fields available in both modes.
Expected audit/trace assertions: field preservation test results.
Primary ADR linkage: ADR-0015.

#### SCN-080
Intent: full foundation smoke pipeline.
Setup: clean environment.
Action: run scaffold build, migrate, doctor, start, smoke interaction.
Expected deterministic result: end-to-end baseline success.
Expected audit/trace assertions: smoke run report generated.
Primary ADR linkage: ADR-0001 through ADR-0019.


---

## 48. ADR Work Packages (Tasks, Subtasks, and Dependency Overlay)

This section converts ADRs into granular work packages that can be mirrored to beads.

Naming convention:
- `WP-<ADR>-<n>` for task-level package.
- `WP-<ADR>-<n>.<m>` for subtask.

### 48.1 Work Package Set for ADR-0001 (Workspace Topology)

**WP-0001-1 Objective:** create workspace root and crate manifests.
- WP-0001-1.1: create workspace root members list.
- WP-0001-1.2: define shared dependency policy at workspace level.
- WP-0001-1.3: configure workspace lints and profile defaults.
- WP-0001-1.4: add resolver configuration explicitly.
- WP-0001-1.5: document crate ownership boundaries.

**WP-0001-2 Objective:** establish boundary checks.
- WP-0001-2.1: add dependency graph check command to docs.
- WP-0001-2.2: add CI check ensuring `core` has no forbidden deps.
- WP-0001-2.3: add simple policy file for allowed crate edges.

**Dependencies:** requires `bd-3d8.12` toolchain validation.
**Exit criteria:** workspace builds and policy checks pass.

### 48.2 Work Package Set for ADR-0002 (Runtime Model)

**WP-0002-1 Objective:** create runtime bootstrap skeleton.
- WP-0002-1.1: initialize tokio runtime entrypoint.
- WP-0002-1.2: define top-level task supervisor abstraction.
- WP-0002-1.3: wire cancellation token propagation.

**WP-0002-2 Objective:** graceful shutdown contract.
- WP-0002-2.1: implement SIGINT/SIGTERM listener.
- WP-0002-2.2: define shutdown timeout config.
- WP-0002-2.3: implement component stop ordering.
- WP-0002-2.4: add shutdown integration test harness.

**Dependencies:** ADR-0001 accepted.
**Exit criteria:** process always exits cleanly under signal test.

### 48.3 Work Package Set for ADR-0003 (Config and Secrets)

**WP-0003-1 Objective:** typed config schema.
- WP-0003-1.1: define root config struct and sections.
- WP-0003-1.2: add serde defaults for optional fields.
- WP-0003-1.3: add enum types for providers/formats.

**WP-0003-2 Objective:** precedence engine.
- WP-0003-2.1: load defaults.
- WP-0003-2.2: apply file source.
- WP-0003-2.3: apply env overrides.
- WP-0003-2.4: apply CLI overrides.
- WP-0003-2.5: freeze effective config.

**WP-0003-3 Objective:** validation and redaction.
- WP-0003-3.1: validate required secret presence by feature.
- WP-0003-3.2: validate URLs/ports/ranges.
- WP-0003-3.3: add redacted diagnostic output.
- WP-0003-3.4: add tests for redaction and precedence.

**Dependencies:** ADR-0001.
**Exit criteria:** config tests pass and startup validation is deterministic.

### 48.4 Work Package Set for ADR-0004 (SQLite Policy)

**WP-0004-1 Objective:** DB connect policy implementation.
- WP-0004-1.1: configure connect options with foreign keys.
- WP-0004-1.2: configure WAL mode target.
- WP-0004-1.3: configure busy timeout.
- WP-0004-1.4: enforce conservative pool sizing.

**WP-0004-2 Objective:** DB policy verification hooks.
- WP-0004-2.1: add pragma verification query on startup.
- WP-0004-2.2: surface policy state in doctor diagnostics.
- WP-0004-2.3: add lock contention metric collection.

**Dependencies:** ADR-0003.
**Exit criteria:** integration tests validate FK behavior and WAL posture.

### 48.5 Work Package Set for ADR-0005 (Migrations)

**WP-0005-1 Objective:** migration command and bootstrap.
- WP-0005-1.1: add migration discovery wiring.
- WP-0005-1.2: implement CLI migrate command.
- WP-0005-1.3: add migration status reporting.

**WP-0005-2 Objective:** migration safety model.
- WP-0005-2.1: define migration naming convention.
- WP-0005-2.2: require rollback notes in migration headers.
- WP-0005-2.3: add CI check for duplicate migration timestamps.
- WP-0005-2.4: add fresh-db and existing-db smoke tests.

**Dependencies:** ADR-0004.
**Exit criteria:** migration path deterministic across repeated runs.

### 48.6 Work Package Set for ADR-0006 (Domain Invariants)

**WP-0006-1 Objective:** entity and value object design.
- WP-0006-1.1: define typed IDs for key aggregates.
- WP-0006-1.2: define Money/value wrappers.
- WP-0006-1.3: define metadata structs for audit references.

**WP-0006-2 Objective:** invariant enforcement API.
- WP-0006-2.1: private fields + smart constructors.
- WP-0006-2.2: typed error taxonomy for invariant violations.
- WP-0006-2.3: mutation methods with guard checks.

**WP-0006-3 Objective:** invariant test suite.
- WP-0006-3.1: constructor rejection cases.
- WP-0006-3.2: mutation rejection cases.
- WP-0006-3.3: serialization roundtrip validity checks.

**Dependencies:** ADR-0001, ADR-0004.
**Exit criteria:** invalid states cannot be persisted or transitioned.

### 48.7 Work Package Set for ADR-0007 (Flow Engine)

**WP-0007-1 Objective:** state machine model.
- WP-0007-1.1: enumerate legal states.
- WP-0007-1.2: define legal transition matrix.
- WP-0007-1.3: define transition event payload types.

**WP-0007-2 Objective:** pure transition executor.
- WP-0007-2.1: implement transition function without side effects.
- WP-0007-2.2: return effect plan tokens for outer layers.
- WP-0007-2.3: encode rejection reasons with required missing data.

**WP-0007-3 Objective:** determinism checks.
- WP-0007-3.1: event replay test harness.
- WP-0007-3.2: transition table completeness assertion.
- WP-0007-3.3: non-determinism sentinel tests.

**Dependencies:** ADR-0006.
**Exit criteria:** same event sequence always yields same final state.

### 48.8 Work Package Set for ADR-0008 (Pricing Precision)

**WP-0008-1 Objective:** money arithmetic policy.
- WP-0008-1.1: decimal-only core arithmetic API.
- WP-0008-1.2: centralize rounding rules.
- WP-0008-1.3: define currency scale validation.

**WP-0008-2 Objective:** persistence representation contract.
- WP-0008-2.1: choose canonical DB storage format.
- WP-0008-2.2: add conversion utilities.
- WP-0008-2.3: add schema constraints where feasible.

**WP-0008-3 Objective:** precision regression suite.
- WP-0008-3.1: golden vectors.
- WP-0008-3.2: random differential checks.
- WP-0008-3.3: replay precision checks.

**Dependencies:** ADR-0006.
**Exit criteria:** no precision drift across repeated calculations.

### 48.9 Work Package Set for ADR-0009 (Rules Ordering)

**WP-0009-1 Objective:** stage-oriented rule model.
- WP-0009-1.1: schema for stage + priority.
- WP-0009-1.2: conflict strategy metadata.
- WP-0009-1.3: rule activation window metadata.

**WP-0009-2 Objective:** deterministic evaluator.
- WP-0009-2.1: stage iteration implementation.
- WP-0009-2.2: tie-break strategy implementation.
- WP-0009-2.3: explainability payload generation.

**WP-0009-3 Objective:** operator validation tooling.
- WP-0009-3.1: rule lint command for conflicts/gaps.
- WP-0009-3.2: preview evaluation command with sample context.

**Dependencies:** ADR-0008.
**Exit criteria:** conflicting rule sets produce deterministic outputs with explanations.

### 48.10 Work Package Set for ADR-0010 (Slack Ingress)

**WP-0010-1 Objective:** ingress parser and ack path.
- WP-0010-1.1: envelope parsing and validation.
- WP-0010-1.2: immediate ack path.
- WP-0010-1.3: normalized event emission.

**WP-0010-2 Objective:** ingress safety controls.
- WP-0010-2.1: auth/token validation checks.
- WP-0010-2.2: payload size and schema guardrails.
- WP-0010-2.3: reject unknown event types safely.

**WP-0010-3 Objective:** ingress observability.
- WP-0010-3.1: ack latency metrics.
- WP-0010-3.2: ingress error taxonomy.
- WP-0010-3.3: reconnect and retry counters.

**Dependencies:** ADR-0002, ADR-0003.
**Exit criteria:** ack latency stays bounded under load and retries are safe.

### 48.11 Work Package Set for ADR-0011 (Command Grammar)

**WP-0011-1 Objective:** command grammar specification.
- WP-0011-1.1: define slash command patterns.
- WP-0011-1.2: define slot extraction contract.
- WP-0011-1.3: define ambiguity/clarification pathways.

**WP-0011-2 Objective:** thread lifecycle binding.
- WP-0011-2.1: bind thread ID to quote context.
- WP-0011-2.2: enforce lifecycle transitions in thread context.
- WP-0011-2.3: define expired/reopened interactions.

**WP-0011-3 Objective:** UX verification.
- WP-0011-3.1: conversation scenario tests.
- WP-0011-3.2: ambiguous intent simulation tests.

**Dependencies:** ADR-0010.
**Exit criteria:** no ambiguous mutation path bypasses deterministic validation.

### 48.12 Work Package Set for ADR-0012 (Idempotency)

**WP-0012-1 Objective:** operation key contract.
- WP-0012-1.1: define key dimensions (source, action, semantic payload hash).
- WP-0012-1.2: normalize payload hashing rules.
- WP-0012-1.3: store operation ledger schema.

**WP-0012-2 Objective:** execution wrapper.
- WP-0012-2.1: reserve operation key transactionally.
- WP-0012-2.2: run business logic.
- WP-0012-2.3: mark completion state and response snapshot.

**WP-0012-3 Objective:** replay and recovery tests.
- WP-0012-3.1: duplicate delivery simulation.
- WP-0012-3.2: crash-after-reserve simulation.
- WP-0012-3.3: crash-after-side-effect simulation.

**Dependencies:** ADR-0010, ADR-0004.
**Exit criteria:** duplicate deliveries never produce duplicate business effects.

### 48.13 Work Package Set for ADR-0013 (Approval Governance)

**WP-0013-1 Objective:** policy matrix model.
- WP-0013-1.1: threshold bands schema.
- WP-0013-1.2: approver role resolution schema.
- WP-0013-1.3: escalation policy schema.

**WP-0013-2 Objective:** routing engine.
- WP-0013-2.1: deterministic route resolution logic.
- WP-0013-2.2: fallback/timeout escalation logic.
- WP-0013-2.3: route explanation output.

**WP-0013-3 Objective:** approval action integrity.
- WP-0013-3.1: approval action idempotency handling.
- WP-0013-3.2: audit event enforcement.
- WP-0013-3.3: multi-approver completion logic.

**Dependencies:** ADR-0009, ADR-0011.
**Exit criteria:** approval routing is deterministic and fully auditable.

### 48.14 Work Package Set for ADR-0014 (CRM Adapter Boundary)

**WP-0014-1 Objective:** anti-corruption layer.
- WP-0014-1.1: define domain-side adapter trait.
- WP-0014-1.2: define mapping DTOs internal to adapter crate.
- WP-0014-1.3: enforce no vendor types in core crate.

**WP-0014-2 Objective:** adapter implementations.
- WP-0014-2.1: fixture adapter implementation.
- WP-0014-2.2: Composio REST adapter skeleton.
- WP-0014-2.3: retry/backoff/timeout policy.

**WP-0014-3 Objective:** contract tests.
- WP-0014-3.1: fixture parity tests.
- WP-0014-3.2: mapping completeness tests.
- WP-0014-3.3: failure-mode tests.

**Dependencies:** ADR-0003, ADR-0012.
**Exit criteria:** adapter boundary stable with deterministic error mapping.

### 48.15 Work Package Set for ADR-0015 (Observability)

**WP-0015-1 Objective:** trace schema definition.
- WP-0015-1.1: define required span fields.
- WP-0015-1.2: define optional diagnostic fields.
- WP-0015-1.3: define naming conventions.

**WP-0015-2 Objective:** propagation implementation.
- WP-0015-2.1: ingress trace context creation.
- WP-0015-2.2: context propagation through flow and DB.
- WP-0015-2.3: outbound propagation to adapter calls.

**WP-0015-3 Objective:** observability verification.
- WP-0015-3.1: trace completeness checks.
- WP-0015-3.2: secret/PII log scrub checks.
- WP-0015-3.3: troubleshooting drill runbook test.

**Dependencies:** ADR-0002, ADR-0012.
**Exit criteria:** one quote lifecycle can be reconstructed with logs + audit rows only.

### 48.16 Work Package Set for ADR-0016 (Security Baseline)

**WP-0016-1 Objective:** secret handling hardening.
- WP-0016-1.1: enforce SecretString wrappers.
- WP-0016-1.2: forbid secret debug display.
- WP-0016-1.3: sanitize error conversion paths.

**WP-0016-2 Objective:** local data posture checks.
- WP-0016-2.1: DB file permission checks in doctor.
- WP-0016-2.2: insecure config posture warnings.
- WP-0016-2.3: startup hard-fail for critical insecurity.

**WP-0016-3 Objective:** security regression tests.
- WP-0016-3.1: redaction tests.
- WP-0016-3.2: misconfiguration tests.
- WP-0016-3.3: log scanning checks.

**Dependencies:** ADR-0003, ADR-0015.
**Exit criteria:** no known secret leakage path in normal/exception flows.

### 48.17 Work Package Set for ADR-0017 (CLI Contract)

**WP-0017-1 Objective:** command model and help contract.
- WP-0017-1.1: define command tree.
- WP-0017-1.2: define shared option semantics.
- WP-0017-1.3: define exit code taxonomy.

**WP-0017-2 Objective:** command implementation skeleton.
- WP-0017-2.1: `start` command wiring.
- WP-0017-2.2: `migrate` command wiring.
- WP-0017-2.3: `seed` command wiring.
- WP-0017-2.4: `doctor` command wiring.
- WP-0017-2.5: `config` command wiring.

**WP-0017-3 Objective:** command verification.
- WP-0017-3.1: help snapshot tests.
- WP-0017-3.2: argument validation tests.
- WP-0017-3.3: JSON output schema tests.

**Dependencies:** ADR-0003, ADR-0005.
**Exit criteria:** stable operator contract with deterministic outputs.

### 48.18 Work Package Set for ADR-0018 (Health + Readiness)

**WP-0018-1 Objective:** liveness endpoint.
- WP-0018-1.1: implement fast `/health` endpoint.
- WP-0018-1.2: include runtime version metadata.
- WP-0018-1.3: keep endpoint side-effect free.

**WP-0018-2 Objective:** doctor readiness checks.
- WP-0018-2.1: DB connectivity and migration state.
- WP-0018-2.2: config/secrets sanity checks.
- WP-0018-2.3: external dependency optional checks.

**WP-0018-3 Objective:** consistency and docs.
- WP-0018-3.1: align health and doctor semantics.
- WP-0018-3.2: publish ops runbook.

**Dependencies:** ADR-0004, ADR-0015.
**Exit criteria:** clear distinction between liveness and readiness surfaces.

### 48.19 Work Package Set for ADR-0019 (Quality Gates)

**WP-0019-1 Objective:** gate definition by scope.
- WP-0019-1.1: planning-only gate set.
- WP-0019-1.2: code-change gate set.
- WP-0019-1.3: release-candidate gate set.

**WP-0019-2 Objective:** automation.
- WP-0019-2.1: reusable gate scripts or command docs.
- WP-0019-2.2: CI workflow alignment.
- WP-0019-2.3: failure artifact retention policy.

**WP-0019-3 Objective:** flake management.
- WP-0019-3.1: define flake detection criteria.
- WP-0019-3.2: define quarantine and fix SLA.

**Dependencies:** ADR-0015, ADR-0017.
**Exit criteria:** contributors run same deterministic gate stack.

### 48.20 Work Package Set for ADR-0020 (Decision Freeze)

**WP-0020-1 Objective:** decision review preparation.
- WP-0020-1.1: summarize proposed vs accepted ADRs.
- WP-0020-1.2: collect unresolved risks and assumptions.
- WP-0020-1.3: collect supersession candidates.

**WP-0020-2 Objective:** freeze operation.
- WP-0020-2.1: mark accepted ADR baseline.
- WP-0020-2.2: publish freeze date and scope.
- WP-0020-2.3: link freeze to bead gate conditions.

**WP-0020-3 Objective:** post-freeze governance.
- WP-0020-3.1: define supersession checklist.
- WP-0020-3.2: define drift audit cadence.
- WP-0020-3.3: define emergency exception path.

**Dependencies:** ADR set 0001..0019 review complete.
**Exit criteria:** architecture baseline stable and auditable.

### 48.21 Cross-ADR Dependency Overlay (Execution Order)

- Cluster 1 (boot): ADR-0001, ADR-0002, ADR-0003.
- Cluster 2 (data core): ADR-0004, ADR-0005, ADR-0006, ADR-0008.
- Cluster 3 (logic): ADR-0007, ADR-0009.
- Cluster 4 (interaction): ADR-0010, ADR-0011, ADR-0012.
- Cluster 5 (governance/integration): ADR-0013, ADR-0014.
- Cluster 6 (ops/security): ADR-0015, ADR-0016, ADR-0017, ADR-0018, ADR-0019.
- Cluster 7 (freeze): ADR-0020.

Critical path summary:
- 0001 -> 0003 -> 0004 -> 0006 -> 0007 -> 0012 -> 0015 -> 0019 -> 0020.

Parallel tracks:
- 0017 and 0018 can advance after 0003/0004/0015 are stable.
- 0013 and 0014 can advance once 0011 and 0012 interfaces are fixed.


---

## 49. Canonical Field Dictionary (Draft v1)

Purpose: provide field-level consistency across core domain, DB schema, audit events, and integrations.

### 49.1 Quote Header Fields

- `quote_id`: stable unique identifier for quote aggregate.
- `quote_version`: monotonic version counter for optimistic concurrency.
- `quote_status`: lifecycle state enum.
- `account_id`: external customer/account reference.
- `opportunity_id`: upstream opportunity reference.
- `owner_user_id`: sales owner identity.
- `owner_team_id`: owning team identity.
- `currency_code`: ISO-like currency code used for monetary arithmetic.
- `price_book_id`: selected price book reference.
- `policy_snapshot_id`: policy version applied for latest pricing.
- `config_snapshot_id`: configuration/rule snapshot reference.
- `created_at`: quote creation timestamp.
- `updated_at`: last mutation timestamp.
- `expires_at`: quote expiration timestamp.
- `approved_at`: timestamp final required approval completed.
- `finalized_at`: timestamp quote finalized.
- `rejected_at`: timestamp quote rejected.
- `rejection_reason_code`: normalized rejection code.
- `rejection_reason_text`: human rationale text.
- `total_list_price`: aggregate list price before adjustments.
- `total_net_price`: aggregate net price after adjustments.
- `total_discount_amount`: absolute discount total.
- `total_discount_pct`: aggregate discount percentage representation.
- `total_margin_amount`: computed margin amount.
- `total_margin_pct`: computed margin percentage.
- `approval_required`: boolean flag for approval requirement.
- `approval_state`: state of approval workflow.
- `thread_id`: Slack thread reference.
- `channel_id`: Slack channel reference.
- `workspace_id`: Slack workspace/team reference.
- `source_command`: initial command or trigger type.
- `correlation_id`: trace correlation ID for lifecycle.
- `operation_id`: latest operation key affecting quote.
- `is_amendment`: flag indicating amendment lineage.
- `parent_quote_id`: source quote for clone/amend flow.
- `line_count`: number of active quote lines.
- `notes_internal`: internal operator notes.
- `notes_customer`: customer-visible notes.
- `terms_template_id`: quote terms template reference.
- `pdf_artifact_id`: generated document artifact reference.

### 49.2 Quote Line Fields

- `quote_line_id`: unique line identifier.
- `quote_id`: parent quote reference.
- `line_number`: deterministic ordering number.
- `product_id`: canonical product reference.
- `product_revision`: product revision or version.
- `sku`: stock keeping identifier.
- `product_name`: display name snapshot.
- `quantity`: ordered quantity.
- `uom`: unit of measure.
- `list_unit_price`: base unit list price.
- `net_unit_price`: final unit net price.
- `list_line_total`: list total.
- `net_line_total`: net total.
- `discount_amount`: absolute discount amount.
- `discount_pct`: discount percentage.
- `margin_amount`: line margin amount.
- `margin_pct`: line margin percentage.
- `price_formula_id`: formula reference used if applicable.
- `price_rule_trace_ref`: pointer to trace segment.
- `constraint_state`: validation state for line configuration.
- `constraint_violation_count`: number of violations.
- `is_optional`: whether line is optional.
- `is_bundled`: whether line belongs to bundle.
- `bundle_parent_line_id`: parent line for bundle hierarchy.
- `metadata_json`: extensible structured metadata.
- `created_at`: line creation time.
- `updated_at`: line update time.
- `is_deleted`: soft-delete flag.
- `delete_reason_code`: reason for soft deletion.
- `source_intent_fragment`: normalized origin intent snippet.

### 49.3 Product and Catalog Fields

- `product_id`: primary key.
- `product_revision`: revision identifier.
- `product_status`: active/inactive/deprecated.
- `product_family`: grouping taxonomy.
- `product_segment`: segment classification.
- `display_name`: current display name.
- `description`: product description text.
- `default_uom`: default unit of measure.
- `default_list_price`: baseline list price.
- `min_quantity`: minimum order quantity.
- `max_quantity`: maximum order quantity.
- `requires_approval_flag`: product-specific approval trigger.
- `constraint_profile_id`: reference to constraints profile.
- `pricing_profile_id`: reference to pricing profile.
- `policy_profile_id`: reference to policy profile.
- `effective_from`: validity start timestamp.
- `effective_to`: validity end timestamp.
- `is_sellable`: sellability flag.
- `is_bundle`: bundle capability flag.
- `bundle_rule_set_id`: bundle rules reference.

### 49.4 Rule and Policy Fields

- `rule_id`: unique rule identifier.
- `rule_type`: constraint/pricing/policy/approval.
- `rule_stage`: deterministic stage for evaluation.
- `rule_priority`: tie-break priority inside stage.
- `rule_name`: human-readable name.
- `rule_description`: explanation text.
- `match_expression`: serialized predicate expression.
- `action_expression`: serialized action expression.
- `conflict_strategy`: behavior for conflicts.
- `is_active`: activation flag.
- `effective_from`: activation start.
- `effective_to`: activation end.
- `author_user_id`: rule author identifier.
- `change_ticket`: external change reference.
- `last_validated_at`: last validation timestamp.
- `validation_status`: lint/verification status.
- `risk_level`: low/medium/high rule risk.
- `requires_dual_review`: governance flag.
- `approval_threshold_ref`: threshold matrix reference.
- `metadata_json`: optional extension data.

### 49.5 Approval Workflow Fields

- `approval_request_id`: unique approval ID.
- `quote_id`: associated quote.
- `approval_policy_version`: policy snapshot version.
- `approval_route_id`: resolved route reference.
- `approval_stage`: current stage index.
- `required_approver_count`: required approvals count.
- `received_approver_count`: completed approvals count.
- `current_approver_user_id`: active approver.
- `fallback_approver_user_id`: fallback identity.
- `escalation_after_secs`: escalation timeout.
- `submitted_at`: submission timestamp.
- `escalated_at`: escalation timestamp.
- `approved_at`: final approval timestamp.
- `denied_at`: denial timestamp.
- `approval_state`: pending/approved/denied/escalated.
- `approval_reason_text`: optional approver rationale.
- `approval_decision_payload`: structured decision context.
- `slack_action_message_ts`: Slack action message reference.
- `slack_action_channel_id`: Slack channel for action message.
- `idempotency_key`: action dedupe key.

### 49.6 Audit Event Fields

- `audit_event_id`: unique event identifier.
- `event_type`: domain event category.
- `event_version`: schema version for event payload.
- `event_ts`: event timestamp.
- `quote_id`: related quote.
- `quote_version`: quote version at event time.
- `actor_type`: user/system/agent/adapter.
- `actor_id`: actor identifier.
- `source_component`: component creating event.
- `correlation_id`: trace correlation ID.
- `operation_id`: idempotent operation key.
- `causation_id`: causal predecessor event ID.
- `event_payload_json`: structured event data.
- `before_snapshot_hash`: optional hash of pre-state.
- `after_snapshot_hash`: optional hash of post-state.
- `policy_snapshot_id`: policy snapshot used.
- `rule_trace_ref`: pointer to trace object.
- `result_code`: success/failure code.
- `result_message`: brief summary.
- `sensitivity_level`: data classification label.

### 49.7 Trace and Telemetry Fields

- `trace_id`: distributed trace identifier.
- `span_id`: span identifier.
- `parent_span_id`: parent span.
- `span_name`: semantic span name.
- `span_kind`: ingress/internal/egress.
- `start_ts`: span start.
- `end_ts`: span end.
- `duration_ms`: derived duration.
- `status_code`: ok/error.
- `error_code`: normalized error identifier.
- `service_name`: service/component name.
- `component_name`: module-level identifier.
- `event_count`: number of log events in span.
- `quote_id`: business context correlation.
- `operation_id`: operation correlation.
- `thread_id`: Slack thread context.
- `channel_id`: Slack channel context.
- `db_tx_id`: database transaction correlation.
- `retry_count`: retry attempt counter.
- `rate_limit_wait_ms`: rate-limit wait duration.

### 49.8 Idempotency Ledger Fields

- `operation_id`: unique operation key.
- `operation_type`: command/action category.
- `operation_source`: ingress source type.
- `operation_hash`: semantic payload hash.
- `first_seen_ts`: first observation timestamp.
- `last_seen_ts`: latest duplicate timestamp.
- `attempt_count`: duplicate attempt count.
- `operation_state`: reserved/running/completed/failed.
- `result_ref`: pointer to result snapshot.
- `error_ref`: pointer to failure detail.
- `expires_at`: ledger cleanup horizon.
- `quote_id`: associated aggregate.
- `correlation_id`: trace correlation.
- `created_by_component`: component reserving key.
- `updated_by_component`: component updating state.

---

## 50. Research Question Backlog (for Deep Follow-On Investigation)

This question set is designed for ongoing architecture hardening. Each question can become a bead if unresolved.

### 50.1 Runtime and Concurrency Questions

1. What is the maximum acceptable graceful shutdown duration before force termination?
2. Should background workers use bounded channel fan-out or task-per-event model?
3. What backpressure strategy should be used when Slack event bursts exceed processing throughput?
4. Should we isolate Slack ingress and command execution onto separate runtimes/processes later?
5. What metrics threshold should trigger concurrency model redesign?
6. How should we prioritize command classes under load (quote mutate vs informational)?
7. Should cancellation be cooperative only, or include hard timeout abort fallback at component level?
8. How do we test race conditions deterministically in CI?
9. What is the strategy for long-running operations that exceed Slack UX expectations?
10. Should we adopt deterministic scheduler simulation for replay tests?
11. Which components are safe to restart independently in future multi-process deployment?
12. What queue semantics are needed before moving beyond single-process architecture?
13. How should task supervision failures be escalated to operator alerts?
14. Which runtime tasks are business-critical vs best-effort?
15. Should we add per-task resource quotas for memory/CPU in-process?
16. What instrumentation is needed to attribute latency to runtime queues vs business logic?
17. How should we model and test cancellation at each boundary layer?
18. Is there value in command prioritization by quote lifecycle state?
19. How should we ensure deterministic ordering when events share same timestamp?
20. Should monotonic sequence counters be added per quote stream?

### 50.2 Data and Persistence Questions

21. Which tables must be append-only for audit integrity?
22. Should we use soft deletes everywhere or hard deletes for derived caches only?
23. What retention policy should apply to idempotency ledger rows?
24. How do we compact audit data without losing explainability?
25. Should we snapshot quote state periodically for replay performance?
26. What indexing strategy minimizes write penalties while preserving query speed?
27. How should schema versioning interact with feature flags?
28. Which migrations require explicit downtime/maintenance mode?
29. Should migration validation include row-count or checksum assertions?
30. What DB backup frequency is required for local-first workflows?
31. How do we secure backup artifacts at rest in local environments?
32. Should we store policy snapshots as immutable blobs per quote mutation?
33. What is the minimum viable data integrity checker for doctor command?
34. How do we detect accidental direct DB edits outside migration flow?
35. Should we enforce strict mode table definitions for key tables?
36. How should decimal values be constrained in schema to avoid malformed precision?
37. What strategy handles timezone correctness across local-first deployments?
38. Should timestamps be UTC-only with explicit serialization format constraints?
39. Do we need archival partitions for large audit/event tables early?
40. How should we measure and cap DB growth over time?

### 50.3 Domain Model Questions

41. Which domain events are mandatory for every quote mutation?
42. What is the canonical boundary between quote and approval aggregates?
43. Should quote line mutation semantics support patch operations or full replace only?
44. How should bundle decomposition be represented for deterministic pricing?
45. What is the policy for line item ordering and renumbering?
46. Should product revision be pinned at line creation or repriced dynamically?
47. How do we represent customer-specific overrides without polluting global catalog?
48. Should policy violations be modeled as first-class value objects?
49. What rules determine when a quote becomes immutable?
50. How do we encode amendment lineage to preserve legal traceability?
51. Which domain invariants should be compile-time encoded vs runtime validated?
52. How should we represent partial quote drafts in a way that preserves invariants?
53. Should we model staged validation states explicitly per quote line?
54. What is the canonical source for deal context metadata used in approvals?
55. How do we prevent stale context from previous thread turns contaminating state?
56. What is the minimal deterministic payload needed for replay correctness?
57. Should domain entities carry explicit provenance metadata for each field?
58. How should we handle unknown or future enum values in persisted records?
59. Which fields are safe for user-editable notes vs system-managed metadata?
60. What is the domain policy for quote cloning with policy changes?

### 50.4 Pricing and Rules Questions

61. Should pricing engine support expression language now or fixed strategy implementations first?
62. How should we lint rule conflicts pre-activation?
63. What tie-break strategy is preferred when equal priority rules collide?
64. Should discount caps apply at line level, quote level, or both?
65. How do we represent cumulative vs exclusive discounts safely?
66. Which pricing operations require fixed rounding direction vs bankers rounding?
67. How do we expose price trace to users without leaking sensitive internal policy details?
68. Should we permit custom formulas per customer for alpha?
69. What sandboxing is needed if formulas are user-defined?
70. How should rule activation windows be validated against overlapping schedules?
71. What fallback behavior applies when no rule matches a required stage?
72. Should policy engine return first violation only or all violations?
73. How do we ensure deterministic evaluation when rules query external context?
74. What is the cache policy for frequently used price books?
75. How should we test pricing determinism across Rust/compiler upgrades?
76. What numeric overflow safeguards are required for extreme quantities?
77. Should we enforce max quote size bounds in foundation?
78. Which pricing metrics should be emitted for performance profiling?
79. How do we detect anomalous discounts relative to historical norms?
80. Should we provide explainability templates per rule family?

### 50.5 Slack and UX Interaction Questions

81. What is the canonical grammar subset for v1 slash commands?
82. How should we map free-form text intents into deterministic command intents?
83. What confidence threshold triggers user confirmation?
84. How many clarification turns before escalation to structured modal?
85. Should thread lifecycle states be visible to users explicitly?
86. How do we handle edited/deleted Slack messages in quote context?
87. Should quote state updates always post in-thread, or only on major transitions?
88. What is the retry strategy for outbound Slack API failures?
89. Should we coalesce noisy status updates to reduce channel spam?
90. How do we present policy violations with actionable remediation in Slack?
91. What accessibility conventions should be enforced in Block Kit layouts?
92. Should approval actions use ephemeral or persistent messages?
93. How do we prevent action buttons from being reused after state changes?
94. What localization support should be considered now vs deferred?
95. How should bot responses differ between DM and channel context?
96. What security checks are required for user identity mapping in approvals?
97. Should we support thread handoff between users/teams?
98. How do we model and display quote timeline summaries in Slack?
99. Should the bot proactively suggest next actions after each transition?
100. What anti-confusion cues are needed when multiple quotes are active in one channel?

### 50.6 Reliability and Recovery Questions

101. Which operations require exactly-once semantics vs at-least-once tolerance?
102. How should idempotency keys be versioned if payload schema changes?
103. What is the retention period for idempotency rows?
104. How should we recover from partial external side effects?
105. Should we implement compensating actions now or defer until adapters mature?
106. What is acceptable duplicate suppression false-positive rate?
107. How do we surface replay/dedup behavior to operators?
108. Should replay responses include prior result references for transparency?
109. What fault injection scenarios are mandatory in CI?
110. How do we test crash recovery without flakiness?
111. Should operation ledger writes be in same transaction as quote updates?
112. How do we guarantee causal ordering of audit events?
113. What is the policy for retrying failed policy evaluations?
114. How should we classify transient vs terminal errors consistently?
115. Which error categories should trigger automatic circuit-breaking behavior?
116. Should we gate Slack reconnection attempts with jitter and caps?
117. What metrics threshold triggers paging or high-priority alerts?
118. How do we detect slow-burn reliability regressions early?
119. What are the minimal SLOs for alpha reliability claims?
120. How do we define and measure "operationally recoverable" incidents?

### 50.7 Security and Compliance Questions

121. What secret rotation workflow is required for Slack and LLM tokens?
122. Should we support encrypted config files locally?
123. What threat model assumptions are valid for local-first deployment?
124. Which audit fields are required for compliance storytelling in enterprise sales?
125. How do we secure temporary artifacts (PDFs, exports, logs)?
126. What default file permissions should be enforced for artifact directories?
127. How should we redact sensitive data in approval rationale text?
128. Should we support PII tagging in audit events now?
129. What log retention and scrubbing policy is required?
130. How do we detect secrets accidentally committed to repository?
131. Should `doctor` include static secret-pattern scans in config files?
132. What identity assurances are needed for approver actions in Slack?
133. How do we handle deprovisioned users still present in approval policies?
134. Should we require dual-control for high-impact policy updates?
135. What policy data integrity checks are required on import?
136. How do we ensure least-privilege for external adapter credentials?
137. What incident response runbook is needed for token compromise?
138. How should we report security posture in operator diagnostics?
139. Which compliance frameworks should influence v1 design boundaries?
140. What is the minimal audit export format for external review?

### 50.8 Observability and Operations Questions

141. Which fields are mandatory on every span across all components?
142. Should span names encode lifecycle stage explicitly?
143. What metric cardinality limits are needed to avoid blow-up?
144. How do we balance debugging detail with log volume costs?
145. Which events should emit counters vs histograms vs logs only?
146. Should we add synthetic transactions for continuous health validation?
147. What operator dashboards are minimum viable for alpha?
148. How do we correlate Slack request identifiers with internal trace IDs?
149. What is the best strategy to capture and replay production incidents locally?
150. Should we persist trace summaries in SQLite for local troubleshooting?
151. How should we expose top error classes in doctor output?
152. What is acceptable telemetry overhead budget?
153. How do we monitor and tune DB contention over time?
154. Should we include structured event IDs for all major user-visible messages?
155. How do we detect silent failures where no explicit error is emitted?
156. What runbook links should be embedded in diagnostic outputs?
157. Should we include build metadata and git SHA in every startup log?
158. How should we version telemetry schemas across releases?
159. What tests guarantee observability continuity after refactors?
160. How do we enforce no-PII logging policy automatically?

### 50.9 Integration and Adapter Questions

161. Which CRM operations are mandatory for initial utility?
162. What consistency model do we need between local quote state and CRM records?
163. Should sync be synchronous in user request path or asynchronous task-based?
164. How do we present partial sync failures to users without confusion?
165. What adapter timeout and retry defaults are safe?
166. Should adapter failures block quote finalization for alpha?
167. How do we support offline mode when adapter unavailable?
168. What contract tests ensure fixture adapter fidelity over time?
169. How do we detect adapter API schema drift quickly?
170. Should adapter mapping include strict unknown-field handling?
171. What error taxonomy should adapter surface to core?
172. How do we avoid leaking vendor semantics into audit language?
173. Should we store external system IDs in separate mapping tables?
174. What reconciliation job is needed for drift correction?
175. Which operations should be idempotent at adapter boundary?
176. How do we stage future non-Composio adapters cleanly?
177. What integration observability fields are mandatory for troubleshooting?
178. Should we support sandbox/prod adapter profiles in config?
179. How do we prevent noisy adapter retries from overwhelming external APIs?
180. What security scope checks are needed for adapter credentials?

### 50.10 Delivery Process and Governance Questions

181. Which ADRs must be accepted before first user pilot?
182. What constitutes sufficient evidence to mark ADR from proposed to accepted?
183. How often should architecture drift audits run?
184. Who signs off on superseding ADRs affecting determinism boundaries?
185. How do we keep bead graph and ADR index synchronized automatically?
186. What process handles emergency architectural exceptions during incidents?
187. Should decision freeze be per release wave or global?
188. How do we prevent process overhead from blocking practical delivery?
189. What minimal documentation is mandatory for every architecture-impacting change?
190. How should we encode architecture ownership by module/team?
191. What triggers a mandatory design review vs lightweight review?
192. Should we maintain architecture decision changelog separate from code changelog?
193. How do we track decision debt explicitly?
194. What metrics show architecture process is helping instead of hindering?
195. How do we sunset obsolete ADRs cleanly?
196. How should research findings from incidents feed back into ADR updates?
197. What training material is needed for new contributors to follow ADR workflow?
198. How do we standardize terminology across docs and code?
199. What rules govern adding new top-level modules/files?
200. How do we preserve execution velocity while maintaining design rigor?

---

## 51. Command Cookbook for Architecture Validation

These commands are intended as repeatable checkpoints during implementation.

### 51.1 Workspace and Build Baseline

- `cargo build --workspace`
- `cargo fmt -- --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

### 51.2 Database and Migration Checks

- `quotey migrate`
- `quotey doctor`
- `sqlite3 <db-file> 'PRAGMA foreign_keys;'`
- `sqlite3 <db-file> 'PRAGMA journal_mode;'`

### 51.3 Runtime and Shutdown Checks

- `quotey start`
- send SIGINT and verify graceful shutdown logs
- rerun `quotey start` to confirm clean restart

### 51.4 Slack and Interaction Smoke Checks

- run local bot with test workspace credentials
- execute `/quote` basic command
- verify thread response and state creation
- repeat same action to validate dedupe behavior

### 51.5 Quality and Supply Chain Checks

- `cargo nextest run`
- `cargo deny check`
- `ubs $(git diff --name-only --cached)`

### 51.6 Planning and Tracking Sync

- `br ready --json`
- `br dep cycles --json`
- `br lint --json`
- `br sync --flush-only`

### 51.7 Suggested pre-push sequence (code changes)

1. `cargo fmt -- --check`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace`
4. `cargo nextest run` (if suite established)
5. `cargo deny check`
6. `ubs $(git diff --name-only --cached)`
7. `br sync --flush-only`
8. `git push`


---

## 52. Final Alignment Checklist Before Marking Research Complete

- Confirm ADR IDs in this dossier map to active bead IDs.
- Confirm critical-path ADRs are either accepted or explicitly trial-gated.
- Confirm unresolved high-risk questions have beads or deferred decisions.
- Confirm command cookbook reflects actual repository tooling.
- Confirm security redaction requirements are reflected in implementation tasks.
- Confirm idempotency and replay contracts are represented in test plan.
- Confirm observability schema includes correlation across Slack, flow, pricing, and approvals.
- Confirm migration and schema assumptions match foundation implementation plan.
- Confirm runtime shutdown behavior has explicit acceptance criteria.
- Confirm this dossier is referenced from next planning handoff.
- Confirm multi-agent contributors know supersession protocol for post-freeze changes.
- Confirm quality gates are documented for planning-only changes and code changes.
- Confirm decision freeze criteria reference `bd-3d8.11.10` and `bd-3d8.11.11`.
- Confirm no unresolved architectural ambiguity blocks foundation scaffold start.
- Confirm timeline and phase ordering remains realistic for available capacity.
- Confirm this research package is pushed so all agents can consume it.

- Confirm source index links remain reachable and current.
- Confirm rejected patterns are communicated to all implementation agents.
- Confirm field dictionary aligns with upcoming schema migrations.
- Confirm scenario catalog coverage includes all foundation gate acceptance criteria.
- Confirm FMEA high-RPN items are prioritized in immediate sprint planning.
- Confirm architecture metrics are tracked from first runnable scaffold.
- Confirm ADR template is copied for all new architecture-impacting work.
- Confirm this checklist is rerun at each decision-freeze checkpoint.
