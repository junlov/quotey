# W2 NXT Deterministic Negotiation Autopilot Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, interfaces, and execution backlog for `bd-3acq`
(`W2 [NXT] Deterministic Negotiation Autopilot`) so reps can negotiate faster in Slack while
keeping pricing, policy, and approval authority deterministic and auditable.

## Problem Statement
Reps lose time in back-and-forth negotiation loops where they need safe next offers quickly but
cannot rely on non-deterministic suggestions. Today, reps manually iterate through discount,
term, and package combinations, then discover policy or approval failures late.

NXT solves this by proposing deterministic counteroffers and concession paths that:
- stay inside hard floor/ceiling boundaries,
- surface explicit tradeoffs and stop reasons,
- route out-of-policy proposals into approval workflows,
- preserve replayability for every negotiation step.

## Product Goal
Increase negotiation velocity and win quality without sacrificing deterministic CPQ guarantees.
NXT must be accretive (higher throughput, better close quality), bounded (never bypass policy),
and explainable (every suggestion has deterministic evidence).

## Scope
### In Scope
- Negotiation session lifecycle with persisted turn-by-turn offer state.
- Deterministic concession envelope evaluation (discount, term, packaging, margin).
- Deterministic counteroffer planning with stable ranking and tie-break behavior.
- Hard boundary/walk-away guardrails with explicit user-safe stop reasons.
- Slack negotiation cockpit cards and command actions bound to deterministic transitions.
- Approval handoff for out-of-policy offers with full negotiation context.
- Audit and explanation artifacts for every suggestion/selection/escalation decision.
- Deterministic replay/simulation harness for negotiation transcripts.
- Safety red-team harness for adversarial negotiation requests.
- KPI telemetry and rollout gate with go/no-go criteria.

### Out of Scope (Wave 2)
- Autonomous acceptance or customer-facing commitment without human action.
- Non-deterministic optimization that can alter prices or policy thresholds directly.
- Real-time external market-price feeds in decision logic.
- Multi-party collaborative negotiation editing across multiple reps in one session.
- Live online learning that mutates production negotiation policy without approval.

## Rollout Slices
- `Slice A` (contracts/spec): scope, KPI contract, deterministic guardrails, risk controls.
- `Slice B` (state/data): negotiation persistence schema and idempotent transition repositories.
- `Slice C` (engine): concession policy, boundary calculator, and counteroffer planner.
- `Slice D` (experience): Slack cockpit/actions and approval handoff integration.
- `Slice E` (assurance): audit/explainability, replay harness, and red-team safety tests.
- `Slice F` (operations): telemetry, demo script, and rollout gate.

`Slice A` must complete before `Slice B/C`.
`Slice C` must pass deterministic and safety tests before `Slice D` user exposure.
`Slice D/E` must complete before `Slice F` rollout decision.

## KPI Contract
| KPI | Baseline | Wave-2 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Negotiation cycle time (first request to accepted offer) | N/A | <= 40% of manual baseline | Product owner | median minutes per negotiation session |
| Safe suggestion rate | N/A | 100% | Safety owner | `% suggestions within deterministic boundaries` |
| Out-of-policy early interception rate | N/A | >= 95% | Policy owner | `% invalid requests blocked before commit path` |
| Approval-ready packet completeness | N/A | >= 99% | Approval owner | `% escalation packets with required deterministic evidence fields` |
| Replay determinism rate | N/A | 100% | Core owner | `identical transcript + versions => identical outputs` |
| Suggestion acceptance rate | N/A | 30-60% | Product owner | `accepted_suggestions / shown_suggestions` |
| Negotiation-induced policy breach incidents | N/A | 0 | Safety owner | count of post-commit breaches linked to NXT suggestions |
| P95 suggestion latency | N/A | <= 600ms | Platform owner | request received to suggestion rendered |

## Deterministic Safety Constraints
- LLMs may summarize options; they cannot authoritatively set price, policy, or approval outcomes.
- NXT suggestions must be generated from persisted quote/session state plus version-pinned rule inputs.
- Any out-of-bounds proposal must return a blocked result with explicit stop reasons and next actions.
- Negotiation sessions must be idempotent by transition key; retries cannot duplicate committed actions.
- Suggestion ranking must be stable and deterministic with documented tie-break fields.
- Approval-required offers cannot be marked accepted until deterministic approval path returns approved.
- Every session transition must emit audit events sufficient to reconstruct transcript and decision evidence.

