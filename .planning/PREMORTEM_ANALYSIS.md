# Premortem Analysis — Quotey Wave 2

**Date:** 2026-02-25
**Scope:** Full project + W2 CLO/NXT tracks
**Method:** Assume total failure at 6 months, work backward to identify causes

---

## Failure Mode 1: Spec-to-Code Ratio Problem

**What went wrong:** ~80 planning documents, 18 migrations, 4000 lines in the policy optimizer — but zero integration tests that run a quote end-to-end through Slack. The test suite (69 unit tests, mostly in `quotey-slack` parsing) validates serialization contracts and Block Kit rendering. Nobody ever tested the actual user flow: "rep types in Slack, gets a PDF quote back."

**False assumption:** "If each deterministic contract is individually correct, the whole system works." In reality, the seams between crates (`agent` orchestration calling `core` engines calling `db` repositories) were never exercised under realistic conditions. The domain contracts are beautiful in isolation and broken in composition.

**Remediation:** Before writing any more Wave 2 code, create 3-5 integration tests that exercise the full happy path from Slack command parsing through pricing/constraint evaluation to quote output. Make these the gate for all future work.

**Priority:** P0 — blocks all other work
**Tracks:** Foundation

---

## Failure Mode 2: Determinism Theatre

**What went wrong:** Enormous effort in replay checksums, canonical JSON, sorted dedup, pinned engine versions — then discovered that SQLite `REAL` rounding, Rust `f64` formatting differences across platforms, and `chrono` timestamp precision made "identical replay" a lie in production. The checksum-mismatch error path (designed as a safety gate) became the most common error users hit.

**False assumption:** "Deterministic checksum === portable deterministic behavior." Floating-point pricing math is inherently platform-sensitive at the edges, and we designed a system that hard-fails on any mismatch instead of distinguishing meaningful vs. epsilon drift.

**Remediation:** Define deterministic replay as "same decisions with tolerance-banded numeric equivalence" rather than bitwise checksum identity. Use integer basis-point arithmetic for all pricing math (already partially implied by `_bps` fields, but not enforced end-to-end). Reserve exact checksums for structural/decision replay, not numeric precision replay.

**Priority:** P0 — undermines core safety claim
**Tracks:** CLO, NXT, Foundation

---

## Failure Mode 3: CLO Useless Below Statistical Mass

**What went wrong:** The Closed-Loop Policy Optimizer needs a meaningful corpus of won/lost outcomes with margin data to produce useful candidates. Early adopters had 20-50 quotes. The optimizer produced noisy, low-confidence candidates that reviewers always rejected. The 20-40% acceptance rate target was aspirational — real rate was 2%.

**False assumption:** "The optimizer will be useful from day one." It requires statistical mass that early customers don't have.

**Remediation:** Add a minimum cohort-size gate to candidate generation (e.g., 200+ outcomes per segment). Below that threshold, CLO should surface descriptive analytics ("here's your win rate by segment") instead of prescriptive candidates. Make the feature degrade gracefully into a dashboard rather than producing garbage suggestions.

**Priority:** P1 — CLO is already implemented, needs guard
**Tracks:** CLO

---

## Failure Mode 4: NXT Without CRM Context Is Strategically Dumb

**What went wrong:** NXT proposes counteroffers based on concession envelopes and margin floors — but reps need to know "this customer renewed 3 times, always pushes for 15% off, and their contract expires in 2 weeks." Without CRM/history context, the "deterministic" suggestions were technically correct but strategically dumb. Reps ignored them.

**False assumption:** "Bounded counteroffers from pricing rules alone provide value." Negotiation is relationship-contextual, not just math-contextual.

**Remediation:** NXT Task 2 (state model) should include a `deal_context` field that pulls from the precedent intelligence graph and customer history. Concession envelopes should factor in customer-level historical behavior, not just global policy floors. This creates a hard dependency on the precedent graph being populated.

**Priority:** P1 — must be designed before NXT implementation begins
**Tracks:** NXT, Precedent Intelligence

---

## Failure Mode 5: Slack-Only UX Is a Trap for Complex Workflows

**What went wrong:** Complex negotiation workflows with multiple offer cards, boundary badges, approval escalations, and concession history made Slack threads unreadable after 10 messages. Reps lost track of which offer was current. The thread-state resolver worked perfectly; the human couldn't parse the wall of Block Kit cards.

**False assumption:** "Slack threads are a sufficient UX surface for complex multi-turn workflows." Slack is great for initiation and notification. It's terrible as a stateful application workspace.

**Remediation:** Add a lightweight web view (single-page read-only dashboard behind `quotey server`) that renders the current negotiation state, offer comparison table, and concession history. Keep Slack as the input/notification channel but provide a link to the "deal cockpit" view for complex sessions.

**Priority:** P2 — not blocking, but limits real adoption
**Tracks:** NXT, Foundation

---

## Failure Mode 6: Migration Complexity Grows Unbounded

**What went wrong:** 18 migrations with implicit cross-feature dependencies. Schema changes for one feature break assumptions in another feature's repository code. A customer upgrading multiple versions hit foreign key constraint failures requiring manual SQLite surgery.

