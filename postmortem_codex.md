# Ultimate Premortem + Revised Hybrid Execution Blueprint (W2/W3)

Date: 2026-02-25  
Author: Codex (revised from prior hybrid draft)  
Track: W2_CLO / W2_NXT / W3_RGN  
Goal: prevent the current plan from failing in production within the first 6 months.

## 0) Premortem context and method

This revision is based on:
- current `postmortem_codex.md` baseline,
- `.planning/PREMORTEM_ANALYSIS.md`,
- `.planning/PREMORTEM_TODO.md`,
- `.planning/W2_CLO_CLOSED_LOOP_POLICY_OPTIMIZER_SPEC.md`,
- `.planning/W2_NXT_DETERMINISTIC_NEGOTIATION_AUTOPILOT_SPEC.md`,
- `.planning/W3_RGN_DEAL_AUTOPSY_REVENUE_GENOME_SPEC.md`,
- repository guardrails in `AGENTS.md`, `CLAUDE.md`, and `.planning/PROJECT.md`.

I treated your mention of three external plan versions as a perspective signal and incorporated the kinds of strengths they usually represent, but I can only compare against artifacts present in this repository during this session. The comparison below is therefore explicit and honest about what is directly observable.

## 1) What likely failed at month 6 (failure tree)

### 1.1 Safety posture drifted from actual operator behavior

Assumption that failed:
- Deterministic engines plus policy checks equal reliable human trust.

Observed risks:
- deterministic checks were mathematically strict but cognitively opaque,
- humans interpreted recommendations as authoritative even with confidence drift,
- approval packets were dense and failed to preserve a clear decision model in the operator’s head.

Likely break:
- noisy advisory outputs looked like policy pressure to approve,
- silent fallback from unknown context to default deterministic behavior,
- users developed “override fatigue” and bypassed controls.

### 1.2 Replay parity became brittle rather than resilient

Assumption that failed:
- byte-level replay identity is always the right gate.

Observed risks:
- floating-point nondeterminism and timestamp granularity differences,
- environment/adapter drift and stale external states causing checksum churn,
- partial snapshots entering scoring paths.

Likely break:
- normal traffic caused repeated “replay mismatch” incidents,
- teams started to treat errors as product defect even when semantics were valid,
- quality teams accepted “least resistance” patches that reduced confidence rather than fixing root causes.

### 1.3 Integration seams were under-tested in sequence

Assumption that failed:
- unit tests and module-level contracts are enough.

Observed risks:
- cross-crate call ordering, retries, and eventual consistency edges,
- race conditions during active negotiation edits,
- migration-dependent assumptions only discovered after users are live.

Likely break:
- production errors in flow transitions and approval routing despite green module tests,
- Slack-visible behavior diverging from engine truth,
- high time-to-recover incidents.

### 1.4 Evidence quality and provenance were treated as “available” rather than “trusted”

Assumption that failed:
- if audit-like data exists, it is trustworthy enough.

Observed risks:
- forged or malformed provenance paths,
- unproven historical attribution used in causal ranking,
- snapshot lineage gaps during replay and re-run.

Likely break:
- candidate quality inflation from weak cohorts,
- bad attributions fed into CLO/NXT/RGN,
- enterprise users lose trust when recommendations conflict with outcomes.

### 1.5 Storage and concurrency assumptions were too optimistic

Assumption that failed:
- local SQLite plus WAL is sufficient for real-team concurrency.

Observed risks:
- write contention in multi-rep bursts,
- long-running event processing and lock extension,
- no graceful degradation path from local write overload.

Likely break:
- 95th percentile latency spikes and user-visible stalls,
- dropped events under burst load,
- escalations requiring manual reruns.

### 1.6 Scope overfit to feature completeness, not usefulness

Assumption that failed:
- comprehensive feature specs imply implementation maturity.

Observed risks:
- too many “perfect-first” ideas before a useful minimum product slice,
- NXT and analytics layers overdesigned while operator-facing outcomes were weak.

Likely break:
- slow launch with uncertain utility,
- low adoption among top-funnel sales users,
- governance pushback due to non-actionable output.

## 2) Assumptions that were false (explicitly)

- “Atomic deterministic behavior can be validated from unit/integration tests alone.”
- “Replay exactness can be a hard binary without meaningful operational thresholds.”
- “Any audit record is equally good evidence.”
- “SQLite alpha success implies production readiness.”
- “Slack is a complete surface for all complex negotiation states.”
- “Cohort sufficiency can be inferred, not enforced.”
- “Approval routing and explainability can be layered after candidate generation.”

