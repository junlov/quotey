# RCH-04: LLM Prompt Engineering for CPQ Research

**Bead:** `bd-256v.4`  
**Status:** Complete (research + benchmark framework + guardrails)  
**Date:** 2026-02-23  
**Researcher:** IvoryBear (Codex)

---

## Executive Summary

Quotey should use an **LLM-as-translator** architecture with deterministic CPQ engines as the source of truth.

For CPQ reliability, prompt strategy should center on:
1. strict structured outputs for intent and slot extraction,
2. explicit abstain/clarify behavior for uncertainty,
3. retrieval-augmented context with bounded catalog snippets,
4. deterministic validation after every model output,
5. comprehensive logging of prompt/version/model metadata for auditability.

Recommended operational pattern:
- Primary cloud model for high-accuracy extraction and ambiguity resolution.
- Secondary lower-cost model for summarization/non-critical drafting.
- Optional local model for offline/demo mode with stricter confidence thresholds.
- Deterministic fallback path when model confidence or validation fails.

---

## 1. Scope and Research Questions

This research covers:
- prompt patterns for CPQ intent extraction and slot filling,
- model comparison framework (GPT family, Claude family, local OSS models),
- benchmark test suite design and scoring methodology,
- failure mode catalog,
- guardrail implementation guide,
- ADR for LLM integration boundaries.

This work aligns with the project non-negotiable safety principle:
- LLMs translate language and summarize context.
- LLMs do not decide prices, configuration validity, policy compliance, or approval routing.

---

## 2. Prompting Strategy: What Works for CPQ

## 2.1 Prompting Patterns by Task

1. **Intent + slot extraction (high criticality)**
- Use structured output schema (JSON schema or tool call).
- Require explicit `needs_clarification` and `missing_fields`.
- Require confidence fields per extracted slot.

2. **Catalog disambiguation (medium criticality)**
- Present bounded candidate list from deterministic catalog query.
- Ask model to select candidate IDs (never free-form IDs).

3. **Conversation summarization for handoff (low criticality)**
- Model may summarize, but include pointers to source message IDs.

4. **Approval rationale drafting (medium criticality)**
- Model drafts narrative; deterministic policy engine provides authoritative decision payload.

5. **Quote intelligence extraction from documents (high criticality)**
- Extract into schema with provenance spans (source text snippet/offset metadata).
- Route low-confidence fields to human confirmation.

## 2.2 Recommended Prompt Stack

Layered prompts:
1. **System prompt**: hard role + safety boundaries.
2. **Domain policy block**: explicitly says “no pricing decisions.”
3. **Schema block**: exact fields and allowed enums.
4. **Context block**: deal/thread/catalog snippets (bounded).
5. **Task directive**: one explicit objective.
6. **Abstain policy**: when uncertain, set `needs_clarification=true`.

## 2.3 Zero-shot vs few-shot vs tool-calling

- Zero-shot: useful for broad draft comprehension tasks, weaker for strict extraction consistency.
- Few-shot: improves format adherence when examples are representative and concise.
- Tool/function calling or strict structured outputs: preferred for production extraction tasks.

Recommendation:
- Use few-shot + structured outputs for extraction-critical paths.
- Keep zero-shot only for non-critical summarization/drafting.

---

## 3. Prompt Library (CPQ v1)

Below are production-oriented template families.

## Template A: Intent Extraction (Structured)

Use for slash/thread command interpretation.

```text
SYSTEM
You are Quotey's language translator. You convert user requests to structured intent.
You must not invent prices, approvals, policy outcomes, or product IDs.
If uncertain, set needs_clarification=true.

DEVELOPER
Return JSON matching schema exactly.

USER
<user message>

CONTEXT
- known_account: <...>
- thread_state: <...>
- catalog_candidates: [id,name,type]...

OUTPUT_SCHEMA
{
  "intent_type": "create_quote|add_line|update_line|request_discount|submit_approval|ask_status|unknown",
  "quote_id": "string|null",
  "line_mutations": [ ... ],
  "requested_discount_pct": "number|null",
  "missing_fields": ["..."] ,
  "needs_clarification": "boolean",
  "confidence": {
    "overall": "0..1",
    "fields": {"field_name": "0..1"}
  },
  "rationale": "short string"
}
```

## Template B: Catalog Disambiguation