## Negotiation Lifecycle Contract
1. `draft`: negotiation requested, session initialized.
2. `active`: deterministic suggestions generated and user can choose next action.
3. `counter_pending`: selected counteroffer staged, awaiting user confirm/escalate.
4. `approval_pending`: out-of-policy offer routed to approval workflow.
5. `approved`: escalation approved and offer is commit-eligible.
6. `accepted`: offer accepted and quote state updated.
7. `rejected`: user or approver rejected path; session remains auditable.
8. `expired` or `cancelled`: session no longer active.

## Interface Boundaries (Draft)
### Domain Contracts
- `NegotiationSession`: session id, quote id, actor, state, deterministic version refs.
- `NegotiationTurn`: input request, candidate offers, chosen action, outcome.
- `ConcessionEnvelope`: allowed ranges by dimension plus blocking reasons.
- `CounterofferPlan`: ranked deterministic alternatives with rationale metadata.
- `BoundaryEvaluation`: floor/ceiling/walk-away results and escalation requirement.

### Service Contracts
- `NegotiationSessionService::start(quote_id, actor_id) -> NegotiationSession`
- `ConcessionService::evaluate(session_id, request) -> ConcessionEnvelope`
- `CounterofferService::plan(session_id, envelope) -> CounterofferPlan`
- `NegotiationService::select(session_id, offer_id, idempotency_key) -> TransitionResult`
- `NegotiationService::escalate(session_id, offer_id) -> ApprovalRequestResult`
- `NegotiationReplayService::replay(session_id) -> ReplayReport`

### Persistence Contracts
- `NegotiationSessionRepo`
- `NegotiationTurnRepo`
- `NegotiationBoundaryRepo`
- `NegotiationAuditRepo`
- `NegotiationReplayRepo`

### Slack Contract
- Thread-level NXT actions render deterministic offer cards and boundary badges.
- Action callbacks map to idempotent transition keys.
- Escalation path renders approval packet with concession deltas and stop reasons.
- UI language clearly marks `suggested` vs `committed` outcomes.

### Crate Boundaries
- `quotey-core`: concession policy, boundary math, planner ranking, replay invariants.
- `quotey-db`: negotiation schema, repositories, idempotency persistence, audit storage.
- `quotey-slack`: command parsing and Block Kit rendering only.
- `quotey-agent`: orchestration across session, planner, guardrails, and approval handoff.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Suggestion appears valid but violates hidden policy threshold | High | Medium | pre-suggestion policy check + hard block path | Policy owner |
| Non-deterministic ranking drift across releases | High | Low | pinned tie-break contract + replay fixtures | Core owner |
| Approval escalation lacks required context | Medium | Medium | packet schema validation + fail-closed escalation | Approval owner |
| Reps over-trust suggestions and skip strategy judgment | Medium | Medium | explicit recommendation confidence + alternatives + manual confirm step | Product owner |
| Safety bypass via crafted prompt/social engineering text | High | Medium | adversarial corpus + strict parser + deny-by-default action mapping | Safety owner |
| Session spam/noise causes operator fatigue | Medium | Medium | throttling, cooldown windows, and telemetry alerts | Runtime owner |

## Execution Backlog (Canonical TODO List)
These items are tracked in beads and are the authoritative NXT implementation checklist.

### Epic
- [ ] `bd-3acq` W2 [NXT] Deterministic Negotiation Autopilot

### Primary Tasks
- [ ] `bd-3acq.1` [NXT] Task 1 Spec KPI Guardrails
- [ ] `bd-3acq.2` [NXT] Task 2 Negotiation State Model + Persistence
- [ ] `bd-3acq.3` [NXT] Task 3 Deterministic Concession Policy Engine
- [ ] `bd-3acq.4` [NXT] Task 4 Counteroffer Planner + Strategy Library
- [ ] `bd-3acq.5` [NXT] Task 5 Boundary Calculator + Walk-Away Guardrails
- [ ] `bd-3acq.6` [NXT] Task 6 Slack Negotiation Cockpit UX + Commands
- [ ] `bd-3acq.7` [NXT] Task 7 Approval Handoff Integration
- [ ] `bd-3acq.8` [NXT] Task 8 Audit + Explainability Artifacts
- [ ] `bd-3acq.9` [NXT] Task 9 Deterministic Replay/Simulation Harness
- [ ] `bd-3acq.10` [NXT] Task 10 Safety + Red-Team Harness
- [ ] `bd-3acq.11` [NXT] Task 11 Telemetry + Business Impact Measurement
- [ ] `bd-3acq.12` [NXT] Task 12 End-to-End Demo + Rollout Gate