## 3) Missed edges and integration faults (high-confidence misses)

- Duplicate Slack actions and re-entrant callbacks entering the same quote session.
- Active editing of quote context while NXT decision loop is mid-flight.
- Replay payload carrying stale or partially canonicalized policy versions.
- Orphaned dependency chains in evidence graphs after session cancellation.
- Cohorts below statistical mass passing into CLO candidate generation.
- Boundary approvals emitted without explicit escalation path for context staleness.
- CRM/persona context unavailable in critical moments of negotiation.
- Concurrency pressure pushing queue time beyond human tolerance thresholds.
- Operator dashboards and Slack states diverging due to stale caches.

## 4) What was stronger in other perspectives

From the observed planning body and historical premortems, the strongest ideas are:

- **Safety-first foundation first**: do not open production-facing feature gates until integration and migration gates are green.
- **Operationally bounded intelligence**: enforce confidence/cohort thresholds and route low-confidence output to advisory mode.
- **Pragmatic delivery sequencing**: ship a minimum viable deterministic core per track before extending optimization layers.
- **Traceable explainability**: short human summary first, evidence second.
- **Runbook-first rollout**: explicit rollback and kill-switch procedures before enablement.

## 5) Revised plan architecture

This plan integrates all three classes of strengths:
- hard correctness from safety-first approaches,
- practical delivery from lean MVP sequencing,
- enterprise survival from operational and observability discipline.

### 5.1 Non-negotiable principles

- Deterministic CPQ, policy, pricing, and approvals remain the system of truth.
- Evidence must be both **complete** and **provenanced**.
- Every non-trivial recommendation includes confidence + uncertainty semantics.
- No feature path ships without explicit end-to-end/transition gates.
- Any output that cannot be explained in <10 seconds is downgraded by default.

### 5.2 Foundation invariants

- Quote state machine transitions are idempotent and externally resumable.
- Pricing calculations are integer-fixed-point in critical paths unless explicit reasoned tolerance is declared.
- Replay checks are split:
  - structural parity (strict),
  - numeric parity (tolerance-gated with policy-owned bounds),
  - provenance parity (must match source-of-truth records).
- All recommendation classes must include:
  - source IDs,
  - decision rationale,
  - confidence grade,
  - bounded risk label.

## 6) Hybrid implementation plan (self-contained, dependency graph included)

The plan below is intentionally granular and includes dependency structure for handoff.

### EPIC A: Foundation hardening and correctness gates

#### A1 — End-to-end `/quote` integration corridor (P0, highest priority)
- **ID:** `A1`
- **Goal:** prove real user flow works before feature growth.
- **Subtasks:**
  - A1.1 Establish a deterministic test corpus for command parse, domain transformation, policy checks, pricing, approvals, and output rendering.
  - A1.2 Add explicit concurrency-path variants:
    - duplicate callback,
    - retry burst,
    - stale external CRM snapshot,
    - half-failed attachment export.
  - A1.3 Add assertions for idempotent retry signatures.
- **Dependencies:** none.
- **Exit condition:** all integration test variants pass; no silent state divergence in session timeline.

#### A2 — Migration safety harness (P0)
- **ID:** `A2`
- **Goal:** prevent schema drift from breaking running workloads.
- **Subtasks:**
  - A2.1 Up-chain bootstrap test from empty DB.
  - A2.2 `up;down;up` for the last migration window.
  - A2.3 Migration dependency map auto-generated in docs and CI artifacts.
- **Dependencies:** `A1`.
- **Exit condition:** migration safety suite passes in CI; documented migration checkpoints.

#### A3 — Concurrency and lock profile baseline (P0/P1)
- **ID:** `A3`
- **Goal:** define and enforce a realistic storage concurrency envelope.
- **Subtasks:**
  - A3.1 15-20 concurrent `/quote` writers benchmark.
  - A3.2 Measure lock wait histograms, timeout tails, and queue growth.
  - A3.3 Establish documented ceiling and auto-degradation policy.
- **Dependencies:** `A2`.
- **Exit condition:** publish SLO and enforce guardrails in config.

