# W2 Draft Quote Builder Implementation (quotey-005-5)

## Objective
Implement deterministic draft quote construction from:
- extracted requirements,
- product-match output,
- product catalog entries.

## Implementation
Added:
- `crates/core/src/cpq/draft_quote_builder.rs`

Core API:
- `DraftQuoteBuilder::build_from_matches(request, extracted, matches, catalog)`

Input contract:
- `DraftQuoteBuildRequest` (`quote_id`, `created_by`, optional account/deal, `currency`)

Output contract:
- `DraftQuoteBuildResult`
  - `quote: Option<Quote>`
  - `warnings: Vec<String>`

## Behavior
1. Each matched product becomes a draft quote line.
2. Quantity source:
   - from matching extracted requirement quantity when present,
   - else default quantity `1`.
3. Unit price source:
   - product `base_price`,
   - fallback `0` with explicit warning for human review.
4. Ambiguities/unmatched requirements are surfaced as warnings.
5. If no matched lines exist, returns `quote=None` with warning.

## Validation
Unit tests cover:
1. successful quote build with quantity defaults + extracted quantity mapping,
2. warning behavior when only ambiguity/unmatched data exists.