```text
Given user phrase and candidate products, choose candidate IDs only.
If no confident match, return unresolved.

USER_PHRASE: "enterprise security add-on"
CANDIDATES:
- PROD_101 Enterprise Security Suite
- PROD_244 Security Add-on Basic
- PROD_778 Data Security Audit Pack

Return:
{ "selected_ids": [...], "unresolved": true|false, "notes": "..." }
```

## Template C: Clarification Question Generator

```text
Given missing fields and current intent, produce at most 2 concise follow-up questions.
Prioritize fields that block deterministic validation.
```

## Template D: Approval Rationale Draft

```text
Draft a concise approval summary from deterministic policy payload.
Do not change policy outcomes. Do not invent facts.
Input includes: margin, discount, threshold rule IDs, prior precedent summary.
Output: markdown summary for approver.
```

## Template E: RFP/Email Slot Extractor

```text
Extract requirements into schema with provenance.
For each extracted field include source_quote text and confidence.
```

## Template F: Thread-to-Handoff Summary

```text
Summarize current quote state for operator handoff.
Must include unresolved blockers and exact next required actions.
```

## Template G: Guarded Rewriter

```text
Rewrite user text into normalized command phrase while preserving intent.
No additional assumptions; unknowns remain unknown.
```

## Template H: Error Explanation Draft

```text
Given deterministic constraint/policy failure payload, produce user-facing explanation and remediation steps.
Never alter failure classification.
```

---

## 4. Model Comparison Matrix (Pragmatic)

Note: This matrix is capability-oriented for architecture selection; live latency/cost benchmarking requires provider keys.

| Model Family | Structured Output Support | Tool/Function Calling | Strengths for CPQ | Risks/Tradeoffs | Recommended Role |
|---|---|---|---|---|---|
| GPT-4 class (modern OpenAI models) | Strong | Strong | high extraction quality, robust schema adherence with strict modes | cost and latency variance by tier | primary extraction + ambiguity resolution |
| GPT-3.5 class (legacy) | Moderate | Available but weaker consistency | low cost for simple text tasks | more format drift/hallucination risk | non-critical summarization only (if used) |
| Claude class (e.g., Sonnet family) | Strong | Strong | high-quality reasoning and instruction following | cost/latency depends on model tier and context length | primary/secondary extraction candidate |
| Local OSS models via Ollama | Varies by model | Available in recent tool-capable models | offline/privacy-friendly demos | lower extraction reliability without tuning, infra burden | offline mode, low-risk assistant tasks |

Selection strategy:
- Use one high-quality cloud model as primary translator.
- Keep one alternate provider/local fallback path to avoid single-provider coupling.

---

## 5. Accuracy Benchmark on CPQ Test Cases

## 5.1 Benchmark Suite Design

Create `CPQ-Intent-Bench-v1` with at least 60 cases:

1. Net-new quote creation intents (10)
2. Renewal/expansion intents (10)
3. Discount/approval intents (10)
4. Ambiguous/underspecified requests (10)
5. Catalog disambiguation edge cases (10)
6. Adversarial/noisy prompts (10)

Each case includes:
- source utterance,
- expected structured output,
- required missing fields,
- accepted intent labels,
- deterministic validator outcome.

## 5.2 Metrics

1. Intent accuracy (% exact intent class)
2. Slot F1 (micro + macro)
3. Schema-valid rate
4. Hallucinated field rate
5. Clarification precision (correctly abstained when underspecified)
6. End-to-end deterministic accept rate (output passes validator)

## 5.3 Acceptance Gates (initial)

- Intent accuracy: >= 0.92
- Slot F1: >= 0.88
- Schema-valid rate: >= 0.995
- Hallucinated field rate: <= 0.02
- Clarification precision: >= 0.90

## 5.4 Execution Status in Current Environment

- `OPENAI_API_KEY`: unset
- `ANTHROPIC_API_KEY`: unset
- `ollama`: not installed in this environment

Implication:
- Live provider benchmark runs are blocked in this session.

Delivered instead:
- full benchmark suite design,
- scoring rubric and gates,
- run protocol for immediate execution when credentials/runtime are available.

## 5.5 Benchmark Run Protocol (when keys exist)

1. Execute benchmark harness across model candidates.
2. Freeze prompt version IDs and test dataset hash.
3. Record metrics with confidence intervals.
4. Select primary/secondary model by quality gate + latency/cost budget.