#### A4 — Numeric integrity standard (P0)
- **ID:** `A4`
- **Goal:** eliminate floating drift failure mode in pricing policy decisions.
- **Subtasks:**
  - A4.1 Inventory every money/margin/discount path.
  - A4.2 Convert to basis-point or fixed-point representation where economically relevant.
  - A4.3 Define tolerance table for parity checks.
  - A4.4 Add canonical rounding policy tests.
- **Dependencies:** `A1`.
- **Exit condition:** deterministic output parity tests demonstrate numeric stability across runtime/platform variants.

#### A5 — Replay provenance hardening (P1)
- **ID:** `A5`
- **Goal:** ensure replay data is trustworthy, not merely present.
- **Subtasks:**
  - A5.1 Snapshot lineage checks against canonical ledger IDs.
  - A5.2 Window integrity checks (time ranges and quote ownership).
  - A5.3 Replay rejection pathways with explicit remediation message.
- **Dependencies:** `A4`.
- **Exit condition:** blocked candidate generation on untrusted snapshots; all rejections logged and traceable.

### EPIC B: Decision confidence and trust envelope

#### B1 — Confidence taxonomy and routing policy (P0)
- **ID:** `B1`
- **Goal:** route recommendations by trust quality.
- **Subtasks:**
  - B1.1 Implement confidence enums: `high`, `normal`, `low`, `insufficient`.
  - B1.2 Define route rules:
    - high -> standard recommendation path,
    - normal -> explicit ack path,
    - low/insufficient -> advisory-only with auto-notification only.
  - B1.3 Add residual-risk labels: sample size, provenance, drift, boundary.
- **Dependencies:** `A5`.
- **Exit condition:** no non-advisory low-quality recommendation can auto-advance.

#### B2 — Human-first summary contract (P0/P1)
- **ID:** `B2`
- **Goal:** make every approval/recommendation understandable in under 10 seconds.
- **Subtasks:**
  - B2.1 Add two-line executive summary in all packets.
  - B2.2 Place summary above evidence lists in UI order.
  - B2.3 Add deterministic rationale links (policy IDs, cohort IDs, trace IDs).
- **Dependencies:** `B1`.
- **Exit condition:** representative users can identify go/no-go rationale from one screen.

#### B3 — Candidate quality gate for CLO (P1)
- **ID:** `B3`
- **Goal:** avoid statistical nonsense during early cold-start.
- **Subtasks:**
  - B3.1 Minimum cohort-size minimums by segment.
  - B3.2 Fallback analytics mode for insufficient cohorts.
  - B3.3 Candidate vs descriptive mode transitions with explicit labeling.
- **Dependencies:** `B1`.
- **Exit condition:** no candidate generated below minimum cohort threshold.

#### B4 — NXT context enrichment baseline (P1)
- **ID:** `N1`  
- **Goal:** stop deterministic-only negotiation outputs from being strategically blind.
- **Subtasks:**
  - N1.1 Define `deal_context` schema in NXT state.
  - N1.2 Add customer and deal-history signals to fallback scoring.
  - N1.3 Add staleness invalidation for context older than configured horizon.
- **Dependencies:** `B1`, `B2`.
- **Exit condition:** recommendation reasons include historical-context evidence when available.

### EPIC C: System resilience and adversarial hardening

#### C1 — Failure-injection matrix and chaos harness (P0/P1)
- **ID:** `C1`
- **Goal:** prove failure behavior before go-live.
- **Subtasks:**
  - C1.1 Duplicate event replay scenarios.
  - C1.2 External timeout and callback duplication.
  - C1.3 Snapshot tamper simulation and malformed audit payloads.
  - C1.4 CRM sync conflict and stale-field tests.
- **Dependencies:** `A1`.
- **Exit condition:** each scenario has deterministic handling and evidence assertions.

#### C2 — Provenance and anti-forgery gates (P1)
- **ID:** `C2`
- **Goal:** block fabricated historical inputs entering optimization logic.
- **Subtasks:**
  - C2.1 Cross-check replay IDs against immutable ledger constraints.
  - C2.2 Reject provenance holes.
- **Dependencies:** `A5`, `C1`.
- **Exit condition:** forged or incomplete cohorts are fully blocked with operator-visible reason.

#### C3 — RGN graph retention and recursion controls (P2/P1)
- **ID:** `C3`
- **Goal:** keep attribution and autopsy data bounded and queryable.
- **Subtasks:**
  - C3.1 Set TTL, depth caps, and partition strategy.
  - C3.2 Add bounded-query budgets with fallback summaries.
  - C3.3 Add confidence intervals to all counterfactual outputs.
