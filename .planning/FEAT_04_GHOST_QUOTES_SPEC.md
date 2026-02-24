# FEAT-04 Ghost Quotes Spec

## Purpose
Define scope, KPI contract, deterministic guardrails, and interface boundaries for `bd-70d.4`
(`Ghost Quotes - Predictive Opportunity Creation`) so buying signals in Slack are converted to actionable draft quotes with appropriate confidence thresholds.

## Scope
### In Scope
- Slack message stream monitoring for buying intent signals.
- Confidence scoring based on keyword extraction and entity detection.
- Ghost quote generation using Deal DNA similarity matching.
- DM delivery of ghost quotes to assigned sales reps.
- User controls: dismiss, convert to real quote, disable per channel.
- Audit trail of all signals detected and ghost quotes generated.

### Out of Scope (for Wave 1)
- Direct customer-facing interactions (rep-only in Wave 1).
- Automatic quote submission without rep approval.
- Multi-channel signal correlation (email, CRM, etc.).
- Predictive timeline estimation beyond keyword matching.

## Rollout Slices
- `Slice A` (contracts): signal schema, confidence scoring model, ghost quote structure.
- `Slice B` (detection): Slack event ingestion, SignalDetector, entity extraction.
- `Slice C` (generation): GhostQuoteGenerator, Deal DNA similarity lookup, discount suggestion.
- `Slice D` (delivery): DM delivery, rep controls, conversion workflow, telemetry.

`Slice A/B` must be complete before `Slice C`; `Slice C` must be complete before `Slice D` production use.

## KPI Contract
| KPI | Baseline | Wave-1 Target | Owner | Measurement |
|---|---:|---:|---|---|
| Signal detection accuracy | N/A | >= 80% | ML owner | true positives / total signals |
| Ghost quote conversion rate | N/A | >= 25% | Product owner | converted to real quote / ghost quotes generated |
| Rep engagement rate | N/A | >= 60% | UX owner | reps who view ghost quote / ghost quotes sent |
| False positive rate | N/A | <= 15% | Determinism owner | false signals / total signals detected |
| Signal-to-delivery latency | N/A | <= 30s | Platform owner | message received to DM delivered |

## Deterministic Safety Constraints
- Ghost quotes are draft-only; never automatically submitted or customer-visible.
- Confidence threshold (default 70) gates all ghost quote generation.
- Discount suggestions computed deterministically from signal keywords and similar deals.
- All signals logged with full context for audit and model improvement.
- Rep controls required: dismiss, convert, or ignore - no automatic actions.

## Interface Boundaries (Draft)
### Domain Contracts
- `Signal`: confidence, keyword_matches, companies, departments, timelines, competitors, above_threshold.
- `GhostQuote`: company, draft_quote, confidence, suggested_discount_pct, similar_quote_id.
- `SignalDetectorConfig`: confidence_threshold, buying_intent_keywords, competitor_keywords.

### Service Contracts
- `SignalDetector::detect(message_text) -> Option<Signal>`
- `GhostQuoteGenerator::generate(signal, history_provider) -> Option<GhostQuote>`
- `GhostQuoteService::deliver(ghost_quote, rep_id) -> DeliveryResult`
- `GhostQuoteService::convert_to_quote(ghost_quote_id, actor) -> QuoteResult`
- `GhostQuoteService::dismiss(ghost_quote_id, actor) -> DismissResult`

### Persistence Contracts
- `GhostQuoteRepo`: store/retrieve ghost quotes with lifecycle status.
- `SignalAuditRepo`: append-only log of all detected signals.
- `GhostQuoteDeliveryRepo`: DM delivery tracking and rep interactions.

### Slack Contract
- Bot listens to `message` events in public channels only (no DMs).
- DM sent to rep when ghost quote generated with confidence >= threshold.
- DM includes: company, detected signal summary, suggested discount, action buttons.
- Action buttons: "View Quote", "Convert", "Dismiss", "Not Relevant".
- Rep can disable ghost quotes per channel via channel settings.

### Crate Boundaries
- `quotey-slack`: message stream handling, DM delivery, event ingestion.
- `quotey-core`: SignalDetector, GhostQuoteGenerator, confidence scoring.
- `quotey-db`: ghost quote persistence, signal audit, delivery tracking.
- `quotey-agent`: orchestration, rep routing, conversion workflow.

## Risk Register
| Risk | Impact | Likelihood | Mitigation | Owner |
|---|---|---|---|---|
| Privacy violation from channel monitoring | High | Low | public channels only + no message content storage | Security owner |
| Rep spam from excessive ghost quotes | Medium | Medium | confidence threshold + per-channel disable + rate limiting | UX owner |
| Incorrect company attribution | Medium | Medium | entity validation + rep confirmation step | Data owner |
| Stale ghost quotes accumulate | Low | Medium | TTL auto-expiry + cleanup job | Platform owner |
| Competitor keyword false positives | Medium | Medium | context scoring + manual feedback loop | ML owner |

## Guardrail Checklist (Pre-implementation Exit)
- [ ] Scope and non-goals agreed.
- [ ] KPI owner and metric formula explicitly documented.
- [ ] Deterministic constraints copied into implementation task templates.
- [ ] Interface contracts reviewed against existing crate boundaries.
- [ ] Risks and mitigations acknowledged by feature owner.

## Migration Contract
### Schema Additions (`0022_ghost_quotes`)
- `ghost_quotes`: draft quote records with signal context and lifecycle.
- `signal_audit`: append-only log of all detected buying signals.
- `ghost_quote_deliveries`: DM delivery tracking and rep interaction log.
- `ghost_quote_settings`: per-channel and per-rep enablement settings.

### Version and Audit Semantics
- Ghost quotes immutable after generation; status tracks lifecycle (pending, viewed, converted, dismissed, expired).
- Signal audit log append-only for model training and compliance.
- Rep actions (dismiss, convert) logged with timestamp and reason.

### Migration Behavior and Rollback
- Migration adds ghost quote tables; no changes to existing quote tables.
- Ghost quotes disabled by default; enable via feature flag per workspace.
- Rollback removes ghost quote tables; no impact on existing quotes.