---

## 6. Failure Mode Catalog

| Failure Mode | Symptom | Root Cause | Detection | Mitigation |
|---|---|---|---|---|
| Schema drift | malformed JSON | loose prompting | parser/schema validation failures | strict schema mode + retries with repair prompt |
| Hallucinated IDs | unknown product/rule IDs | open-ended generation | deterministic lookup misses | candidate-list constrained prompts |
| Overconfident wrong extraction | incorrect slots with high confidence | insufficient ambiguity handling | mismatch vs deterministic validator | require per-field confidence + abstain policy |
| Context poisoning | irrelevant thread context dominates | unbounded context injection | spike in invalid intents | bounded context windows + ranked context |
| Prompt injection in user text | model ignores safety guardrails | weak role boundaries | policy violations in outputs | delimiter discipline + safety rule precedence |
| Multi-turn drift | inconsistent intent across turns | missing state grounding | high correction frequency | inject canonical state summary each turn |
| Cost blowout | long prompts/token usage | verbose context/examples | token metrics | prompt compression and staged prompting |

---

## 7. Guardrail Implementation Guide

## 7.1 Input Guardrails

- Sanitize and normalize Slack/user payloads.
- Reject oversized payloads.
- Strip/escape known prompt injection markers for non-semantic sections.

## 7.2 Prompt Guardrails

- Enforce system-level deterministic boundaries every request.
- Use explicit output schema with enums.
- Require abstain behavior for unknowns.

## 7.3 Output Guardrails

- Validate schema strictly.
- Validate IDs against deterministic catalog/policy stores.
- Validate lifecycle legality via flow engine.
- Reject and reprompt on invalid output with error-guided repair prompt.

## 7.4 Business Guardrails

- LLM output cannot finalize quote lifecycle mutations directly.
- All monetary and approval decisions must pass deterministic engines.
- Every LLM interaction is auditable with prompt/version/model metadata.

## 7.5 Human-in-the-loop Triggers

Require explicit user/operator confirmation when:
- confidence below threshold,
- multiple plausible catalog matches,
- discount/approval-critical fields inferred from ambiguous text,
- deterministic validator reports unresolved blockers.

---

## 8. Recommended Runtime Architecture for LLM Integration

1. `IntentTranslator` interface in agent runtime.
2. Provider adapters (`OpenAIAdapter`, `AnthropicAdapter`, `LocalAdapter`).
3. `PromptRegistry` with versioned templates.
4. `ValidationGateway` applying schema + deterministic checks.
5. `AuditLogger` for all model requests/responses (redacted where necessary).
6. `FallbackRouter` selecting secondary model or clarification flow.

---

## 9. Implementation Notes for Upcoming Beads

1. Add prompt versioning table in SQLite (`prompt_template`, `prompt_version`, `active_from`).
2. Store benchmark dataset hash and model eval runs for reproducibility.
3. Add CLI helpers:
- `quotey prompt validate`
- `quotey prompt benchmark`
- `quotey prompt diff`
4. Add integration test harness for schema-valid + deterministic-accept metrics.

---

## 10. Acceptance Criteria Traceability (`bd-256v.4`)

- **Prompt library for common tasks:** Section 3 complete.
- **Model comparison matrix:** Section 4 complete.
- **Accuracy benchmark on test cases:** Section 5 complete (framework + protocol + gates; live run blocked by missing runtime credentials in this session).
- **Failure mode catalog:** Section 6 complete.
- **Guardrail implementation guide:** Section 7 complete.
- **ADR for LLM integration architecture:** companion ADR provided.

---

## 11. References

1. OpenAI Prompt Engineering guide: https://platform.openai.com/docs/guides/prompt-engineering  
2. OpenAI Structured Outputs guide: https://platform.openai.com/docs/guides/structured-outputs  
3. OpenAI Function calling guide: https://platform.openai.com/docs/guides/function-calling  
4. Anthropic prompting overview: https://docs.anthropic.com/en/docs/build-with-claude/prompt-engineering/overview  
5. Anthropic tool use overview: https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/overview  
6. Anthropic structured output (JSON mode): https://docs.anthropic.com/en/docs/test-and-evaluate/strengthen-guardrails/increase-consistency  
7. Ollama structured outputs: https://ollama.com/blog/structured-outputs  
8. Ollama tool calling: https://ollama.com/blog/tool-support