- **Dependencies:** `C2`.
- **Exit condition:** bounded query latency and reproducible summaries under load.

### EPIC D: Multi-surface UX and operations

#### D1 — Surface split: Slack + negotiated state cockpit (P2)
- **ID:** `D1`
- **Goal:** stop state-loss in long negotiation sessions.
- **Subtasks:**
  - D1.1 Keep Slack as command/notification surface.
  - D1.2 Add read-only deep-state page for active sessions:
    - offer history,
    - concessions and rationale,
    - timeline and pending approvals.
  - D1.3 Add deep-link from Slack to focused state page.
- **Dependencies:** `B2`, `C1`.
- **Exit condition:** no >10 step negotiation thread degrades operator comprehension metrics in test.

#### D2 — Noise shaping and escalation ergonomics (P1)
- **ID:** `D2`
- **Goal:** reduce fatigue and preserve attention.
- **Subtasks:**
  - D2.1 Introduce noise budget and throttling for repetitive micro updates.
  - D2.2 Prioritize critical escalations over advisory churn.
  - D2.3 Add operator controls for verbosity and summary frequency.
- **Dependencies:** `B2`.
- **Exit condition:** reduction in “low-value” messages by configured thresholds.

#### D3 — Runbook + kill-switch execution package (P0)
- **ID:** `D3`
- **Goal:** make rollback and pause paths immediate and clean.
- **Subtasks:**
  - D3.1 Define kill-switch modes:
    - write-freeze,
    - apply-freeze,
    - candidate-freeze,
    - integration-freeze.
  - D3.2 Define escalation matrix for auto-disable by track and reason.
  - D3.3 Pre-runbook drill with evidence capture and audit summary.
- **Dependencies:** `A1`, `A5`.
- **Exit condition:** every mode tested and auditable in staging.

## 7) Full dependency chain (recommended execution order)

1. `A1` → `A2` → `A3`  
2. `A1` → `A4` → `A5`  
3. `A5` → `B1` → `B2` → `B3`  
4. `B1` + `B2` → `N1`  
5. `B2` + `A1` → `C1`  
6. `A5` + `C1` → `C2` → `C3`  
7. `B2` + `C1` → `D1`  
8. `B2` → `D2`  
9. `A1` + `A5` → `D3`

## 8) Acceptance gates (what “done” means)

### Foundation gates
- `A1` passes with replay and transition correctness.
- `A2` migration tests pass and are versioned.
- `A3` documents storage ceiling and enforces behavior.

### Product gates
- `A4` and `A5` are enforced in all recommendation paths.
- `B1` confidence policy prevents low-quality auto-advancement.
- `B2` gives users immediate rationale in every decision surface.
- `B3`, `N1`, `C2` prevent low-trust and unproven advice.

### Delivery gates
- `C1` and `C3` can reproduce failure modes and bounded recovery.
- `D3` is drill-tested before production rollouts.
- `D1` improves negotiation comprehensibility and reduces escalation confusion.

## 9) Immediate next-step plan with explicit task decomposition

### Sprint 1 (6-week minimum)
- `A1`, `A4`, `A2`, `B1`, `D3`.

### Sprint 2
- `A3`, `A5`, `B2`, `C1`.

### Sprint 3
- `B3`, `N1`, `D2`, `C2`.

### Sprint 4
- `C3`, `D1`, harden dependency between tracks.

## 10) Risk register (with monitoring triggers)

- **Risk:** false confidence in numerically equivalent-but-not-structurally-safe outputs.  
  - Trigger: repeated parity mismatch warnings not tied to boundary exceptions.
  - Mitigation: escalate to confidence route and block non-advisory actions.

- **Risk:** tracker backlog growth during rollback windows due duplicate retries.  
  - Trigger: duplicate event count spike.
  - Mitigation: idempotency + de-duplication + queue-level backoff.

- **Risk:** operator fatigue from recommendation volume.  
  - Trigger: low-importance messages > configured ratio.
  - Mitigation: enable noise shaping and summary-first rendering.

- **Risk:** hidden data loss from migration mismatch.  
  - Trigger: migration health check variance across versions.
  - Mitigation: gate rollout until `up;down;up` validates.

## 11) Open decisions requiring owner sign-off

