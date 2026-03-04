# W2 AutoQuote Requirement Extraction Prompt (quotey-005-1)

## Objective
Define a deterministic prompt and output contract for converting unstructured customer text
(email, RFP, Slack thread) into structured requirement candidates.

The LLM remains a translator only. It does not set prices, approve discounts, or route approvals.

## Output Contract
Schema version: `requirement_extraction.v1`

Primary payload (`ExtractedRequirements`):
- `schema_version`: fixed to `requirement_extraction.v1`
- `source_type`: `email | rfp | slack_thread`
- `sender_hint` (optional): sender identity when detectable (`name`, `email`, or org hint)
- `context_hint` (optional): short context summary (deal motion/urgency/background)
- `requirements[]`:
  - `requirement_type` (`product | feature | billing | service | compliance`)
  - `name`
  - `quantity` (optional positive integer)
  - `confidence` (0.0 to 1.0)
  - `raw_excerpt` (optional source citation)
- `ambiguities[]`:
  - `text`
  - `question`
  - `options` (minimum 2)
  - `confidence` (0.0 to 1.0)
- `missing_info[]`: unresolved required fields

Validation is implemented in:
- `crates/core/src/domain/requirement_extraction.rs`

## Prompt Design Rules
Implemented prompt builder:
- `crates/agent/src/prompts.rs::build_requirement_extraction_prompt(...)`

The prompt enforces:
1. JSON-only output (no prose/markdown).
2. explicit schema/version + source_type constraints.
3. confidence rubric with omit threshold (<0.40 should become ambiguity/missing_info).
4. prohibition on invented pricing/policy decisions.

## Deterministic Safety Constraints
1. CPQ authority boundary:
   LLM output is candidate data only. Deterministic engines remain source of truth.
2. Bounded confidence:
   confidence outside `[0.0, 1.0]` is rejected.
3. Ambiguity discipline:
   ambiguous terms must include a clarifying question and >=2 options.
4. Schema versioning:
   payloads with unknown `schema_version` fail validation.

## Follow-On Beads
1. `quotey-005-2`: email parser should call this prompt builder and validate payloads.
2. `quotey-005-3`: RFP parser should apply same contract with source_type=`rfp`.
3. `quotey-005-4`: product matcher should consume `requirements[]` with confidence-aware matching.
