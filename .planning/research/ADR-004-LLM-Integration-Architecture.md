# ADR-004: LLM Integration Architecture for CPQ Translator Workloads

**Status:** Accepted  
**Date:** 2026-02-23  
**Related Bead:** `bd-256v.4`

## Context

Quotey requires natural-language interaction while preserving deterministic CPQ correctness and auditability. LLM behavior is probabilistic and can produce hallucinations or inconsistent formatting if unguided.

We need an architecture that keeps UX gains while protecting pricing/policy/legal correctness.

## Decision

Adopt an **LLM Translator + Deterministic Validator** architecture:

1. LLM responsibilities:
- intent extraction,
- slot filling proposals,
- disambiguation assistance,
- summarization and narrative drafting.

2. Deterministic engine responsibilities:
- pricing decisions,
- constraint validity,
- policy compliance,
- approval routing,
- lifecycle transitions.

3. Enforce strict structured outputs for extraction-critical flows.
4. Require confidence + abstain pathways for uncertain outputs.
5. Use provider abstraction with primary and fallback models.
6. Log prompt/version/model metadata for audit and reproducibility.

## Rationale

- Separates probabilistic language interpretation from contractual business logic.
- Reduces hallucination risk in high-stakes CPQ operations.
- Supports provider flexibility and offline fallback paths.
- Enables measurable quality gates via benchmark suite.

## Consequences

### Positive
- High safety for pricing/approval correctness.
- Better auditability of model influence on operations.
- Clear ownership boundaries for runtime components.

### Negative
- Additional integration complexity (validation, retries, fallback routing).
- Requires benchmark maintenance and prompt versioning discipline.

## Guardrails

1. LLM output never directly mutates final price/approval status.
2. Every extraction output passes schema + deterministic validation.
3. Unknown/ambiguous fields must trigger clarification, not guessing.
4. Prompt templates are versioned and auditable.

## Verification Plan

1. Run CPQ-Intent-Bench-v1 against model candidates.
2. Enforce quality gates for intent/slot/schema metrics.
3. Run adversarial prompt-injection tests.
4. Verify all high-impact operations have deterministic confirmation and audit records.

## Revisit Triggers

- If benchmark gates fail for current primary model.
- If provider outages materially degrade availability.
- If new model families provide significantly better structured extraction quality.

## References

- https://platform.openai.com/docs/guides/structured-outputs
- https://platform.openai.com/docs/guides/function-calling
- https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/overview
- https://ollama.com/blog/structured-outputs