### Granular Subtasks
- [ ] `bd-3acq.2.1` [NXT-DATA] Migration schema + fixtures for negotiation sessions
- [ ] `bd-3acq.2.2` [NXT-DATA] Repository methods for advance/replay/idempotent transitions
- [ ] `bd-3acq.3.1` [NXT-ENGINE] Concession rule DSL + parser contract
- [ ] `bd-3acq.3.2` [NXT-ENGINE] Threshold matrix evaluator + invariant tests
- [ ] `bd-3acq.4.1` [NXT-PLANNER] Deterministic ranking + tie-break contract
- [ ] `bd-3acq.4.2` [NXT-PLANNER] Strategy template registry + version pinning
- [ ] `bd-3acq.5.1` [NXT-SAFETY] Margin/discount floor calculators
- [ ] `bd-3acq.5.2` [NXT-SAFETY] Walk-away trigger evaluation + stop reason taxonomy
- [ ] `bd-3acq.6.1` [NXT-SLACK] Block Kit cockpit cards + action wiring
- [ ] `bd-3acq.6.2` [NXT-SLACK] Command grammar + thread-state resolver
- [ ] `bd-3acq.7.1` [NXT-APPROVAL] Escalation context pack serializer
- [ ] `bd-3acq.8.1` [NXT-AUDIT] Explanation artifact schema + source references
- [ ] `bd-3acq.8.2` [NXT-AUDIT] Transcript reconstruction invariants
- [ ] `bd-3acq.9.1` [NXT-REPLAY] Deterministic transcript harness fixtures
- [ ] `bd-3acq.9.2` [NXT-REPLAY] Drift diagnostics and diff tooling
- [ ] `bd-3acq.10.1` [NXT-SAFETY] Adversarial negotiation corpus
- [ ] `bd-3acq.10.2` [NXT-SAFETY] Safety regression gate in CI
- [ ] `bd-3acq.11.1` [NXT-METRICS] KPI query pack + alert thresholds
- [ ] `bd-3acq.12.1` [NXT-ROLLOUT] Demo checklist + go/no-go gate sheet

### Dependency Order (Execution Plan)
1. `bd-3acq.1`
2. `bd-3acq.2` -> (`bd-3acq.2.1`, `bd-3acq.2.2`)
3. `bd-3acq.3` -> (`bd-3acq.3.1`, `bd-3acq.3.2`)
4. `bd-3acq.4` -> (`bd-3acq.4.1`, `bd-3acq.4.2`)
5. `bd-3acq.5` -> (`bd-3acq.5.1`, `bd-3acq.5.2`)
6. `bd-3acq.6` -> (`bd-3acq.6.1`, `bd-3acq.6.2`)
7. `bd-3acq.7` -> (`bd-3acq.7.1`)
8. `bd-3acq.8` -> (`bd-3acq.8.1`, `bd-3acq.8.2`)
9. `bd-3acq.9` -> (`bd-3acq.9.1`, `bd-3acq.9.2`)
10. `bd-3acq.10` -> (`bd-3acq.10.1`, `bd-3acq.10.2`)
11. `bd-3acq.11` -> (`bd-3acq.11.1`)
12. `bd-3acq.12` -> (`bd-3acq.12.1`)

## Quality Gates
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-targets --all-features`
- `ubs --diff` scoped to changed files
- Replay determinism and safety-regression suites must pass before rollout gate

## Guardrail Exit Checklist (Before Task 2 Coding)
- [ ] Scope/non-goals accepted.
- [ ] KPI formulas and owners locked.
- [ ] Deterministic guardrails mapped to testable invariants.
- [ ] Negotiation lifecycle/state transitions reviewed across core/db/slack/agent boundaries.
- [ ] Risk mitigations mapped to owning beads.

## Notes
- This spec is planning source-of-truth for the NXT track.
- Beads remain the execution system-of-record; status updates belong in `br`.