1. Exact numeric tolerance policy by track and function.
2. Confidence thresholds and override conditions.
3. Kill-switch privileges and activation ownership model.
4. The exact minimum cohort floor by segment.
5. Whether advisory-only mode includes machine-generated next-step templates.

## 12) Bead-ready mapping (for execution tooling)

The following identifiers are intended for `br` issue creation with explicit dependencies:

- `A1` through `A5` foundation hardening chain.
- `B1` through `B3` trust and confidence chain.
- `C1` through `C3` resilience chain.
- `D1` through `D3` UX and operations chain.
- `N1` handles NXT-specific context gating and is tied to `B1`.

For each ID, create:
- a parent issue (`epic`) with full context and acceptance.
- child task issues for each numbered subtask.
- dependency edges exactly as listed in Section 7.

## 12.1 Bead graph (self-documenting execution manifest)

Use these records directly in `br` as the source-of-truth execution plan.

- `A1` `epic` / `priority:0`
  - `goal`: prove deterministic correctness end-to-end before optimization features.
  - `why`: every later layer assumes baseline workflow safety.
  - `blocked_by`: none.
  - `done_when`: `/quote` corridor supports policy/price/approval transitions with no silent divergence.
  - child tasks:
    - `A1.1` integration corpus definition.
      - `why`: codify real flow states before any optimization path ships.
      - `done_when`: parse→policy→pricing→approval→render is covered.
    - `A1.2` concurrency variants.
      - `blocked_by`: `A1.1`.
      - `done_when`: duplicate callback/retry/bad-export handling are deterministic.
    - `A1.3` idempotent retry assertions.
      - `blocked_by`: `A1.2`.
      - `done_when`: replayed actions produce one canonical state delta.

- `A2` `epic` / `priority:0`
  - `goal`: ensure schema evolution cannot break active flows.
  - `why`: migration defects are the fastest path from green tests to production regressions.
  - `blocked_by`: `A1`.
  - `done_when`: `up`, `down`, `up` migration safety tests pass.
  - child tasks:
    - `A2.1` bootstrap-from-empty test.
    - `A2.2` reversible migration window test.
    - `A2.3` migration checkpoint docs generation.

- `A3` `epic` / `priority:1`
  - `goal`: establish and enforce realistic concurrency envelopes.
  - `why`: local-first and SQLite defaults can fail under real bursts.
  - `blocked_by`: `A2`.
  - `done_when`: lock and queue SLOs are enforced in config.
  - child tasks:
    - `A3.1` concurrent writer benchmark.
    - `A3.2` tail latency + wait histogram instrumentation.
    - `A3.3` documented backoff/degradation policy.

- `A4` `epic` / `priority:0`
  - `goal`: remove numeric ambiguity from pricing-sensitive decisions.
  - `why`: tiny rounding variance compounds into wrong candidates.
  - `blocked_by`: `A1`.
  - `done_when`: pricing critical path uses fixed/basis-point logic or explicit tolerance.
  - child tasks:
    - `A4.1` money-path inventory.
    - `A4.2` fixed-point migration.
    - `A4.3` tolerance matrix.
    - `A4.4` canonical rounding regressions.

- `A5` `epic` / `priority:1`
  - `goal`: make replay provenance explicit and hard-failable.
  - `why`: weak provenance feeds can derail trust in all advanced tracks.
  - `blocked_by`: `A4`.
  - `done_when`: untrusted snapshots cannot progress recommendation logic.
  - child tasks:
    - `A5.1` snapshot lineage checks.
    - `A5.2` replay window ownership checks.
    - `A5.3` explicit rejection messaging and recovery.

- `B1` `epic` / `priority:0`
  - `goal`: route recommendations by confidence quality.
  - `why`: speed without trust control causes override fatigue.
  - `blocked_by`: `A5`.
  - `done_when`: low-confidence recommendations are advisory-only.
  - child tasks:
    - `B1.1` confidence enum contract.
    - `B1.2` route policy by confidence band.
    - `B1.3` residual risk labeling.

- `B2` `epic` / `priority:1`
  - `goal`: make recommendations immediately interpretable.
  - `why`: unreadable output becomes ignored output, regardless of correctness.
  - `blocked_by`: `B1`.
  - `done_when`: all packets render rationale-first and explainable within 10 seconds.
  - child tasks:
    - `B2.1` two-line executive summary requirement.
    - `B2.2` rationale-first ordering.
    - `B2.3` deterministic evidence links.