**False assumption:** "Idempotent `IF NOT EXISTS` migrations compose safely." They do for creation, but cross-feature queries and index assumptions create implicit dependencies.

**Remediation:** Add a migration integration test that runs the full up-chain from scratch AND from each historical migration version. Test `up;down;up` for the last 5 migrations in CI. Consider a migration consolidation checkpoint at each wave boundary.

**Priority:** P1 — data loss risk
**Tracks:** Foundation

---

## Failure Mode 7: Approval Packets Are Unreadable by Humans

**What went wrong:** The first real approver (a VP of Sales) opened the packet, saw a wall of JSON-like deterministic evidence, and said "I don't know what any of this means." They approved everything without reading it, defeating the entire safety model.

**False assumption:** "Providing complete deterministic evidence = effective human review."

**Remediation:** Every approval surface (CLO packets, NXT escalations) needs a 2-sentence human-language summary at the top: "This changes discount floor from 15% to 12% for enterprise accounts. Based on 340 historical quotes, this would have increased win rate by 3% and decreased margin by 1.2%." The deterministic evidence stays attached for audit, but the decision surface must be readable by a non-technical approver in under 10 seconds.

**Priority:** P1 — safety model depends on this
**Tracks:** CLO, NXT

---

## Failure Mode 8: Red-Team Tests Known Attacks Only

**What went wrong:** Three adversarial scenarios are thorough for imagined attacks. A real attacker found that by submitting a candidate with fabricated "historical" cohort data, the replay engine dutifully computed favorable metrics and the candidate sailed through review.

**False assumption:** "If replay inputs are checksummed, they're trustworthy." Checksums verify integrity, not provenance.

**Remediation:** Replay snapshots must be sourced from immutable audit trail records with independently verifiable provenance. Add a provenance verification step before replay that cross-references snapshot quote IDs against the actual `quote_ledger` table.

**Priority:** P1 — security gap
**Tracks:** CLO

---

## Failure Mode 9: SQLite Concurrency Ceiling

**What went wrong:** SQLite with WAL mode works great for single-user CLI testing. When 15 reps hit `/quote` simultaneously, write contention caused `SQLITE_BUSY` errors. P95 latency for quote creation spiked to 8 seconds — well above the 600ms NXT target.

**False assumption:** "Local-first SQLite scales to a real sales team."

**Remediation:** Benchmark with 15-20 concurrent simulated sessions. If write contention is real, consider (a) read replicas with single writer queue, (b) sharding by quote ID, or (c) acknowledging SQLite is alpha storage and Postgres is the production path. Document the concurrency ceiling honestly.

**Priority:** P2 — doesn't block alpha, blocks production
**Tracks:** Foundation

---

## Failure Mode 10: Planning Debt Is Product Debt

**What went wrong:** More time writing specs, runbooks, demo checklists, KPI query packs, and risk registers than writing working code. The `.planning/` directory has more words than the `crates/` directory has lines of code. Every new feature started with a 400-line spec before a single function was written.

**False assumption:** "Comprehensive specification reduces implementation risk." Past a threshold, specification becomes procrastination.

**Remediation:** For NXT (which is 0% implemented), cut the spec to Slice A+B+C only. Ship the concession engine and boundary calculator as a CLI-testable library. Validate that it produces useful suggestions on 10 real deal scenarios before building the Slack cockpit, approval handoff, replay harness, red-team harness, telemetry, and rollout gate.

**Priority:** P0 — cultural/process issue
**Tracks:** All

---

## Prioritized Action Items

### P0 — Do Before Any New Feature Work

| # | Action | Tracks | Estimated Scope |
|---|--------|--------|-----------------|
| 1 | Write 3-5 cross-crate integration tests (quote create, pricing+constraints, approval routing) | Foundation | New test crate or `tests/` dir |
| 2 | Audit all pricing math for integer-bps enforcement end-to-end | CLO, NXT, Foundation | Core + DB audit |
| 3 | NXT: engine-first approach — implement concession engine + boundary calc as pure `quotey-core` lib with CLI exerciser before Slack surface | NXT | Core + CLI |

### P1 — Do During Wave 2 Implementation

| # | Action | Tracks | Estimated Scope |
|---|--------|--------|-----------------|
| 4 | CLO: add minimum cohort-size gate to candidate generation | CLO | Core optimizer |
| 5 | NXT: add `deal_context` to state model pulling from precedent graph + customer history | NXT | Core + DB |
| 6 | Migration up-chain integration test + `up;down;up` test for last 5 migrations | Foundation | DB tests |
| 7 | Human-readable 2-sentence executive summary on all approval packets | CLO, NXT | Core + Slack |
| 8 | Replay snapshot provenance verification against quote ledger | CLO | Core optimizer |

### P2 — Do Before Production Readiness

| # | Action | Tracks | Estimated Scope |
|---|--------|--------|-----------------|
| 9 | Lightweight read-only deal cockpit web view | NXT | Server crate |
| 10 | SQLite 15-user concurrent session load test + document ceiling | Foundation | New bench |