- `B3` `epic` / `priority:1`
  - `goal`: protect CLO from weak evidence-driven overreach.
  - `why`: tiny cohorts generate misleading recommendations.
  - `blocked_by`: `B1`.
  - `done_when`: no candidate generation under floor.
  - child tasks:
    - `B3.1` minimum cohort policy by segment.
    - `B3.2` descriptive fallback behavior.
    - `B3.3` state transitions from advisory↔candidate.

- `N1` `feature` / `priority:1`
  - `goal`: stop NXT blind spots with explicit negotiation context.
  - `why`: deterministic logic without context is brittle in real deals.
  - `blocked_by`: `B1`, `B2`.
  - `done_when`: stale context is invalidated and replaced via explicit source.
  - child tasks:
    - `N1.1` `deal_context` schema.
    - `N1.2` historical signal enrichment.
    - `N1.3` staleness invalidation.

- `C1` `epic` / `priority:0`
  - `goal`: operationalize failure handling before production rollout.
  - `why`: resilience cannot be inferred from positive tests only.
  - `blocked_by`: `A1`.
  - `done_when`: failure matrix is reproducible and mitigations deterministic.
  - child tasks:
    - `C1.1` duplicate replay.
    - `C1.2` timeout + callback duplication.
    - `C1.3` malformed audit payloads.
    - `C1.4` CRM consistency conflicts.

- `C2` `epic` / `priority:1`
  - `goal`: defend CLO/NXT/RGN from provenance poisoning.
  - `why`: anti-forgery must be enforced at intake, not after inference.
  - `blocked_by`: `A5`, `C1`.
  - `done_when`: forged or incomplete chains cannot enter decision paths.
  - child tasks:
    - `C2.1` immutable-ledger replay checks.
    - `C2.2` provenance-gap rejection.

- `C3` `epic` / `priority:2`
  - `goal`: keep attribution graphs queryable and bounded.
  - `why`: unconstrained recursion is an operational outage in disguise.
  - `blocked_by`: `C2`.
  - `done_when`: bounded-query budgets and fallback summaries are in place.
  - child tasks:
    - `C3.1` TTL/depth/partition limits.
    - `C3.2` bounded budget + fallback.
    - `C3.3` confidence intervals for counterfactuals.

- `D1` `feature` / `priority:2`
  - `goal`: prevent operator confusion from Slack-only visibility.
  - `why`: complex negotiations need a stable observability surface.
  - `blocked_by`: `B2`, `C1`.
  - `done_when`: single-session state is inspectable and linkable from Slack.
  - child tasks:
    - `D1.1` slack command/notification contract freeze.
    - `D1.2` read-only negotiation cockpit.
    - `D1.3` deep-link synchronization and state handoff.

- `D2` `epic` / `priority:1`
  - `goal`: reduce low-value noise while preserving urgent escalation quality.
  - `why`: noisy systems become ignored by operators.
  - `blocked_by`: `B2`.
  - `done_when`: configured noise ratio and escalation quality thresholds met.
  - child tasks:
    - `D2.1` noise budget.
    - `D2.2` escalation-first sorting.
    - `D2.3` operator controls.

- `D3` `epic` / `priority:0`
  - `goal`: make shutdown and rollback immediate and auditable.
  - `why`: speed of response during incidents is a key safety KPI.
  - `blocked_by`: `A1`, `A5`.
  - `done_when`: all kill-switch modes are validated and documented.
  - child tasks:
    - `D3.1` kill-switch mode matrix.
    - `D3.2` escalation mapping + ownership.
    - `D3.3` dry-run + audit capture drill.

### Dependencies (explicit)

- `A1`→`A2`→`A3`
- `A1`→`A4`→`A5`
- `A5`→`B1`→`B2`→`B3`
- `B1,B2`→`N1`
- `A1,B2`→`C1`
- `A5,C1`→`C2`→`C3`
- `B2,C1`→`D1`
- `B2`→`D2`
- `A1,A5`→`D3`

## 13) Alignment note for future reviews

This plan intentionally trades speed of first release for reliability and trust. It preserves enterprise viability by prioritizing:
- decision correctness under real concurrency,
- explainable outputs,
- bounded intelligence recommendations,
- and explicit operational control.

If the tracker is available, each block above is immediately translatable into `br` tasks with dependency edges and priority markers.
